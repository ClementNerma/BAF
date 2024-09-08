use anyhow::Result;

use crate::{diagnostic::Diagnostic, ensure_only_one_version, source::ReadableSource};

use super::{
    directory::{Directory, DIRECTORY_ENTRY_SIZE},
    file::{File, FILE_ENTRY_SIZE},
    header::SourceWithHeader,
};

/// Representation of a file table segment
pub struct FileTableSegment {
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
        input: &mut SourceWithHeader<impl ReadableSource>,
    ) -> Result<(Self, Vec<Diagnostic>)> {
        // Only there to ensure at compile time there is only one possible version
        ensure_only_one_version!(input.header.version);

        let next_segment_addr = input.source.consume_next_value::<u64>()?;

        let dirs_count = input.source.consume_next_value::<u32>()?;
        let files_count = input.source.consume_next_value::<u32>()?;

        let mut diagnostics = Vec::new();

        let dirs = (0..dirs_count)
            .map(|_| {
                input.source.position().and_then(|ft_entry_addr| {
                    Directory::consume_from_reader(input).map(|entry| {
                        entry.and_then(|dir| {
                            dir.map_err(|err| {
                                diagnostics.push(Diagnostic::InvalidItemName {
                                    is_dir: true,
                                    ft_entry_addr,
                                    error: err,
                                });
                            })
                            .ok()
                        })
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let files = (0..files_count)
            .map(|_| {
                input.source.position().and_then(|ft_entry_addr| {
                    File::consume_from_reader(input).map(|entry| {
                        entry.and_then(|file| {
                            file.map_err(|err| {
                                diagnostics.push(Diagnostic::InvalidItemName {
                                    is_dir: false,
                                    ft_entry_addr,
                                    error: err,
                                });
                            })
                            .ok()
                        })
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok((
            Self {
                next_segment_addr: match next_segment_addr {
                    0 => None,
                    _ => Some(next_segment_addr),
                },

                dirs,
                files,
            },
            diagnostics,
        ))
    }

    /// Encode a raw file segment
    pub fn encode(&self) -> Vec<u8> {
        let Self {
            next_segment_addr,
            dirs,
            files,
        } = self;

        let mut bytes = vec![];

        bytes.extend(next_segment_addr.unwrap_or(0).to_be_bytes());
        bytes.extend(u32::try_from(dirs.len()).unwrap().to_be_bytes());
        bytes.extend(u32::try_from(files.len()).unwrap().to_be_bytes());

        for dir in dirs {
            bytes.extend(match dir {
                Some(dir) => dir.encode(),
                None => vec![0; usize::try_from(DIRECTORY_ENTRY_SIZE).unwrap()],
            });
        }

        for file in files {
            bytes.extend(match file {
                Some(file) => file.encode(),
                None => vec![0; usize::try_from(FILE_ENTRY_SIZE).unwrap()],
            });
        }

        bytes
    }

    pub fn dir_entry_offset(&self, index: u32) -> u64 {
        assert!(index < u32::try_from(self.dirs.len()).unwrap());

        16 + u64::from(index) * DIRECTORY_ENTRY_SIZE
    }

    pub fn file_entry_offset(&self, index: u32) -> u64 {
        assert!(index < u32::try_from(self.files.len()).unwrap());

        16 + (u64::try_from(self.dirs.len()).unwrap() * DIRECTORY_ENTRY_SIZE)
            + (u64::from(index) * FILE_ENTRY_SIZE)
    }

    pub fn dirs(&self) -> &[Option<Directory>] {
        &self.dirs
    }

    pub fn files(&self) -> &[Option<File>] {
        &self.files
    }

    pub fn consume_next_segment(
        &self,
        input: &mut SourceWithHeader<impl ReadableSource>,
    ) -> Option<Result<(u64, Self, Vec<Diagnostic>)>> {
        self.next_segment_addr.map(|addr| {
            input.source.set_position(addr)?;
            Self::decode(input).map(|(segment, diags)| (addr, segment, diags))
        })
    }

    pub fn encoded_len(&self) -> u64 {
        16 + u64::try_from(self.dirs.len()).unwrap() * DIRECTORY_ENTRY_SIZE
            + u64::try_from(self.files.len()).unwrap() * FILE_ENTRY_SIZE
    }
}
