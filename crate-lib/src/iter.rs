use std::io::{Read, Seek};

use crate::{
    archive::{Archive, ArchiveError, DirEntry},
    data::{
        directory::{DirectoryId, DirectoryIdOrRoot},
        file::FileId,
    },
};

/// See [`Archive::ordered_iter`] and [`Archive::unordered_iter`]
pub struct ArchiveIter<'a, R: Read + Seek> {
    archive: &'a Archive<R>,
    dir_id: DirectoryIdOrRoot,
    state: IterState<'a, R>,
}

impl<'a, R: Read + Seek> ArchiveIter<'a, R> {
    pub(crate) fn new(
        archive: &'a Archive<R>,
        dir_id: DirectoryIdOrRoot,
    ) -> Result<Self, ArchiveError> {
        let (dirs, _) = archive.get_dir_content(dir_id)?;

        let mut dirs = dirs
            .iter()
            .copied()
            .filter_map(|dir_id| {
                Some((dir_id, archive.get_dir(dir_id)?.name.clone()))
            })
            .collect::<Vec<_>>();
        dirs.sort_by(|a, b| a.1.cmp(&b.1));
        let mut dirs = dirs.into_iter().map(|(id, _)| id).collect::<Vec<_>>();

        // We .pop() from the Vec<_> during iteration
        dirs.reverse();

        Ok(Self {
            archive,
            dir_id,
            state: IterState::Dirs {
                curr: None,
                next: dirs,
            },
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
                        let child_iter =
                            ArchiveIter::new(self.archive, DirectoryIdOrRoot::NonRoot(next))
                                .ok()?;

                        *curr = Some(Box::new(child_iter));

                        Some(DirEntry::Directory(
                        self.archive.get_dir(next)?,
                        ))
                    }
                    None => {
                        let (_, files) = self.archive.get_dir_content(self.dir_id).ok()?;

                        let mut files = files
                            .iter()
                            .copied()
                            .filter_map(|file_id| {
                                Some((file_id, self.archive.get_file(file_id)?.name.clone()))
                            })
                            .collect::<Vec<_>>();
                        files.sort_by(|a, b| a.1.cmp(&b.1));
                        let mut files = files.into_iter().map(|(id, _)| id).collect::<Vec<_>>();

                        // We .pop() from the Vec<_> during iteration
                        files.reverse();

                        self.state = IterState::Files(files);
                        self.next()
                    }
                },
            },

            IterState::Files(files) => files
                .pop()
                .and_then(|file_id| Some(DirEntry::File(self.archive.get_file(file_id)?))),
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
