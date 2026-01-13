use std::io::{Read, Seek};

use anyhow::{Context, Result};

use crate::{
    archive::{Archive, DirEntry},
    data::{
        directory::{DirectoryId, DirectoryIdOrRoot},
        file::FileId,
    },
};

pub struct ArchiveEasyIter<'a, R: Read + Seek> {
    archive: &'a Archive<R>,
    dir_id: DirectoryIdOrRoot,
    state: IterState<'a, R>,
}

impl<'a, R: Read + Seek> ArchiveEasyIter<'a, R> {
    pub(crate) fn new(archive: &'a Archive<R>) -> Self {
        Self::new_for_dir(archive, DirectoryIdOrRoot::Root).unwrap()
    }

    fn new_for_dir(archive: &'a Archive<R>, dir_id: DirectoryIdOrRoot) -> Result<Self> {
        let dirs = archive
            .get_children_dirs_of(dir_id)
            .context("Provided directory ID was not found")?;

        Ok(Self {
            archive,
            dir_id,
            state: IterState::Dirs {
                curr: None,
                next: dirs.clone().into_iter(),
            },
        })
    }
}

impl<'a, R: Read + Seek> Iterator for ArchiveEasyIter<'a, R> {
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            IterState::Dirs { curr, next } => match curr.as_mut().and_then(|curr| curr.next()) {
                Some(item) => Some(item),
                None => match next.next() {
                    Some(next) => {
                        *curr = Some(Box::new(
                            ArchiveEasyIter::new_for_dir(
                                self.archive,
                                DirectoryIdOrRoot::NonRoot(next),
                            )
                            .unwrap(),
                        ));

                        Some(DirEntry::Directory(self.archive.get_dir(next).unwrap()))
                    }
                    None => {
                        let files = self.archive.get_children_files_of(self.dir_id).unwrap();
                        self.state = IterState::Files(files.clone().into_iter());
                        self.next()
                    }
                },
            },

            IterState::Files(files) => files
                .next()
                .map(|file_id| DirEntry::File(self.archive.get_file(file_id).unwrap())),
        }
    }
}

pub enum IterState<'a, R: Read + Seek> {
    Dirs {
        curr: Option<Box<ArchiveEasyIter<'a, R>>>,
        next: std::collections::hash_set::IntoIter<DirectoryId>,
    },
    Files(std::collections::hash_set::IntoIter<FileId>),
}
