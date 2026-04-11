use std::{
    io::{Cursor, Read, Seek, Write},
    num::NonZero,
    ops::Deref,
};

use anyhow::Result;

use tempfile::NamedTempFile;

use crate::{
    Archive, ArchiveConfig, DirEntry, DirectoryId, DirectoryIdOrRoot, FileId, ItemName, Timestamp,
};

static FILE_CONTENT: &[u8] = b"Hello world!";

#[test]
fn test_in_memory() -> Result<()> {
    perform_tests_with(|| Cursor::new(vec![]))
}

#[test]
fn test_on_real_file() -> Result<()> {
    let test_file = NamedTempFile::new().unwrap();
    perform_tests_with(|| test_file.as_file())
}

fn perform_tests_with<S: Read + Write + Seek>(create_source: impl Fn() -> S + Clone) -> Result<()> {
    perform_complex_manipulations(create_source)?;

    Ok(())
}

fn perform_complex_manipulations<S: Read + Write + Seek>(
    create_source: impl Fn() -> S,
) -> Result<()> {
    // Create archive
    let source = create_source();

    let mut archive = Archive::create(source, ArchiveConfig::default()).unwrap();

    let directory_id = archive
        .create_dir(
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

        let dir = archive.create_dir(
            DirectoryIdOrRoot::Root,
            ItemName::new("should be removed".to_owned()).unwrap(),
            Timestamp::now(),
        )?;
        archive.remove_dir(dir)?;
    }

    {
        let dir = archive.create_dir(
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

        archive.remove_dir(dir)?;
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

    let mut file_content = vec![];

    archive
        .read_file(FileId(NonZero::new(2).unwrap()))
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, FILE_CONTENT);

    let mut file_reader = archive.read_file(FileId(NonZero::new(2).unwrap())).unwrap();
    let mut file_content = vec![];

    assert_eq!(
        file_reader.read_to_end(&mut file_content)?,
        FILE_CONTENT.len()
    );

    assert_eq!(file_content, FILE_CONTENT);

    // Ensure metadata are correctly updated on disk when updating a file
    let new_timestamp = Timestamp::now();
    archive
        .replace_file_content(
            FileId(NonZero::new(2).unwrap()),
            new_timestamp,
            Cursor::new(vec![1]),
        )
        .unwrap();

    let file = archive.get_file(FileId(NonZero::new(2).unwrap())).unwrap();

    assert_eq!(file.modif_time, new_timestamp);
    assert_eq!(file.content_len, 1);

    let stream = archive.close().unwrap();
    let archive = Archive::open(stream, ArchiveConfig::default()).unwrap();

    let file = archive.get_file(FileId(NonZero::new(2).unwrap())).unwrap();

    assert_eq!(file.modif_time, new_timestamp);
    assert_eq!(file.content_len, 1);

    Ok(())
}
