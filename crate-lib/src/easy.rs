use std::{collections::HashMap, path::Path, time::SystemTime};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    archive::{Archive, ArchiveDecodingError, DirEntry},
    config::ArchiveConfig,
    data::{
        directory::{Directory, DirectoryId, DirectoryIdOrRoot},
        file::{File, FileId},
        name::ItemName,
        path::PathInArchive,
        timestamp::Timestamp,
    },
    file_reader::FileReader,
    source::{ReadableSource, ReadonlyFile, RealFile, WritableSource, WriteableFile},
};

/// Representation of an abstraction over the base [`Archive`] type
///
/// This type is easier to use, while the [`Archive`] type is tailored for lower-level manipulations
///
/// The main difference is that instead of dealing with unique identifiers, this type deals with string paths (just like in a real filesystem)
pub struct EasyArchive<S: ReadableSource> {
    archive: Archive<S>,
}

impl<S: ReadableSource> EasyArchive<S> {
    /// Create from an [`Archive`] value
    pub fn new(archive: Archive<S>) -> Self {
        Self { archive }
    }

    /// Get the underlying [`Archive`] value
    pub fn inner(&self) -> &Archive<S> {
        &self.archive
    }

    /// Get a mutable access to the underlying [`Archive`] value
    pub fn inner_mut(&mut self) -> &mut Archive<S> {
        &mut self.archive
    }

    /// Consume this value to get the underlying [`Archive`] value
    pub fn into_inner(self) -> Archive<S> {
        self.archive
    }

    /// Get the item located the provided path
    pub fn get_item_at(&self, path: &str) -> Option<DirEntry<'_>> {
        let mut curr_dir_entry = None::<DirEntry>;

        for segment in PathInArchive::new(path).ok()?.components() {
            let mut dir_entries = match curr_dir_entry {
                None => self.archive.read_dir(DirectoryIdOrRoot::Root)?,

                Some(id) => match id {
                    DirEntry::Directory(directory) => self
                        .archive
                        .read_dir(DirectoryIdOrRoot::NonRoot(directory.id))?,

                    DirEntry::File(_) => return None,
                },
            };

            curr_dir_entry = Some(dir_entries.find(|item| item.name() == segment)?);
        }

        curr_dir_entry
    }

    /// Check if an item exists
    pub fn exists(&self, path: &str) -> bool {
        self.get_item_at(path).is_some()
    }

    /// Get the directory located the provided path
    ///
    /// Will return `None` if a file exists at this location
    pub fn get_directory(&self, path: &str) -> Option<&Directory> {
        match self.get_item_at(path) {
            Some(DirEntry::Directory(dir)) => Some(dir),
            Some(DirEntry::File(_)) | None => None,
        }
    }

    /// Get the file located the provided path
    ///
    /// Will return `None` if a directory exists at this location
    pub fn get_file(&self, path: &str) -> Option<&File> {
        match self.get_item_at(path) {
            Some(DirEntry::File(file)) => Some(file),
            Some(DirEntry::Directory(_)) | None => None,
        }
    }

    /// Iterate over a directory's items
    pub fn read_dir(&self, path: &str) -> Option<impl Iterator<Item = DirEntry<'_>>> {
        let dir = self.get_directory(path)?;
        Some(
            self.archive
                .read_dir(DirectoryIdOrRoot::NonRoot(dir.id))
                .unwrap(),
        )
    }
}

impl<S: WritableSource> EasyArchive<S> {
    /// Get or create a directory
    ///
    /// Either returns the existing directory's informations or the newly-created one's
    pub fn get_or_create_dir(&mut self, path: &str) -> Result<Directory> {
        let mut curr_dir = None::<Directory>;
        let mut curr_path = PathInArchive::empty();

        let validated_path = PathInArchive::new(path)
            .map_err(|err| anyhow!("Provided path '{path}' is invalid: {err}"))?;

        for segment in validated_path.components() {
            let curr_id = curr_dir
                .map(|item| DirectoryIdOrRoot::NonRoot(item.id))
                .unwrap_or(DirectoryIdOrRoot::Root);

            let item = self
                .archive
                .read_dir(curr_id)
                .unwrap()
                .find(|item| item.name() == segment);

            let dir = match item {
                Some(DirEntry::Directory(dir)) => dir.clone(),
                Some(DirEntry::File(_)) => {
                    bail!("Cannot crate path '{path}' in archive: '{curr_path}' is a file",)
                }
                None => {
                    let dir_id = self.archive.create_directory(
                        curr_id,
                        segment.clone(),
                        Timestamp::from(SystemTime::now()),
                    )?;

                    self.archive.get_dir(dir_id).unwrap().clone()
                }
            };

            curr_path.append(dir.name.clone());
            curr_dir = Some(dir);
        }

        curr_dir.context("Cannot get or create root directory in archive")
    }

