use std::{
    collections::{hash_map::Values, HashMap, HashSet},
    path::Path,
};

use anyhow::{bail, Context, Result};
use sha3::{Digest, Sha3_256};

use crate::{
    config::ArchiveConfig,
    coverage::{Coverage, Segment},
    data::{
        directory::{Directory, DIRECTORY_ENTRY_SIZE, DIRECTORY_NAME_OFFSET_IN_ENTRY},
        file::{File, FILE_ENTRY_SIZE, FILE_NAME_OFFSET_IN_ENTRY},
        ft_segment::FileTableSegment,
        header::{Header, HEADER_SIZE},
        name::ItemName,
    },
    diagnostic::Diagnostic,
    easy::EasyArchive,
    file_reader::FileReader,
    source::{InMemorySource, ReadableSource, RealFile, WritableSource},
};

// TODO: check item names during decoding
// TODO: check if parent dirs do exist during decoding -> requires to have decoded all directories first
// TODO: ensure no files or segment overlap (= no overlap in coverage when calling .mark_as_used)

/// Representation of an archive
///
/// This type is designed for pretty low-level stuff, for easier manipulation see the [`Archive::easy`] method.
pub struct Archive<S: ReadableSource> {
    conf: ArchiveConfig,
    source: S,
    header: Header,
    file_segments: Vec<FileTableSegment>,
    dirs: HashMap<u64, Directory>,
    files: HashMap<u64, File>,
    names_in_dirs: HashMap<Option<u64>, HashSet<ItemName>>,
    coverage: Coverage,
}

