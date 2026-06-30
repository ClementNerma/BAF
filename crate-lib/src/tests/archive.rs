use std::io::Cursor;

use crate::{
    Archive, ArchiveConfig, ArchiveError, ArchiveMetadataDecodingError, DirEntry, DirectoryId,
    DirectoryIdOrRoot, FileId, ItemName, Timestamp,
};

fn create_empty_archive() -> Archive<Cursor<Vec<u8>>> {
    Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap()
}

fn add_test_dir(
    archive: &mut Archive<impl std::io::Read + std::io::Write + std::io::Seek>,
) -> DirectoryId {
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("testdir".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap()
}

#[test]
fn test_create_dir_in_nonexistent_parent() {
    let mut archive = create_empty_archive();
    let fake_id = DirectoryId(std::num::NonZero::new(999).unwrap());
    let err = archive
        .create_dir(
            DirectoryIdOrRoot::NonRoot(fake_id),
            ItemName::new("orphan".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap_err();
    assert!(matches!(err, ArchiveError::DirectoryNotFound));
}

#[test]
fn test_create_duplicate_name() {
    let mut archive = create_empty_archive();
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dup".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    let err = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dup".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap_err();
    assert!(matches!(err, ArchiveError::DuplicateName { .. }));
    assert!(format!("{err}").contains("dup"));
}

#[test]
fn test_read_nonexistent_file() {
    let mut archive = create_empty_archive();
    let fake_id = FileId(std::num::NonZero::new(999).unwrap());
    let result = archive.read_file(fake_id);
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(err, ArchiveError::FileNotFound));
}

#[test]
fn test_read_nonexistent_dir() {
    let archive = create_empty_archive();
    let fake_id = DirectoryIdOrRoot::NonRoot(DirectoryId(std::num::NonZero::new(999).unwrap()));
    let result = archive.read_dir(fake_id);
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(err, ArchiveError::DirectoryNotFound));
}

#[test]
fn test_replace_nonexistent_file() {
    let mut archive = create_empty_archive();
    let fake_id = FileId(std::num::NonZero::new(999).unwrap());
    let err = archive
        .replace_file_content(fake_id, Timestamp::now(), Cursor::new(vec![]))
        .unwrap_err();
    assert!(matches!(err, ArchiveError::FileNotFound));
}

#[test]
fn test_rename_nonexistent_dir() {
    let mut archive = create_empty_archive();
    let fake_id = DirectoryId(std::num::NonZero::new(999).unwrap());
    let err = archive
        .rename_directory(fake_id, ItemName::new("new_name".to_owned()).unwrap())
        .unwrap_err();
    assert!(matches!(err, ArchiveError::DirectoryNotFound));
}

#[test]
fn test_rename_nonexistent_file() {
    let mut archive = create_empty_archive();
    let fake_id = FileId(std::num::NonZero::new(999).unwrap());
    let err = archive
        .rename_file(fake_id, ItemName::new("new_name".to_owned()).unwrap())
        .unwrap_err();
    assert!(matches!(err, ArchiveError::FileNotFound));
}

#[test]
fn test_rename_duplicate_name_dir() {
    let mut archive = create_empty_archive();
    let dir1 = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dir_a".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dir_b".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    let err = archive
        .rename_directory(dir1, ItemName::new("dir_b".to_owned()).unwrap())
        .unwrap_err();
    assert!(matches!(err, ArchiveError::DuplicateName { .. }));
}

#[test]
fn test_rename_duplicate_name_file() {
    let mut archive = create_empty_archive();
    let file1 = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("file_a".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"a".to_vec()),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("file_b".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"b".to_vec()),
        )
        .unwrap();
    let err = archive
        .rename_file(file1, ItemName::new("file_b".to_owned()).unwrap())
        .unwrap_err();
    assert!(matches!(err, ArchiveError::DuplicateName { .. }));
}

#[test]
fn test_remove_nonexistent_dir() {
    let mut archive = create_empty_archive();
    let fake_id = DirectoryId(std::num::NonZero::new(999).unwrap());
    let err = archive.remove_dir(fake_id).unwrap_err();
    assert!(matches!(err, ArchiveError::DirectoryNotFound));
}

#[test]
fn test_remove_nonexistent_file() {
    let mut archive = create_empty_archive();
    let fake_id = FileId(std::num::NonZero::new(999).unwrap());
    let err = archive.remove_file(fake_id).unwrap_err();
    assert!(matches!(err, ArchiveError::FileNotFound));
}

