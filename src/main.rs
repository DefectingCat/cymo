use std::cmp::Ordering;
use std::path::PathBuf;
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
use crate::eudora::{connect_and_init, get_args, recursive_read_file, upload_files};

mod args;
mod eudora;

// Arguments
static ARG: OnceLock<Args> = OnceLock::new();
// Used for skip folders
static PARAM_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
// Remote path, used for detect remote path
static REMOTE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn main() -> Result<()> {
    let args = Args::parse();
    PARAM_PATH.get_or_init(|| {
        let local_path = PathBuf::from(&args.local_path);
        let parent = local_path.parent();
        parent.map(PathBuf::from)
    });
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
        let depth = depth.lock().await;
        files.sort_by_key(|a| a.iter().count());
        let param_path = PARAM_PATH.get().ok_or(anyhow!("Parse args error"))?;
        let start = if param_path.is_none() {
            0
        } else {
            param_path.iter().count()
        };
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
        let len = files.len();
        println!("Find {} file(s)", len);
        AOk(())
    });
    main_handle?;
    main_rt.shutdown_background();

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
            // Div by cpu nums
            let (quotient, remainder) = (len / cpus, len % cpus); // calculate the quotient and remainders.send()
            let start = 0;
            let sender = (0..cpus)
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
        // TODO retry, when files is empty, exit threads
        let _ = result.map_err(|err| {
            eprintln!(
                "Send files to thread failed {:?}. {} files not send",
                err, len
            );
        });
    };
    thread::spawn(task);

    let thread_task = |i| {
        let r = r.clone();
        let task = move || {
            let rt = runtime::Builder::new_current_thread().build().unwrap();
            let handle = rt.block_on(async {
                let Args { server, .. } = get_args()?;
                println!("Thread {} connecting {}", i, &server);
                // TODO add server port configuration
                let mut ftp_stream = AsyncFtpStream::connect(format!("{}:21", server)).await?;
                connect_and_init(&mut ftp_stream, i).await?;

                // Receive files from main thread.
                // TODO thread continue
                for path in r.recv()? {
                    upload_files(&mut ftp_stream, i, &path)
                        .await
                        .with_context(|| format!("Thread {} upload {:?} failed", i, &path))?;
                    println!("Thread {} upload file {:?} success", i, &path);
                }
                ftp_stream.quit().await?;
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
    Ok(())
}
