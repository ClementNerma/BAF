use std::io::{Error, Read};

use sha3::{Digest, Sha3_256};

use crate::source::ReadableSource;

/// Abstraction over a file with checksum verification
///
/// Designed to be used for reading / extracting files from BAF archives.
///
/// **NOTE:** Checksum validation only occurs *after* the very last byte has been read.
pub struct FileReader<'a, S: ReadableSource> {
    source: &'a mut S,
    len: u64,
    expected_checksum: [u8; 32],
    pending_checksum: Sha3_256,
    pos: u64,
}

impl<'a, S: ReadableSource> FileReader<'a, S> {
    pub(crate) fn new(source: &'a mut S, len: u64, expected_checksum: [u8; 32]) -> Self {
        Self {
            source,
            len,
            expected_checksum,
            pending_checksum: Sha3_256::new(),
            pos: 0,
        }
    }
}

impl<'a, S: ReadableSource> Read for FileReader<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // TODO: some typecasts are unneeded in this function
        let read_len = std::cmp::min(u64::try_from(buf.len()).unwrap(), self.len - self.pos);
        let read_len_usize = usize::try_from(read_len).unwrap();

        let bytes = self
            .source
            .consume_into_vec(usize::try_from(read_len).unwrap())
            .map_err(|err| Error::other(format!("{err:?}")))?;

        buf[0..read_len_usize].copy_from_slice(&bytes);

        self.pending_checksum.update(&bytes);

        self.pos += read_len;

        // When the entire file has been read, check its validity by comparing the checksums
        if self.pos == self.len {
            let hash: [u8; 32] = self.pending_checksum.clone().finalize().into();

            if hash != self.expected_checksum {
                return Err(Error::other(format!(
                    "File's hash doesn't match: expected {:#?}, got {hash:#?}",
                    self.expected_checksum
                )));
            }
        }

        Ok(read_len_usize)
    }
}
