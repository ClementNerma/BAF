use std::io::Cursor;

use crate::{
    Archive, ArchiveConfig, DirectoryIdOrRoot, ItemIdOrRoot, ItemName, PathAccessError, Timestamp,
};

fn create_archive_with_structure() -> Archive<Cursor<Vec<u8>>> {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();

    let subdir = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("subdir".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();

    archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(subdir),
            ItemName::new("nested.txt".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"nested content".to_vec()),
        )
        .unwrap();

    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("rootfile.txt".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"root content".to_vec()),
        )
        .unwrap();

    archive
}

#[test]
fn test_compute_dir_path() {
    let archive = create_archive_with_structure();
    let paths = archive.with_paths();
    let binding = archive.with_paths();
    let dir = binding.get_dir_at("subdir").unwrap();
    let path = paths.compute_dir_path(dir.id).unwrap();
    assert_eq!(path, "subdir");
}

#[test]
fn test_compute_file_path() {
    let archive = create_archive_with_structure();
    let paths = archive.with_paths();
    let binding = archive.with_paths();
    let file = binding.get_file_at("subdir/nested.txt").unwrap();
    let path = paths.compute_file_path(file.id).unwrap();
    assert_eq!(path, "subdir/nested.txt");
}

#[test]
fn test_get_item_at_root() {
    let archive = create_archive_with_structure();
    let item = archive.with_paths().get_item_at("");
    assert!(item.is_none());
}

#[test]
fn test_get_item_at_dir() {
    let archive = create_archive_with_structure();
    let item = archive.with_paths().get_item_at("subdir").unwrap();
    assert!(matches!(item, ItemIdOrRoot::NonRootDirectory(_)));
}

#[test]
fn test_get_item_at_file() {
    let archive = create_archive_with_structure();
    let item = archive.with_paths().get_item_at("rootfile.txt").unwrap();
    assert!(matches!(item, ItemIdOrRoot::File(_)));
}

#[test]
fn test_get_item_at_nonexistent() {
    let archive = create_archive_with_structure();
    assert!(archive.with_paths().get_item_at("nonexistent").is_none());
}

#[test]
fn test_get_dir_at() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let dir = binding.get_dir_at("subdir").unwrap();
    assert_eq!(dir.name.as_ref(), "subdir");
}

#[test]
fn test_get_dir_at_file_path() {
    let archive = create_archive_with_structure();
    assert!(archive.with_paths().get_dir_at("rootfile.txt").is_none());
}

#[test]
fn test_get_dir_at_root() {
    let archive = create_archive_with_structure();
    assert!(archive.with_paths().get_dir_at("/").is_none());
}

#[test]
fn test_get_file_at() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let file = binding.get_file_at("rootfile.txt").unwrap();
    assert_eq!(file.name.as_ref(), "rootfile.txt");
}

#[test]
fn test_get_file_at_dir_path() {
    let archive = create_archive_with_structure();
    assert!(archive.with_paths().get_file_at("subdir").is_none());
}

#[test]
fn test_read_dir_at_root() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let entries: Vec<_> = binding.read_dir_at("/").unwrap().collect();
    assert_eq!(entries.len(), 2);
    let names: Vec<_> = entries.iter().map(|e| e.name().as_ref()).collect();
    assert!(names.contains(&"subdir"));
    assert!(names.contains(&"rootfile.txt"));
}

#[test]
fn test_read_dir_at_subdir() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let entries: Vec<_> = binding.read_dir_at("subdir").unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name().as_ref(), "nested.txt");
}

#[test]
fn test_read_dir_at_file_path() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let result = binding.read_dir_at("rootfile.txt");
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(err, PathAccessError::FileExistsAtPath));
}

#[test]
fn test_read_dir_at_nonexistent() {
    let archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let result = binding.read_dir_at("nope");
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(err, PathAccessError::ItemNotFound));
}

#[test]
fn test_read_file_at() {
    let mut archive = create_archive_with_structure();
    let binding = archive.with_paths();
    let file = binding.get_file_at("rootfile.txt").unwrap();
    let expected_len = file.content_len;
    let _ = binding;
    let mut binding = archive.with_paths_mut();
    let reader = binding.read_file_at("rootfile.txt").unwrap();
    assert_eq!(reader.file_len(), expected_len);
}

#[test]
fn test_create_dir_at() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    let dir_id = archive
        .with_paths_mut()
        .create_dir_at("newdir", Timestamp::now())
        .unwrap();
    let dir = archive.get_dir(dir_id).unwrap();
    assert_eq!(dir.name.as_ref(), "newdir");
}

#[test]
fn test_create_dir_at_nested() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    archive
        .with_paths_mut()
        .create_dir_at("a/b/c", Timestamp::now())
        .unwrap();
    assert!(archive.with_paths().get_dir_at("a").is_some());
    assert!(archive.with_paths().get_dir_at("a/b").is_some());
    assert!(archive.with_paths().get_dir_at("a/b/c").is_some());
}

#[test]
fn test_get_or_create_dir_at_existing() {
    let mut archive = create_archive_with_structure();
    let dir = archive
        .with_paths_mut()
        .get_or_create_dir_at("subdir")
        .unwrap();
    assert_eq!(dir.name.as_ref(), "subdir");
}

#[test]
fn test_get_or_create_dir_at_new() {
    let mut archive = create_archive_with_structure();
    let dir = archive
        .with_paths_mut()
        .get_or_create_dir_at("newdir")
        .unwrap();
    assert_eq!(dir.name.as_ref(), "newdir");
}

