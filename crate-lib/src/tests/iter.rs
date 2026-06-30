use std::io::Cursor;

use crate::{Archive, ArchiveConfig, DirEntry, DirectoryIdOrRoot, ItemName, Timestamp};

#[test]
fn test_empty_archive() {
    let archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    let count = archive.items_iter().count();
    assert_eq!(count, 0);
}

#[test]
fn test_nested_ordering() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();

    let dir = archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("parent".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();

    archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(dir),
            ItemName::new("child".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"data".to_vec()),
        )
        .unwrap();

    let items: Vec<_> = archive.items_iter().collect();
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], DirEntry::Directory(_)));
    let dir_name = items[0].name().as_ref();
    assert_eq!(dir_name, "parent");
    assert!(matches!(items[1], DirEntry::File(_)));
}

#[test]
fn test_alphabetical_ordering() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();

    for name in ["banana", "apple", "cherry"] {
        archive
            .create_dir(
                DirectoryIdOrRoot::Root,
                ItemName::new(name.to_owned()).unwrap(),
                Timestamp::now(),
            )
            .unwrap();
    }

    let names: Vec<_> = archive
        .items_iter()
        .map(|e| e.name().as_ref().to_owned())
        .collect();
    assert_eq!(names, vec!["apple", "banana", "cherry"]);
}

#[test]
fn test_dir_before_files() {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();

    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("zzz_file".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(b"".to_vec()),
        )
        .unwrap();

    archive
        .create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("aaa_dir".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();

    let items: Vec<_> = archive.items_iter().collect();
    let names: Vec<_> = items.iter().map(|e| e.name().as_ref()).collect();

    let dir_pos = names.iter().position(|n| *n == "aaa_dir").unwrap();
    let file_pos = names.iter().position(|n| *n == "zzz_file").unwrap();
    assert!(dir_pos < file_pos, "directories should come before files");
}
