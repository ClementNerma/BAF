use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::{Context, Result};

use super::{ConsumableSource, ReadableSource};

impl<T: AsRef<[u8]>> ConsumableSource for Cursor<T> {
    fn consume_into_buffer(&mut self, bytes: u64, buf: &mut [u8]) -> Result<()> {
        self.read_exact(&mut buf[0..usize::try_from(bytes).unwrap()])
            .context("Failed to read from inner cursor")
    }
}

impl<T: AsRef<[u8]>> ReadableSource for Cursor<T> {
    fn position(&mut self) -> Result<u64> {
        self.stream_position()
            .context("Failed to get current cursor's position")
    }

    fn set_position(&mut self, addr: u64) -> Result<()> {
        self.seek(SeekFrom::Start(addr))
            .context("Failed to move cursor's position")?;

        Ok(())
    }

    fn len(&mut self) -> Result<u64> {
        // NOTE: cursor's position may have changed in case of failure
        self.stream_len().context("Failed to get data length")
    }
}
