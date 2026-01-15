use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::{
    DirectoryId, DirectoryIdOrRoot, FileId, ItemId, ItemName, data::ft_segment::FileTableSegment,
};

// TODO: return computed DirContent
pub fn check_file_table_correctness(
    segments: &[FileTableSegment],
) -> Result<HashMap<DirectoryIdOrRoot, DirContent>, Vec<FileTableCorrectnessError>> {
    let mut errors = vec![];
    let mut dirs_content = HashMap::from([(DirectoryIdOrRoot::Root, DirContent::default())]);

    for dir in segments.iter().flat_map(|segment| &segment.dirs).flatten() {
        if dirs_content
            .insert(DirectoryIdOrRoot::NonRoot(dir.id), DirContent::default())
            .is_some()
        {
            errors.push(FileTableCorrectnessError::DuplicateDirectoryId {
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
            errors.push(FileTableCorrectnessError::DuplicateItemInDirName {
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
            errors.push(FileTableCorrectnessError::DuplicateItemInDirName {
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

/// Error while validating the correctness of an archive's file table
#[derive(Debug)]
pub enum FileTableCorrectnessError {
    /// At least two directories have the same ID
    DuplicateDirectoryId {
        /// Second directory to use the ID
        faulty_dir_id: DirectoryId,

        /// Second directory's name
        faulty_dir_name: ItemName,
    },

    /// At least two items have the same name in the same parent directory
    DuplicateItemInDirName {
        /// Second item to use the name
        faulty_item_id: ItemId,

        /// Second item's name
        faulty_item_name: ItemName,

        /// ID of the faulty items' parent directory
        parent_dir_id: DirectoryIdOrRoot,
    },
}

#[derive(Default)]
pub struct DirContent {
    pub dirs: HashSet<DirectoryId>,
    pub files: HashSet<FileId>,
    pub names: HashSet<ItemName>,
}
