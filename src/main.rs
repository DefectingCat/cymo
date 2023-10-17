use crate::args::Args;
use clap::Parser;

mod args;

fn main() {
    let Args {
        remote_path,
        local_path,
    } = Args::parse();

    println!("Hello, world! {:?} {:?}", remote_path, local_path);
}
