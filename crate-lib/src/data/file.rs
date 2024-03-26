use anyhow::Result;

use crate::{
    data::utils::{decode_name, encode_name, none_if_zero},
    ensure_only_one_version,
    source::ReadableSource,
};

use super::header::SourceWithHeader;

pub static FILE_ENTRY_SIZE: u64 = 328;
pub static FILE_NAME_OFFSET_IN_ENTRY: u64 = 16;

#[derive(Debug, Clone)]
pub struct File {
    pub id: u64,
    pub parent_dir: Option<u64>,
    pub name: String,
    pub modif_time: u64,
    pub content_addr: u64,
    pub content_len: u64,
    pub sha3_checksum: [u8; 32],
}

impl File {
    pub fn decode(input: &mut SourceWithHeader<impl ReadableSource>) -> Result<Option<Self>> {
        ensure_only_one_version!(input.header.version);

        let id = input.source.consume_next_value()?;

        let file = Self {
            id,
            parent_dir: none_if_zero(input.source.consume_next_value()?),
            name: decode_name(input.source)?,
            modif_time: input.source.consume_next_value()?,
            content_addr: input.source.consume_next_value()?,
            content_len: input.source.consume_next_value()?,
            sha3_checksum: input.source.consume_next_value()?,
        };

        Ok(if id != 0 { Some(file) } else { None })
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
        bytes.extend(encode_name(name).unwrap());
        bytes.extend(modif_time.to_be_bytes());
        bytes.extend(content_addr.to_be_bytes());
        bytes.extend(content_len.to_be_bytes());
        bytes.extend(sha3_checksum);

        assert_eq!(u64::try_from(bytes.len()).unwrap(), FILE_ENTRY_SIZE);

        bytes
    }
}
