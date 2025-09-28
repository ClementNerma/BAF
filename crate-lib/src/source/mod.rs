//! Collection of source types from which an archive can be read
//!
//! See [`file::RealFile`] and [`in_memory::InMemorySource`]

mod cursor;
mod in_memory;
mod real_file;
mod seekables;

use std::num::{NonZero, NonZeroU64};

pub use self::{
    in_memory::InMemoryData,
    real_file::{ReadonlyFile, RealFile, WriteableFile},
    seekables::SeekWrapper,
};

use anyhow::{Context, Result};

/// A source that allows consuming data
///
/// This trait is used to prevent data-consuming types to move the cursor's position by themselves
pub trait ConsumableSource {
    /// Consume the amount of provided bytes after the current position,
    /// then advance the cursor by the same amount of bytes.
    fn consume_into_buffer(&mut self, bytes: usize, buf: &mut [u8]) -> Result<()>;

    /// Consume precisely n bytes, discard the result
    fn advance(&mut self, bytes: usize) -> Result<()> {
        let mut buf = vec![0u8; bytes];
        self.consume_into_buffer(bytes, &mut buf)?;
        Ok(())
    }

    /// Consume the amount of provided bytes after the current position,
    /// then advance the cursor by the same amount of bytes.
    fn consume_to_array<const BYTES: usize>(&mut self) -> Result<[u8; BYTES]> {
        let mut buf = [0; BYTES];
        self.consume_into_buffer(BYTES, &mut buf)?;
        Ok(buf)
    }

    /// Consume the amount of provided bytes after the current position into a vector,
    /// then advance the cursor by the same amount of bytes.
    fn consume_into_vec(&mut self, bytes: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0; bytes];
        self.consume_into_buffer(bytes, &mut buf)?;
        assert_eq!(buf.len(), bytes);
        Ok(buf)
    }

    /// Consume a value that will manipulate the source itself
    fn consume_next_value<F: FromSourceBytes>(&mut self) -> Result<F>
    where
        Self: Sized,
    {
        F::decode(self)
    }
}

/// A readable source
///
/// It contains a cursor, which starts at byte 0.
#[allow(clippy::len_without_is_empty)]
pub trait ReadableSource: ConsumableSource {
    /// Get the cursor's position (offset in bytes)
    fn position(&mut self) -> Result<u64>;

    /// Set the cursor's position (offset in bytes)
    fn set_position(&mut self, addr: u64) -> Result<()>;

    /// Get the total length, in bytes
    fn len(&mut self) -> Result<u64>;
}

/// A writable wource
///
/// It acts as a [`ReadableSource`] that also happens to be writable at the same time.
#[allow(clippy::len_without_is_empty)]
pub trait WritableSource: ReadableSource {
    /// Write all the provided data and advance the cursor by the provided data's length
    ///
    /// Writes don't need to be persisted (e.g. to the disk) before a call to [`WritableSource::flush`] occurs.
    fn write_all(&mut self, data: &[u8]) -> Result<()>;

    /// Save all changes (e.g. to the disk)
    ///
    /// This function may not return before changes have been throroughly saved.
    ///
    /// This allows the program to exit after ensuring the archive is in a consistent state.
    fn flush(&mut self) -> Result<()>;
}

/// Type that can manipulate a readable source to get data
pub trait FromSourceBytes {
    /// Decode the value type from a readable source
    ///
    /// The provided function allows to try to read the next amount of the provided bytes,
    /// advancing the reader's cursor from the same amount in the process.
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized;
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

impl FromSourceBytes for NonZero<u64> {
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        let num = source.consume_to_array::<8>().map(u64::from_le_bytes)?;
        NonZeroU64::new(num).context("Integer should be non-zero")
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
