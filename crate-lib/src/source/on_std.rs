use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};

use anyhow::{Context, Result};

use super::{ConsumableSource, FromSourceBytes, ReadableSource};

impl<T: Read + Seek> ConsumableSource for BufReader<T> {
    fn consume_into_buffer(&mut self, bytes: usize, buf: &mut [u8]) -> Result<()> {
        self.read_exact(&mut buf[0..bytes])
            .context("Failed to read from BufReader")
    }
}

impl<T: Read + Seek> ReadableSource for BufReader<T> {
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
fn stream_len_default(stream: &mut impl Seek) -> Result<u64> {
    let old_pos = stream.stream_position()?;
    let len = stream.seek(SeekFrom::End(0))?;
    stream.seek(SeekFrom::Start(old_pos))?;
    Ok(len)
}

impl FromSourceBytes for u8 {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source
            .consume_to_array::<1>()
            .map(|bytes| *bytes.first().unwrap())
    }
}

impl FromSourceBytes for u16 {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source.consume_to_array::<2>().map(u16::from_le_bytes)
    }
}

impl FromSourceBytes for u32 {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source.consume_to_array::<4>().map(u32::from_le_bytes)
    }
}

impl FromSourceBytes for u64 {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source.consume_to_array::<8>().map(u64::from_le_bytes)
    }
}

impl<const N: usize, F: FromSourceBytes + Copy + Default> FromSourceBytes for [F; N] {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        let mut arr = [F::default(); N];

        for val in arr.iter_mut() {
            *val = F::decode(source)?;
        }

        Ok(arr)
    }
}
