mod file;
mod in_memory;

pub use self::{file::RealFile, in_memory::InMemorySource};

use anyhow::Result;

#[allow(clippy::len_without_is_empty)]
pub trait ReadableSource {
    fn position(&mut self) -> Result<u64>;
    fn set_position(&mut self, addr: u64) -> Result<()>;

    fn consume_next(&mut self, bytes: u64) -> Result<Vec<u8>>;
    fn consume_next_value<F: FromSourceBytes>(&mut self) -> Result<F>
    where
        Self: Sized,
    {
        F::decode(self)
    }

    fn len(&self) -> Result<u64>;
}

#[allow(clippy::len_without_is_empty)]
pub trait WritableSource: ReadableSource {
    fn write_all(&mut self, data: &[u8]) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
}

pub trait FromSourceBytes {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
    where
        Self: Sized;
}

impl FromSourceBytes for u8 {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source.consume_next(1).map(|bytes| *bytes.first().unwrap())
    }
}

impl FromSourceBytes for u16 {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source
            .consume_next(2)
            .map(|bytes| u16::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl FromSourceBytes for u32 {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source
            .consume_next(4)
            .map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl FromSourceBytes for u64 {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source
            .consume_next(8)
            .map(|bytes| u64::from_be_bytes(bytes.try_into().unwrap()))
    }
}

impl<const N: usize, F: FromSourceBytes + Copy + Default> FromSourceBytes for [F; N] {
    fn decode(source: &mut impl ReadableSource) -> Result<Self>
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
