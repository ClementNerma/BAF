use crate::{ItemName, NameDecodingError, NameDecodingErrorReason, NameValidationError};

#[test]
fn test_new_valid() {
    assert!(ItemName::new("hello".to_owned()).is_ok());
    assert!(ItemName::new("foo.txt".to_owned()).is_ok());
    assert!(ItemName::new("a".repeat(255)).is_ok());
}

#[test]
fn test_new_empty() {
    let err = ItemName::new(String::new()).unwrap_err();
    assert!(matches!(err, NameValidationError::NameIsEmpty));
}

#[test]
fn test_new_too_long() {
    let err = ItemName::new("a".repeat(256)).unwrap_err();
    assert!(matches!(err, NameValidationError::NameIsTooLong));
}

#[test]
fn test_new_reserved_dot() {
    let err = ItemName::new(".".to_owned()).unwrap_err();
    assert!(matches!(err, NameValidationError::ForbiddenName(".")));
}

#[test]
fn test_new_reserved_dotdot() {
    let err = ItemName::new("..".to_owned()).unwrap_err();
    assert!(matches!(err, NameValidationError::ForbiddenName("..")));
}

#[test]
fn test_new_forbidden_chars() {
    for ch in ['/', '\\', '\n', '\r', '\0'] {
        let err = ItemName::new(format!("foo{ch}bar")).unwrap_err();
        assert!(matches!(err, NameValidationError::ForbiddenChar(c) if c == ch));
    }
}

#[test]
fn test_encode_decode_roundtrip() {
    let original = ItemName::new("hello_world.txt".to_owned()).unwrap();
    let bytes = original.encode();
    let decoded = ItemName::decode(bytes).unwrap();
    assert_eq!(decoded.into_string(), "hello_world.txt");
}

#[test]
fn test_decode_invalid_utf8() {
    let mut bytes = [0u8; 256];
    bytes[0] = 3;
    bytes[1] = 0xFF;
    bytes[2] = 0x80;
    bytes[3] = 0x80;
    let err = ItemName::decode(bytes).unwrap_err();
    assert!(matches!(err.cause, NameDecodingErrorReason::InvalidUtf8));
}

#[test]
fn test_decode_valid_utf8_bad_name() {
    let mut bytes = [0u8; 256];
    bytes[0] = 1;
    bytes[1] = b'/';
    let err = ItemName::decode(bytes).unwrap_err();
    assert!(matches!(
        err.cause,
        NameDecodingErrorReason::NameValidationFailed(NameValidationError::ForbiddenChar('/'))
    ));
}

#[test]
fn test_into_string() {
    let name = ItemName::new("test".to_owned()).unwrap();
    assert_eq!(name.into_string(), "test");
}

#[test]
fn test_display() {
    let name = ItemName::new("display_test".to_owned()).unwrap();
    assert_eq!(format!("{name}"), "display_test");
}

#[test]
fn test_deref() {
    let name = ItemName::new("deref_test".to_owned()).unwrap();
    let s: &str = &name;
    assert_eq!(s, "deref_test");
    assert_eq!(name.len(), 10);
}

#[test]
fn test_as_ref() {
    let name = ItemName::new("asref_test".to_owned()).unwrap();
    let s: &str = name.as_ref();
    assert_eq!(s, "asref_test");
}

#[test]
fn test_ordering() {
    let a = ItemName::new("apple".to_owned()).unwrap();
    let b = ItemName::new("banana".to_owned()).unwrap();
    let c = ItemName::new("cherry".to_owned()).unwrap();
    assert!(a < b);
    assert!(b < c);
    assert!(a < c);
}

#[test]
fn test_display_on_name_validation_error() {
    assert_eq!(
        format!("{}", NameValidationError::NameIsEmpty),
        "name is empty"
    );
    assert_eq!(
        format!("{}", NameValidationError::NameIsTooLong),
        "name contains more than 255 bytes"
    );
    assert_eq!(
        format!("{}", NameValidationError::ForbiddenChar('/')),
        "name contains invalid character '/'"
    );
    assert_eq!(
        format!("{}", NameValidationError::ForbiddenName(".")),
        "name is reserved: '.'"
    );
}

#[test]
fn test_display_on_name_decoding_error_reason() {
    assert_eq!(
        format!("{}", NameDecodingErrorReason::InvalidUtf8),
        "Provided name is not a valid UTF-8 string"
    );
    let validation_err = NameValidationError::NameIsEmpty;
    let reason = NameDecodingErrorReason::NameValidationFailed(validation_err);
    assert_eq!(format!("{reason}"), "Name validation failed: name is empty");
}

#[test]
fn test_display_on_name_decoding_error() {
    let err = NameDecodingError {
        bytes: vec![1, b'x'],
        cause: NameDecodingErrorReason::InvalidUtf8,
    };
    let displayed = format!("{err}");
    assert!(displayed.contains("Failed to decode name"));
    assert!(displayed.contains("2"));
    assert!(displayed.contains("not a valid UTF-8 string"));
}

#[test]
fn test_name_validation_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NameValidationError>();
}

#[test]
fn test_name_decoding_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NameDecodingError>();
}
