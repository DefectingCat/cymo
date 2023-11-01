use std::cmp::Ordering;
use std::path::PathBuf;
use std::process::exit;
use std::sync::OnceLock;
use std::{sync::Arc, thread};

use anyhow::{anyhow, Context, Ok as AOk, Result};
use clap::Parser;
use crossbeam_channel::unbounded;
use futures::future::try_join_all;
use glob::{glob, GlobResult};
use suppaftp::AsyncFtpStream;
use tokio::{runtime, spawn, sync::Mutex};

use crate::args::Args;
use crate::eudora::{connect_and_init, get_args, recursive_read_file, upload};

mod args;
mod eudora;

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
    let files = Arc::new(Mutex::new(vec![]));
    // Local directory depth, params not included
    let depth = Arc::new(Mutex::new(0_usize));

    let main_rt = runtime::Builder::new_multi_thread().build()?;
    let main_handle = main_rt.block_on(async {
        let local_path = &args.local_path;
        let local_path = glob(local_path)?;

        let task = |path: GlobResult| {
            let files = files.clone();
            let depth = depth.clone();
            let task = async move {
                let path = path.with_context(|| "Read file failed")?;
                recursive_read_file(files.clone(), depth, path).await?;
                AOk(())
            };
            spawn(task)
        };
        try_join_all(local_path.into_iter().map(task)).await?;

        let mut files = files.lock().await;
        if files.len() == 0 {
            println!("Local target file is empty no not exist.");
            exit(0);
        }
        let depth = depth.lock().await;
        files.sort_by_key(|a| a.iter().count());
        let param_path = PARAM_PATH.get().ok_or(anyhow!("Parse args error"))?;
        let start = param_path.iter().count();
        if *depth > 0 {
            (start..*depth - 1).for_each(|i| {
                files.sort_by(|a, b| {
                    let empty = PathBuf::new();
                    let child_a = a
                        .parent()
                        .unwrap_or(&empty)
                        .components()
                        .collect::<Vec<_>>();
                    let child_a = child_a.get(i);
                    let child_b = b
                        .parent()
                        .unwrap_or(&empty)
                        .components()
                        .collect::<Vec<_>>();
                    let child_b = child_b.get(i);
                    match (child_a, child_b) {
                        (Some(a), Some(b)) => a.cmp(b),
                        _ => Ordering::Equal,
                    }
                })
            });
        }
        let len = files.len();
        println!("Find {} file(s)", len);
        AOk(())
    });
    main_handle?;
    main_rt.shutdown_background();

    // TODO add custom thread number
    let cpus = thread::available_parallelism()?.get();
    // This channel used by send all files to be upload to child threads
    let (s, r) = unbounded();
    // This thread prepare each threads files to upload.
    let task = move || {
        let rt = runtime::Builder::new_current_thread().build().unwrap();
        let task = async {
            let mut files = files.lock().await;
            // Total files length
            let len = files.len();
            // Div by cpu nums - 1
            let div = cpus - 1;
            let (quotient, remainder) = (len / div, len % div); // calculate the quotient and remainders.send()
            let start = 0;
            let sender = (0..div)
                .map(|i| {
                    let end = if i < remainder {
                        // if i is less than the remainder, add one extra element to the smaller array
                        start + quotient + 1
                    } else {
                        // otherwise, use the quotient as the size of the smaller array
                        start + quotient
                    };
                    let file_data = files.drain(start..end).collect::<Vec<_>>();
                    s.send(file_data)?;
                    AOk(())
                })
                .collect::<Result<Vec<_>>>();
            (sender, len)
        };
        let (result, len) = rt.block_on(task);
        match result {
            // TODO retry, when files is empty, exit threads
            Ok(_) => {
                println!("Total send {} files", len);
            }
            Err(err) => {
                eprintln!(
                    "Send files to thread failed {:?}. {} files not send",
                    err, len
                );
            }
        }
    };
    thread::spawn(task);

    let file_count = Arc::new(std::sync::Mutex::new(0_usize));
    let thread_task = |i| {
        let r = r.clone();
        let file_count = file_count.clone();
        let task = move || {
            let rt = runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let handle = rt.block_on(async {
                let Args { server, .. } = get_args()?;
                println!("Thread {} connecting {}", i, &server);
                // TODO add server port configuration
                // TODO read username and password in environment
                let mut ftp_stream = AsyncFtpStream::connect(format!("{}:21", server)).await?;
                connect_and_init(&mut ftp_stream, i).await?;

                // Receive files from main thread.
                // TODO add retry, add maximum retry times.
                let mut thread_count = 0_usize;
                for (count, path) in (r.recv()?).into_iter().enumerate() {
                    upload(&mut ftp_stream, i, &path).await?;
                    println!("Thread {} upload file {:?} success", i, &path);
                    thread_count = count
                }
                ftp_stream.quit().await?;
                thread_count += 1;
                match file_count.lock() {
                    Ok(mut file_count) => {
                        *file_count += thread_count;
                        println!("Thread {} uploaded {} files", i, thread_count);
                    }
                    Err(err) => {
                        eprintln!("Thread {} write file cout failed {}", i, err);
                    }
                }
                println!("Thread {} exiting", i);
                AOk(())
            });
            if let Err(err) = handle {
                eprintln!("Thread {} got error {}", i, err);
            };
        };

        thread::spawn(task)
    };
    let threads = (1..cpus).map(thread_task).collect::<Vec<_>>();
    for thread in threads {
        thread.join().map_err(|err| anyhow!("{:?}", err))?;
    }

    match file_count.lock() {
        Ok(count) => {
            println!("Total upload {}", count);
        }
        Err(err) => {
            println!("Main thread read file count failed {}", err);
        }
    }
    Ok(())
}
