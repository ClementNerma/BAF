//! Collection of source types from which an archive can be read
//!
//! See [`file::RealFile`] and [`in_memory::InMemorySource`]

mod file;
mod in_memory;

pub use self::{file::RealFile, in_memory::InMemorySource};

use anyhow::Result;

/// A readable source
///
/// It contains a cursor, which starts at byte 0.
#[allow(clippy::len_without_is_empty)]
pub trait ReadableSource {
    /// Get the cursor's position (offset in bytes)
    fn position(&mut self) -> Result<u64>;

    /// Set the cursor's position (offset in bytes)
    fn set_position(&mut self, addr: u64) -> Result<()>;

    /// Consume the amount of provided bytes after the current position,
    /// then advance the cursor by the same amount of bytes.
    fn consume_next(&mut self, bytes: u64) -> Result<Vec<u8>>;

    /// Consume a value that will manipulate the source itself
    fn consume_next_value<F: FromSourceBytes>(&mut self) -> Result<F>
    where
        Self: Sized,
    {
        let pos = self.position()?;
        let result = F::decode(&mut |bytes| self.consume_next(bytes));

        // Ensure the cursor didn't go backwards
        assert!(self.position()? >= pos);

        result
    }

    /// Get the total length, in bytes
    fn len(&self) -> Result<u64>;
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
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized;
}

impl FromSourceBytes for u8 {
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        read(1).map(|bytes| *bytes.first().unwrap())
    }
}

impl FromSourceBytes for u16 {
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        read(2).map(|bytes| u16::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl FromSourceBytes for u32 {
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        read(4).map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl FromSourceBytes for u64 {
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        read(8).map(|bytes| u64::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl<const N: usize, F: FromSourceBytes + Copy + Default> FromSourceBytes for [F; N] {
    fn decode(read: &mut impl FnMut(u64) -> Result<Vec<u8>>) -> Result<Self>
    where
        Self: Sized,
    {
        let mut arr = [F::default(); N];

        for val in arr.iter_mut() {
            *val = F::decode(read)?;
        }

        Ok(arr)
    }
}
