use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct CmdArgs {
    #[clap(help = "Path to the archive")]
    pub path: PathBuf,

    #[clap(subcommand)]
    pub action: Action,
}

#[derive(Parser)]
pub enum Action {
    Create,

    #[clap(alias = "ls")]
    List,

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
            short = 'd',
            long,
            help = "Display the path and size of each added file"
        )]
        detailed: bool,
    },
}
