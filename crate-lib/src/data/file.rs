use std::num::NonZero;

use anyhow::Result;

use crate::{ensure_only_one_version, source::ReadableSource};

use super::{
    directory::DirectoryIdOrRoot,
    header::SourceWithHeader,
    name::{ItemName, NameDecodingError},
    timestamp::Timestamp,
};

pub static FILE_ENTRY_SIZE: usize = 328;
pub static FILE_NAME_OFFSET_IN_ENTRY: usize = 16;

/// Representation of a file inside an archive
#[derive(Debug, Clone)]
pub struct File {
    /// Unique identifier (in the archive)
    pub id: FileId,

    /// ID of the parent directory
    pub parent_dir: DirectoryIdOrRoot,

    /// Name of the file (must be a valid UTF-8 string)
    pub name: ItemName,

    /// Last modification time
    pub modif_time: Timestamp,

    /// Offset, in bytes inside the archive, of the file's content
    pub content_addr: u64,

    /// Length, in bytes, of the file's content
    pub content_len: u64,

    /// SHA-3 checksum of the file's content
    pub sha3_checksum: [u8; 32],
}

impl File {
    pub(crate) fn consume_from_reader(
        input: &mut SourceWithHeader<impl ReadableSource>,
    ) -> Result<Option<Self>, FileDecodingError> {
        ensure_only_one_version!(input.header.version);

        let id = input
            .source
            .consume_next_value::<u64>()
            .map_err(FileDecodingError::InvalidEntry)?;

        // If an entry starts with a zero, it means its empty
        let Some(id) = NonZero::new(id) else {
            input
                .source
                .advance(FILE_ENTRY_SIZE - 8)
                .map_err(FileDecodingError::IoError)?;

            return Ok(None);
        };

        let parent_dir = input
            .source
            .consume_next_value()
            .map_err(FileDecodingError::InvalidEntry)?;

        let name = ItemName::consume_from_reader(input.source)
            .map_err(FileDecodingError::InvalidEntry)?
            .map_err(FileDecodingError::InvalidName)?;

        let modif_time = input
            .source
            .consume_next_value()
            .map_err(FileDecodingError::InvalidEntry)?;

        let content_addr = input
            .source
            .consume_next_value()
            .map_err(FileDecodingError::InvalidEntry)?;

        let content_len = input
            .source
            .consume_next_value()
            .map_err(FileDecodingError::InvalidEntry)?;

        let sha3_checksum = input
            .source
            .consume_next_value()
            .map_err(FileDecodingError::InvalidEntry)?;

        Ok(Some(Self {
            id: FileId(id),
            parent_dir,
            name,
            modif_time,
            content_addr,
            content_len,
            sha3_checksum,
        }))
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

        bytes.extend(id.inner().get().to_le_bytes());
        bytes.extend(
            match parent_dir {
                DirectoryIdOrRoot::Root => 0,
                DirectoryIdOrRoot::NonRoot(directory_id) => directory_id.inner().get(),
            }
            .to_le_bytes(),
        );
        bytes.extend(name.encode());
        bytes.extend(modif_time.encode());
        bytes.extend(content_addr.to_le_bytes());
        bytes.extend(content_len.to_le_bytes());
        bytes.extend(sha3_checksum);

        assert_eq!(bytes.len(), FILE_ENTRY_SIZE);

        bytes
    }
}

// TODO: docs
#[derive(Debug)]
pub enum FileDecodingError {
    IoError(anyhow::Error),
    InvalidEntry(anyhow::Error),
    InvalidName(NameDecodingError),
}

/// ID of a file, unique inside a given archive
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct FileId(pub(crate) NonZero<u64>);

impl FileId {
    pub(crate) fn inner(&self) -> NonZero<u64> {
        self.0
    }
}
