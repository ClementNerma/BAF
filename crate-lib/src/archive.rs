use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Seek, Write},
    num::NonZero,
};

use anyhow::{Context, Result, bail};
use sha3::{Digest, Sha3_256};

use crate::{
    config::ArchiveConfig,
    coverage::{Coverage, Segment},
    data::{
        directory::{
            DIRECTORY_ENTRY_SIZE, DIRECTORY_NAME_OFFSET_IN_ENTRY, Directory, DirectoryId,
            DirectoryIdOrRoot,
        },
        file::{FILE_ENTRY_SIZE, FILE_NAME_OFFSET_IN_ENTRY, File, FileId},
        ft_segment::{FileTableSegment, FileTableSegmentDecodingError},
        header::{HEADER_SIZE, Header},
        name::ItemName,
        timestamp::Timestamp,
    },
    easy::EasyArchive,
    file_reader::FileReader,
    source::Source,
};

// TODO: check if parent dirs do exist during decoding -> requires to have decoded all directories first
// TODO: ensure no files or segment overlap (= no overlap in coverage when calling .mark_as_used)

/// Representation of an archive
///
/// This type is designed for pretty low-level stuff, for easier manipulation see the [`Archive::easy`] method
/// or open the archive from the [`EasyArchive`] struct.
pub struct Archive<S: Read + Seek> {
    conf: ArchiveConfig,
    source: Source<S>,
    header: Header,
    file_segments: Vec<FileTableSegment>,
    dirs: HashMap<DirectoryId, Directory>,
    files: HashMap<FileId, File>,
    dirs_content: HashMap<DirectoryIdOrRoot, DirContent>,
    coverage: Coverage,
}

impl<S: Read + Seek> Archive<S> {
    /// Open an existing archive
    ///
    /// May return a set of warnings about ill-formed archives
    ///
    /// Will read the entire archive's metadata segments before returning.
    pub fn open(source: S, conf: ArchiveConfig) -> Result<Self, ArchiveDecodingError> {
        let mut source = Source::new(source);

        let mut source_with_header =
            Header::decode(&mut source).map_err(ArchiveDecodingError::InvalidHeader)?;
        let header = source_with_header.header;

        let mut file_segments = vec![];
        let mut file_segments_addr = vec![HEADER_SIZE as u64];
        let mut prev_segment = FileTableSegment::decode(&mut source_with_header)
            .map_err(ArchiveDecodingError::InvalidFileTableSegment)?;

        while let Some(next_segment) = prev_segment.consume_next_segment(&mut source_with_header) {
            let next_segment =
                next_segment.map_err(ArchiveDecodingError::InvalidFileTableSegment)?;

            file_segments.push(prev_segment);

            let (segment_addr, segment) = next_segment;

            file_segments_addr.push(segment_addr);
            prev_segment = segment;
        }

        file_segments.push(prev_segment);

        let coverage = compute_coverage(
            file_segments
                .iter()
                .enumerate()
                .map(|(i, segment)| (*file_segments_addr.get(i).unwrap(), segment)),
            source.seek_len().map_err(ArchiveDecodingError::IoError)?,
        );

        let dirs = file_segments
            .iter()
            .flat_map(FileTableSegment::dirs)
            .flatten()
            .map(|dir| (dir.id, dir.clone()))
            .collect();

        let files = file_segments
            .iter()
            .flat_map(FileTableSegment::files)
            .flatten()
            .map(|file| (file.id, file.clone()))
            .collect();

        let dirs_content = compute_dirs_content(&file_segments)
            .map_err(ArchiveDecodingError::DuplicateItemNames)?;

        Ok(Self {
            source,
            conf,
            header,
            dirs,
            files,
            file_segments,
            dirs_content,
            coverage,
        })
    }

    /// Get an [`EasyArchive`] abstraction for easier handling of this archive.
    pub fn easy(self) -> EasyArchive<S> {
        EasyArchive::new(self)
    }

    /// Get access to the underlying source
    pub fn source(&mut self) -> &mut Source<S> {
        &mut self.source
    }

    /// Get the content of the archive's header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the list of all directories contained inside the archive
    pub fn dirs(&self) -> impl Iterator<Item = &Directory> {
        self.dirs.values()
    }

    /// Get the list of all files contained inside the archive
    pub fn files(&self) -> impl Iterator<Item = &File> {
        self.files.values()
    }

    /// Get informations about a directory from the archive
    pub fn get_dir(&self, id: DirectoryId) -> Option<&Directory> {
        self.dirs.get(&id)
    }

