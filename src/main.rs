use crate::args::Args;
use crate::eudora::is_hidden;
use crate::utils::{build_worker_task, create_thread_task};
use anyhow::{anyhow, Result};
use clap::Parser;
use crossbeam_channel::unbounded;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    thread,
};

use walkdir::WalkDir;

mod args;
mod eudora;
mod utils;

// Arguments
static ARG: OnceLock<Args> = OnceLock::new();
// Used for skip folders
static PARAM_PATH: OnceLock<PathBuf> = OnceLock::new();
// Remote path, used for detect remote path
static REMOTE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn main() -> Result<()> {
    let args = Args::parse();
    PARAM_PATH.get_or_init(|| PathBuf::from(&args.local_path));
    REMOTE_PATH.get_or_init(|| PathBuf::from(&args.remote_path));
    let args = ARG.get_or_init(|| args);
    let mut files = WalkDir::new(&args.local_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !is_hidden(e))
        .map(|e| PathBuf::from(e.path()))
        .filter(|e| e.is_file())
        .collect::<Vec<_>>();
    files.sort_by_key(|a| a.components().count());
    // Found files
    let files_count = files.len();
    // let files = Arc::new(Mutex::new(files));

    // One more thread for send task for others
    let cpus = args
        .thread
        .unwrap_or(thread::available_parallelism()?.get());
    let cpus = if files_count < cpus {
        files_count
    } else {
        cpus
    };

    // This channel used by send all files to be upload to child threads
    let (s, r) = unbounded();
    thread::spawn(build_worker_task(files, cpus, s));

    // All threads total uploads count
    let file_count = Arc::new(StdMutex::new(0_usize));
    // All threads failed files
    let failed_files = Arc::new(StdMutex::new(Vec::<PathBuf>::new()));
    let threads = (1..=cpus)
        .map(create_thread_task(
            r,
            file_count.clone(),
            failed_files.clone(),
        ))
        .collect::<Vec<_>>();
    threads
        .into_iter()
        .try_for_each(|thread| thread.join().map_err(|err| anyhow!("{:?}", err)))?;

    let failed_count = failed_files
        .lock()
        .map_err(|err| anyhow!("Main thread read failed list failed {}", err))?
        .len();
    let count = file_count
        .lock()
        .map_err(|err| anyhow!("Main thread read file count failed {}", err))?;
    println!(
        "Total find {} file(s) upload {} file(s), {} file(s) failed",
        files_count, count, failed_count
    );
    Ok(())
}
