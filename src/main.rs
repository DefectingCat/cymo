use std::fs;

use anyhow::Result;
use clap::Parser;
use glob::{glob, GlobResult};

use crate::args::Args;

mod args;

fn main() -> Result<()> {
    let Args {
        remote_path,
        local_path,
    } = Args::parse();

    println!("Hello, world! {:?} {:?}", remote_path, local_path);
    let local_path = glob(&local_path)?;
    let local_path = local_path
        .into_iter()
        .filter_map(|path| path.ok())
        .collect::<Vec<_>>();

    // let local_files =  local_path.iter().map(|path| {
    //       dbg!(path);
    //   });

    Ok(())
}
