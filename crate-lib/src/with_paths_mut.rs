use std::io::{Read, Seek, Write};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    Archive, DirEntry, Directory, DirectoryId, DirectoryIdOrRoot, FileReader, PathInArchive,
    Timestamp,
};

/// Allows reading and manipulating an archive using human-readable paths instead of IDs
///
/// Obtained from [`Archive::with_paths`]
///
/// Complements methods obtained from [`Archive::with_paths`]
pub struct WithPathsMut<'a, S: Read + Seek> {
    archive: &'a mut Archive<S>,
}

impl<'a, S: Read + Seek> WithPathsMut<'a, S> {
    pub(crate) fn new(archive: &'a mut Archive<S>) -> Self {
        Self { archive }
    }

    /// Get a [`FileReader`] over the file at the provided path inside the archive
    pub fn read_file_at(&mut self, path: &str) -> Result<FileReader<'_, S>> {
        let id = self
            .archive
            .with_paths()
            .get_file_at(path)
            .context("File was not found")?
            .id;

        self.archive.read_file(id)
    }
}

impl<'a, S: Read + Write + Seek> WithPathsMut<'a, S> {
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
            .create_dir(parent_dir, filename, modif_time)
            .context("Failed to create directory in archive")
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
                    let dir_id =
                        self.archive
                            .create_dir(curr_id, segment.clone(), Timestamp::now())?;

                    self.archive.get_dir(dir_id).unwrap().clone()
                }
            };

            curr_path.append(dir.name.clone());
            curr_dir = Some(dir);
        }

        curr_dir.context("Cannot get or create root directory in archive")
    }

    /// Remove the directory at the provided path, recursively
    pub fn remove_dir_at(&mut self, path: &str) -> Result<()> {
        let dir = self
            .archive
            .with_paths()
            .get_dir_at(path)
            .context("Provided directory was not found")?
            .id;

        self.archive.remove_dir(dir)?;

        Ok(())
    }

    /// Either create a file or replace an existing one at the provided path
    pub fn write_file_at(
        &mut self,
        path: &str,
        content: impl Read + Seek,
        modif_time: Timestamp,
    ) -> Result<()> {
        if let Some(path) = self.archive.with_paths().get_file_at(path) {
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
            .context("Failed to create file in archive")?;

        Ok(())
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
        if self.archive.with_paths().get_file_at(path).is_some() {
            bail!("File already exists in archive at path '{path}'");
        }

        self.write_file_at(path, content, modif_time)
    }

    /// Update an existing file at the provided path
    pub fn update_file_at(
        &mut self,
        path: &str,
        content: impl Read + Seek,
        modif_time: Timestamp,
    ) -> Result<()> {
        if self.archive.with_paths().get_file_at(path).is_none() {
            bail!("File not found in archive at path '{path}'");
        }

        self.write_file_at(path, content, modif_time)
    }

    /// Remove the file at the provided path
    pub fn remove_file_at(&mut self, path: &str) -> Result<()> {
        let file = self
            .archive
            .with_paths()
            .get_file_at(path)
            .context("Provided file was not found")?
            .id;

        self.archive.remove_file(file)?;

        Ok(())
    }
}
