use std::io::Cursor;

use crate::{
    data::header::{Header, HeaderDecodingError, MAGIC_NUMBER},
    source::Source,
};

#[test]
fn test_decode_valid() {
    let mut bytes = vec![];
    bytes.extend(MAGIC_NUMBER);
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(vec![0u8; 256 - bytes.len()]);
    let mut source = Source::new(Cursor::new(bytes));
    let result = Header::decode(&mut source);
    assert!(result.is_ok());
    let source_with_header = result.unwrap();
    assert_eq!(source_with_header.header.version.version_number(), 1);
}

#[test]
fn test_invalid_magic() {
    let bytes = vec![b'X'; 256];
    let mut source = Source::new(Cursor::new(bytes));
    let err = Header::decode(&mut source).unwrap_err();
    assert!(matches!(
        err,
        HeaderDecodingError::InvalidMagicNumber { .. }
    ));
}

#[test]
fn test_unknown_version() {
    let mut bytes = vec![];
    bytes.extend(MAGIC_NUMBER);
    bytes.extend(99u32.to_le_bytes());
    bytes.extend(vec![0u8; 256 - bytes.len()]);
    let mut source = Source::new(Cursor::new(bytes));
    let err = Header::decode(&mut source).unwrap_err();
    assert!(matches!(err, HeaderDecodingError::UnknownVersion { input } if input == 99));
}

#[test]
fn test_nonzero_padding() {
    let mut bytes = vec![];
    bytes.extend(MAGIC_NUMBER);
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(vec![0u8; 256 - bytes.len() - 1]);
    bytes.push(42);
    let mut source = Source::new(Cursor::new(bytes));
    let err = Header::decode(&mut source).unwrap_err();
    assert!(matches!(err, HeaderDecodingError::NonZeroPadding));
}

#[test]
fn test_encode() {
    let header = Header::default();
    let encoded = header.encode();
    assert_eq!(encoded.len(), 256);
    assert_eq!(&encoded[0..8], MAGIC_NUMBER);
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 1);
    assert!(encoded[12..].iter().all(|b| *b == 0));
}

#[test]
fn test_header_decoding_error_display() {
    let err = HeaderDecodingError::InvalidMagicNumber {
        got: 0xDEAD,
        expected: 0xBEEF,
    };
    let displayed = format!("{err}");
    assert!(displayed.contains("Invalid magic number"));

    let err = HeaderDecodingError::NonZeroPadding;
    assert_eq!(format!("{err}"), "Header padding is not filled with zeroes");

    let err = HeaderDecodingError::UnknownVersion { input: 42 };
    assert!(format!("{err}").contains("Unknown archive version"));
    assert!(format!("{err}").contains("42"));
}
