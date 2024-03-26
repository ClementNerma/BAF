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

        #[clap(help = "File or directory to add")]
        item_path: PathBuf,

        #[clap(help = "Add with a specific path", long = "as")]
        rename_as: Option<String>,
    },
}