#[test]
fn test_remove_dir_at() {
    let mut archive = create_archive_with_structure();
    let dir_id = archive.with_paths().get_dir_at("subdir").unwrap().id;
    archive.with_paths_mut().remove_dir_at("subdir").unwrap();
    assert!(archive.get_dir(dir_id).is_none());
    assert!(
        archive
            .with_paths()
            .get_item_at("subdir/nested.txt")
            .is_none()
    );
}

#[test]
fn test_remove_dir_at_nonexistent() {
    let mut archive = create_archive_with_structure();
    let err = archive.with_paths_mut().remove_dir_at("nope").unwrap_err();
    assert!(matches!(err, PathAccessError::ItemNotFound));
}

#[test]
fn test_write_file_at_create() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    archive
        .with_paths_mut()
        .write_file_at(
            "hello.txt",
            Cursor::new(b"world".to_vec()),
            Timestamp::now(),
        )
        .unwrap();
    let binding = archive.with_paths();
    let file = binding.get_file_at("hello.txt").unwrap();
    let content = archive.read_file_to_vec(file.id).unwrap();
    assert_eq!(content, b"world");
}

#[test]
fn test_write_file_at_replace() {
    let mut archive = create_archive_with_structure();
    let file_id = archive.with_paths().get_file_at("rootfile.txt").unwrap().id;
    archive
        .with_paths_mut()
        .write_file_at(
            "rootfile.txt",
            Cursor::new(b"updated".to_vec()),
            Timestamp::now(),
        )
        .unwrap();
    let content = archive.read_file_to_vec(file_id).unwrap();
    assert_eq!(content, b"updated");
}

#[test]
fn test_write_file_at_nested_path() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    archive
        .with_paths_mut()
        .write_file_at(
            "deep/nested/file.txt",
            Cursor::new(b"deep".to_vec()),
            Timestamp::now(),
        )
        .unwrap();
    assert!(archive.with_paths().get_dir_at("deep").is_some());
    assert!(archive.with_paths().get_dir_at("deep/nested").is_some());
    let binding = archive.with_paths();
    let file = binding.get_file_at("deep/nested/file.txt").unwrap();
    let content = archive.read_file_to_vec(file.id).unwrap();
    assert_eq!(content, b"deep");
}

#[test]
fn test_create_file_at_success() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    archive
        .with_paths_mut()
        .create_file_at("new.txt", Cursor::new(b"hi".to_vec()), Timestamp::now())
        .unwrap();
    assert!(archive.with_paths().get_file_at("new.txt").is_some());
}

#[test]
fn test_create_file_at_already_exists() {
    let mut archive = create_archive_with_structure();
    let err = archive
        .with_paths_mut()
        .create_file_at(
            "rootfile.txt",
            Cursor::new(b"hi".to_vec()),
            Timestamp::now(),
        )
        .unwrap_err();
    assert!(matches!(err, PathAccessError::FileAlreadyExists { path } if path == "rootfile.txt"));
}

#[test]
fn test_update_file_at_success() {
    let mut archive = create_archive_with_structure();
    archive
        .with_paths_mut()
        .update_file_at(
            "rootfile.txt",
            Cursor::new(b"updated".to_vec()),
            Timestamp::now(),
        )
        .unwrap();
    let binding = archive.with_paths();
    let file = binding.get_file_at("rootfile.txt").unwrap();
    let content = archive.read_file_to_vec(file.id).unwrap();
    assert_eq!(content, b"updated");
}

#[test]
fn test_update_file_at_not_found() {
    let mut archive = create_archive_with_structure();
    let err = archive
        .with_paths_mut()
        .update_file_at("nope.txt", Cursor::new(b"hi".to_vec()), Timestamp::now())
        .unwrap_err();
    assert!(matches!(err, PathAccessError::FileNotFound { path } if path == "nope.txt"));
}

#[test]
fn test_remove_file_at() {
    let mut archive = create_archive_with_structure();
    assert!(archive.with_paths().get_file_at("rootfile.txt").is_some());
    archive
        .with_paths_mut()
        .remove_file_at("rootfile.txt")
        .unwrap();
    assert!(archive.with_paths().get_file_at("rootfile.txt").is_none());
}

#[test]
fn test_remove_file_at_nonexistent() {
    let mut archive = create_archive_with_structure();
    let err = archive
        .with_paths_mut()
        .remove_file_at("nope.txt")
        .unwrap_err();
    assert!(matches!(err, PathAccessError::FileNotFound { path } if path == "nope.txt"));
}

#[test]
fn test_path_access_error_display() {
    assert_eq!(
        format!("{}", PathAccessError::FileExistsAtPath),
        "A file exists at the provided path"
    );
    assert_eq!(
        format!("{}", PathAccessError::EmptyPath),
        "Path cannot be empty"
    );
    assert_eq!(
        format!("{}", PathAccessError::ItemNotFound),
        "Provided path was not found inside the archive"
    );
    let fc = PathAccessError::FileCollision {
        path: "a/b".to_owned(),
    };
    assert!(format!("{fc}").contains("a/b"));
    let fa = PathAccessError::FileAlreadyExists {
        path: "x".to_owned(),
    };
    assert!(format!("{fa}").contains("x"));
    let fnf = PathAccessError::FileNotFound {
        path: "y".to_owned(),
    };
    assert!(format!("{fnf}").contains("y"));
}
