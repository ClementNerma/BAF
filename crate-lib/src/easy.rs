use std::{
    path::{Component, Path},
    time::SystemTime,
};

use anyhow::{bail, Context, Result};

use crate::{
    archive::{Archive, DirEntry},
    data::{directory::Directory, file::File},
    source::{ReadableSource, WritableSource},
};

/// Representation of an abstraction over the base [`Archive`] type
///
/// This type is easier to use, while the [`Archive`] type is tailored for lower-level manipulations
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

    /// Split a path as a list of components
    ///
    /// Handles `.` and `..` symbol, prevents escapes from root
    ///
    /// Does not preserve the root symbol (`/` at the beginning of a path)
    pub fn split_path(path: &str) -> Vec<String> {
        let mut out = vec![];

        for component in Path::new(path).components() {
            match component {
                Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
                Component::ParentDir => {
                    out.pop();
                }
                Component::Normal(normal) => out.push(normal.to_string_lossy().into_owned()),
            }
        }

        out
    }

    /// Get the item located the provided path
    pub fn get_item_at(&self, path: &str) -> Option<DirEntry> {
        let mut curr_item = None::<DirEntry>;

        for segment in Self::split_path(path) {
            let curr_id = curr_item.map(|item| item.id());

            let new_item = self
                .archive
                .read_dir(curr_id)?
                .find(|item| item.name() == segment)?;

            curr_item = Some(new_item);
        }

        curr_item
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
    pub fn read_dir(&self, path: &str) -> Option<impl Iterator<Item = DirEntry>> {
        let dir = self.get_directory(path)?;
        Some(self.archive.read_dir(Some(dir.id)).unwrap())
    }
}

impl<S: WritableSource> EasyArchive<S> {
    /// Get or create a directory
    ///
    /// Either returns the existing directory's informations or the newly-created one's
    pub fn get_or_create_dir(&mut self, path: &str) -> Result<Directory> {
        let mut curr_dir = None::<Directory>;
        let mut curr_path = vec![];

        for segment in Self::split_path(path) {
            let curr_id = curr_dir.map(|item| item.id);

            let item = self
                .archive
                .read_dir(curr_id)
                .unwrap()
                .find(|item| item.name() == segment);

            let dir = match item {
                Some(DirEntry::Directory(dir)) => dir.clone(),
                Some(DirEntry::File(_)) => bail!(
                    "Cannot crate path '{path}' in archive: '{}' is a file",
                    curr_path.join("/")
                ),
                None => {
                    let dir_id = self.archive.create_directory(
                        curr_id,
                        segment.to_owned(),
                        translate_time_for_archive(SystemTime::now()),
                    )?;

                    self.archive.get_dir(dir_id).unwrap().clone()
                }
            };

            curr_path.push(dir.name.clone());
            curr_dir = Some(dir);
        }

        curr_dir.context("Cannot get or create root directory in archive")
    }

    /// Create a directory
    pub fn create_directory(&mut self, path: &str, modif_time: u64) -> Result<u64> {
        let mut path = Self::split_path(path);

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            None
        } else {
            Some(self.get_or_create_dir(&path.join("/"))?.id)
        };

        self.archive
            .create_directory(parent_dir, filename, modif_time)
            .context("Failed to create file")
    }

    /// Either create a file with or replace an existing one
    pub fn create_or_update_file(
        &mut self,
        path: &str,
        content: impl ReadableSource,
        modif_time: u64,
    ) -> Result<()> {
        if let Some(path) = self.get_file(path) {
            return self
                .archive
                .replace_file_content(path.id, modif_time, content);
        }

        let mut path = Self::split_path(path);

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            None
        } else {
            Some(self.get_or_create_dir(&path.join("/"))?.id)
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
        modif_time: u64,
    ) -> Result<()> {
        if self.get_file(path).is_some() {
            bail!("File already exists in archive at path '{path}'");
        }

        self.create_or_update_file(path, content, modif_time)
    }

    /// Update an existing file
    pub fn update_file(
        &mut self,
        path: &str,
        content: impl ReadableSource,
        modif_time: u64,
    ) -> Result<()> {
        if self.get_file(path).is_none() {
            bail!("File not found in archive at path '{path}'");
        }

        self.create_or_update_file(path, content, modif_time)
    }

    /// Remove a directory, recursively
    pub fn remove_directory(&mut self, path: &str) -> Result<()> {
        let dir = self
            .get_directory(path)
            .context("Provided directory was not found")?;

        self.archive.remove_directory(dir.id)?;

        Ok(())
    }

    /// Remove a file
    pub fn remove_file(&mut self, path: &str) -> Result<()> {
        let file = self.get_file(path).context("Provided file was not found")?;

        self.archive.remove_file(file.id)?;

        Ok(())
    }

    /// Flush all changes (e.g. to the disk)
    pub fn flush(&mut self) -> Result<()> {
        self.archive.flush()
    }
}

/// Translate a [`SystemTime`] into a timestamp for an archive
pub fn translate_time_for_archive(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
