use anyhow::Result;

use crate::{
    data::utils::{decode_name_from_source, encode_name, none_if_zero},
    ensure_only_one_version,
    source::ReadableSource,
};

use super::header::SourceWithHeader;

pub static DIRECTORY_ENTRY_SIZE: u64 = 280;
pub static DIRECTORY_NAME_OFFSET_IN_ENTRY: u64 = 16;

/// Representation of a directory inside an archive
#[derive(Debug, Clone)]
pub struct Directory {
    /// Unique identifier (in the archive)
    pub id: u64,

    /// Unique identifier of the parent directory
    pub parent_dir: Option<u64>,

    /// Name of the file (must be valid UTF-8)
    pub name: String,

    /// Modification time, in seconds since Unix' Epoch
    pub modif_time: u64,
}

impl Directory {
    /// Decode a raw directory entry from an archive
    pub fn decode(input: &mut SourceWithHeader<impl ReadableSource>) -> Result<Option<Self>> {
        ensure_only_one_version!(input.header.version);

        let directory = Self {
            id: input.source.consume_next_value()?,
            parent_dir: none_if_zero(input.source.consume_next_value()?),
            name: decode_name_from_source(input.source)?,
            modif_time: input.source.consume_next_value()?,
        };

        Ok(if directory.id != 0 {
            Some(directory)
        } else {
            None
        })
    }

    /// Encode as a raw directory entry
    pub fn encode(&self) -> Vec<u8> {
        let Self {
            id,
            parent_dir,
            name,
            modif_time,
        } = self;

        let mut bytes = vec![];

        bytes.extend(id.to_be_bytes());
        bytes.extend(parent_dir.unwrap_or(0).to_be_bytes());
        bytes.extend(encode_name(name).unwrap());
        bytes.extend(modif_time.to_be_bytes());

        assert_eq!(u64::try_from(bytes.len()).unwrap(), DIRECTORY_ENTRY_SIZE);

        bytes
    }
}