impl<S: ReadableSource> Archive<S> {
    /// Open an existing archive
    ///
    /// May return a set of warnings about ill-formed archives
    ///
    /// Will read the entire archive's metadata segments before returning.
    pub fn open(mut source: S, conf: ArchiveConfig) -> Result<(Self, Vec<Diagnostic>)> {
        let mut source_with_header = Header::decode(&mut source)?;
        let header = source_with_header.header;

        let mut diags = vec![];

        let mut file_segments = vec![];
        let mut file_segments_addr = vec![HEADER_SIZE];
        let (mut prev_segment, new_diags) = FileTableSegment::decode(&mut source_with_header)?;

        diags.extend(new_diags);

        while let Some(next_segment) = prev_segment.consume_next_segment(&mut source_with_header) {
            file_segments.push(prev_segment);

            let (segment_addr, segment, new_diags) = next_segment?;
            file_segments_addr.push(segment_addr);
            prev_segment = segment;

            diags.extend(new_diags);
        }

        file_segments.push(prev_segment);

        let coverage = Self::compute_coverage(
            file_segments
                .iter()
                .enumerate()
                .map(|(i, segment)| (*file_segments_addr.get(i).unwrap(), segment)),
            source.len()?,
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

        let names_in_dirs = Self::compute_names_in_dirs(&file_segments, &mut diags);

        Ok((
            Self {
                source,
                conf,
                header,
                names_in_dirs,
                files,
                dirs,
                file_segments,
                coverage,
            },
            diags,
        ))
    }

    /// Get an [`crate::easy::EasyArchive`] abstraction for easier handling of this archive.
    pub fn easy(self) -> EasyArchive<S> {
        EasyArchive::new(self)
    }

    /// Get the content of the archive's header
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the list of all directories contained inside the archive
    pub fn dirs(&self) -> Values<u64, Directory> {
        self.dirs.values()
    }

    /// Get the list of all files contained inside the archive
    pub fn files(&self) -> Values<u64, File> {
        self.files.values()
    }

    /// Get informations about a directory from the archive
    pub fn get_dir(&self, id: u64) -> Option<&Directory> {
        self.dirs.get(&id)
    }

    /// Get informations about a file from the archive
    pub fn get_file(&self, id: u64) -> Option<&File> {
        self.files.get(&id)
    }

    fn segment_addr(&self, segment_index: usize) -> u64 {
        assert!(segment_index < self.file_segments.len());

        if segment_index == 0 {
            HEADER_SIZE
        } else {
            self.file_segments[segment_index - 1]
                .next_segment_addr
                .unwrap()
        }
    }

    /// Iterate over all items inside a directory contained inside the archive
    pub fn read_dir(&self, id: Option<u64>) -> Option<impl Iterator<Item = DirEntry>> {
        if let Some(id) = id {
            if !self.dirs.contains_key(&id) {
                return None;
            }
        }

        let dirs = self
            .dirs
            .values()
            .filter(move |dir| dir.parent_dir == id)
            .map(DirEntry::Directory);

        let files = self
            .files
            .values()
            .filter(move |file| file.parent_dir == id)
            .map(DirEntry::File);

        Some(dirs.chain(files))
    }

    /// Get the content of a file contained inside the archive
    pub fn get_file_content(&mut self, id: u64) -> Result<Vec<u8>> {
        let file = self.files.get(&id).context("File not found in archive")?;

        self.source.set_position(file.content_addr)?;

        let bytes = self.source.consume_next(file.content_len)?;

        let mut hash = Sha3_256::new();
        hash.update(&bytes);

        let hash: [u8; 32] = hash.finalize().into();

        if hash != file.sha3_checksum {
            bail!(
                "File's hash doesn't match: expected {:#?}, got {hash:#?}",
                file.sha3_checksum
            );
        }

        Ok(bytes)
    }

    /// Get a [`crate::file_reader::FileReader`] over a file contained inside the archive
    pub fn get_file_reader(&mut self, id: u64) -> Result<FileReader<S>> {
        let file = self.files.get(&id).context("File not found in archive")?;

        self.source.set_position(file.content_addr)?;

        Ok(FileReader::new(
            &mut self.source,
            file.content_len,
            file.sha3_checksum,
        ))
    }

    fn get_item_entry(&self, id: u64, item_type: ItemType) -> Result<SegmentEntry> {
        self.file_segments
            .iter()
            .enumerate()
            .find_map(|(segment_index, segment)| {
                let entry_index = match item_type {
                    ItemType::Directory => {
                        segment.dirs.iter().flatten().position(|dir| dir.id == id)
                    }
                    ItemType::File => segment
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
                            + match item_type {
                                ItemType::Directory => segment.dir_entry_offset(entry_index_u32),
                                ItemType::File => segment.file_entry_offset(entry_index_u32),
                            },
                    }
                })
            })
            .context(match item_type {
                ItemType::Directory => "Directory not found",
                ItemType::File => "File not found",
            })
    }

    fn compute_coverage<'a>(
        file_segments: impl IntoIterator<Item = (u64, &'a FileTableSegment)>,
        len: u64,
    ) -> Coverage {
        let mut coverage = Coverage::new(len);
        coverage.mark_as_used(0, HEADER_SIZE);

        for (segment_addr, segment) in file_segments.into_iter() {
            coverage.mark_as_used(segment_addr, segment.encoded_len());

            for file in segment.files.iter().flatten() {
                coverage.mark_as_used(file.content_addr, file.content_len);
            }
        }

        coverage
    }

    fn compute_names_in_dirs<'a>(
        file_segments: impl IntoIterator<Item = &'a FileTableSegment>,
        diags: &mut Vec<Diagnostic>,
    ) -> HashMap<Option<u64>, HashSet<ItemName>> {
        let mut names_in_dirs = HashMap::from([(None, HashSet::new())]);

        for segment in file_segments {
            for dir in segment.dirs().iter().flatten() {
                if !names_in_dirs
                    .entry(dir.parent_dir)
                    .or_default()
                    .insert(dir.name.clone())
                {
                    diags.push(Diagnostic::ItemHasDuplicateName {
                        is_dir: true,
                        item_id: dir.id,
                        parent_dir_id: dir.parent_dir,
                        name: dir.name.clone(),
                    });
                }

                assert!(names_in_dirs.insert(Some(dir.id), HashSet::new()).is_none());
            }

            for file in segment.files().iter().flatten() {
                if !names_in_dirs
                    .entry(file.parent_dir)
                    .or_default()
                    .insert(file.name.clone())
                {
                    diags.push(Diagnostic::ItemHasDuplicateName {
                        is_dir: false,
                        item_id: file.id,
                        parent_dir_id: file.parent_dir,
                        name: file.name.clone(),
                    });
                }
            }
        }

        names_in_dirs
    }
}

