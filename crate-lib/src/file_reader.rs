use std::io::{Error, ErrorKind, Read};

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

    fn validate_checksum_after_reading(&self) -> std::io::Result<()> {
        assert_eq!(self.pos, self.len);

        let hash: [u8; 32] = self.pending_checksum.clone().finalize().into();

        if hash != self.expected_checksum {
            Err(Error::new(
                ErrorKind::Other,
                format!(
                    "File's hash doesn't match: expected {:#?}, got {hash:#?}",
                    self.expected_checksum
                ),
            ))
        } else {
            Ok(())
        }
    }
}

impl<'a, S: ReadableSource> Read for FileReader<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.len == 0 {
            self.validate_checksum_after_reading()?;
            return Ok(0);
        }

        let read_len = std::cmp::min(u64::try_from(buf.len()).unwrap(), self.len - self.pos);
        let read_len_usize = usize::try_from(read_len).unwrap();

        let bytes = self
            .source
            .consume_next(read_len)
            .map_err(|err| Error::new(ErrorKind::Other, format!("{err:?}")))?;

        buf[0..read_len_usize].copy_from_slice(&bytes);

        self.pending_checksum.update(&bytes);

        self.pos += read_len;

        if self.pos == self.len {
            self.validate_checksum_after_reading()?;
        }

        Ok(read_len_usize)
    }
}