    /// Get informations about a file from the archive
    pub fn get_file(&self, id: FileId) -> Option<&File> {
        self.files.get(&id)
    }

    fn segment_addr(&self, segment_index: usize) -> u64 {
        assert!(segment_index < self.file_segments.len());

        if segment_index == 0 {
            HEADER_SIZE as u64
        } else {
            self.file_segments[segment_index - 1]
                .next_segment_addr
                .unwrap()
        }
    }

    /// Get the list of all children directories inside the provided directory
    pub fn get_children_dirs_of(&self, id: DirectoryIdOrRoot) -> Result<&HashSet<DirectoryId>> {
        self.dirs_content
            .get(&id)
            .map(|content| &content.dirs)
            .context("Provided directory ID was not found")
    }

    /// Get the list of all children files inside the provided directory
    pub fn get_children_files_of(&self, id: DirectoryIdOrRoot) -> Result<&HashSet<FileId>> {
        self.dirs_content
            .get(&id)
            .map(|content| &content.files)
            .context("Provided directory ID was not found")
    }

    /// Iterate over all items inside a directory contained inside the archive
    pub fn read_dir(&self, id: DirectoryIdOrRoot) -> Result<impl Iterator<Item = DirEntry<'_>>> {
        let dir_content = self
            .dirs_content
            .get(&id)
            .context("Provided directory ID was not found")?;

        Ok(dir_content
            .dirs
            .iter()
            .map(|dir_id| DirEntry::Directory(self.dirs.get(dir_id).unwrap()))
            .chain(
                dir_content
                    .files
                    .iter()
                    .map(|file_id| DirEntry::File(self.files.get(file_id).unwrap())),
            ))
    }

    /// Get a [`FileReader`] over a file contained inside the archive
    pub fn read_file(&mut self, id: FileId) -> Result<FileReader<'_, S>> {
        let file = self.files.get(&id).context("File not found in archive")?;

        self.source.set_position(file.content_addr)?;

        Ok(FileReader::new(
            &mut self.source,
            file.content_len,
            file.sha3_checksum,
        ))
    }

    /// Get the content of a file contained inside the archive into a vector of bytes
    pub fn read_file_to_vec(&mut self, id: FileId) -> Result<Vec<u8>> {
        self.read_file(id).and_then(FileReader::read_at_once)
    }

    fn get_item_entry(&self, item_id: ItemId) -> Result<SegmentEntry> {
        self.file_segments
            .iter()
            .enumerate()
            .find_map(|(segment_index, segment)| {
                let entry_index = match item_id {
                    ItemId::Directory(id) => {
                        segment.dirs.iter().flatten().position(|dir| dir.id == id)
                    }

                    ItemId::File(id) => segment
                        .files
                        .iter()
                        .flatten()
                        .position(|file| file.id == id),
                };

                entry_index.map(|entry_index| {
                    let entry_index_u32 = u32::try_from(entry_index).unwrap();

                    SegmentEntry {
                        segment_index,
                        entry_index,
                        entry_addr: self.segment_addr(segment_index)
                            + match item_id {
                                ItemId::Directory(_) => segment.dir_entry_offset(entry_index_u32),
                                ItemId::File(_) => segment.file_entry_offset(entry_index_u32),
                            },
                    }
                })
            })
            .context(match item_id {
                ItemId::Directory(_) => "Directory not found",
                ItemId::File(_) => "File not found",
            })
    }
}

impl<S: Read + Write + Seek> Archive<S> {
    /// Create a new archive
    pub fn create(source: S, conf: ArchiveConfig) -> Result<Self> {
        let mut source = Source::new(source);

        let header = Header::default();

        let segment = FileTableSegment {
            next_segment_addr: None,
            dirs: vec![
                None;
                usize::try_from(
                    conf.first_segment_dirs_capacity_override
                        .unwrap_or(conf.default_dirs_capacity_by_ft_segment)
                        .get()
                )
                .unwrap()
            ],

            files: vec![
                None;
                usize::try_from(
                    conf.first_segment_files_capacity_override
                        .unwrap_or(conf.default_files_capacity_by_ft_segment)
                        .get()
                )
                .unwrap()
            ],
        };

        source.set_position(0)?;
        source.write_all(&header.encode())?;
        source.write_all(&segment.encode())?;

        Ok(Self {
            conf,
            header,
            coverage: compute_coverage([((HEADER_SIZE as u64), &segment)], source.seek_len()?),
            dirs: HashMap::new(),
            files: HashMap::new(),
            dirs_content: HashMap::from([(DirectoryIdOrRoot::Root, DirContent::default())]),
            file_segments: vec![segment],
            source,
        })
    }

