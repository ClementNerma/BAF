#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

use std::{
    fs::{self, File},
    num::NonZero,
    path::{Path, PathBuf},
    process::ExitCode,
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, bail};
use baf::{Archive, ArchiveConfig, DirEntry, Timestamp};
use clap::Parser;
use walkdir::WalkDir;

use self::args::{Action, CmdArgs};

mod args;

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
    let CmdArgs { path, action } = CmdArgs::parse();

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
            let archive = Archive::open_from_file_readonly(path, ArchiveConfig::default())
                .map_err(|err| anyhow!("Failed to open archive: {err:?}") /* TODO: display instead of debug */)?;

            for item in archive.iter() {
                match item {
                    DirEntry::Directory(directory) => {
                        println!("[Dir ] {}", archive.compute_dir_path(directory.id).unwrap());
                    }

                    DirEntry::File(file) => {
                        println!(
                            "[File] {} ({} bytes)",
                            archive.compute_file_path(file.id).unwrap(),
                            human_size(file.content_len, Some(2)),
                        );
                    }
                }
            }
        }

        Action::Add {
            items_path,
            under_dir,
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

            println!("> Creating {} directories in archive...", dirs.len());

            for ItemToAdd {
                real_path,
                path_in_archive,
            } in dirs
            {
                archive
                    .with_paths()
                    .create_dir_at(&path_in_archive, get_item_mtime(&real_path)?)?;
            }

            for ItemToAdd {
                real_path,
                path_in_archive,
            } in files
            {
                println!("> Adding file: {}", real_path.display());

                let file = File::open(&real_path)
                    .with_context(|| format!("Failed to open file: {}", real_path.display()))?;

                archive
                    .with_paths()
                    .write_file_at(&path_in_archive, file, get_item_mtime(&real_path)?)
                    .context("Failed to add file to archive")?;
            }

            archive.flush().context("Failed to close archive")?;
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
                eprintln!(
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
                eprintln!("WARN: Failed to get the item's modification time ({err}) ; falling back to system's current time");
                SystemTime::now()
            });

    Ok(Timestamp::from(mtime))
}

pub fn human_size(size: u64, precision: Option<u8>) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];

    let (unit, unit_base) = units
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, unit)| {
            let base = 1024_u64.pow(i.try_into().unwrap());

            if size >= base || base == 1 {
                Some((unit, base))
            } else {
                None
            }
        })
        .unwrap();

    format!(
        "{} {unit}",
        approx_int_div(size, unit_base, precision.unwrap_or(2))
    )
}

/// Perform an approximate integer division
///
/// The last decimal will be rounded to the nearest.
///
/// The `precision` parameter is the number of floating-point decimals to keep.
pub fn approx_int_div(a: u64, b: u64, precision: u8) -> String {
    let max_prec = 10_u128.pow(u32::from(precision));

    let div = u128::from(a) * max_prec * 10 / u128::from(b);
    let div = (div / 10) + if div % 10 >= 5 { 1 } else { 0 };

    let int_part = div / max_prec;
    let frac_part = div % max_prec;

    let mut out = int_part.to_string();

    if frac_part > 0 && precision > 0 {
        out.push('.');
        out.push_str(&format!(
            "{:#0precision$}",
            frac_part,
            precision = precision.into()
        ));
    }

    out
}
