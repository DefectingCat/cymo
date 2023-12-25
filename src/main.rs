use crate::args::Args;
use crate::eudora::{connect_and_init, get_args, is_hidden, remote_mkdir, upload};
use crate::utils::{build_worker_task, fold_parents};
use anyhow::{anyhow, Ok as AOk, Result};
use clap::Parser;
use crossbeam_channel::unbounded;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    thread,
};
use suppaftp::AsyncFtpStream;
use tokio::{runtime, sync::Mutex};
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
    let files = Arc::new(Mutex::new(files));

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
    thread::spawn(build_worker_task(files.clone(), cpus, s));

    // All threads total uploads count
    let file_count = Arc::new(StdMutex::new(0_usize));
    // All threads failed files
    let failed_files = Arc::new(StdMutex::new(Vec::<PathBuf>::new()));
    let thread_task = |i| {
        let r = r.clone();
        let file_count = file_count.clone();
        let failed_files = failed_files.clone();
        let task = move || {
            let rt = runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let handle = rt.block_on(async {
                let Args { server, port, .. } = get_args()?;
                let addr = format!("{}:{}", server, port);
                println!("Thread {} connecting {}", i, &addr);
                // TODO read username and password in environment
                let mut ftp_stream = AsyncFtpStream::connect(addr).await.map_err(|err| {
                    eprintln!("Thread {} connnect failed {}", i, err);
                    anyhow!("{}", err)
                });
                let _ = connect_and_init(ftp_stream.as_mut(), i).await;

                let mut current_failed = vec![];
                // Receive files from main thread.
                let mut thread_count = 0_usize;
                for (count, path) in (r.recv()?).into_iter().enumerate() {
                    let ftp_stream = if let Ok(stream) = ftp_stream.as_mut() {
                        stream
                    } else {
                        current_failed.push(path);
                        continue;
                    };
                    match upload(ftp_stream, i, &path, 0).await {
                        Ok(_) => {
                            thread_count = count + 1;
                        }
                        Err(err) => {
                            eprintln!("Thread {} upload {:?} failed, {}", i, path, err);
                            current_failed.push(path);
                        }
                    }
                }
                file_count
                    .lock()
                    .map(|mut file_count| {
                        if thread_count == 0 {
                            return;
                        }
                        *file_count += thread_count;
                        println!("Thread {} uploaded {} files", i, thread_count);
                    })
                    .map_err(|err| anyhow!("Thread {} write file cout failed {}", i, err))?;
                if !current_failed.is_empty() {
                    failed_files
                        .lock()
                        .map(|mut failed_files| {
                            failed_files.append(&mut current_failed);
                        })
                        .map_err(|err| {
                            anyhow!("Thread {} collect failed files failed {}", i, err)
                        })?;
                }
                println!("Thread {} exiting", i);
                ftp_stream?.quit().await?;
                AOk(())
            });
            if let Err(err) = handle {
                eprintln!("Thread {} got error {}", i, err);
            };
        };

        thread::spawn(task)
    };
    let threads = (1..=cpus).map(thread_task).collect::<Vec<_>>();
    for thread in threads {
        thread.join().map_err(|err| anyhow!("{:?}", err))?;
    }

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
