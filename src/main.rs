use std::{sync::Arc, thread};

use anyhow::{anyhow, Ok as AOk, Result};
use clap::Parser;
use crossbeam_channel::unbounded;
use glob::glob;
use suppaftp::AsyncFtpStream;
use tokio::{
    runtime, spawn,
    sync::{Mutex, RwLock},
};

use crate::args::Args;
use crate::eudora::{connect_and_init, recursive_read_file, upload_files};

mod args;
mod eudora;

fn main() -> Result<()> {
    let args = Arc::new(RwLock::new(Args::parse()));
    let files = Arc::new(Mutex::new(vec![]));

    let main_rt = runtime::Builder::new_multi_thread().build()?;
    let main_handle = main_rt.block_on(async {
        let local_path = &args.read().await.local_path;
        let local_path = glob(local_path)?;
        let tasks = local_path
            .into_iter()
            .map(|path| {
                let files = files.clone();
                let task = async move {
                    match path {
                        Ok(path) => {
                            recursive_read_file(files.clone(), path).await?;
                            AOk(())
                        }
                        Err(err) => {
                            eprintln!("Read file failed {}", err);
                            AOk(())
                        }
                    }
                };
                spawn(task)
            })
            .collect::<Vec<_>>();

        for task in tasks {
            let _ = task.await?;
        }

        let files = files.lock().await;
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
    thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let mut files = files.lock().await;
            // Total files length
            let len = files.len();
            // Div by cpu nums
            let (quotient, remainder) = (len / cpus, len % cpus); // calculate the quotient and remainders.send()
            let start = 0;
            (0..cpus).for_each(|i| {
                let end = if i < remainder {
                    // if i is less than the remainder, add one extra element to the smaller array
                    start + quotient + 1
                } else {
                    // otherwise, use the quotient as the size of the smaller array
                    start + quotient
                };
                let file_data = files.drain(start..end).collect::<Vec<_>>();
                let len = file_data.len();
                if let Err(err) = s.send(file_data) {
                    eprintln!(
                        "Send files to thread failed {:?}. {} files not send",
                        err, len
                    );
                }
            });
        });
    });

    let threads = (1..cpus)
        .map(|i| {
            let args = args.clone();
            let r = r.clone();
            let task = move || {
                let rt = runtime::Builder::new_current_thread().build().unwrap();
                let handle = rt.block_on(async {
                    let args = &args.read().await;
                    let Args {
                        username,
                        password,
                        server,
                        remote_path,
                        ..
                    } = &*(*(args));
                    let mut ftp_stream = AsyncFtpStream::connect(format!("{}:21", server)).await?;
                    let current_remote = connect_and_init(
                        &mut ftp_stream,
                        i,
                        server,
                        username.as_ref(),
                        password.as_ref(),
                        remote_path,
                    )
                    .await?;

                    // Receive files from main thread.
                    for path in r.recv()? {
                        match upload_files(&mut ftp_stream, i, &path, &current_remote).await {
                            Ok(_) => {
                                println!("Thread {} upload file {:?} success", i, &path);
                            }
                            Err(err) => {
                                eprintln!("Thread {} upload {:?} failed {}", i, &path, err)
                            }
                        }
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
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().map_err(|err| anyhow!("{:?}", err))?;
    }
    Ok(())
}
