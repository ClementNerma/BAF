use std::io::{Read, Seek};

use thiserror::Error;

use crate::{ensure_only_one_version, source::Source};

pub static MAGIC_NUMBER: &[u8] = b"BASICARC";
pub static HEADER_SIZE: usize = 256;

/// Representation of an archive's header
///
/// This may contain other fields in the future.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Header {
    /// Version of the header
    pub version: ArchiveVersion,
}

impl Header {
    /// Decode the header of an archive from a readable source
    pub fn decode<S: Read + Seek>(
        source: &mut Source<S>,
    ) -> Result<SourceWithHeader<'_, S>, HeaderDecodingError> {
        source.set_position(0)?;

        let got_magic_number = source.read_into_array::<8>()?;

        if got_magic_number != MAGIC_NUMBER {
            return Err(HeaderDecodingError::InvalidMagicNumber {
                got: u32::from_le_bytes([
                    got_magic_number[0],
                    got_magic_number[1],
                    got_magic_number[2],
                    got_magic_number[3],
                ]),
                expected: u32::from_le_bytes([
                    MAGIC_NUMBER[0],
                    MAGIC_NUMBER[1],
                    MAGIC_NUMBER[2],
                    MAGIC_NUMBER[3],
                ]),
            });
        }

        let version = source.read_value::<u32>()?;
        let version = ArchiveVersion::decode(version)?;

        ensure_only_one_version!(version);

        let padding_len = (HEADER_SIZE as u64) - source.position()?;
        let padding_len = usize::try_from(padding_len).unwrap();

        let mut buf = [0; 256];
        source.read_exact(&mut buf[0..padding_len])?;

        if buf.iter().any(|b| *b != 0) {
            return Err(HeaderDecodingError::NonZeroPadding);
        }

        debug_assert_eq!(source.position()?, HEADER_SIZE as u64);

        let header = Self { version };

        Ok(SourceWithHeader { source, header })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend(MAGIC_NUMBER);
        bytes.extend(self.version.encode());
        bytes.extend(vec![0; 256 - bytes.len()]);

        debug_assert_eq!(bytes.len(), HEADER_SIZE);

        bytes
    }
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: ArchiveVersion::One,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ArchiveVersion {
    One,
}

impl ArchiveVersion {
    pub fn decode(input: u32) -> Result<ArchiveVersion, HeaderDecodingError> {
        match input {
            1 => Ok(Self::One),
            _ => Err(HeaderDecodingError::UnknownVersion { input }),
        }
    }

    pub fn version_number(&self) -> u32 {
        match self {
            ArchiveVersion::One => 1,
        }
    }

    pub fn encode(&self) -> [u8; 4] {
        self.version_number().to_le_bytes()
    }
}

/// A mutable reference to a readable source along with the read archive's header
///
/// The readable source's cursor will have advanced by the header's length
#[derive(Debug)]
pub struct SourceWithHeader<'s, S: Read> {
    /// Source the header was read from
    pub source: &'s mut Source<S>,

    /// Decoded and validated header
    pub header: Header,
}

/// Error while decoding an archive header
#[derive(Error, Debug)]
pub enum HeaderDecodingError {
    /// Native I/O error while reading the header
    #[error("I/O error reading header: {0}")]
    Io(#[from] std::io::Error),

    /// Magic number does not match
    #[error("Invalid magic number: got {got:X?}, expected {expected:X?}")]
    InvalidMagicNumber {
        /// Magic number that was read
        got: u32,
        /// Expected magic number
        expected: u32,
    },

    /// Header padding is not filled with zeroes
    #[error("Header padding is not filled with zeroes")]
    NonZeroPadding,

    /// The archive version is unknown/unsupported
    #[error("Unknown archive version: {input}")]
    UnknownVersion {
        /// Raw version value that was read
        input: u32,
    },
}
