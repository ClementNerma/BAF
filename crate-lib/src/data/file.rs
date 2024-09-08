use anyhow::Result;

use crate::{ensure_only_one_version, source::ReadableSource};

use super::{
    header::SourceWithHeader,
    name::{ItemName, NameDecodingError},
};

pub static FILE_ENTRY_SIZE: u64 = 328;
pub static FILE_NAME_OFFSET_IN_ENTRY: u64 = 16;

#[derive(Debug, Clone)]
pub struct File {
    pub id: u64,
    pub parent_dir: Option<u64>,
    pub name: ItemName,
    pub modif_time: u64,
    pub content_addr: u64,
    pub content_len: u64,
    pub sha3_checksum: [u8; 32],
}

impl File {
    pub fn consume_from_reader(
        input: &mut SourceWithHeader<impl ReadableSource>,
    ) -> Result<Option<Result<Self, NameDecodingError>>> {
        ensure_only_one_version!(input.header.version);

        let id = input.source.consume_next_value()?;
        let parent_dir = input.source.consume_next_value()?;
        let name = ItemName::consume_from_reader(input.source)?;
        let modif_time = input.source.consume_next_value()?;
        let content_addr = input.source.consume_next_value()?;
        let content_len = input.source.consume_next_value()?;
        let sha3_checksum = input.source.consume_next_value()?;

        if id == 0 {
            return Ok(None);
        }

        let file = Self {
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
            content_addr,
            content_len,
            sha3_checksum,
        };

        Ok(if id != 0 { Some(Ok(file)) } else { None })
    }

    pub fn encode(&self) -> Vec<u8> {
        let Self {
            id,
            parent_dir,
            name,
            modif_time,
            content_addr,
            content_len,
            sha3_checksum,
        } = self;

        let mut bytes = vec![];

        bytes.extend(id.to_be_bytes());
        bytes.extend(parent_dir.unwrap_or(0).to_be_bytes());
        bytes.extend(name.encode());
        bytes.extend(modif_time.to_be_bytes());
        bytes.extend(content_addr.to_be_bytes());
        bytes.extend(content_len.to_be_bytes());
        bytes.extend(sha3_checksum);

        assert_eq!(u64::try_from(bytes.len()).unwrap(), FILE_ENTRY_SIZE);

        bytes
    }
}
