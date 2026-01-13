use std::io::{BufReader, Read, Seek, SeekFrom, Write};

use anyhow::{Context, Result};

/// Represent a source from which an [`crate::archive::Archive`] can be opened.
///
/// The source may be read-only or read & write.
///
/// It basically wraps an existing `Read` / `Write` stream through a buffered reader,
/// exposing utility functions that allow easier reading and writing.
pub struct Source<S: Read> {
    reader: BufReader<S>,
}

impl<S: Read> Source<S> {
    /// Wrap a stream into a source
    pub fn new(source: S) -> Self {
        Self {
            reader: BufReader::new(source),
        }
    }

    /// Read as many bytes as needed to fill the provided buffer
    ///
    /// If not enough bytes can be read, an error will be returned.
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.reader.read_exact(buf).with_context(|| {
            format!(
                "Failed to read the requested {} bytes from source",
                buf.len()
            )
        })
    }

    /// Read a constant number of bytes and return the resulting array
    ///
    /// If not enough bytes can be read, an error will be returned.
    pub fn read_into_array<const LEN: usize>(&mut self) -> Result<[u8; LEN]> {
        let mut buf = [0; LEN];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Call [`FromSourceBytes::read_from`] on `self`
    pub fn read_value<T: FromSourceBytes>(&mut self) -> Result<T> {
        T::read_from(self)
    }

    /// Get the underlying stream
    pub fn into_inner(self) -> S {
        self.reader.into_inner()
    }
}

impl<S: Read + Seek> Source<S> {
    /// Set the stream's position
    pub fn set_position(&mut self, pos: u64) -> Result<()> {
        self.reader
            .seek(SeekFrom::Start(pos))
            .with_context(|| format!("Failed to seek source at byte {pos}"))?;

        Ok(())
    }

    /// Get the current stream's position
    pub fn position(&mut self) -> Result<u64> {
        self.reader
            .stream_position()
            .context("Failed to get current cursor's position")
    }

    /// Advance the stream's position
    pub fn advance(&mut self, bytes: usize) -> Result<()> {
        self.reader
            .seek_relative(i64::try_from(bytes).unwrap())
            .with_context(|| format!("Failed to advance source of {bytes} bytes"))
    }

    /// Seek the stream's total length
    pub fn seek_len(&mut self) -> Result<u64> {
        // TODO: remove once https://github.com/rust-lang/rust/issues/59359 is resolved
        fn stream_len_default(stream: &mut impl Seek) -> Result<u64> {
            let old_pos = stream.stream_position()?;
            let len = stream.seek(SeekFrom::End(0))?;
            stream.seek(SeekFrom::Start(old_pos))?;
            Ok(len)
        }

        stream_len_default(&mut self.reader)
    }
}

// NOTE: In this impl block, we write without buffering (e.g. no `BufWriter`)
//
// The reason is that most writes are already made in chunks, and smaller ones
// involve a lot of hopping around, which would null the benefits of a buffered
// writer anyway.
impl<S: Read + Write> Source<S> {
    /// Write the provided buffer
    ///
    /// Calling [`Self::flush`] will be required to avoid losing data.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.reader
            .get_mut()
            .write_all(buf)
            .with_context(|| format!("Failed to write the provided {} bytes buffer", buf.len()))?;

        Ok(())
    }

    /// Flush all changes to the underlying stream
    pub fn flush(&mut self) -> Result<()> {
        self.reader
            .get_mut()
            .flush()
            .context("Failed to flush the source")
    }
}

/// A trait representing a value that can be read from a source
pub trait FromSourceBytes {
    /// Read the required bytes to make the value from the provided source
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized;
}

impl FromSourceBytes for u8 {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        source
            .read_into_array::<1>()
            .map(|bytes| *bytes.first().unwrap())
    }
}

impl FromSourceBytes for u16 {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        source.read_into_array::<2>().map(u16::from_le_bytes)
    }
}

impl FromSourceBytes for u32 {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        source.read_into_array::<4>().map(u32::from_le_bytes)
    }
}

impl FromSourceBytes for u64 {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        source.read_into_array::<8>().map(u64::from_le_bytes)
    }
}

impl<const N: usize, F: FromSourceBytes + Copy + Default> FromSourceBytes for [F; N] {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        let mut arr = [F::default(); N];

        for val in arr.iter_mut() {
            *val = F::read_from(source)?;
        }

        Ok(arr)
    }
}
