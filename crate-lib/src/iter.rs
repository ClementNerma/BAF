use std::io::{Read, Seek};

use anyhow::{Context, Result};

use crate::{
    archive::{Archive, DirEntry},
    data::{
        directory::{DirectoryId, DirectoryIdOrRoot},
        file::FileId,
    },
};

/// See [`Archive::iter`]
pub struct ArchiveIter<'a, R: Read + Seek> {
    archive: &'a Archive<R>,
    dir_id: DirectoryIdOrRoot,
    state: IterState<'a, R>,
    ordered: bool,
}

impl<'a, R: Read + Seek> ArchiveIter<'a, R> {
    pub(crate) fn new(archive: &'a Archive<R>, ordered: bool) -> Self {
        Self::new_for_dir(archive, DirectoryIdOrRoot::Root, ordered).unwrap()
    }

    fn new_for_dir(
        archive: &'a Archive<R>,
        dir_id: DirectoryIdOrRoot,
        ordered: bool,
    ) -> Result<Self> {
        let dirs = archive
            .get_children_dirs_of(dir_id)
            .context("Provided directory ID was not found")?;

        let mut dirs = dirs.iter().copied().collect::<Vec<_>>();

        if ordered {
            dirs.sort_by_key(|dir_id| &archive.get_dir(*dir_id).unwrap().name);
        }

        // We .pop() from the Vec<_> during iteration
        dirs.reverse();

        Ok(Self {
            archive,
            dir_id,
            state: IterState::Dirs {
                curr: None,
                next: dirs,
            },
            ordered,
        })
    }
}

impl<'a, R: Read + Seek> Iterator for ArchiveIter<'a, R> {
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            IterState::Dirs { curr, next } => match curr.as_mut().and_then(|curr| curr.next()) {
                Some(item) => Some(item),
                None => match next.pop() {
                    Some(next) => {
                        *curr = Some(Box::new(
                            ArchiveIter::new_for_dir(
                                self.archive,
                                DirectoryIdOrRoot::NonRoot(next),
                                self.ordered,
                            )
                            .unwrap(),
                        ));

                        Some(DirEntry::Directory(self.archive.get_dir(next).unwrap()))
                    }
                    None => {
                        let files = self.archive.get_children_files_of(self.dir_id).unwrap();

                        let mut files = files.iter().copied().collect::<Vec<_>>();

                        if self.ordered {
                            files.sort_by_key(|file_id| {
                                &self.archive.get_file(*file_id).unwrap().name
                            });
                        }

                        // We .pop() from the Vec<_> during iteration
                        files.reverse();

                        self.state = IterState::Files(files);
                        self.next()
                    }
                },
            },

            IterState::Files(files) => files
                .pop()
                .map(|file_id| DirEntry::File(self.archive.get_file(file_id).unwrap())),
        }
    }
}

enum IterState<'a, R: Read + Seek> {
    Dirs {
        curr: Option<Box<ArchiveIter<'a, R>>>,
        next: Vec<DirectoryId>,
    },
    Files(Vec<FileId>),
}
