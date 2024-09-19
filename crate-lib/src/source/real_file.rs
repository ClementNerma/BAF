use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, Result};

use super::{ConsumableSource, ReadableSource, WritableSource};

/// Representation of a real file (e.g. on-disk)
///
/// Uses buffer reading and writing
pub struct RealFile<const WRITABLE: bool> {
    file: File,
    buffered: Buffered,
    position: u64,
}

pub type ReadonlyFile = RealFile<false>;
pub type WriteableFile = RealFile<true>;

impl<const WRITABLE: bool> RealFile<WRITABLE> {
    fn open_file(
        path: impl AsRef<Path>,
        map: impl FnOnce(&mut OpenOptions) -> &mut OpenOptions,
    ) -> Result<RealFile<WRITABLE>> {
        let file = map(OpenOptions::new()
            .truncate(false)
            .read(true)
            .write(WRITABLE))
        .open(path)?;

        Ok(RealFile {
            buffered: Buffered::writer(&file)?,
            file,
            position: 0,
        })
    }

    /// Get a buffered reader
    fn reader(&mut self) -> Result<&mut BufReader<File>> {
        match self.buffered {
            Buffered::Reader(ref mut reader) => return Ok(reader),
            Buffered::Writer(ref mut prev) => {
                prev.flush().context("Failed to flush previous writer")?;
            }
        }

        self.buffered = Buffered::reader(&self.file)?;

        match &mut self.buffered {
            Buffered::Reader(reader) => {
                reader
                    .seek(SeekFrom::Start(self.position))
                    .context("Failed to advance reader")?;

                Ok(reader)
            }

            Buffered::Writer(_) => unreachable!(),
        }
    }
}

impl RealFile<false> {
    /// Open an existing file file as readonly
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_file(path, |opts| opts)
    }
}

impl RealFile<true> {
    /// Open an existing file in writeable mode
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_file(path, |opts| opts)
    }

    /// Create a new archive (will not write any data by itself)
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_file(path, |opts| opts.create_new(true))
    }

    /// Get a buffered writer
    fn writer(&mut self) -> Result<&mut BufWriter<File>> {
        match self.buffered {
            Buffered::Reader(_) => {}
            Buffered::Writer(ref mut writer) => return Ok(writer),
        }

        self.buffered = Buffered::writer(&self.file)?;

        match self.buffered {
            Buffered::Reader(_) => unreachable!(),
            Buffered::Writer(ref mut writer) => {
                writer
                    .seek(SeekFrom::Start(self.position))
                    .context("Failed to advance writer")?;

                Ok(writer)
            }
        }
    }
}

impl<const WRITABLE: bool> ConsumableSource for RealFile<WRITABLE> {
    fn consume_into_buffer(&mut self, bytes: u64, buf: &mut [u8]) -> Result<()> {
        self.reader()?
            .read_exact(&mut buf[0..usize::try_from(bytes).unwrap()])
            .with_context(|| format!("Failed to read {bytes} bytes"))?;

        self.position += bytes;

        Ok(())
    }
}

impl<const WRITABLE: bool> ReadableSource for RealFile<WRITABLE> {
    fn position(&mut self) -> Result<u64> {
        Ok(self.position)
    }

    fn set_position(&mut self, addr: u64) -> Result<()> {
        self.position = match &mut self.buffered {
            Buffered::Reader(reader) => reader
                .seek(SeekFrom::Start(addr))
                .context("Failed to set position for reader")?,

            Buffered::Writer(writer) => writer
                .seek(SeekFrom::Start(addr))
                .context("Failed to set position for writer")?,
        };

        assert_eq!(self.position, addr);

        Ok(())
    }

    fn len(&mut self) -> Result<u64> {
        Ok(self
            .file
            .metadata()
            .context("Failed to get file's metadata")?
            .len())
    }
}

impl WritableSource for RealFile<true> {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.writer()?
            .write_all(data)
            .context("Failed to write all of the provided data")?;

        self.position += u64::try_from(data.len()).unwrap();

        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer()?
            .flush()
            .context("Failed to flush written data")
    }
}

/// Allow a [`RealFile`] to switch between a reader and a writer
enum Buffered {
    Reader(BufReader<File>),
    Writer(BufWriter<File>),
}

impl Buffered {
    pub fn reader(file: &File) -> Result<Self> {
        let file = file.try_clone().context("Failed to clone file instance")?;
        Ok(Self::Reader(BufReader::new(file)))
    }

    pub fn writer(file: &File) -> Result<Self> {
        let file = file.try_clone().context("Failed to clone file instance")?;
        Ok(Self::Writer(BufWriter::new(file)))
    }
}