    /// Create a directory
    pub fn create_directory(&mut self, path: &str, modif_time: Timestamp) -> Result<DirectoryId> {
        let mut path = PathInArchive::new(path)
            .map_err(|err| anyhow!("Provided path '{path}' is invalid: {err}"))?;

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            DirectoryIdOrRoot::Root
        } else {
            DirectoryIdOrRoot::NonRoot(self.get_or_create_dir(&path.to_string())?.id)
        };

        self.archive
            .create_directory(parent_dir, filename, modif_time)
            .context("Failed to create file")
    }

    /// Either create a file or replace an existing one
    pub fn write_file(
        &mut self,
        path: &str,
        content: impl ReadableSource,
        modif_time: Timestamp,
    ) -> Result<()> {
        if let Some(path) = self.get_file(path) {
            return self
                .archive
                .replace_file_content(path.id, modif_time, content);
        }

        let mut path = PathInArchive::new(path)
            .map_err(|err| anyhow!("Provided path '{path}' is invalid: {err}"))?;

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            DirectoryIdOrRoot::Root
        } else {
            DirectoryIdOrRoot::NonRoot(self.get_or_create_dir(&path.to_string())?.id)
        };

        self.archive
            .create_file(parent_dir, filename, modif_time, content)
            .context("Failed to create file")?;

        Ok(())
    }

    /// Create a file at the provided path and the provided content
    ///
    /// Will fail if a file already exists at this location
    pub fn create_file(
        &mut self,
        path: &str,
        content: impl ReadableSource,
        modif_time: Timestamp,
    ) -> Result<()> {
        if self.get_file(path).is_some() {
            bail!("File already exists in archive at path '{path}'");
        }

        self.write_file(path, content, modif_time)
    }

    /// Update an existing file
    pub fn update_file(
        &mut self,
        path: &str,
        content: impl ReadableSource,
        modif_time: Timestamp,
    ) -> Result<()> {
        if self.get_file(path).is_none() {
            bail!("File not found in archive at path '{path}'");
        }

        self.write_file(path, content, modif_time)
    }

    /// Get a [`FileReader`] over a file contained inside the archive
    pub fn read_file(&mut self, path: &str) -> Result<FileReader<'_, S>> {
        let id = self.get_file(path).context("File was not found")?.id;
        self.archive.read_file(id)
    }

    /// Get the content of a file contained inside the archive into a vector of bytes
    pub fn read_file_to_vec(&mut self, path: &str) -> Result<Vec<u8>> {
        let id = self.get_file(path).context("File was not found")?.id;
        self.archive.read_file_to_vec(id)
    }

    /// Get the content of a file contained inside the archive as a string
    pub fn read_file_to_string(&mut self, path: &str) -> Result<String> {
        let bytes = self.read_file_to_vec(path)?;
        String::from_utf8(bytes).context("File's content is not a valid UTF-8 string")
    }

    /// Remove a file
    pub fn remove_file(&mut self, path: &str) -> Result<()> {
        let file = self.get_file(path).context("Provided file was not found")?;

        self.archive.remove_file(file.id)?;

        Ok(())
    }

    /// Remove a directory, recursively
    pub fn remove_directory(&mut self, path: &str) -> Result<()> {
        let dir = self
            .get_directory(path)
            .context("Provided directory was not found")?;

        self.archive.remove_directory(dir.id)?;

        Ok(())
    }

    /// Flush all changes (e.g. to the disk)
    pub fn flush(&mut self) -> Result<()> {
        self.archive.flush()
    }

    /// (Internal) Compute the full path of an item inside the archive
    ///
    /// Used by [`Self::compute_dir_path`] and [`Self::compute_file_path`]
    fn compute_item_path(
        &self,
        item_name: &ItemName,
        first_parent_dir: DirectoryIdOrRoot,
    ) -> String {
        let mut components = vec![];

        let mut next = first_parent_dir;

        loop {
            match next {
                DirectoryIdOrRoot::Root => break,
                DirectoryIdOrRoot::NonRoot(directory_id) => {
                    let curr = self.archive.get_dir(directory_id).unwrap();
                    components.push(curr.name.as_ref());
                    next = curr.parent_dir;
                }
            }
        }

        let predic_size =
            components.iter().map(|c| c.len()).sum::<usize>() + components.len() + item_name.len();

        let mut name = String::with_capacity(predic_size);

        for component in components.iter().rev() {
            name.push_str(component);
            name.push('/');
        }

        name.push_str(item_name);

        // Ensure the optimization was correctly performed
        assert_eq!(name.len(), predic_size);

        name
    }

    /// Compute the full path of a directory inside the archive
    pub fn compute_dir_path(&self, dir_id: DirectoryId) -> Result<String> {
        let dir = self
            .archive
            .get_dir(dir_id)
            .context("Directory was not found in archive")?;

        Ok(self.compute_item_path(&dir.name, dir.parent_dir))
    }

    /// Compute the full path of a file inside the archive
    pub fn compute_file_path(&self, file_id: FileId) -> Result<String> {
        let file = self
            .archive
            .get_file(file_id)
            .context("File was not found in archive")?;

        Ok(self.compute_item_path(&file.name, file.parent_dir))
    }

    /// Iterate over the list of files and directories
    ///
    /// * Items are listed in ascending alphabetical order
    /// * Directories are listed before files
    pub fn iter(&self) -> impl Iterator<Item = IterArchiveItem<'_>> {
        let mut dirs = self.archive.dirs().collect::<Vec<_>>();

        let dirs_name = dirs
            .iter()
            .map(|dir| (dir.id, self.compute_dir_path(dir.id).unwrap()))
            .collect::<HashMap<_, _>>();

        dirs.sort_by(|a, b| {
            let a_parent_name = match a.parent_dir {
                DirectoryIdOrRoot::Root => None,
                DirectoryIdOrRoot::NonRoot(directory_id) => {
                    Some(dirs_name.get(&directory_id).unwrap())
                }
            };

            let b_parent_name = match b.parent_dir {
                DirectoryIdOrRoot::Root => None,
                DirectoryIdOrRoot::NonRoot(directory_id) => {
                    Some(dirs_name.get(&directory_id).unwrap())
                }
            };

            a_parent_name
                .cmp(&b_parent_name)
                .then_with(|| a.name.cmp(&b.name))
        });

        let mut root_files = vec![];
        let mut files_by_parent_dir = HashMap::<DirectoryId, Vec<FileId>>::new();

        for file in self.archive.files() {
            match file.parent_dir {
                DirectoryIdOrRoot::Root => root_files.push(file.id),
                DirectoryIdOrRoot::NonRoot(parent) => {
                    files_by_parent_dir.entry(parent).or_default().push(file.id)
                }
            }
        }

        for files in files_by_parent_dir.values_mut() {
            files.sort_by(|a, b| {
                let a = self.archive.get_file(*a).unwrap();
                let b = self.archive.get_file(*b).unwrap();

                a.name.cmp(&b.name)
            });
        }

        dirs.into_iter()
            .flat_map(move |dir| {
                [IterArchiveItem::Directory(
                    self.archive.get_dir(dir.id).unwrap(),
                )]
                .into_iter()
                .chain(
                    files_by_parent_dir
                        .remove(&dir.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|file_id| {
                            IterArchiveItem::File(self.archive.get_file(file_id).unwrap())
                        }),
                )
            })
            .chain(
                root_files
                    .into_iter()
                    .map(|file_id| IterArchiveItem::File(self.archive.get_file(file_id).unwrap())),
            )
    }
}

