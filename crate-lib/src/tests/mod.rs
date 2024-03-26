use std::io::Read;

use anyhow::{Context, Result};

use tempfile::NamedTempFile;

use crate::{
    archive::{Archive, ReadItem},
    config::Config,
    coverage::{Coverage, Segment},
    source::{InMemorySource, RealFile, WritableSource},
};

static FILE_CONTENT: &[u8] = b"Hello world!";

#[test]
fn test_in_memory() -> Result<()> {
    perform_test_with(InMemorySource::default())
}

#[test]
fn test_on_real_file() -> Result<()> {
    let test_file = NamedTempFile::new().unwrap();

    perform_test_with(RealFile::open(test_file.path(), true).context("Failed to create file")?)
}

fn perform_test_with(source: impl WritableSource) -> Result<()> {
    // Create archive
    let mut archive = Archive::create(source, Config::default()).unwrap();

    let directory_id = archive.create_directory(None, "dir".to_owned(), 0).unwrap();

    let file_id = archive
        .create_file(
            Some(directory_id),
            "file".to_owned(),
            0,
            InMemorySource::new(FILE_CONTENT.to_vec()),
        )
        .unwrap();

    archive
        .rename_directory(directory_id, "dir_renamed".to_owned())
        .unwrap();

    archive
        .rename_file(file_id, "file_renamed".to_owned())
        .unwrap();

    {
        let file = archive.create_file(
            None,
            "should be removed".to_owned(),
            0,
            InMemorySource::empty(),
        )?;
        archive.remove_file(file)?;

        let dir = archive.create_directory(None, "should be removed".to_owned(), 0)?;
        archive.remove_directory(dir)?;
    }

    {
        let dir = archive.create_directory(None, "should be removed".to_owned(), 0)?;

        archive.create_file(
            Some(dir),
            "should be removed".to_owned(),
            0,
            InMemorySource::empty(),
        )?;

        archive.remove_directory(dir)?;
    }

    let source = archive.close();

    // Open archive
    let (mut archive, _) = Archive::open(source, Config::default()).unwrap();

    assert_eq!(archive.dirs().count(), 1);
    assert_eq!(archive.dirs().next().unwrap().name, "dir_renamed");

    assert_eq!(archive.files().count(), 1);
    assert_eq!(archive.files().next().unwrap().name, "file_renamed");

    assert_eq!(archive.read_dir(None).unwrap().count(), 1);
    assert!(
        matches!(archive.read_dir(None).unwrap().next().unwrap(), ReadItem::Directory(dir) if dir.name == "dir_renamed")
    );

    assert_eq!(archive.read_dir(Some(1)).unwrap().count(), 1);
    assert!(
        matches!(archive.read_dir(Some(1)).unwrap().next().unwrap(), ReadItem::File(file) if file.name == "file_renamed")
    );

    assert_eq!(archive.get_file_content(2).unwrap(), FILE_CONTENT);

    let mut file_reader = archive.get_file_reader(2).unwrap();
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