#[test]
fn test_read_file_to_vec() {
    let mut archive = create_empty_archive();
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("data".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"hello world".to_vec()),
        )
        .unwrap();
    let content = archive.read_file_to_vec(file_id).unwrap();
    assert_eq!(content, b"hello world");
}

#[test]
fn test_read_file_to_string() {
    let mut archive = create_empty_archive();
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("text".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"hello world".to_vec()),
        )
        .unwrap();
    let content = archive.read_file_to_string(file_id).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn test_get_dir_content() {
    let mut archive = create_empty_archive();
    let dir_id = add_test_dir(&mut archive);
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(dir_id),
            ItemName::new("child".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"data".to_vec()),
        )
        .unwrap();
    let (dirs, files) = archive
        .get_dir_content(DirectoryIdOrRoot::NonRoot(dir_id))
        .unwrap();
    assert!(dirs.is_empty());
    assert!(files.contains(&file_id));

    let (root_dirs, root_files) = archive.get_dir_content(DirectoryIdOrRoot::Root).unwrap();
    assert!(root_dirs.contains(&dir_id));
    assert!(root_files.is_empty());
}

#[test]
fn test_read_dir_recursive() {
    let mut archive = create_empty_archive();
    let dir_a = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("a".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    let dir_b = archive
        .create_dir(
            DirectoryIdOrRoot::NonRoot(dir_a),
            ItemName::new("b".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(dir_b),
            ItemName::new("f".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"x".to_vec()),
        )
        .unwrap();

    let items: Vec<_> = archive
        .read_dir_recursive(DirectoryIdOrRoot::Root)
        .unwrap()
        .collect();
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], DirEntry::Directory(_)));
    assert!(matches!(items[1], DirEntry::Directory(_)));
    assert!(matches!(items[2], DirEntry::File(_)));
}

#[test]
fn test_items_iter_ordering() {
    let mut archive = create_empty_archive();
    let dir_c = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("c".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("a".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("f1".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"".to_vec()),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(dir_c),
            ItemName::new("f2".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"".to_vec()),
        )
        .unwrap();

    let items: Vec<_> = archive.items_iter().collect();
    let names: Vec<_> = items.iter().map(|e| e.name().as_ref()).collect();

    let dir_a_pos = names.iter().position(|n| *n == "a").unwrap();
    let dir_c_pos = names.iter().position(|n| *n == "c").unwrap();
    assert!(dir_a_pos < dir_c_pos);

    let f1_pos = names.iter().position(|n| *n == "f1").unwrap();
    assert!(dir_c_pos < f1_pos);
}

#[test]
fn test_replace_file_larger() {
    let mut archive = create_empty_archive();
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("f".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"small".to_vec()),
        )
        .unwrap();
    let new_content = vec![0u8; 10000];
    archive
        .replace_file_content(file_id, Timestamp::now(), Cursor::new(new_content.clone()))
        .unwrap();
    let content = archive.read_file_to_vec(file_id).unwrap();
    assert_eq!(content.len(), 10000);
    assert_eq!(content, new_content);
}

#[test]
fn test_replace_file_smaller() {
    let mut archive = create_empty_archive();
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("f".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(vec![0u8; 1000]),
        )
        .unwrap();
    archive
        .replace_file_content(file_id, Timestamp::now(), Cursor::new(b"tiny".to_vec()))
        .unwrap();
    let content = archive.read_file_to_vec(file_id).unwrap();
    assert_eq!(content, b"tiny");
}

#[test]
fn test_version() {
    let archive = create_empty_archive();
    assert_eq!(archive.version().version_number(), 1);
}

#[test]
fn test_get_dir() {
    let mut archive = create_empty_archive();
    let dir_id = add_test_dir(&mut archive);
    let dir = archive.get_dir(dir_id).unwrap();
    assert_eq!(dir.name.as_ref(), "testdir");
    assert_eq!(dir.id, dir_id);
}

#[test]
fn test_flush_and_close() {
    let source = Cursor::new(vec![]);
    let mut archive = Archive::create(source, ArchiveConfig::default()).unwrap();
    archive.flush().unwrap();
    let source = archive.close().unwrap();
    let len = source.get_ref().len();
    assert!(len > 0);
}

