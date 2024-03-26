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
    config::Config,
    data::file::File,
    easy_archive::{translate_time_for_archive, EasyArchive},
    source::{RealFile, WritableSource},
};
use clap::Parser;

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

            let file = RealFile::open(&path, true).context("Failed to create file")?;

            Archive::create(file, Config::default()).context("Failed to create archive")?;
        }

        Command::List { path } => {
            let file = RealFile::open(&path, false)
                .with_context(|| format!("Failed to open file at {}", path.display()))?;

            let (archive, diags) =
                Archive::open(file, Config::default()).context("Failed to open archive")?;

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
                        path.join(&name).display()
                    );
                }
            }
        }

        Command::Add {
            path,
            item_path,
            rename_as,
        } => {
            if !item_path.exists() {
                bail!("No item found at path '{}'", item_path.display());
            }

            let existing_archive = path.exists();

            let file = RealFile::open(&path, !existing_archive)
                .with_context(|| format!("Failed to open file at path '{}'", path.display()))?;

            let (archive, diags) = if existing_archive {
                Archive::open(file, Config::default()).context("Failed to open archive")?
            } else {
                (
                    Archive::create(file, Config::default()).context("Failed to create archive")?,
                    vec![],
                )
            };

            for diag in diags {
                eprintln!("WARNING: {diag}");
            }

            let mut archive = archive.easy();

            add_item_to_archive(&mut archive, &item_path, rename_as, vec![])?;

            archive.flush().context("Failed to close archive")?;
        }
    }

    Ok(())
}

fn add_item_to_archive(
    archive: &mut EasyArchive<impl WritableSource>,
    item_path: &Path,
    rename_as: Option<String>,
    append_to_rename_as: Vec<String>,
) -> Result<()> {
    if !item_path.exists() {
        bail!("No item found at path '{}'", item_path.display());
    }

    let (item_path_str, item_path_display) = match rename_as.clone() {
        Some(mut path_str) => {
            if !append_to_rename_as.is_empty() {
                for item in &append_to_rename_as {
                    path_str.push('/');
                    path_str.push_str(item);
                }
            }

            (
                path_str.clone(),
                format!("'{}' (as '{path_str}')", item_path.display()),
            )
        }

        None => {
            let path_str = item_path
                .to_str()
                .with_context(|| {
                    format!(
                        "Cannot add path '{}' to archive as it contains invalid UTF-8 characters",
                        item_path.display()
                    )
                })?
                .to_owned();

            (path_str.clone(), format!("'{}'", item_path.display()))
        }
    };

    let item_mtime = item_path.metadata().with_context(|| format!("Failed to get metadata for item '{}'", item_path.display()))?
        .modified().unwrap_or_else(|err| {
            eprintln!("WARN: Failed to get modification time for item at path '{}': {err} ; falling back to current time", item_path.display());
            SystemTime::now()
        });

    let item_mtime = translate_time_for_archive(item_mtime);

    // Add directory
    if item_path.is_dir() {
        println!("Adding directory {item_path_display}...");

        archive
            .create_directory(&item_path_str, item_mtime)
            .with_context(|| format!("Failed to create directory '{item_path_str}' in archive"))?;

        let read_dir = fs::read_dir(item_path)
            .and_then(|results| results.collect::<Result<Vec<_>, _>>())
            .with_context(|| {
                format!(
                    "Failed to read content of directory at path '{}'",
                    item_path.display()
                )
            })?;

        for item in read_dir {
            let path = item.path();

            let filename = path
                .file_name()
                .with_context(|| format!("Item at path '{}' does not have a filename", path.display()))?
                .to_str()
                .with_context(|| format!("Cannot create path '{}' in archive as it does contain invalid UTF-8 characters", path.display()))?
                .to_owned();

            let mut rename_as_append = append_to_rename_as.clone();
            rename_as_append.push(filename);

            add_item_to_archive(archive, &path, rename_as.clone(), rename_as_append)?;
        }

        Ok(())
    }
    // Add file
    else if item_path.is_file() {
        println!("Adding file {item_path_display}...");

        let content = RealFile::open(item_path, false)
            .with_context(|| format!("Failed to open file at path '{}'", item_path.display()))?;

        archive
            .create_file(&item_path_str, content, item_mtime)
            .with_context(|| format!("Failed to create file '{item_path_str}' in archive"))?;

        Ok(())
    }
    // Otherwise, it's an unsupported file type
    else {
        bail!("Unsupported item type at path '{}'", item_path.display());
    }
}
