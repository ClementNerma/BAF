use std::{
    io::{Read, Seek, Write},
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    FileId,
    archive::{Archive, DirEntry},
    data::{
        directory::{Directory, DirectoryId, DirectoryIdOrRoot},
        file::File,
        path::PathInArchive,
        timestamp::Timestamp,
    },
    file_reader::FileReader,
};

pub struct WithPaths<'a, S: Read + Seek> {
    archive: &'a mut Archive<S>,
}

impl<'a, S: Read + Seek> WithPaths<'a, S> {
    pub(crate) fn new(archive: &'a mut Archive<S>) -> Self {
        Self { archive }
    }

    /// Get the item located the provided path
    pub fn get_item_at(&self, path: &str) -> Option<ItemIdOrRoot> {
        let mut curr_dir_entry = Some(ItemIdOrRoot::Root);

        for segment in PathInArchive::new(path).ok()?.components() {
            let mut dir_entries = match curr_dir_entry {
                None => self.archive.read_dir(DirectoryIdOrRoot::Root).ok()?,

                Some(id) => match id {
                    ItemIdOrRoot::Root => self.archive.read_dir(DirectoryIdOrRoot::Root).ok()?,

                    ItemIdOrRoot::NonRootDirectory(dir_id) => self
                        .archive
                        .read_dir(DirectoryIdOrRoot::NonRoot(dir_id))
                        .ok()?,

                    ItemIdOrRoot::File(_) => return None,
                },
            };

            let next = dir_entries.find(|item| item.name() == segment)?;

            curr_dir_entry = Some(match next {
                DirEntry::Directory(directory) => ItemIdOrRoot::NonRootDirectory(directory.id),
                DirEntry::File(file) => ItemIdOrRoot::File(file.id),
            });
        }

        curr_dir_entry
    }

    /// Get the directory located the provided path
    ///
    /// Will return [`None`] if a file exists at this location, or if the path points to the root
    pub fn get_dir_at(&self, path: &str) -> Option<&Directory> {
        match self.get_item_at(path)? {
            ItemIdOrRoot::Root | ItemIdOrRoot::File(_) => None,
            ItemIdOrRoot::NonRootDirectory(dir_id) => Some(self.archive.get_dir(dir_id).unwrap()),
        }
    }

    /// Get the file located the provided path
    ///
    /// Will return [`None`] if a directory exists at this location
    pub fn get_file_at(&self, path: &str) -> Option<&File> {
        match self.get_item_at(path)? {
            ItemIdOrRoot::File(file_id) => Some(self.archive.get_file(file_id).unwrap()),
            ItemIdOrRoot::Root | ItemIdOrRoot::NonRootDirectory(_) => None,
        }
    }

    /// Iterate over all items inside a directory contained inside the archive
    pub fn read_dir_at(&self, path: &str) -> Result<impl Iterator<Item = DirEntry<'_>>> {
        match self
            .get_item_at(path)
            .context("Provided path was not found inside the archive")?
        {
            ItemIdOrRoot::Root => Ok(self.archive.read_dir(DirectoryIdOrRoot::Root).unwrap()),
            ItemIdOrRoot::NonRootDirectory(dir_id) => Ok(self
                .archive
                .read_dir(DirectoryIdOrRoot::NonRoot(dir_id))
                .unwrap()),
            ItemIdOrRoot::File(_) => bail!("A file exists at the provided path"),
        }
    }

    /// Get a [`FileReader`] over the file at the provided path inside the archive
    pub fn read_file_at(&mut self, path: &str) -> Result<FileReader<'_, S>> {
        let id = self.get_file_at(path).context("File was not found")?.id;
        self.archive.read_file(id)
    }
}

impl<'a, S: Read + Write + Seek> WithPaths<'a, S> {
    /// Create a directory at the provided path
    pub fn create_dir_at(&mut self, path: &str, modif_time: Timestamp) -> Result<DirectoryId> {
        let mut path = PathInArchive::new(path)
            .map_err(|err| anyhow!("Provided path '{path}' is invalid: {err}"))?;

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            DirectoryIdOrRoot::Root
        } else {
            DirectoryIdOrRoot::NonRoot(self.get_or_create_dir_at(&path.to_string())?.id)
        };

        self.archive
            .create_directory(parent_dir, filename, modif_time)
            .context("Failed to create file")
    }

    /// Get or create a directory at the provided path
    ///
    /// Either returns the existing directory's informations or the newly-created one's
    pub fn get_or_create_dir_at(&mut self, path: &str) -> Result<Directory> {
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

    /// Create a file at the provided path and the provided content
    ///
    /// Will fail if a file already exists at this location
    pub fn create_file_at(
        &mut self,
        path: &str,
        content: impl Read + Seek,
        modif_time: Timestamp,
    ) -> Result<()> {
        if self.get_file_at(path).is_some() {
            bail!("File already exists in archive at path '{path}'");
        }

        self.write_file_at(path, content, modif_time)
    }

    /// Either create a file or replace an existing one at the provided path
    pub fn write_file_at(
        &mut self,
        path: &str,
        content: impl Read + Seek,
        modif_time: Timestamp,
    ) -> Result<()> {
        if let Some(path) = self.get_file_at(path) {
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
            DirectoryIdOrRoot::NonRoot(self.get_or_create_dir_at(&path.to_string())?.id)
        };

        self.archive
            .create_file(parent_dir, filename, modif_time, content)
            .context("Failed to create file")?;

        Ok(())
    }

    /// Update an existing file at the provided path
    pub fn update_file_at(
        &mut self,
        path: &str,
        content: impl Read + Seek,
        modif_time: Timestamp,
    ) -> Result<()> {
        if self.get_file_at(path).is_none() {
            bail!("File not found in archive at path '{path}'");
        }

        self.write_file_at(path, content, modif_time)
    }
    /// Remove the directory at the provided path, recursively
    pub fn remove_dir_at(&mut self, path: &str) -> Result<()> {
        let dir = self
            .get_dir_at(path)
            .context("Provided directory was not found")?;

        self.archive.remove_dir(dir.id)?;

        Ok(())
    }

    /// Remove the file at the provided path
    pub fn remove_file_at(&mut self, path: &str) -> Result<()> {
        let file = self
            .get_file_at(path)
            .context("Provided file was not found")?;

        self.archive.remove_file(file.id)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum ItemIdOrRoot {
    Root,
    NonRootDirectory(DirectoryId),
    File(FileId),
}
