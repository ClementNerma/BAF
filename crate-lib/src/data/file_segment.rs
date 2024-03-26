use anyhow::Result;

use crate::{data::utils::none_if_zero, ensure_only_one_version, source::ReadableSource};

use super::{
    directory::{Directory, DIRECTORY_ENTRY_SIZE},
    file::{File, FILE_ENTRY_SIZE},
    header::SourceWithHeader,
};

pub struct FileSegment {
    pub next_segment_addr: Option<u64>,
    pub dirs: Vec<Option<Directory>>,
    pub files: Vec<Option<File>>,
}

impl FileSegment {
    pub fn decode(input: &mut SourceWithHeader<impl ReadableSource>) -> Result<Self> {
        // Only there to ensure at compile time there is only one possible version
        ensure_only_one_version!(input.header.version);

        let next_segment_addr = input.source.consume_next_value::<u64>()?;

        let dirs_count = input.source.consume_next_value::<u32>()?;
        let files_count = input.source.consume_next_value::<u32>()?;

        Ok(Self {
            next_segment_addr: none_if_zero(next_segment_addr),

            dirs: (0..dirs_count)
                .map(|_| Directory::decode(input))
                .collect::<Result<Vec<_>, _>>()?,

            files: (0..files_count)
                .map(|_| File::decode(input))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

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
    ) -> Option<Result<(u64, Self)>> {
        self.next_segment_addr.map(|addr| {
            input.source.set_position(addr)?;
            Self::decode(input).map(|segment| (addr, segment))
        })
    }

    pub fn encoded_len(&self) -> u64 {
        16 + u64::try_from(self.dirs.len()).unwrap() * DIRECTORY_ENTRY_SIZE
            + u64::try_from(self.files.len()).unwrap() * FILE_ENTRY_SIZE
    }
}
