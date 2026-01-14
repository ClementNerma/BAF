//! Library for manipulation BAF (*B*asic *A*rchive *F*ormat), a modern alternative to the well-known TAR archives.
//!
//! The BAF format provides several advantages over TAR, notably its simplicity, random-seeking capabilities, and the possibility of modifying items without rebuilding the archive.s
//!
//! To get started, see the [`archive::Archive`] type.

#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

mod archive;
mod config;
mod coverage;
mod data;
mod file_reader;
mod iter;
mod source;
mod with_paths;

#[cfg(test)]
mod tests;

// Re-export useful types directly from the root
pub use self::{
    archive::{Archive, ArchiveDecodingError, ArchiveDuplicateItemNameError, DirEntry, ItemId},
    config::ArchiveConfig,
    data::{
        directory::{Directory, DirectoryDecodingError, DirectoryId, DirectoryIdOrRoot},
        file::{File, FileDecodingError, FileId},
        name::{ItemName, NameDecodingError, NameDecodingErrorReason, NameValidationError},
        path::PathInArchive,
        timestamp::Timestamp,
    },
    file_reader::FileReader,
    iter::ArchiveIter,
    source::Source,
    with_paths::{ItemIdOrRoot, WithPaths},
};

/// This macro is used to ensure, at compile-time, that only one single
/// version of the BAF archives are supported.
///
/// This allows to simplify code by not dealing with different versions.
///
/// This will be removed when multiple versions will exist.
#[macro_export]
macro_rules! ensure_only_one_version {
    ($version: expr) => {
        match $version {
            $crate::data::header::ArchiveVersion::One => {}
        }
    };
}
