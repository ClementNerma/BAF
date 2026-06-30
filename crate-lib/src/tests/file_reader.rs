use std::io::{Cursor, Read};

use crate::{Archive, ArchiveConfig, DirectoryIdOrRoot, ItemName, Timestamp};

fn create_archive_with_file(content: &[u8]) -> Archive<Cursor<Vec<u8>>> {
    let mut archive = Archive::create(Cursor::new(vec![]), ArchiveConfig::default()).unwrap();
    archive
        .create_file(
            DirectoryIdOrRoot::Root,
            ItemName::new("test".to_owned()).unwrap(),
            Timestamp::now(),
            Cursor::new(content.to_vec()),
        )
        .unwrap();
    archive
}

fn get_file_id(archive: &Archive<Cursor<Vec<u8>>>) -> crate::FileId {
    archive.files().next().unwrap().id
}

#[test]
fn test_read_to_vec() {
    let content = b"hello world";
    let mut archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);
    let result = archive.read_file(file_id).unwrap().read_to_vec().unwrap();
    assert_eq!(result, content);
}

#[test]
fn test_read_to_string() {
    let content = b"hello utf8 world!";
    let mut archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);
    let result = archive
        .read_file(file_id)
        .unwrap()
        .read_to_string()
        .unwrap();
    assert_eq!(result, "hello utf8 world!");
}

#[test]
fn test_read_to_string_invalid_utf8() {
    let content = b"\xFF\xFE invalid utf8";
    let mut archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);
    let err = archive
        .read_file(file_id)
        .unwrap()
        .read_to_string()
        .unwrap_err();
    assert!(matches!(err, crate::FileReaderError::InvalidUtf8(_)));
}

#[test]
fn test_file_len() {
    let content = b"some bytes";
    let mut archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);
    let reader = archive.read_file(file_id).unwrap();
    assert_eq!(reader.file_len(), content.len() as u64);
}

#[test]
fn test_partial_read() {
    let content = b"0123456789";
    let mut archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);
    let mut reader = archive.read_file(file_id).unwrap();
    let mut buf = [0u8; 3];
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..n], b"012");
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..n], b"345");
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..n], b"678");
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(n, 1);
    assert_eq!(&buf[..n], b"9");
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn test_checksum_mismatch() {
    let content = b"original content";
    let archive = create_archive_with_file(content);
    let file_id = get_file_id(&archive);

    let file = archive.get_file(file_id).unwrap();
    let content_addr = file.content_addr;

    let mut cursor = archive.close().unwrap();

    cursor.get_mut()[content_addr as usize] ^= 0xFF;

    let mut archive = Archive::open(cursor, ArchiveConfig::default()).unwrap();
    let mut reader = archive.read_file(file_id).unwrap();
    let mut buf = vec![];
    let err = reader.read_to_end(&mut buf).unwrap_err();
    assert!(format!("{err}").contains("hash doesn't match"));
}

#[test]
fn test_file_reader_error_display() {
    let err =
        crate::FileReaderError::InvalidUtf8(String::from_utf8(b"\xFF\xFE".to_vec()).unwrap_err());
    let displayed = format!("{err}");
    assert!(displayed.contains("not a valid UTF-8 string"));
}
