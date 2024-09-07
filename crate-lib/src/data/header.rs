use anyhow::{bail, Result};

use crate::{ensure_only_one_version, source::ReadableSource};

pub static MAGIC_NUMBER: &[u8] = b"BASICARC";
pub static HEADER_SIZE: u64 = 256;

/// Representation of an archive's header
///
/// This may contain other fields in the future.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct Header {
    /// Version of the header
    pub version: ArchiveVersion,
}

impl Header {
    pub fn decode<S: ReadableSource>(source: &mut S) -> Result<SourceWithHeader<S>> {
        source.set_position(0)?;

        let got_magic_number = source.consume_next(8)?;

        if got_magic_number != MAGIC_NUMBER {
            bail!("Invalid magic number: got {got_magic_number:X?}, expected {MAGIC_NUMBER:X?}");
        }

        let version = source.consume_next_value::<u32>()?;
        let version = ArchiveVersion::decode(version)?;

        ensure_only_one_version!(version);

        let bytes = HEADER_SIZE - source.position()?;

        if source.consume_next(bytes)?.iter().any(|b| *b != 0) {
            bail!("Rest of the header is not filled with zeroes");
        }

        assert_eq!(source.position()?, 256);

        let header = Self { version };

        Ok(SourceWithHeader { source, header })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend(MAGIC_NUMBER);
        bytes.extend(self.version.encode().to_be_bytes());
        bytes.extend(vec![0; 256 - bytes.len()]);

        assert_eq!(bytes.len(), 256);

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

#[derive(Clone, Copy)]
pub enum ArchiveVersion {
    One,
}

impl ArchiveVersion {
    pub fn decode(input: u32) -> Result<ArchiveVersion> {
        match input {
            1 => Ok(Self::One),
            _ => bail!("Unknown archive version: {input:X?}"),
        }
    }

    pub fn encode(&self) -> u32 {
        match self {
            ArchiveVersion::One => 1,
        }
    }
}

pub struct SourceWithHeader<'s, S: ReadableSource> {
    pub source: &'s mut S,
    pub header: Header,
}