impl<S: WritableSource> Archive<S> {
    /// Create a new archive
    pub fn create(mut source: S, conf: ArchiveConfig) -> Result<Self> {
        let header = Header::default();

        let segment = FileTableSegment {
            next_segment_addr: None,
            dirs: vec![
                None;
                usize::try_from(
                    conf.first_segment_dirs_capacity_override
                        .unwrap_or(conf.default_dirs_capacity_by_ft_segment)
                )
                .unwrap()
            ],

            files: vec![
                None;
                usize::try_from(
                    conf.first_segment_files_capacity_override
                        .unwrap_or(conf.default_files_capacity_by_ft_segment)
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
            coverage: Self::compute_coverage([(HEADER_SIZE, &segment)], source.len()?),
            names_in_dirs: Self::compute_names_in_dirs([&segment], &mut vec![]),
            source,
            file_segments: vec![segment],
            dirs: HashMap::new(),
            files: HashMap::new(),
        })
    }

    fn write_data_where_possible(
        &mut self,
        mut data: impl ReadableSource,
    ) -> Result<(u64, Sha3_256)> {
        let len = data.len()?;

        let (addr, growing) = match self.coverage.find_free_zone_for(len) {
            Some(segment) => (segment.start, false),
            None => (self.coverage.next_writable_addr(), true),
        };

        data.set_position(0)?;
        self.source.set_position(addr)?;

        let mut checksum = Sha3_256::new();
        let mut written = 0;

        while written < len {
            let data = data.consume_next(4096.min(len - written))?;

            self.source.write_all(&data)?;
            written += u64::try_from(data.len()).unwrap();
            checksum.update(&data);
        }

        if growing {
            self.coverage.grow_to(self.source.len()?);
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
                usize::try_from(self.conf.default_dirs_capacity_by_ft_segment).unwrap()
            ],
            files: vec![
                None;
                usize::try_from(self.conf.default_files_capacity_by_ft_segment).unwrap()
            ],
        };

        // Write new segment
        let (new_segment_addr, _) =
            self.write_data_where_possible(InMemorySource::from_data(segment.encode()))?;

        // Update previous segment's 'next address'
        self.source
            .set_position(self.segment_addr(self.file_segments.len() - 1))?;

        self.source.write_all(&new_segment_addr.to_be_bytes())?;

        // Update in-memory representation
        self.file_segments.last_mut().unwrap().next_segment_addr = Some(new_segment_addr);
        self.file_segments.push(segment);

        Ok(self.file_segments.len() - 1)
    }

