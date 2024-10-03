use baf::{
    archive::Archive,
    data::{directory::Directory, file::File},
    source::ReadableSource,
};

pub struct Tree {
    path: Vec<String>,
    dirs: Vec<(Directory, Tree)>,
    files: Vec<File>,
}

impl Tree {
    pub fn new(archive: &Archive<impl ReadableSource>) -> Self {
        let dirs = archive.dirs().collect::<Vec<_>>();
        let files = archive.files().collect::<Vec<_>>();

        Self::_new(vec![], dirs.as_slice(), files.as_slice())
    }

    fn _new(path: Vec<(u64, String)>, dirs: &[&Directory], files: &[&File]) -> Self {
        let dir_id = path.last().map(|(id, _)| *id);

        Self {
            path: path.iter().map(|(_, name)| name.clone()).collect(),
            dirs: dirs
                .iter()
                .filter(|dir| dir.parent_dir == dir_id)
                .map(|dir| {
                    let mut path = path.clone();
                    path.push((dir.id, dir.name.clone().into_string()));

                    (Directory::clone(dir), Self::_new(path, dirs, files))
                })
                .collect(),
            files: files
                .iter()
                .filter(|file| file.parent_dir == dir_id)
                .map(|file| File::clone(file))
                .collect(),
        }
    }

    pub fn flattened(&self) -> FlattenedTreeIter {
        FlattenedTreeIter::new(self)
    }
}

pub struct FlattenedEntryDir<'a> {
    pub path: &'a Vec<String>,
    pub files: &'a Vec<File>,
}

pub struct FlattenedTreeIter<'a> {
    tree: &'a Tree,
    sent_self: bool,
    child_iter: Option<(usize, Box<FlattenedTreeIter<'a>>)>,
}

impl<'a> FlattenedTreeIter<'a> {
    fn new(tree: &'a Tree) -> Self {
        Self {
            tree,
            sent_self: false,
            child_iter: None,
        }
    }
}

impl<'a> Iterator for FlattenedTreeIter<'a> {
    type Item = FlattenedEntryDir<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tree = &self.tree;

        if !self.sent_self {
            self.sent_self = true;

            return Some(FlattenedEntryDir {
                path: &tree.path,
                files: &tree.files,
            });
        }

        let Some((child_pos, child_iter)) = &mut self.child_iter else {
            let (_, first_child) = tree.dirs.first()?;

            self.child_iter = Some((0, Box::new(FlattenedTreeIter::new(first_child))));

            return self.next();
        };

        if let Some(next) = child_iter.next() {
            return Some(next);
        }

        if *child_pos == tree.dirs.len() {
            return None;
        }

        let (_, next_child) = &tree.dirs[*child_pos];

        self.child_iter = Some((*child_pos + 1, Box::new(FlattenedTreeIter::new(next_child))));

        self.next()
    }
}
