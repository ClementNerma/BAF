use std::path::PathBuf;

use clap::Parser;
use log::LevelFilter;

#[derive(Parser)]
pub struct CmdArgs {
    #[clap(help = "Path to the archive")]
    pub path: PathBuf,

    #[clap(short, long, help = "Display verbose output", default_value = "info")]
    pub verbosity: LevelFilter,

    #[clap(subcommand)]
    pub action: Action,
}

#[derive(Parser)]
pub enum Action {
    Create,

    #[clap(alias = "ls")]
    List,

    Tree,

    Add {
        #[clap(help = "Items to add (files or directories)")]
        items_path: Vec<PathBuf>,

        #[clap(
            short = 'u',
            long,
            help = "Directory to add the items into in the archive"
        )]
        under_dir: Option<String>,

        #[clap(
            long = "merge-dirs",
            help = "Merge with existing directories if they already exist in the archive"
        )]
        merge_dirs: bool,

        #[clap(
            long = "merge-files",
            help = "Merge with existing files if they already exist in the archive"
        )]
        merge_files: bool,
    },
}
