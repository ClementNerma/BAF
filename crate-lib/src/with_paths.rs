use std::io::{Read, Seek};

use anyhow::{Context, Result, bail};

use crate::{
    FileId, ItemName,
    archive::{Archive, DirEntry},
    data::{
        directory::{Directory, DirectoryId, DirectoryIdOrRoot},
        file::File,
        path::PathInArchive,
    },
};

/// Allows reading an archive using human-readable paths instead of IDs
///
/// Obtained from [`Archive::with_paths`]
///
/// To access methods that require mutating the archive, see [`Archive::with_paths_mut`]
pub struct WithPaths<'a, S: Read + Seek> {
    archive: &'a Archive<S>,
}

impl<'a, S: Read + Seek> WithPaths<'a, S> {
    pub(crate) fn new(archive: &'a Archive<S>) -> Self {
        Self { archive }
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
}

/// ID of an item, or root ; unique inside a given archive
#[derive(Debug)]
pub enum ItemIdOrRoot {
    /// Archive's root
    Root,

    /// ID of a non-root directory
    NonRootDirectory(DirectoryId),

    /// ID of a file
    File(FileId),
}
