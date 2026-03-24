#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

use std::{
    collections::HashMap,
    fs::{self, File},
    num::NonZero,
    path::{Path, PathBuf},
    process::ExitCode,
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, bail};
use baf::{Archive, ArchiveConfig, DirEntry, Timestamp};
use clap::Parser;
use log::{debug, error, info, warn};
use walkdir::WalkDir;

use self::{
    args::{Action, CmdArgs},
    logger::Logger,
    tree::ArchiveContentTree,
    utils::human_size,
};

mod args;
mod logger;
mod tree;
mod utils;

fn main() -> ExitCode {
    let args = CmdArgs::parse();

    Logger::new(args.verbosity).init().unwrap();

    match inner_main(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

fn inner_main(args: CmdArgs) -> Result<()> {
    let CmdArgs {
        path,
        action,
        verbosity: _,
    } = args;

    match action {
        Action::Create => {
            if path.exists() {
                bail!("Path {} already exists", path.display());
            }

            let mut archive = Archive::create_as_file(path, ArchiveConfig::default())
                .context("Failed to create archive")?;

            archive.flush().context("Failed to flush the archive")?;
        }

        Action::List => {
            let  archive = Archive::open_from_file_readonly(path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            for item in archive.ordered_iter() {
                match item {
                    DirEntry::Directory(directory) => {
                        info!(
                            "|  {}/",
                            archive.with_paths().compute_dir_path(directory.id).unwrap()
                        );
                    }

                    DirEntry::File(file) => {
                        info!(
                            "|> {} ({})",
                            archive.with_paths().compute_file_path(file.id).unwrap(),
                            human_size(file.content_len, Some(2)),
                        );
                    }
                }
            }
        }

        Action::Tree => {
            let archive = Archive::open_from_file_readonly(path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            info!("{}", ArchiveContentTree::build(&archive));
        }

        Action::Add {
            items_path,
            under_dir,
            merge_dirs,
            merge_files,
        } => {
            for item_path in &items_path {
                if !item_path.exists() {
                    bail!("No item found at path '{}'", item_path.display());
                }
            }

            let ItemsToAdd { dirs, files } = find_items_to_add(&items_path, under_dir.as_deref())?;

            let config = ArchiveConfig {
                first_segment_dirs_capacity_override: Some(
                    NonZero::new(u32::try_from(dirs.len()).unwrap() + 1).unwrap(),
                ),

                first_segment_files_capacity_override: Some(
                    NonZero::new(u32::try_from(files.len()).unwrap() + 1).unwrap(),
                ),

                ..Default::default()
            };

            let mut archive = if path.exists() {
                // TODO: reserve space ahead of time for the computed number of files + dirs
                Archive::open_from_file(&path, config).map_err(|err| {
                    anyhow!(
                        "Failed to open archive at path '{}': {err:?}",
                        path.display()
                    ) // TODO: display instead of debug
                })?
            } else {
                Archive::create_as_file(&path, config).with_context(|| {
                    format!("Failed to create archive at path '{}'", path.display())
                })?
            };

            info!("Creating {} directories in archive...", dirs.len());

            // Check files and directories beforehand
            for ItemToAdd {
                real_path: _,
                path_in_archive,
            } in &dirs
            {
                if archive.with_paths().get_item_at(path_in_archive).is_some() {
                    if !merge_dirs {
                        bail!(
                            "Failed to add directory '{}' to archive: path already exists in the archive",
                            path_in_archive
                        );
                    }

                    debug!(
                        "> Directory '{}' already exists in archive, going to merge",
                        path_in_archive
                    );
                }
            }

            for ItemToAdd {
                real_path: _,
                path_in_archive,
            } in &files
            {
                if archive.with_paths().get_item_at(path_in_archive).is_some() {
                    if !merge_files {
                        bail!(
                            "Failed to add file '{}' to archive: path already exists in the archive",
                            path_in_archive
                        );
                    }

                    debug!(
                        "> File '{}' already exists in archive, going to overwrite",
                        path_in_archive
                    );
                }
            }

            // Create directories first, so that files can be added into them
            for ItemToAdd {
                real_path,
                path_in_archive,
            } in dirs
            {
                archive
                    .with_paths_mut()
                    .create_dir_at(&path_in_archive, get_item_mtime(&real_path)?)?;
            }

            // Get files size beforehand to display it
            let files_size = files
                .iter()
                .map(|file| {
                    file.real_path
                        .metadata()
                        .map(|mt| (&file.real_path, mt.len()))
                        .with_context(|| {
                            format!(
                                "Failed to get metadata about file: {}",
                                file.real_path.display()
                            )
                        })
                })
                .collect::<Result<HashMap<_, _>, _>>()?;

            assert_eq!(files.len(), files_size.len());

            info!(
                "Adding {} files (total: {})",
                files.len(),
                human_size(files_size.values().sum::<u64>(), Some(2))
            );

            for ItemToAdd {
                real_path,
                path_in_archive,
            } in &files
            {
                debug!(
                    "> Adding file: {} ({})",
                    real_path.display(),
                    human_size(*files_size.get(&real_path).unwrap(), Some(2))
                );

                let file = File::open(real_path)
                    .with_context(|| format!("Failed to open file: {}", real_path.display()))?;

                archive
                    .with_paths_mut()
                    .write_file_at(path_in_archive, file, get_item_mtime(real_path)?)
                    .context("Failed to add file to archive")?;
            }

            archive.flush().context("Failed to close archive")?;

            info!("Done!");
        }
    }

    Ok(())
}

struct ItemsToAdd {
    dirs: Vec<ItemToAdd>,
    files: Vec<ItemToAdd>,
}

struct ItemToAdd {
    real_path: PathBuf,
    path_in_archive: String,
}

fn find_items_to_add<P: AsRef<Path>>(items: &[P], under_dir: Option<&str>) -> Result<ItemsToAdd> {
    let mut dirs = vec![];
    let mut files = vec![];

    for item_path in items {
        let item_path = item_path.as_ref();

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

        if mt.file_type().is_file() {
            let filename = item_path
                .file_name()
                .context("Provided path does not have a file name")?;

            let filename = filename
                .to_str()
                .context("Filename contains invalid UTF-8 characters")?;

            files.push(ItemToAdd {
                real_path: canon_path,
                path_in_archive: match under_dir {
                    Some(dir) => format!("{dir}/{filename}"),
                    None => filename.to_owned(),
                },
            });

            continue;
        } else if !mt.file_type().is_dir() {
            bail!("Unkown item type at path '{}'", canon_path.display());
        }

        let under_dir = match under_dir {
            Some(dir) => dir,
            None => {
                let basename = canon_path.file_name().with_context(|| {
                    format!("Failed to determine file name of: {}", canon_path.display())
                })?;

                basename.to_str().with_context(|| {
                    format!("Directory name contains invalid UTF-8 characters: {basename:?}",)
                })?
            }
        };

        for item in WalkDir::new(&canon_path) {
            let item = item.context("Failed to read directory")?;

            let stripped_path = item.path().strip_prefix(&canon_path).unwrap();

            if stripped_path.as_os_str().is_empty() {
                continue;
            }

            let stripped_path = stripped_path.to_str().with_context(|| {
                format!(
                    "Path '{}' contains invalid UTF-8 characters",
                    stripped_path.display()
                )
            })?;

            let path_in_archive = format!("{under_dir}/{stripped_path}");

            if item.file_type().is_file() {
                files.push(ItemToAdd {
                    real_path: item.path().to_owned(),
                    path_in_archive,
                });
            } else if item.file_type().is_dir() {
                dirs.push(ItemToAdd {
                    real_path: item.path().to_owned(),
                    path_in_archive,
                });
            } else {
                warn!(
                    "WARN: Ignoring unknown item type at path '{}'",
                    item.path().display()
                );
            }
        }
    }

    Ok(ItemsToAdd { dirs, files })
}

fn get_item_mtime(path: &Path) -> Result<Timestamp> {
    let mtime = path
            .metadata()
            .context("Failed to get metadata for item")?
            .modified()
            .unwrap_or_else(|err| {
                warn!("WARN: Failed to get the item's modification time ({err}) ; falling back to system's current time");
                SystemTime::now()
            });

    Ok(Timestamp::from(mtime))
}
