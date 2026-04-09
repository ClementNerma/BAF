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
            long,
            help = "Merge with existing directories if they already exist in the archive"
        )]
        merge_dirs: bool,

        #[clap(
            long,
            help = "Overwrite existing files if they already exist in the archive"
        )]
        overwrite_files: bool,
    },

    Extract {
        #[clap(help = "Directory to extract the archive into (default: current directory)")]
        output_dir: Option<PathBuf>,

        #[clap(
            long,
            help = "Merge with existing directories if they already exist in the output directory"
        )]
        merge_dirs: bool,

        #[clap(
            long,
            help = "Overwrite existing files if they already exist in the output directory"
        )]
        overwrite_files: bool,
    },
}
