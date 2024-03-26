use std::{
    path::{Component, Path},
    time::SystemTime,
};

use anyhow::{bail, Context, Result};

use crate::{
    archive::{Archive, ReadItem},
    data::{directory::Directory, file::File},
    source::{ReadableSource, WritableSource},
};

pub struct EasyArchive<S: ReadableSource> {
    archive: Archive<S>,
}

impl<S: ReadableSource> EasyArchive<S> {
    pub fn new(archive: Archive<S>) -> Self {
        Self { archive }
    }

    pub fn inner(&self) -> &Archive<S> {
        &self.archive
    }

    pub fn inner_mut(&mut self) -> &mut Archive<S> {
        &mut self.archive
    }

    pub fn into_inner(self) -> Archive<S> {
        self.archive
    }

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

    pub fn get_item_at(&self, path: &str) -> Option<ReadItem> {
        let mut curr_item = None::<ReadItem>;

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

    pub fn get_directory(&self, path: &str) -> Option<&Directory> {
        match self.get_item_at(path) {
            Some(ReadItem::Directory(dir)) => Some(dir),
            Some(ReadItem::File(_)) | None => None,
        }
    }

    pub fn get_file(&self, path: &str) -> Option<&File> {
        match self.get_item_at(path) {
            Some(ReadItem::File(file)) => Some(file),
            Some(ReadItem::Directory(_)) | None => None,
        }
    }

    pub fn read_dir(&self, path: &str) -> Option<impl Iterator<Item = ReadItem>> {
        let dir = self.get_directory(path)?;
        Some(self.archive.read_dir(Some(dir.id)).unwrap())
    }
}

impl<S: WritableSource> EasyArchive<S> {
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
                Some(ReadItem::Directory(dir)) => dir.clone(),
                Some(ReadItem::File(_)) => bail!(
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

    pub fn create_directory(&mut self, path: &str, modif_time: u64) -> Result<()> {
        let mut path = Self::split_path(path);

        let filename = path.pop().context("Path cannot be empty")?;

        let parent_dir = if path.is_empty() {
            None
        } else {
            Some(self.get_or_create_dir(&path.join("/"))?.id)
        };

        self.archive
            .create_directory(parent_dir, filename, modif_time)
            .context("Failed to create file")?;

        Ok(())
    }

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

    pub fn remove_directory(&mut self, path: &str) -> Result<Directory> {
        let dir = self
            .get_directory(path)
            .context("Provided directory was not found")?;

        self.archive.remove_directory(dir.id)
    }

    pub fn remove_file(&mut self, path: &str) -> Result<File> {
        let file = self.get_file(path).context("Provided file was not found")?;

        self.archive.remove_file(file.id)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.archive.flush()
    }
}

// pub fn split_path(path: &str) -> Result<Vec<String>> {
//     let mut out = vec![];

//     let path = path
//         .strip_prefix('/')
//         .or_else(|| path.strip_prefix('\\'))
//         .unwrap_or(path);

//     for component in Path::new(path).components() {
//         match component {
//             Component::Prefix(prefix) => {
//                 bail!(
//                     "Prefixes (like '{:?}') are not supported in archive paths",
//                     prefix.as_os_str()
//                 )
//             }

//             Component::RootDir => {
//                 bail!("Root directory marker is not supported in archive paths");
//             }

//             Component::CurDir => {
//                 bail!("Current directory (.) marker is not supported in archive paths")
//             }

//             Component::ParentDir => {
//                 bail!("Parent directory (..) marker is not supported in archive paths")
//             }

//             Component::Normal(normal) => {
//                 out.push(
//                     normal
//                         .to_str()
//                         .context("Only valid UTF-8 paths are supported inside archives")?
//                         .to_owned(),
//                 );
//             }
//         }
//     }

//     Ok(out)
// }

pub fn translate_time_for_archive(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
