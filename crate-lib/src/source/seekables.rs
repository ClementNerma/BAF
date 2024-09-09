use std::io::{BufReader, Read, Seek, SeekFrom};

use anyhow::{Context, Result};

use super::{ConsumableSource, ReadableSource};

pub struct SeekWrapper<T: Read + Seek> {
    reader: BufReader<T>,
}

impl<T: Read + Seek> SeekWrapper<T> {
    pub fn new(reader: T) -> Self {
        Self {
            reader: BufReader::new(reader),
        }
    }
}

impl<T: Read + Seek> ConsumableSource for SeekWrapper<T> {
    fn consume_into_buffer(&mut self, bytes: u64, buf: &mut [u8]) -> Result<()> {
        self.reader
            .read_exact(&mut buf[0..usize::try_from(bytes).unwrap()])
            .context("Failed to read from BufReader")
    }
}

impl<T: Read + Seek> ReadableSource for SeekWrapper<T> {
    fn position(&mut self) -> Result<u64> {
        self.reader
            .stream_position()
            .context("Failed to get current cursor's position")
    }

    fn set_position(&mut self, addr: u64) -> Result<()> {
        self.reader
            .seek(SeekFrom::Start(addr))
            .context("Failed to move cursor's position")?;

        Ok(())
    }

    fn len(&mut self) -> Result<u64> {
        // NOTE: cursor's position may have changed in case of failure
        self.reader
            .stream_len()
            .context("Failed to get data length")
    }
}