    /// Write some data (file table segment, file content, etc.) wherever there is some free space
    fn write_data_where_possible(
        &mut self,
        mut data: Source<impl Read + Seek>,
    ) -> Result<(u64, Sha3_256)> {
        let len = data.seek_len()?;

        let (addr, growing) = match self.coverage.find_free_zone_for(len) {
            Some(segment) => (segment.start, false),
            None => (self.coverage.next_writable_addr(), true),
        };

        data.set_position(0)?;
        self.source.set_position(addr)?;

        let mut checksum = Sha3_256::new();
        let mut written = 0;

        // Progressively write the data using 4KB chunks and compute the checksum in the meantime
        const CHUNK_SIZE: usize = 4096;

        while written < len {
            let mut buf = [0; CHUNK_SIZE];
            let len = (CHUNK_SIZE as u64).min(len - written);
            let len_usize = usize::try_from(len).unwrap();
            data.read_exact(&mut buf[0..len_usize])?;

            let data = &buf[0..len_usize];

            self.source.write_all(data)?;
            written += len;
            checksum.update(data);
        }

        if growing {
            self.coverage.grow_to(self.source.seek_len()?);
        }

        self.coverage.mark_as_used(addr, len);

        Ok((addr, checksum))
    }

    // returns address of first entry
    fn create_segment(&mut self) -> Result<usize> {
        let segment = FileTableSegment {
            next_segment_addr: None,
            dirs: vec![
                None;
                usize::try_from(self.conf.default_dirs_capacity_by_ft_segment.get()).unwrap()
            ],
            files: vec![
                None;
                usize::try_from(self.conf.default_files_capacity_by_ft_segment.get())
                    .unwrap()
            ],
        };

        // Write new segment
        let (new_segment_addr, _) = self.write_data_where_possible(
            // TODO: improve this mess
            Source::new(Cursor::new(segment.encode())),
        )?;

        // Update previous segment's 'next address'
        self.source
            .set_position(self.segment_addr(self.file_segments.len() - 1))?;

        self.source.write_all(&new_segment_addr.to_le_bytes())?;

        // Update in-memory representation
        self.file_segments.last_mut().unwrap().next_segment_addr = Some(new_segment_addr);
        self.file_segments.push(segment);

        Ok(self.file_segments.len() - 1)
    }

    fn get_addr_for_item_insert(&mut self, item_type: ItemType) -> Result<SegmentEntry> {
        let free_entry_addr =
            match item_type {
                ItemType::Directory => {
                    // TODO: reverse search as it's more likely free entries are the end
                    self.file_segments
                        .iter()
                        .enumerate()
                        .find_map(|(segment_index, segment)| {
                            segment.dirs.iter().position(|entry| entry.is_none()).map(
                                |entry_index| SegmentEntry {
                                    segment_index,
                                    entry_index,
                                    entry_addr: self.segment_addr(segment_index)
                                        + segment
                                            .dir_entry_offset(u32::try_from(entry_index).unwrap()),
                                },
                            )
                        })
                }

                ItemType::File => {
                    // TODO: same thing here
                    self.file_segments
                        .iter()
                        .enumerate()
                        .find_map(|(segment_index, segment)| {
                            segment.files.iter().position(|entry| entry.is_none()).map(
                                |entry_index| SegmentEntry {
                                    segment_index,
                                    entry_index,
                                    entry_addr: self.segment_addr(segment_index)
                                        + segment
                                            .file_entry_offset(u32::try_from(entry_index).unwrap()),
                                },
                            )
                        })
                }
            };

        match free_entry_addr {
            Some(addr) => Ok(addr),

            None => {
                let segment_index = self.create_segment()?;
                let segment = self.file_segments.get(segment_index).unwrap();

                Ok(SegmentEntry {
                    segment_index,
                    entry_index: 0,
                    entry_addr: self.segment_addr(segment_index)
                        + match item_type {
                            ItemType::Directory => segment.dir_entry_offset(0),
                            ItemType::File => segment.file_entry_offset(0),
                        },
                })
            }
        }
    }

