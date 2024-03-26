#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

pub mod archive;
pub mod config;
pub mod data;
pub mod diagnostic;
pub mod easy_archive;
pub mod file_reader;
pub mod source;

mod coverage;

#[cfg(test)]
mod tests;

/// This macro is used to ensure, at compile-time, that only one single
/// version of the BAF archives are supported.
/// This allows to simplify code by not dealing with different versions.
#[macro_export]
macro_rules! ensure_only_one_version {
    ($version: expr) => {
        match $version {
            $crate::data::header::ArchiveVersion::One => {}
        }
    };
}
