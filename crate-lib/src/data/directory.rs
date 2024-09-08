use anyhow::Result;

use crate::{ensure_only_one_version, source::ReadableSource};

use super::{
    header::SourceWithHeader,
    name::{ItemName, NameDecodingError},
    timestamp::Timestamp,
};

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
    pub name: ItemName,

    /// Modification time, in seconds since Unix' Epoch
    pub modif_time: Timestamp,
}

impl Directory {
    /// Decode a raw directory entry from an archive
    pub fn consume_from_reader(
        input: &mut SourceWithHeader<impl ReadableSource>,
    ) -> Result<Option<Result<Self, NameDecodingError>>> {
        ensure_only_one_version!(input.header.version);

        let id = input.source.consume_next_value()?;
        let parent_dir = input.source.consume_next_value()?;
        let name = ItemName::consume_from_reader(input.source)?;
        let modif_time = input.source.consume_next_value()?;

        if id == 0 {
            return Ok(None);
        }

        let dir = Self {
            id,
            parent_dir: match parent_dir {
                0 => None,
                _ => Some(parent_dir),
            },
            name: match name {
                Ok(name) => name,
                Err(err) => return Ok(Some(Err(err))),
            },
            modif_time,
        };

        Ok(if id != 0 { Some(Ok(dir)) } else { None })
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
        bytes.extend(name.encode());
        bytes.extend(modif_time.encode());

        assert_eq!(u64::try_from(bytes.len()).unwrap(), DIRECTORY_ENTRY_SIZE);

        bytes
    }
}

pub struct DirectoryNameDecodingError {
    pub dir_id: u64,
    pub ft_entry_addr: u64,
    pub error: NameDecodingError,
}
