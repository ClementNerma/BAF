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
                    path.push((dir.id, dir.name.clone()));

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

    // TODO: could be optimized to use iterators, would avoid lots of allocations
    pub fn flatten_ordered(&self) -> Vec<FlattenedEntryDir> {
        let mut out = vec![];

        self.flatten_unordered(&mut out);

        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn flatten_unordered(&self, out: &mut Vec<FlattenedEntryDir>) {
        let Self { path, dirs, files } = &self;

        out.push(FlattenedEntryDir {
            path: path.clone(),
            files: files.clone(),
        });

        for (_, tree) in dirs {
            tree.flatten_unordered(out);
        }
    }
}

pub struct FlattenedEntryDir {
    pub path: Vec<String>,
    pub files: Vec<File>,
}
