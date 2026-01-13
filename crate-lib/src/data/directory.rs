use std::{
    io::{Read, Seek},
    num::NonZero,
};

use anyhow::Result;

use crate::{
    ensure_only_one_version,
    source::{FromSourceBytes, Source},
};

use super::{
    header::SourceWithHeader,
    name::{ItemName, NameDecodingError},
    timestamp::Timestamp,
};

pub static DIRECTORY_ENTRY_SIZE: usize = 280;
pub static DIRECTORY_NAME_OFFSET_IN_ENTRY: usize = 16;

/// Representation of a directory inside an archive
#[derive(Debug, Clone)]
pub struct Directory {
    /// Unique identifier (in the archive)
    pub id: DirectoryId,

    /// Unique identifier of the parent directory
    pub parent_dir: DirectoryIdOrRoot,

    /// Name of the file (must be valid UTF-8)
    pub name: ItemName,

    /// Modification time, in seconds since Unix' Epoch
    pub modif_time: Timestamp,
}

impl Directory {
    /// Decode a raw directory entry from an archive
    pub(crate) fn consume_from_reader(
        input: &mut SourceWithHeader<impl Read + Seek>,
    ) -> Result<Option<Self>, DirectoryDecodingError> {
        ensure_only_one_version!(input.header.version);

        let id = input
            .source
            .read_value::<u64>()
            .map_err(DirectoryDecodingError::InvalidEntry)?;

        // If an entry starts with a zero, it means its empty
        let Some(id) = NonZero::new(id) else {
            input
                .source
                .advance(DIRECTORY_ENTRY_SIZE - 8)
                .map_err(DirectoryDecodingError::IoError)?;

            return Ok(None);
        };

        let parent_dir = input
            .source
            .read_value()
            .map_err(DirectoryDecodingError::InvalidEntry)?;

        let name = ItemName::consume_from_reader(input.source)
            .map_err(DirectoryDecodingError::InvalidEntry)?
            .map_err(DirectoryDecodingError::InvalidName)?;

        let modif_time = input
            .source
            .read_value()
            .map_err(DirectoryDecodingError::InvalidEntry)?;

        Ok(Some(Self {
            id: DirectoryId(id),
            parent_dir,
            name,
            modif_time,
        }))
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

        assert_eq!(bytes.len(), DIRECTORY_ENTRY_SIZE);

        bytes
    }
}

// TODO: docs
#[derive(Debug)]
pub enum DirectoryDecodingError {
    IoError(anyhow::Error),
    InvalidEntry(anyhow::Error),
    InvalidName(NameDecodingError),
}

/// ID of a directory, unique inside a given archive
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct DirectoryId(pub(crate) NonZero<u64>);

impl DirectoryId {
    pub(crate) fn inner(&self) -> NonZero<u64> {
        self.0
    }
}

/// ID of a directory, unique inside a given archive
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DirectoryIdOrRoot {
    Root,
    NonRoot(DirectoryId),
}

impl FromSourceBytes for DirectoryIdOrRoot {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        let id = u64::read_from(source)?;

        Ok(match NonZero::new(id) {
            None => Self::Root,
            Some(id) => Self::NonRoot(DirectoryId(id)),
        })
    }
}
