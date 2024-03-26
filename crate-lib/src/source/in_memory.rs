use anyhow::{bail, Result};

use super::{ReadableSource, WritableSource};

pub struct InMemorySource {
    data: Vec<u8>,
    position: u64,
}

impl InMemorySource {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    pub fn empty() -> Self {
        Self::new(vec![])
    }

    pub fn size(&self) -> u64 {
        u64::try_from(self.data.len()).unwrap()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Default for InMemorySource {
    fn default() -> Self {
        Self::empty()
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

    fn consume_next(&mut self, bytes: u64) -> Result<Vec<u8>> {
        if self.position + bytes > self.size() {
            bail!("End of input");
        }

        let position = usize::try_from(self.position).unwrap();
        let bytes_usize = usize::try_from(bytes).unwrap();
        let slice = &self.data[position..position + bytes_usize];

        self.position += bytes;

        Ok(slice.to_vec())
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