    fn get_addr_for_item_insert(&mut self, item_type: ItemType) -> Result<SegmentEntry> {
        let free_entry_addr =
            match item_type {
                ItemType::Directory => {
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

    fn ensure_no_duplicate_name(&self, name: &str, parent_dir: Option<u64>) -> Result<()> {
        match self.names_in_dirs.get(&parent_dir) {
            Some(names_in_parent_dir) => {
                if !names_in_parent_dir.contains(name) {
                    Ok(())
                } else {
                    bail!(
                        "Name '{name}' is already used in parent directory with ID {parent_dir:?}"
                    );
                }
            }

            None => bail!("Provided parent directory ID does not exist"),
        }
    }

    /// Create a new directory
    ///
    /// Modification time is in seconds since Unix' Epoch
    pub fn create_directory(
        &mut self,
        parent_dir: Option<u64>,
        name: ItemName,
        modif_time: u64,
    ) -> Result<u64> {
        self.ensure_no_duplicate_name(&name, parent_dir)?;

        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_addr_for_item_insert(ItemType::Directory)?;

        let id = self
            .dirs
            .keys()
            .chain(self.files.keys())
            .max()
            .map_or(1, |max| max + 1);

        let directory = Directory {
            id,
            name,
            parent_dir,
            modif_time,
        };

        // Write the directory entry itself
        self.source.set_position(entry_addr)?;
        self.source.write_all(directory.encode().as_ref())?;

        // Update names listing for parent directory
        assert!(self
            .names_in_dirs
            .get_mut(&directory.parent_dir)
            .unwrap()
            .insert(directory.name.clone()));

        // Create names listing for this directory
        assert!(self
            .names_in_dirs
            .insert(Some(id), HashSet::new())
            .is_none());

        // Update in-memory file segments
        self.file_segments[segment_index].dirs[entry_index] = Some(directory.clone());

        // Register the new directory
        assert!(self.dirs.insert(id, directory).is_none());

        Ok(id)
    }

    /// Create a new file
    ///
    /// Modification time is in seconds since Unix' Epoch
    ///
    /// Content is provided through a [`crate::source::ReadableSource`]
    pub fn create_file(
        &mut self,
        parent_dir: Option<u64>,
        name: ItemName,
        modif_time: u64,
        content: impl ReadableSource,
    ) -> Result<u64> {
        self.ensure_no_duplicate_name(&name, parent_dir)?;

        match self.names_in_dirs.get(&parent_dir) {
            Some(names_in_parent_dir) => {
                if names_in_parent_dir.contains(&name) {
                    bail!(
                        "File name '{}' is already used in parent directory with ID {parent_dir:?}",
                        *name
                    );
                }
            }

            None => bail!("Provided parent directory ID does not exist"),
        }

        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_addr_for_item_insert(ItemType::File)?;

        // Write the file's content
        let content_len = content.len()?;
        let (content_addr, sha3_checksum) = self.write_data_where_possible(content)?;

        // Get a new ID for the file
        let id = self
            .dirs
            .keys()
            .chain(self.files.keys())
            .max()
            .map_or(1, |max| max + 1);

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
        assert!(self
            .names_in_dirs
            .get_mut(&file.parent_dir)
            .unwrap()
            .insert(file.name.clone()));

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
        id: u64,
        new_modif_time: u64,
        new_content: impl ReadableSource,
    ) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self
            .get_item_entry(id, ItemType::File)
            .context("Provided file ID was not found")?;

        let content_len = new_content.len()?;
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
    pub fn rename_directory(&mut self, id: u64, new_name: ItemName) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(id, ItemType::Directory)?;

        let dir = self.dirs.get(&id).unwrap().clone();

        self.ensure_no_duplicate_name(&new_name, dir.parent_dir)?;

        self.source
            .set_position(entry_addr + DIRECTORY_NAME_OFFSET_IN_ENTRY)?;

        self.source.write_all(&new_name.encode())?;

        self.file_segments[segment_index].dirs[entry_index]
            .as_mut()
            .unwrap()
            .name
            .clone_from(&new_name);

        self.dirs.get_mut(&id).unwrap().name.clone_from(&new_name);

        let names_in_parent_dir = self.names_in_dirs.get_mut(&dir.parent_dir).unwrap();
        assert!(names_in_parent_dir.remove(&dir.name));
        assert!(names_in_parent_dir.insert(new_name));

        Ok(())
    }

    /// Rename a file
    pub fn rename_file(&mut self, id: u64, new_name: ItemName) -> Result<()> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(id, ItemType::File)?;

        let file = self.files.get(&id).unwrap().clone();

        self.ensure_no_duplicate_name(&new_name, file.parent_dir)?;

        self.source
            .set_position(entry_addr + FILE_NAME_OFFSET_IN_ENTRY)?;

        self.source.write_all(&new_name.encode())?;

        self.file_segments[segment_index].files[entry_index]
            .as_mut()
            .unwrap()
            .name
            .clone_from(&new_name);

        self.files.get_mut(&id).unwrap().name.clone_from(&new_name);

        let names_in_parent_dir = self.names_in_dirs.get_mut(&file.parent_dir).unwrap();
        assert!(names_in_parent_dir.remove(&file.name));
        assert!(names_in_parent_dir.insert(new_name));

        Ok(())
    }

    /// Remove a directory, recursively
    ///
    /// Returns the removed directory entry
    pub fn remove_directory(&mut self, id: u64) -> Result<Directory> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(id, ItemType::Directory)?;

        let sub_dirs = self
            .dirs
            .values()
            .filter_map(|dir| {
                if dir.parent_dir == Some(id) {
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
                if file.parent_dir == Some(id) {
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

        self.source
            .write_all(&vec![0; usize::try_from(DIRECTORY_ENTRY_SIZE).unwrap()])?;

        // Remove from in-memory file segments
        self.file_segments[segment_index].dirs[entry_index]
            .take()
            .unwrap();

        // Unregister the directory and remove its name from the listing
        let dir = self.dirs.remove(&id).unwrap();

        assert!(self
            .names_in_dirs
            .get_mut(&dir.parent_dir)
            .unwrap()
            .remove(&dir.name));

        // Remove names listing for this directory
        let names_in_dir = self.names_in_dirs.remove(&Some(id)).unwrap();
        assert!(names_in_dir.is_empty());

        Ok(dir)
    }

    /// Remove a file
    ///
    /// Returns the removed file entry
    pub fn remove_file(&mut self, id: u64) -> Result<File> {
        let SegmentEntry {
            segment_index,
            entry_index,
            entry_addr,
        } = self.get_item_entry(id, ItemType::File)?;

        // Remove the file entry itself
        self.source.set_position(entry_addr)?;

        self.source
            .write_all(&vec![0; usize::try_from(FILE_ENTRY_SIZE).unwrap()])?;

        // Remove from in-memory file segments
        self.file_segments[segment_index].files[entry_index]
            .take()
            .unwrap();

        // Unregister the file and remove its name from the listing
        let file = self.files.remove(&id).unwrap();

        assert!(self
            .names_in_dirs
            .get_mut(&file.parent_dir)
            .unwrap()
            .remove(&file.name));

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
    pub fn close(self) -> S {
        self.source
    }
}

#[derive(Debug)]
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
    pub fn id(&self) -> u64 {
        match self {
            DirEntry::Directory(dir) => dir.id,
            DirEntry::File(file) => file.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            DirEntry::Directory(dir) => &dir.name,
            DirEntry::File(file) => &file.name,
        }
    }
}

impl Archive<RealFile> {
    /// Open from a file (on-disk)
    pub fn open_from_file(
        path: impl AsRef<Path>,
        conf: ArchiveConfig,
    ) -> Result<(Self, Vec<Diagnostic>)> {
        let file = RealFile::open(&path)
            .with_context(|| format!("Failed to open file at path: {}", path.as_ref().display()))?;

        Self::open(file, conf)
    }

    /// Create an archive into a file
    pub fn create_as_file(path: impl AsRef<Path>, conf: ArchiveConfig) -> Result<Self> {
        let file = RealFile::create(&path)
            .with_context(|| format!("Failed to open file at path: {}", path.as_ref().display()))?;

        Self::create(file, conf)
    }
}
