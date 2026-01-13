use std::io::{Read, Seek};

use crate::ensure_only_one_version;

use super::{
    directory::{DIRECTORY_ENTRY_SIZE, Directory, DirectoryDecodingError},
    file::{FILE_ENTRY_SIZE, File, FileDecodingError},
    header::SourceWithHeader,
};

/// Representation of a file table segment
#[derive(Debug)]
pub(crate) struct FileTableSegment {
    /// Address of the next segment inside the archive
    pub next_segment_addr: Option<u64>,

    /// List of directory slots (each one may be filled or not)
    pub dirs: Vec<Option<Directory>>,

    /// List of file slots (eah one may be filled or not)
    pub files: Vec<Option<File>>,
}

impl FileTableSegment {
    /// Decode a raw file table segment
    pub fn decode(
        input: &mut SourceWithHeader<impl Read + Seek>,
    ) -> Result<Self, FileTableSegmentDecodingError> {
        // Only there to ensure at compile time there is only one possible version
        ensure_only_one_version!(input.header.version);

        let next_segment_addr = input
            .source
            .read_value::<u64>()
            .map_err(FileTableSegmentDecodingError::InvalidHeader)?;

        let dirs_count = input
            .source
            .read_value::<u32>()
            .map_err(FileTableSegmentDecodingError::InvalidHeader)?;

        let files_count = input
            .source
            .read_value::<u32>()
            .map_err(FileTableSegmentDecodingError::InvalidHeader)?;

        let dirs = (0..dirs_count)
            .map(|_| {
                input
                    .source
                    .position()
                    .map_err(FileTableSegmentDecodingError::IoError)
                    .and_then(|ft_entry_addr| {
                        Directory::consume_from_reader(input).map_err(|err| {
                            FileTableSegmentDecodingError::InvalidDirectoryEntry {
                                ft_entry_addr,
                                err,
                            }
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let files = (0..files_count)
            .map(|_| {
                input
                    .source
                    .position()
                    .map_err(FileTableSegmentDecodingError::IoError)
                    .and_then(|ft_entry_addr| {
                        File::consume_from_reader(input).map_err(|err| {
                            FileTableSegmentDecodingError::InvalidFileEntry { ft_entry_addr, err }
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            next_segment_addr: match next_segment_addr {
                0 => None,
                _ => Some(next_segment_addr),
            },

            dirs,
            files,
        })
    }

    /// Encode a raw file segment
    pub fn encode(&self) -> Vec<u8> {
        let Self {
            next_segment_addr,
            dirs,
            files,
        } = self;

        let mut bytes = vec![];

        bytes.extend(next_segment_addr.unwrap_or(0).to_le_bytes());
        bytes.extend(u32::try_from(dirs.len()).unwrap().to_le_bytes());
        bytes.extend(u32::try_from(files.len()).unwrap().to_le_bytes());

        for dir in dirs {
            bytes.extend(match dir {
                Some(dir) => dir.encode(),
                None => vec![0; DIRECTORY_ENTRY_SIZE],
            });
        }

        for file in files {
            bytes.extend(match file {
                Some(file) => file.encode(),
                None => vec![0; FILE_ENTRY_SIZE],
            });
        }

        bytes
    }

    pub fn dir_entry_offset(&self, index: u32) -> u64 {
        assert!(index < u32::try_from(self.dirs.len()).unwrap());

        16 + u64::from(index) * (DIRECTORY_ENTRY_SIZE as u64)
    }

    pub fn file_entry_offset(&self, index: u32) -> u64 {
        assert!(index < u32::try_from(self.files.len()).unwrap());

        16 + (u64::try_from(self.dirs.len()).unwrap() * (DIRECTORY_ENTRY_SIZE as u64))
            + (u64::from(index) * (FILE_ENTRY_SIZE as u64))
    }

    pub fn dirs(&self) -> &[Option<Directory>] {
        &self.dirs
    }

    pub fn files(&self) -> &[Option<File>] {
        &self.files
    }

    pub fn consume_next_segment(
        &self,
        input: &mut SourceWithHeader<impl Read + Seek>,
    ) -> Option<Result<(u64, Self), FileTableSegmentDecodingError>> {
        self.next_segment_addr.map(|addr| {
            input
                .source
                .set_position(addr)
                .map_err(FileTableSegmentDecodingError::IoError)?;

            Self::decode(input).map(|segment| (addr, segment))
        })
    }

    pub fn encoded_len(&self) -> u64 {
        16 + u64::try_from(self.dirs.len()).unwrap() * u64::try_from(DIRECTORY_ENTRY_SIZE).unwrap()
            + u64::try_from(self.files.len()).unwrap() * (FILE_ENTRY_SIZE as u64)
    }
}

#[derive(Debug)]
pub enum FileTableSegmentDecodingError {
    // TODO: add context
    InvalidHeader(anyhow::Error),
    IoError(anyhow::Error),
    InvalidDirectoryEntry {
        ft_entry_addr: u64,
        err: DirectoryDecodingError,
    },
    InvalidFileEntry {
        ft_entry_addr: u64,
        err: FileDecodingError,
    },
}
