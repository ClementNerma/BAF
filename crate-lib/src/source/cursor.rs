use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::{Context, Result};

use super::{ConsumableSource, ReadableSource};

impl<T: AsRef<[u8]>> ConsumableSource for Cursor<T> {
    fn consume_into_buffer(&mut self, bytes: usize, buf: &mut [u8]) -> Result<()> {
        self.read_exact(&mut buf[0..bytes])
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
        stream_len_default(self).context("Failed to get data length")
    }
}

// TODO: remove once https://github.com/rust-lang/rust/issues/59359 is resolved
pub fn stream_len_default(stream: &mut impl Seek) -> Result<u64> {
    let old_pos = stream.stream_position()?;
    let len = stream.seek(SeekFrom::End(0))?;
    stream.seek(SeekFrom::Start(old_pos))?;
    Ok(len)
}
