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
    config::ArchiveConfig,
    data::{file::File, timestamp::Timestamp},
    easy::EasyArchive,
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

            EasyArchive::create_as_file(path, ArchiveConfig::default())
                .context("Failed to create archive")?;
        }

        Command::List { path } => {
            let (archive, diags) = EasyArchive::open_from_file(path, ArchiveConfig::default())
                .context("Failed to open archive")?;

            for diag in diags {
                eprintln!("WARNING: {diag}");
            }

            let tree = Tree::new(archive.inner());

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

        Command::Add {
            path,
            items_path,
            under_dir,
        } => {
            for item_path in &items_path {
                if !item_path.exists() {
                    bail!("No item found at path '{}'", item_path.display());
                }
            }

            let config = ArchiveConfig::default();

            let mut archive = if path.exists() {
                let (archive, diags) =
                    EasyArchive::open_from_file(&path, config).with_context(|| {
                        format!("Failed to open archive at path '{}'", path.display())
                    })?;

                for diag in diags {
                    eprintln!("WARNING: {diag}");
                }

                archive
            } else {
                EasyArchive::create_as_file(&path, config).with_context(|| {
                    format!("Failed to create archive at path '{}'", path.display())
                })?
            };

            for item_path in &items_path {
                add_item_to_archive(&mut archive, item_path, under_dir.as_deref())?;
            }

            archive.flush().context("Failed to close archive")?;
        }
    }

    Ok(())
}

fn add_item_to_archive(
    archive: &mut EasyArchive<impl WritableSource>,
    item_path: &Path,
    under_dir: Option<&str>,
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
        under_dir: Option<&str>,
    ) -> Result<()> {
        let path_in_archive = under_dir.map_or_else(
            || path_in_archive.to_owned(),
            |under_dir| format!("{under_dir}/{path_in_archive}"),
        );

        println!("Adding file '{path_in_archive}'...");

        let mtime = get_item_mtime(canon_path)?;

        let content = RealFile::open(canon_path).context("Failed to open file in read mode")?;

        archive
            .write_file(&path_in_archive, content, mtime)
            .context("Failed to add file to archive")?;

        Ok(())
    }

    fn get_item_mtime(path: &Path) -> Result<Timestamp> {
        let mtime = path
            .metadata()
            .context("Failed to get metadata for file")?
            .modified()
            .unwrap_or_else(|err| {
                eprintln!("WARN: Failed to get the file's modification time ({err}) ; falling back to current time");
                SystemTime::now()
            });

        Ok(Timestamp::from(mtime))
    }

    if mt.file_type().is_file() {
        let filename = item_path
            .file_name()
            .context("Provided path does not have a file name")?;

        let filename = filename
            .to_str()
            .context("Filename contains invalid UTF-8 characters")?;

        add_file_to_archive(archive, &canon_path, filename, under_dir)
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
                add_file_to_archive(archive, item.path(), path_in_archive, under_dir)?;
            } else if item.file_type().is_dir() {
                println!("Creating directory '{path_in_archive}'...");

                let mtime = get_item_mtime(item.path())?;

                archive.create_directory(path_in_archive, mtime)?;
            } else {
                eprintln!(
                    "WARN: Ignoring unknown item type at path '{}'",
                    item.path().display()
                );
            }
        }

        Ok(())
    } else {
        bail!("Unkown item type at path '{}'", canon_path.display());
    }
}
