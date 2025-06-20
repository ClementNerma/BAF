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
    /// Decode the header of an archive from a readable source
    pub fn decode<S: ReadableSource>(source: &mut S) -> Result<SourceWithHeader<'_, S>> {
        source.set_position(0)?;

        let got_magic_number = source.consume_to_array::<8>()?;

        if got_magic_number != MAGIC_NUMBER {
            bail!("Invalid magic number: got {got_magic_number:X?}, expected {MAGIC_NUMBER:X?}");
        }

        let version = source.consume_next_value::<u32>()?;
        let version = ArchiveVersion::decode(version)?;

        ensure_only_one_version!(version);

        let bytes = HEADER_SIZE - source.position()?;

        let mut buf = [0; 256];

        source.consume_into_buffer(bytes, &mut buf)?;

        if buf.iter().any(|b| *b != 0) {
            bail!("Rest of the header is not filled with zeroes");
        }

        assert_eq!(source.position()?, 256);

        let header = Self { version };

        Ok(SourceWithHeader { source, header })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend(MAGIC_NUMBER);
        bytes.extend(self.version.encode());
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
pub struct SourceWithHeader<'s, S: ReadableSource> {
    /// Source the header was read from
    pub source: &'s mut S,

    /// Decoded and validated header
    pub header: Header,
}