    fn ensure_no_duplicate_name(&self, name: &str, parent_dir: DirectoryIdOrRoot) -> Result<()> {
        let parent_dir_content = self
            .dirs_content
            .get(&parent_dir)
            .context("Provided parent directory ID does not exist")?;

        if !parent_dir_content.names.contains(name) {
            Ok(())
        } else {
            bail!(
                "Name '{name}' is already used in {}",
                match parent_dir {
                    DirectoryIdOrRoot::Root => "root directory".to_owned(),
                    DirectoryIdOrRoot::NonRoot(id) => format!("parent directory with ID {id:?}",),
                }
            );
        }
    }

    /// Create a new directory
    ///
    /// Modification time is in seconds since Unix' Epoch
    pub fn create_directory(
        &mut self,
        parent_dir: DirectoryIdOrRoot,
        name: ItemName,
        modif_time: Timestamp,
    ) -> Result<DirectoryId> {
        self.ensure_no_duplicate_name(&name, parent_dir)?;

        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_addr_for_item_insert(ItemType::Directory)?;

        let id = self
            .dirs
            .keys()
            .map(|id| id.inner())
            .chain(self.files.keys().map(|id| id.inner()))
            .max();

        let id = DirectoryId(NonZero::new(id.map_or(1, |max| max.get() + 1)).unwrap());

        let dir = Directory {
            id,
            name,
            parent_dir,
            modif_time,
        };

        // Write the directory entry itself
        self.source.set_position(entry_addr)?;
        self.source.write_all(dir.encode().as_ref())?;

        // Update names listing for parent directory
        let parent_dir_content = self.dirs_content.get_mut(&dir.parent_dir).unwrap();
        assert!(parent_dir_content.names.insert(dir.name.clone()));
        assert!(parent_dir_content.dirs.insert(dir.id));

        // Create content listing for the directory
        self.dirs_content
            .insert(DirectoryIdOrRoot::NonRoot(dir.id), DirContent::default());

        // Update in-memory file segments
        self.file_segments[segment_index].dirs[entry_index] = Some(dir.clone());

        // Register the new directory
        assert!(self.dirs.insert(id, dir).is_none());

        Ok(id)
    }

    /// Create a new file
    ///
    /// Modification time is in seconds since Unix' Epoch
    ///
    /// Content is provided through a [`crate::source::ReadableSource`]
    pub fn create_file(
        &mut self,
        parent_dir: DirectoryIdOrRoot,
        name: ItemName,
        modif_time: Timestamp,
        content: impl Read + Seek,
    ) -> Result<FileId> {
        let mut content = Source::new(content);

        self.ensure_no_duplicate_name(&name, parent_dir)?;

        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_addr_for_item_insert(ItemType::File)?;

        // Write the file's content
        let content_len = content.seek_len()?;
        let (content_addr, sha3_checksum) = self.write_data_where_possible(content)?;

        // Get a new ID for the file
        let id = self
            .dirs
            .keys()
            .map(|id| id.inner())
            .chain(self.files.keys().map(|id| id.inner()))
            .max();

        let id = FileId(NonZero::new(id.map_or(1, |max| max.get() + 1)).unwrap());

        let file = File {
            id,
            parent_dir,
            name,
            modif_time,
            content_addr,
            content_len,
            sha3_checksum: sha3_checksum.finalize().into(),
        };

        // Write the file's entry
        self.source.set_position(entry_addr)?;
        self.source.write_all(file.encode().as_ref())?;

        // Update names listing for parent directory
        let parent_dir_content = self.dirs_content.get_mut(&file.parent_dir).unwrap();
        assert!(parent_dir_content.names.insert(file.name.clone()));
        assert!(parent_dir_content.files.insert(file.id));

        // Update in-memory segments
        self.file_segments[segment_index].files[entry_index] = Some(file.clone());

        // Register the file
        assert!(self.files.insert(id, file).is_none());

        Ok(id)
    }

    // TODO: re-use the space used by the file (if relevant)

    /// Overwrite an existing file's content and modification time
    pub fn replace_file_content(
        &mut self,
        id: FileId,
        new_modif_time: Timestamp,
        new_content: impl Read + Seek,
    ) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self
            .get_item_entry(ItemId::File(id))
            .context("Provided file ID was not found")?;

        let mut new_content = Source::new(new_content);

        let content_len = new_content.seek_len()?;
        let (content_addr, sha3_checksum) = self.write_data_where_possible(new_content)?;

        let update = |file: &mut File| {
            file.content_addr = content_addr;
            file.content_len = content_len;
            file.sha3_checksum = sha3_checksum.clone().finalize().into();
            file.modif_time = new_modif_time;
        };

