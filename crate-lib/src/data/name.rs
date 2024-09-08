use std::{borrow::Borrow, fmt::Display, ops::Deref};

use anyhow::Result;

use crate::source::ReadableSource;

/// Representation of an item's (file or directory) name
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemName(String);

impl ItemName {
    /// Represent an item name
    pub fn new(name: String) -> Result<Self, NameValidationError> {
        Self::check_validity(&name).map(|()| Self(name))
    }

    /// Check if a name is a valid archive item's name
    pub fn check_validity(name: &str) -> Result<(), NameValidationError> {
        if name.is_empty() {
            return Err(NameValidationError::NameIsEmpty);
        }

        if name.len() > 255 {
            return Err(NameValidationError::NameIsTooLong);
        }

        for char in ['/', '\\', '\n', '\r', '\0'] {
            if name.contains(char) {
                return Err(NameValidationError::ForbiddenChar(char));
            }
        }

        Ok(())
    }

    pub fn consume_from_reader(
        source: &mut impl ReadableSource,
    ) -> Result<Result<Self, NameDecodingError>> {
        source.consume_next_value::<[u8; 256]>().map(Self::decode)
    }

    /// Decode an item name from a list of bytes
    pub fn decode(bytes: [u8; 256]) -> Result<Self, NameDecodingError> {
        let len = usize::from(bytes[0]);

        let name = std::str::from_utf8(&bytes[1..=len]).map_err(|_| NameDecodingError {
            bytes: bytes.to_vec(),
            cause: NameDecodingErrorReason::InvalidUtf8,
        })?;

        Self::new(name.to_owned()).map_err(|err| NameDecodingError {
            bytes: bytes.to_vec(),
            cause: NameDecodingErrorReason::NameValidationFailed(err),
        })
    }

    /// Encode the name as a list of bytes
    pub fn encode(&self) -> [u8; 256] {
        let Self(name) = &self;

        let mut bytes = [0; 256];

        bytes[0] = u8::try_from(name.len()).unwrap();
        bytes[1..=name.len()].copy_from_slice(name.as_bytes());

        bytes
    }

    /// Consume the value to get the underlying string
    pub fn into_string(self) -> String {
        let Self(string) = self;

        string
    }
}

impl Deref for ItemName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for ItemName {
    fn borrow(&self) -> &str {
        let Self(name) = &self;
        name
    }
}

/// Error that occurred during name decoding
#[derive(Debug)]
pub struct NameDecodingError {
    /// Provided name bytes
    pub bytes: Vec<u8>,

    /// Cause of the error
    pub cause: NameDecodingErrorReason,
}

/// Cause of a name decoding error
#[derive(Debug)]
pub enum NameDecodingErrorReason {
    /// The provided bytes do not form a valid UTF-8 string
    InvalidUtf8,

    /// Name is invalid
    NameValidationFailed(NameValidationError),
}

/// Cause of a name validation error
#[derive(Debug)]
pub enum NameValidationError {
    /// The name is empty
    NameIsEmpty,

    /// The name is too long (= longer than 255 bytes)
    NameIsTooLong,

    /// A forbidden character was found in the name
    ForbiddenChar(char),
}

impl Display for NameDecodingErrorReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUtf8 => write!(f, "Provided name is not a valid UTF-8 string"),
            Self::NameValidationFailed(err) => write!(f, "Name validation failed: {err}"),
        }
    }
}

impl Display for NameValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NameIsEmpty => write!(f, "name is empty"),
            Self::NameIsTooLong => write!(f, "name contains more than 255 bytes"),
            Self::ForbiddenChar(c) => write!(f, "name contains invalid character {c:?}"),
        }
    }
}
