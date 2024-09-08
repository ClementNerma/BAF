use anyhow::{bail, Result};

use super::{ConsumableSource, ReadableSource, WritableSource};

/// An archive reprensentation that's stored exclusively in memory
///
/// It does not persist any data.
/// If you want to store an archive with a file, see [`super::RealFile`] instead.
pub struct InMemorySource {
    data: Vec<u8>,
    position: u64,
}

impl InMemorySource {
    /// Create an empty source
    pub fn new() -> Self {
        Self::from_data(vec![])
    }

    /// Create a new memory source from an existing set of data
    ///
    /// Please note that the data's content is not validated for validity.
    /// Please use at your own risk.
    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    /// Get the size of the archive in memory (in bytes)
    pub fn size(&self) -> u64 {
        u64::try_from(self.data.len()).unwrap()
    }

    /// Get the underlying array of bytes representing the archive
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Default for InMemorySource {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsumableSource for InMemorySource {
    fn consume_into_buffer(&mut self, bytes: u64, buf: &mut [u8]) -> Result<()> {
        if self.position + bytes > self.size() {
            bail!("End of input");
        }

        let position = usize::try_from(self.position).unwrap();
        let bytes_usize = usize::try_from(bytes).unwrap();
        let slice = &self.data[position..position + bytes_usize];

        self.position += bytes;

        buf[0..bytes_usize].copy_from_slice(slice);

        Ok(())
    }
}

impl ReadableSource for InMemorySource {
    fn position(&mut self) -> Result<u64> {
        Ok(self.position)
    }

    fn set_position(&mut self, addr: u64) -> Result<()> {
        assert!(addr <= self.size(), "{addr} > {}", self.size());
        self.position = addr;
        Ok(())
    }

    fn len(&self) -> Result<u64> {
        Ok(self.size())
    }
}

impl WritableSource for InMemorySource {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        if self.position < self.size() {
            let position = usize::try_from(self.position).unwrap();
            self.data[position..position + data.len()].copy_from_slice(data);
        } else {
            self.data.extend(data);
        }

        self.position += u64::try_from(data.len()).unwrap();

        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
