use std::{
    io::{Cursor, Read, Seek, Write},
    num::NonZero,
    ops::Deref,
};

use anyhow::Result;

use tempfile::NamedTempFile;

use crate::{
    archive::{Archive, DirEntry},
    config::ArchiveConfig,
    coverage::{Coverage, Segment},
    data::{
        directory::{DirectoryId, DirectoryIdOrRoot},
        file::FileId,
        name::ItemName,
        timestamp::Timestamp,
    },
};

static FILE_CONTENT: &[u8] = b"Hello world!";

#[test]
fn test_in_memory() -> Result<()> {
    perform_test_with(Cursor::new(vec![]))
}

#[test]
fn test_on_real_file() -> Result<()> {
    let test_file = NamedTempFile::new().unwrap();
    perform_test_with(test_file.as_file())
}

fn perform_test_with(source: impl Read + Write + Seek) -> Result<()> {
    // Create archive
    let mut archive = Archive::create(source, ArchiveConfig::default()).unwrap();

    let directory_id = archive
        .create_directory(
            DirectoryIdOrRoot::Root,
            ItemName::new("dir".to_owned()).unwrap(),
            Timestamp::now(),
        )
        .unwrap();

    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::NonRoot(directory_id),
            ItemName::new("file".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(FILE_CONTENT.to_vec()),
        )
        .unwrap();

    archive
        .rename_directory(
            directory_id,
            ItemName::new("dir_renamed".to_owned()).unwrap(),
        )
        .unwrap();

    archive
        .rename_file(file_id, ItemName::new("file_renamed".to_owned()).unwrap())
        .unwrap();

    {
        let file = archive.create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("should be removed".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(vec![]),
        )?;
        archive.remove_file(file)?;

        let dir = archive.create_directory(
            DirectoryIdOrRoot::Root,
            ItemName::new("should be removed".to_owned()).unwrap(),
            Timestamp::now(),
        )?;
        archive.remove_directory(dir)?;
    }

    {
        let dir = archive.create_directory(
            DirectoryIdOrRoot::Root,
            ItemName::new("should be removed".to_owned()).unwrap(),
            Timestamp::now(),
        )?;

        archive.create_file(
            DirectoryIdOrRoot::NonRoot(dir),
            ItemName::new("should be removed".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(vec![]),
        )?;

        archive.remove_directory(dir)?;
    }

    // Close archive, get back the source
    let source = archive.close().unwrap();

    // Open archive
    let mut archive = Archive::open(source, ArchiveConfig::default()).unwrap();

    assert_eq!(archive.dirs().count(), 1);
    assert_eq!(archive.dirs().next().unwrap().name.deref(), "dir_renamed");

    assert_eq!(archive.files().count(), 1);
    assert_eq!(archive.files().next().unwrap().name.deref(), "file_renamed");

    assert_eq!(
        archive.read_dir(DirectoryIdOrRoot::Root).unwrap().count(),
        1
    );
    assert!(
        matches!(archive.read_dir(DirectoryIdOrRoot::Root).unwrap().next().unwrap(), DirEntry::Directory(dir) if dir.name.deref() == "dir_renamed")
    );

    assert_eq!(
        archive
            .read_dir(DirectoryIdOrRoot::NonRoot(DirectoryId(
                NonZero::new(1).unwrap()
            )))
            .unwrap()
            .count(),
        1
    );
    assert!(
        matches!(archive.read_dir(DirectoryIdOrRoot::NonRoot(DirectoryId(NonZero::new(1).unwrap()))).unwrap().next().unwrap(), DirEntry::File(file) if file.name.deref() == "file_renamed")
    );

    assert_eq!(
        archive
            .read_file_to_vec(FileId(NonZero::new(2).unwrap()))
            .unwrap(),
        FILE_CONTENT
    );

    let mut file_reader = archive.read_file(FileId(NonZero::new(2).unwrap())).unwrap();
    let mut file_content = vec![];

    assert_eq!(
        file_reader.read_to_end(&mut file_content)?,
        FILE_CONTENT.len()
    );

    assert_eq!(file_content, FILE_CONTENT);

    Ok(())
}

#[test]
fn coverage() {
    let mut coverage = Coverage::new(100);

    assert_eq!(
        coverage.find_free_zones().next(),
        Some(Segment { start: 0, len: 100 })
    );
    assert_eq!(coverage.find_free_zones().nth(1), None);

    coverage.mark_as_used(0, 10);

    assert_eq!(
        coverage.find_free_zones().next(),
        Some(Segment { start: 10, len: 90 })
    );
    assert_eq!(coverage.find_free_zones().nth(1), None);

    coverage.mark_as_used(90, 100);

    assert_eq!(
        coverage.find_free_zones().next(),
        Some(Segment { start: 10, len: 80 })
    );
    assert_eq!(coverage.find_free_zones().nth(1), None);

    coverage.mark_as_used(40, 20);

    assert_eq!(
        coverage.find_free_zones().next(),
        Some(Segment { start: 10, len: 30 })
    );
    assert_eq!(
        coverage.find_free_zones().nth(1),
        Some(Segment { start: 60, len: 30 })
    );
    assert_eq!(coverage.find_free_zones().nth(2), None);
}
