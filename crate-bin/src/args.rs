use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub enum Command {
    Create {
        #[clap(help = "Path to create")]
        path: PathBuf,
    },

    List {
        #[clap(help = "Path to the archive")]
        path: PathBuf,
    },

    Add {
        #[clap(help = "Path to the archive")]
        path: PathBuf,

        #[clap(help = "Items to add (files or directories)")]
        items_path: Vec<PathBuf>,
    },
}
