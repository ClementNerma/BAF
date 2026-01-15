use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::{
    DirectoryId, DirectoryIdOrRoot, FileId, ItemId, ItemName, data::ft_segment::FileTableSegment,
};

// TODO: return computed DirContent
pub fn check_file_table_correctness(
    segments: &[FileTableSegment],
) -> Result<HashMap<DirectoryIdOrRoot, DirContent>, Vec<FtCorrectnessError>> {
    let mut errors = vec![];
    let mut dirs_content = HashMap::from([(DirectoryIdOrRoot::Root, DirContent::default())]);

    for dir in segments.iter().flat_map(|segment| &segment.dirs).flatten() {
        if dirs_content
            .insert(DirectoryIdOrRoot::NonRoot(dir.id), DirContent::default())
            .is_some()
        {
            errors.push(FtCorrectnessError::DuplicateDirectoryId {
                faulty_dir_id: dir.id,
                faulty_dir_name: dir.name.clone(),
            });
        }
    }

    for dir in segments.iter().flat_map(|segment| &segment.dirs).flatten() {
        let parent_dir_content = match dir.parent_dir {
            DirectoryIdOrRoot::Root => dirs_content.get_mut(&DirectoryIdOrRoot::Root).unwrap(),
            DirectoryIdOrRoot::NonRoot(parent_dir) => dirs_content
                .entry(DirectoryIdOrRoot::NonRoot(parent_dir))
                .or_default(),
        };

        assert!(parent_dir_content.dirs.insert(dir.id));

        if !parent_dir_content.names.insert(dir.name.clone()) {
            errors.push(FtCorrectnessError::DuplicateItemInDirName {
                faulty_item_id: ItemId::Directory(dir.id),
                faulty_item_name: dir.name.clone(),
                parent_dir_id: dir.parent_dir,
            })
        }
    }

    for file in segments.iter().flat_map(|segment| &segment.files).flatten() {
        let parent_dir_content = match file.parent_dir {
            DirectoryIdOrRoot::Root => dirs_content.get_mut(&DirectoryIdOrRoot::Root).unwrap(),
            DirectoryIdOrRoot::NonRoot(parent_dir) => dirs_content
                .entry(DirectoryIdOrRoot::NonRoot(parent_dir))
                .or_default(),
        };

        assert!(parent_dir_content.files.insert(file.id));

        if !parent_dir_content.names.insert(file.name.clone()) {
            errors.push(FtCorrectnessError::DuplicateItemInDirName {
                faulty_item_id: ItemId::File(file.id),
                faulty_item_name: file.name.clone(),
                parent_dir_id: file.parent_dir,
            })
        }
    }

    if errors.is_empty() {
        Ok(dirs_content)
    } else {
        Err(errors)
    }
}

#[derive(Debug)]
pub enum FtCorrectnessError {
    DuplicateDirectoryId {
        faulty_dir_id: DirectoryId,
        faulty_dir_name: ItemName,
    },

    DuplicateItemInDirName {
        faulty_item_id: ItemId,
        faulty_item_name: ItemName,
        parent_dir_id: DirectoryIdOrRoot,
    },
}

#[derive(Default)]
pub struct DirContent {
    pub dirs: HashSet<DirectoryId>,
    pub files: HashSet<FileId>,
    pub names: HashSet<ItemName>,
}
