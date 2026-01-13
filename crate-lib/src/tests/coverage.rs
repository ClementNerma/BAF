use std::io::Cursor;

use crate::{
    archive::Archive,
    config::ArchiveConfig,
    coverage::{Coverage, Segment},
    data::{directory::DirectoryIdOrRoot, name::ItemName, timestamp::Timestamp},
};

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

#[test]
fn reuse_file_space() {
    const REUSE: usize = 1000;

    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();

    let file_id = archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("test".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(vec![0; REUSE]),
        )
        .unwrap();

    assert_eq!(archive.get_file(file_id).unwrap().content_len, REUSE as u64);

    let in_mem = archive.close().unwrap();
    let len = in_mem.get_ref().len();

    let mut archive = Archive::open(in_mem, ArchiveConfig::default()).unwrap();

    archive
        .replace_file_content(file_id, Timestamp::now(), Cursor::new(vec![1; REUSE]))
        .unwrap();

    let in_mem = archive.close().unwrap();

    assert_eq!(in_mem.get_ref().len(), len);

    todo!()
}
