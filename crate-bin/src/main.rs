#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io,
    num::NonZero,
    path::{Path, PathBuf},
    process::ExitCode,
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, bail};
use baf::{Archive, ArchiveConfig, DirEntry, DirectoryIdOrRoot, ItemId, ItemIdOrRoot, Timestamp};
use clap::Parser;
use colored::Colorize;
use jiff::{Zoned, civil};
use log::{debug, error, info, trace, warn};
use walkdir::WalkDir;
use zip::{DateTime, ZipWriter, write::SimpleFileOptions};

use self::{
    args::{Action, CmdArgs},
    logger::Logger,
    tree::ArchiveContentTree,
    utils::{human_size, human_time},
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

            for item in archive.items_iter() {
                match item {
                    DirEntry::Directory(directory) => {
                        info!(
                            "|  {}/",
                            archive.with_paths().compute_dir_path(directory.id)?
                        );
                    }

                    DirEntry::File(file) => {
                        info!(
                            "|> {} ({}, modified on {})",
                            archive.with_paths().compute_file_path(file.id)?,
                            human_size(file.content_len, Some(2)).bright_yellow(),
                            human_time(file.modif_time).bright_green()
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
            overwrite_files,
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
                    if !overwrite_files {
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
                    human_size(
                        *files_size.get(&real_path).with_context(|| format!(
                            "Failed to get size for file '{}'",
                            real_path.display()
                        ))?,
                        Some(2)
                    )
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

        Action::Extract {
            items_to_extract,
            output_dir,
            merge_dirs,
            overwrite_files,
        } => {
            let output_dir = match output_dir {
                Some(dir) => {
                    if !dir.exists() {
                        fs::create_dir(&dir).with_context(|| {
                            format!(
                                "Failed to create output directory at path '{}'",
                                dir.display()
                            )
                        })?;
                    } else if !merge_dirs {
                        bail!(
                            "Failed to extract archive: output directory '{}' already exists",
                            dir.display()
                        );
                    }

                    dir
                }

                None => {
                    let current_dir = std::env::current_dir()
                        .context("Failed to get current directory for extraction")?;

                    if items_to_extract.is_empty() {
                        current_dir.join(
                            path.file_stem()
                                .context("Failed to get archive's file stem")?,
                        )
                    } else {
                        current_dir
                    }
                }
            };

            let mut archive = Archive::open_from_file_readonly(path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            let archive_items: Vec<_> = if items_to_extract.is_empty() {
                archive.items_iter().map(|item| item.id()).collect()
            } else {
                let mut to_extract_ids = vec![];

                for item_path in items_to_extract {
                    let item = archive
                        .with_paths()
                        .get_item_at(&item_path)
                        .with_context(|| {
                            format!("Failed to find item at path '{}' in archive", item_path)
                        })?;

                    match item {
                        ItemIdOrRoot::Root => {
                            warn!(
                                "WARN: Root directory was specified, which means all other items in the archive will be extracted as well"
                            );
                            to_extract_ids = archive.items_iter().map(|item| item.id()).collect();
                            break;
                        }

                        ItemIdOrRoot::NonRootDirectory(dir) => {
                            to_extract_ids.push(ItemId::Directory(dir));

                            to_extract_ids.extend(
                                archive
                                    .read_dir_recursive(DirectoryIdOrRoot::NonRoot(dir))?
                                    .map(|item| item.id()),
                            );
                        }

                        ItemIdOrRoot::File(file) => {
                            to_extract_ids.push(ItemId::File(file));
                        }
                    }
                }

                to_extract_ids
            };

            if !merge_dirs {
                if output_dir.exists() {
                    bail!(
                        "Failed to extract archive: output directory '{}' already exists",
                        output_dir.display()
                    );
                }

                for item_id in &archive_items {
                    if let ItemId::Directory(dir_id) = item_id {
                        let path = archive.with_paths().compute_dir_path(*dir_id)?;
                        let output_path = output_dir.join(&path);

                        if output_path.exists() {
                            bail!(
                                "Failed to extract archive: output directory '{}' already exists",
                                output_path.display()
                            );
                        }
                    }
                }
            }

            fs::create_dir_all(&output_dir).with_context(|| {
                format!(
                    "Failed to create output directory at path '{}'",
                    output_dir.display()
                )
            })?;

            if !overwrite_files {
                for item_id in &archive_items {
                    if let ItemId::File(file_id) = item_id {
                        let path = archive.with_paths().compute_file_path(*file_id)?;
                        let output_path = output_dir.join(&path);

                        if output_path.exists() {
                            bail!(
                                "Failed to extract archive: output file '{}' already exists",
                                output_path.display()
                            );
                        }
                    }
                }
            }

            for item_id in archive_items {
                match item_id {
                    ItemId::Directory(dir_id) => {
                        let path = archive.with_paths().compute_dir_path(dir_id).unwrap();
                        debug!("Creating output directory: {path}");

                        let output_path = output_dir.join(path);

                        fs::create_dir(&output_path).with_context(|| {
                            format!(
                                "Failed to create output directory at path '{}'",
                                output_path.display()
                            )
                        })?;
                    }

                    ItemId::File(file_id) => {
                        let path = archive.with_paths().compute_file_path(file_id).unwrap();
                        debug!("Extracting output file: {path}");

                        let output_path = output_dir.join(&path);

                        let mut file = archive.read_file(file_id).with_context(|| {
                            format!("Failed to read file with id {path} from archive")
                        })?;

                        let mut output_file = File::create(&output_path).with_context(|| {
                            format!(
                                "Failed to create output file at path '{}'",
                                output_path.display()
                            )
                        })?;

                        io::copy(&mut file, &mut output_file).with_context(|| {
                            format!(
                                "Failed to write to output file at path '{}'",
                                output_path.display()
                            )
                        })?;
                    }
                }
            }

            info!(
                "Successfully extracted archive to '{}'",
                output_dir.display()
            );
        }

        Action::Delete { items_to_delete } => {
            let mut archive = Archive::open_from_file(&path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            let mut to_delete_ids = HashSet::new();

            for item_path in items_to_delete {
                let item = archive
                    .with_paths()
                    .get_item_at(&item_path)
                    .with_context(|| {
                        format!("Failed to find item at path '{}' in archive", item_path)
                    })?;

                match item {
                    ItemIdOrRoot::Root => bail!("The archive's root directory cannot be deleted."),

                    ItemIdOrRoot::NonRootDirectory(dir) => {
                        to_delete_ids.insert(ItemId::Directory(dir));
                    }

                    ItemIdOrRoot::File(file) => {
                        to_delete_ids.insert(ItemId::File(file));
                    }
                }
            }

            for item in to_delete_ids {
                match item {
                    ItemId::Directory(dir_id) => {
                        if archive.get_dir(dir_id).is_some() {
                            debug!(
                                "Deleting directory from archive: {}",
                                archive.with_paths().compute_dir_path(dir_id).unwrap()
                            );

                            archive.remove_dir(dir_id)?;
                        } else {
                            trace!(
                                "Directory with ID {dir_id:?} does not exist anymore in archive, skipping deletion"
                            );
                        }
                    }

                    ItemId::File(file_id) => {
                        if archive.get_file(file_id).is_some() {
                            debug!(
                                "Deleting file from archive: {}",
                                archive.with_paths().compute_file_path(file_id).unwrap()
                            );

                            archive.remove_file(file_id)?;
                        } else {
                            trace!(
                                "File with ID {file_id:?} does not exist anymore in archive, skipping deletion"
                            );
                        }
                    }
                }
            }

            archive.flush().context("Failed to close archive")?;

            info!("Successfully deleted items from archive");
        }

        Action::Zip { output } => {
            let output = match output {
                Some(output) => output,
                None => {
                    let mut output = path.clone();
                    output.set_extension("zip");
                    output
                }
            };

            if output.exists() {
                bail!(
                    "Failed to convert archive: output file '{}' already exists",
                    output.display()
                );
            }

            let mut archive = Archive::open_from_file_readonly(path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            let archive_items: Vec<_> = archive.items_iter().map(|item| item.id()).collect();

            let output_file = File::create(&output).with_context(|| {
                format!(
                    "Failed to create output file at path '{}'",
                    output.display()
                )
            })?;

            let mut zip_writer = ZipWriter::new(output_file);

            for item_id in archive_items {
                match item_id {
                    ItemId::Directory(dir_id) => {
                        let path = archive
                            .with_paths()
                            .compute_dir_path(dir_id)
                            .with_context(|| {
                                format!("Failed to compute path of directory with ID {dir_id:?}")
                            })?;

                        debug!("Adding directory to ZIP: {path}");

                        let modif_time = archive
                            .get_dir(dir_id)
                            .context("Failed to get directory from archive")?
                            .modif_time;

                        zip_writer
                            .add_directory(
                                path,
                                SimpleFileOptions::default()
                                    .last_modified_time(zip_datetime(modif_time)?),
                            )
                            .context("Failed to add directory to ZIP")?;
                    }

                    ItemId::File(file_id) => {
                        let path = archive
                            .with_paths()
                            .compute_file_path(file_id)
                            .with_context(|| {
                                format!("Failed to compute path of file with ID {file_id:?}")
                            })?;

                        debug!("Adding file to ZIP: {path}");

                        let modif_time = archive
                            .get_file(file_id)
                            .context("Failed to get file from archive")?
                            .modif_time;

                        let mut file = archive.read_file(file_id).with_context(|| {
                            format!("Failed to read file with id {path} from archive")
                        })?;

                        zip_writer
                            .start_file(
                                &path,
                                SimpleFileOptions::default()
                                    .last_modified_time(zip_datetime(modif_time)?),
                            )
                            .context("Failed to add file to ZIP")?;

                        io::copy(&mut file, &mut zip_writer).with_context(|| {
                            format!("Failed to write file '{path}' to ZIP")
                        })?;
                    }
                }
            }

            zip_writer.finish().context("Failed to finalize ZIP file")?;

            info!("Successfully converted archive to '{}'", output.display());
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
            bail!("Unknown item type at path '{}'", canon_path.display());
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

        for item in WalkDir::new(&canon_path).follow_links(false) {
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

    Ok(Timestamp::try_from(mtime)?)
}

fn zip_datetime(timestamp: Timestamp) -> Result<DateTime> {
    let zoned = Zoned::try_from(SystemTime::from(timestamp))
        .context("Failed to convert modification time")?;

    let civil_datetime: civil::DateTime = zoned.into();

    match DateTime::from_date_and_time(
        u16::try_from(civil_datetime.year()).unwrap(),
        u8::try_from(civil_datetime.month()).unwrap(),
        u8::try_from(civil_datetime.day()).unwrap(),
        u8::try_from(civil_datetime.hour()).unwrap(),
        u8::try_from(civil_datetime.minute()).unwrap(),
        u8::try_from(civil_datetime.second()).unwrap(),
    ) {
        Ok(date) => Ok(date),

        Err(err) => {
            warn!(
                "WARN: Modification time {timestamp:?} cannot be represented in ZIP format ({err}), falling back to 1980-01-01"
            );

            Ok(DateTime::DEFAULT)
        }
    }
}