        // Update file metadata
        let mut new_file = self.files.get_mut(&id).unwrap().clone();
        update(&mut new_file);

        self.source.set_position(entry_addr)?;
        self.source.write_all(&new_file.encode())?;

        // Update in-memory representation
        update(self.files.get_mut(&id).unwrap());

        update(
            self.file_segments
                .get_mut(segment_index)
                .unwrap()
                .files
                .get_mut(entry_index)
                .unwrap()
                .as_mut()
                .unwrap(),
        );

        Ok(())
    }

    /// Rename a directory
    pub fn rename_directory(&mut self, id: DirectoryId, new_name: ItemName) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(ItemId::Directory(id))?;

        let dir = self.dirs.get(&id).unwrap().clone();

        self.ensure_no_duplicate_name(&new_name, dir.parent_dir)?;

        self.source
            .set_position(entry_addr + (DIRECTORY_NAME_OFFSET_IN_ENTRY as u64))?;

        self.source.write_all(&new_name.encode())?;

        self.file_segments[segment_index].dirs[entry_index]
            .as_mut()
            .unwrap()
            .name
            .clone_from(&new_name);

        self.dirs.get_mut(&id).unwrap().name.clone_from(&new_name);

        let parent_dir_content = self.dirs_content.get_mut(&dir.parent_dir).unwrap();
        assert!(parent_dir_content.names.remove(&dir.name));
        assert!(parent_dir_content.names.insert(new_name));

        Ok(())
    }

    /// Rename a file
    pub fn rename_file(&mut self, id: FileId, new_name: ItemName) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(ItemId::File(id))?;

        let file = self.files.get(&id).unwrap().clone();

        self.ensure_no_duplicate_name(&new_name, file.parent_dir)?;

        self.source
            .set_position(entry_addr + (FILE_NAME_OFFSET_IN_ENTRY as u64))?;

        self.source.write_all(&new_name.encode())?;

        self.file_segments[segment_index].files[entry_index]
            .as_mut()
            .unwrap()
            .name
            .clone_from(&new_name);

        self.files.get_mut(&id).unwrap().name.clone_from(&new_name);

        let parent_dir_content = self.dirs_content.get_mut(&file.parent_dir).unwrap();
        assert!(parent_dir_content.names.remove(&file.name));
        assert!(parent_dir_content.names.insert(new_name));

        Ok(())
    }

    /// Remove a directory, recursively
    ///
    /// Returns the removed directory entry
    pub fn remove_directory(&mut self, id: DirectoryId) -> Result<Directory> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(ItemId::Directory(id))?;

        let sub_dirs = self
            .dirs
            .values()
            .filter_map(|dir| {
                if dir.parent_dir == DirectoryIdOrRoot::NonRoot(id) {
                    Some(dir.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let sub_files = self
            .files
            .values()
            .filter_map(|file| {
                if file.parent_dir == DirectoryIdOrRoot::NonRoot(id) {
                    Some(file.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Remove sub-directories, recursively
        for sub_dir in sub_dirs {
            self.remove_directory(sub_dir)?;
        }

        // Remove files
        for sub_file in sub_files {
            self.remove_file(sub_file)?;
        }

        // Remove the directory entry itself
        self.source.set_position(entry_addr)?;

        self.source.write_all(&vec![0; DIRECTORY_ENTRY_SIZE])?;

        // Remove from in-memory file segments
        self.file_segments[segment_index].dirs[entry_index]
            .take()
            .unwrap();

        // Unregister the directory and remove its name from the listing
        let dir = self.dirs.remove(&id).unwrap();

        let parent_dir_content = self.dirs_content.get_mut(&dir.parent_dir).unwrap();

        assert!(parent_dir_content.dirs.remove(&dir.id));
        assert!(parent_dir_content.names.remove(&dir.name));

        // Remove the directory's content listing
        let DirContent { dirs, files, names } = self
            .dirs_content
            .remove(&DirectoryIdOrRoot::NonRoot(dir.id))
            .unwrap();

        assert!(dirs.is_empty());
        assert!(files.is_empty());
        assert!(names.is_empty());

        Ok(dir)
    }

    /// Remove a file
    ///
    /// Returns the removed file entry
    pub fn remove_file(&mut self, id: FileId) -> Result<File> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(ItemId::File(id))?;

        // Remove the file entry itself
        self.source.set_position(entry_addr)?;

        self.source.write_all(&vec![0; FILE_ENTRY_SIZE])?;

        // Remove from in-memory file segments
        self.file_segments[segment_index].files[entry_index]
            .take()
            .unwrap();

        // Unregister the file and remove its name from the listing
        let file = self.files.remove(&id).unwrap();

        let parent_dir_content = self.dirs_content.get_mut(&file.parent_dir).unwrap();

        assert!(parent_dir_content.files.remove(&file.id));
        assert!(parent_dir_content.names.remove(&file.name));

        // Update coverage
        self.coverage.mark_as_free(Segment {
            start: file.content_addr,
            len: file.content_len,
        });

        Ok(file)
    }

    /// Flush all changes
    pub fn flush(&mut self) -> Result<()> {
        self.source.flush()
    }

    /// Close the archive
    ///
    /// Returns the original source provided at type construction
    pub fn close(mut self) -> Result<S> {
        self.source.flush()?;
        Ok(self.source.into_inner())
    }
}

#[derive(Debug)]
pub enum ArchiveDecodingError {
    IoError(anyhow::Error),
    DuplicateItemNames(Vec<ArchiveDuplicateItemNameError>),
    InvalidHeader(anyhow::Error),
    InvalidFileTableSegment(FileTableSegmentDecodingError),
}

#[derive(Debug)]
pub struct ArchiveDuplicateItemNameError {
    pub id: ItemId,
    pub parent_dir: DirectoryIdOrRoot,
    pub name: ItemName,
}

#[derive(Debug)]
pub enum ItemId {
    Directory(DirectoryId),
    File(FileId),
}

enum ItemType {
    Directory,
    File,
}

struct SegmentEntry {
    segment_index: usize,
    entry_index: usize,
    entry_addr: u64,
}

/// Entry in a directory
#[derive(Debug, Clone)]
pub enum DirEntry<'a> {
    Directory(&'a Directory),
    File(&'a File),
}

impl<'a> DirEntry<'a> {
    pub fn name(&self) -> &ItemName {
        match self {
            DirEntry::Directory(dir) => &dir.name,
            DirEntry::File(file) => &file.name,
        }
    }
}

#[derive(Default)]
struct DirContent {
    dirs: HashSet<DirectoryId>,
    files: HashSet<FileId>,
    names: HashSet<ItemName>,
}

fn compute_coverage<'a>(
    file_segments: impl IntoIterator<Item = (u64, &'a FileTableSegment)>,
    len: u64,
) -> Coverage {
    let mut coverage = Coverage::new(len);
    coverage.mark_as_used(0, HEADER_SIZE as u64);

    for (segment_addr, segment) in file_segments.into_iter() {
        coverage.mark_as_used(segment_addr, segment.encoded_len());

        for file in segment.files.iter().flatten() {
            coverage.mark_as_used(file.content_addr, file.content_len);
        }
    }

    coverage
}

fn compute_dirs_content<'a>(
    file_segments: impl IntoIterator<Item = &'a FileTableSegment>,
) -> Result<HashMap<DirectoryIdOrRoot, DirContent>, Vec<ArchiveDuplicateItemNameError>> {
    let mut names_in_dirs = HashMap::from([(DirectoryIdOrRoot::Root, DirContent::default())]);

    let mut errors = vec![];

    for segment in file_segments {
        for dir in segment.dirs().iter().flatten() {
            let parent_dir_content = names_in_dirs.entry(dir.parent_dir).or_default();

            assert!(parent_dir_content.dirs.insert(dir.id));

            if !parent_dir_content.names.insert(dir.name.clone()) {
                errors.push(ArchiveDuplicateItemNameError {
                    id: ItemId::Directory(dir.id),
                    parent_dir: dir.parent_dir,
                    name: dir.name.clone(),
                });
            }

            assert!(
                names_in_dirs
                    .insert(DirectoryIdOrRoot::NonRoot(dir.id), DirContent::default())
                    .is_none()
            );
        }

        for file in segment.files().iter().flatten() {
            let parent_dir_content = names_in_dirs.entry(file.parent_dir).or_default();

            assert!(parent_dir_content.files.insert(file.id));

            if !parent_dir_content.names.insert(file.name.clone()) {
                errors.push(ArchiveDuplicateItemNameError {
                    id: ItemId::File(file.id),
                    parent_dir: file.parent_dir,
                    name: file.name.clone(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(names_in_dirs)
    } else {
        Err(errors)
    }
}
