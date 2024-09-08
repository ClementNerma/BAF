#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::SystemTime,
};

use anyhow::{bail, Context, Result};
use baf::{
    archive::Archive,
    config::ArchiveConfig,
    data::file::File,
    easy::{translate_time_for_archive, EasyArchive},
    source::{RealFile, WritableSource},
};
use clap::Parser;
use walkdir::WalkDir;

use self::{
    args::Command,
    tree::{FlattenedEntryDir, Tree},
};

mod args;
mod tree;

fn main() -> ExitCode {
    match inner_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

fn inner_main() -> Result<()> {
    match Command::parse() {
        Command::Create { path } => {
            if path.exists() {
                bail!("Path {} already exists", path.display());
            }

            Archive::create_as_file(path, ArchiveConfig::default())
                .context("Failed to create archive")?;
        }

        Command::List { path } => {
            let (archive, diags) = Archive::open_from_file(path, ArchiveConfig::default())
                .context("Failed to open archive")?;

            for diag in diags {
                eprintln!("WARNING: {diag}");
            }

            let tree = Tree::new(&archive);

            for FlattenedEntryDir { path, files } in tree.flatten_ordered() {
                let path = PathBuf::from(path.join(std::path::MAIN_SEPARATOR_STR));

                if path.components().count() != 0 {
                    println!("[Dir ] {}", path.display());
                }

                for file in files {
                    let File {
                        id: _,
                        parent_dir: _,
                        name,
                        modif_time: _,
                        content_addr: _,
                        content_len,
                        sha3_checksum: _,
                    } = file;

                    println!(
                        "[File] {} ({content_len} bytes)",
                        path.join(&*name).display()
                    );
                }
            }
        }

        Command::Add { path, item_path } => {
            if !item_path.exists() {
                bail!("No item found at path '{}'", item_path.display());
            }

            let config = ArchiveConfig::default();

            let archive = if path.exists() {
                let (archive, diags) =
                    Archive::open_from_file(&path, config).with_context(|| {
                        format!("Failed to open archive at path '{}'", path.display())
                    })?;

                for diag in diags {
                    eprintln!("WARNING: {diag}");
                }

                archive
            } else {
                Archive::create_as_file(&path, config).with_context(|| {
                    format!("Failed to create archive at path '{}'", path.display())
                })?
            };

            let mut archive = archive.easy();

            add_item_to_archive(&mut archive, &item_path)?;

            archive.flush().context("Failed to close archive")?;
        }
    }

    Ok(())
}

fn add_item_to_archive(
    archive: &mut EasyArchive<impl WritableSource>,
    item_path: &Path,
) -> Result<()> {
    if !item_path.exists() {
        bail!("Item at path '{}' does not exist", item_path.display());
    }

    let canon_path = fs::canonicalize(item_path)
        .with_context(|| format!("Failed to canonicalize path '{}'", item_path.display()))?;

    let mt = canon_path.metadata().with_context(|| {
        format!(
            "Failed to get metadata on item at path '{}'",
            canon_path.display()
        )
    })?;

    fn add_file_to_archive(
        archive: &mut EasyArchive<impl WritableSource>,
        canon_path: &Path,
        path_in_archive: &str,
    ) -> Result<()> {
        println!("Adding file '{path_in_archive}'...");

        let mtime = get_item_mtime(canon_path)?;

        let content = RealFile::open(canon_path).context("Failed to open file in read mode")?;

        archive
            .create_or_update_file(path_in_archive, content, mtime)
            .context("Failed to add file to archive")?;

        Ok(())
    }

    fn get_item_mtime(path: &Path) -> Result<u64> {
        let mtime = path
            .metadata()
            .context("Failed to get metadata for file")?
            .modified()
            .unwrap_or_else(|err| {
                eprintln!("WARN: Failed to get the file's modification time ({err}) ; falling back to current time");
                SystemTime::now()
            });

        Ok(translate_time_for_archive(mtime))
    }

    if mt.file_type().is_file() {
        let filename = item_path
            .file_name()
            .context("Provided path does not have a file name")?;

        let filename = filename
            .to_str()
            .context("Filename contains invalid UTF-8 characters")?;

        add_file_to_archive(archive, &canon_path, filename)
    } else if mt.file_type().is_dir() {
        for item in WalkDir::new(&canon_path) {
            let item = item.context("Failed to read directory")?;

            let path_in_archive = item.path().strip_prefix(&canon_path).unwrap();

            if path_in_archive.as_os_str().is_empty() {
                continue;
            }

            let path_in_archive = path_in_archive.to_str().with_context(|| {
                format!(
                    "Path '{}' contains invalid UTF-8 characters",
                    path_in_archive.display()
                )
            })?;

            if item.file_type().is_file() {
                add_file_to_archive(archive, item.path(), path_in_archive)?;
            } else if item.file_type().is_dir() {
                println!("Creating directory '{path_in_archive}'...",);

                let mtime = get_item_mtime(item.path())?;

                archive.create_directory(path_in_archive, mtime)?;
            } else {
                eprintln!(
                    "WARN: Ignoring unknown item type at path '{}'",
                    canon_path.display()
                );
            }
        }

        Ok(())
    } else {
        bail!("Unkown item type at path '{}'", canon_path.display());
    }
}