pub enum IterArchiveItem<'a> {
    File(&'a File),
    Directory(&'a Directory),
}

impl EasyArchive<ReadonlyFile> {
    /// Open from a file (on-disk)
    pub fn open_from_file_readonly(
        path: impl AsRef<Path>,
        conf: ArchiveConfig,
    ) -> Result<Self, ArchiveDecodingError> {
        let file = RealFile::open_readonly(&path)
            .with_context(|| format!("Failed to open file at path: {}", path.as_ref().display()))
            .map_err(ArchiveDecodingError::IoError)?;

        Archive::open(file, conf).map(EasyArchive::new)
    }
}

impl EasyArchive<WriteableFile> {
    /// Open from a file (on-disk)
    pub fn open_from_file(
        path: impl AsRef<Path>,
        conf: ArchiveConfig,
    ) -> Result<Self, ArchiveDecodingError> {
        let file = RealFile::open(&path)
            .with_context(|| format!("Failed to open file at path: {}", path.as_ref().display()))
            .map_err(ArchiveDecodingError::IoError)?;

        Archive::open(file, conf).map(EasyArchive::new)
    }

    /// Create an archive into a file
    pub fn create_as_file(path: impl AsRef<Path>, conf: ArchiveConfig) -> Result<Self> {
        let file = RealFile::create(&path)
            .with_context(|| format!("Failed to open file at path: {}", path.as_ref().display()))?;

        Archive::create(file, conf).map(Archive::easy)
    }
}