#[test]
fn test_multiple_segments() {
    let config = ArchiveConfig {
        default_dirs_capacity_by_ft_segment: std::num::NonZero::new(1).unwrap(),
        default_files_capacity_by_ft_segment: std::num::NonZero::new(1).unwrap(),
        ..ArchiveConfig::default()
    };
    let mut archive = Archive::create(Cursor::new(vec![]), config.clone()).unwrap();

    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dir1".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("dir2".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("file1".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"a".to_vec()),
        )
        .unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("file2".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"b".to_vec()),
        )
        .unwrap();

    let source = archive.close().unwrap();
    let archive = Archive::open(source, config).unwrap();
    assert_eq!(archive.dirs().count(), 2);
    assert_eq!(archive.files().count(), 2);
}

#[test]
fn test_dir_entry_methods() {
    let mut archive = create_empty_archive();
    let dir_id = add_test_dir(&mut archive);
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(dir_id),
            ItemName::new("child".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"data".to_vec()),
        )
        .unwrap();

    let entries: Vec<_> = archive
        .read_dir(DirectoryIdOrRoot::NonRoot(dir_id))
        .unwrap()
        .collect();
    let file_entry = entries.first().unwrap();
    assert_eq!(file_entry.name().as_ref(), "child");
    assert!(!file_entry.is_dir());
    assert!(file_entry.is_file());
    assert!(matches!(file_entry.id(), crate::ItemId::File(id) if id == file_id));

    let root_entries: Vec<_> = archive.read_dir(DirectoryIdOrRoot::Root).unwrap().collect();
    let dir_entry = root_entries.first().unwrap();
    assert_eq!(dir_entry.name().as_ref(), "testdir");
    assert!(dir_entry.is_dir());
    assert!(!dir_entry.is_file());
    assert!(matches!(
        dir_entry.id(),
        crate::ItemId::Directory(id) if id == dir_id
    ));
}

#[test]
fn test_open_invalid_magic() {
    let mut bytes = vec![b'X'; 256];
    bytes.extend(vec![0u8; 512]);
    let result = Archive::open(Cursor::new(bytes), ArchiveConfig::default());
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(
        err,
        ArchiveMetadataDecodingError::InvalidHeader(_)
    ));
}

#[test]
fn test_open_invalid_header_io() {
    let bytes = vec![0u8; 4];
    let result = Archive::open(Cursor::new(bytes), ArchiveConfig::default());
    assert!(result.is_err());
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(matches!(
        err,
        ArchiveMetadataDecodingError::InvalidHeader(_)
    ));
}

#[test]
fn test_open_from_file_readonly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.baf");
    let mut archive = Archive::create_as_file(&path, ArchiveConfig::default()).unwrap();
    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("f".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"hello".to_vec()),
        )
        .unwrap();
    archive.flush().unwrap();
    drop(archive);

    let mut archive = Archive::open_from_file_readonly(&path, ArchiveConfig::default()).unwrap();
    let content = archive.read_file_to_vec(file_id).unwrap();
    assert_eq!(content, b"hello");
}

#[test]
fn test_open_from_file_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.baf");
    let mut archive = Archive::create_as_file(&path, ArchiveConfig::default()).unwrap();
    archive.flush().unwrap();
    drop(archive);

    let mut archive = Archive::open_from_file(&path, ArchiveConfig::default()).unwrap();
    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("newdir".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();
    archive.flush().unwrap();
    drop(archive);

    let archive = Archive::open_from_file_readonly(&path, ArchiveConfig::default()).unwrap();
    assert_eq!(archive.dirs().count(), 1);
}

#[test]
fn test_create_as_file_success() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new_archive.baf");
    Archive::create_as_file(&path, ArchiveConfig::default()).unwrap();
}

#[test]
fn test_archive_error_display() {
    assert_eq!(
        format!("{}", ArchiveError::DirectoryNotFound),
        "Directory was not found in archive"
    );
    assert_eq!(
        format!("{}", ArchiveError::FileNotFound),
        "File was not found in archive"
    );
    let dup = ArchiveError::DuplicateName {
        name: "test".to_owned(),
        parent_dir: "root directory".to_owned(),
    };
    assert!(format!("{dup}").contains("test"));
    assert!(format!("{dup}").contains("root directory"));
}

#[test]
fn test_archive_metadata_decoding_error_display() {
    let io_err = ArchiveMetadataDecodingError::IoError(std::io::Error::other(
        "test io error",
    ));
    assert!(format!("{io_err}").contains("test io error"));
}
