use clap::Parser;
use std::path::PathBuf;

/// Cymo. Multi-threads ftp upload tool.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub remote_path: PathBuf,

    #[arg(short, long)]
    pub local_path: PathBuf,
}
