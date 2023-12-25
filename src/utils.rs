use crate::{
    args::Args,
    eudora::{connect_and_init, get_args, remote_mkdir, upload},
};
use anyhow::{anyhow, Ok as AOk, Result};
use crossbeam_channel::{Receiver, Sender};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use suppaftp::AsyncFtpStream;
use tokio::runtime;

/// Find parents of all files
///
/// ## Arguments
///
/// `local_path`: local files path from params
///
/// ## Return
///
/// `Fn`: used for `fold()`'s callback
pub fn fold_parents(local_path: &String) -> impl Fn(Vec<PathBuf>, &PathBuf) -> Vec<PathBuf> {
    let local_path = PathBuf::from(local_path);

    move |mut prev: Vec<_>, cur: &PathBuf| -> Vec<PathBuf> {
        let skip_count = local_path
            .parent()
            .unwrap_or(&PathBuf::new())
            .components()
            .count();
        let skip_count = if local_path.is_dir() && local_path.components().count() == 1 {
            1
        } else {
            skip_count
        };
        let parent = cur
            .parent()
            .map(|parent| parent.components().skip(skip_count))
            .filter(|p| p.clone().count() > 0)
            .map(|p| p.collect::<PathBuf>());
        if let Some(p) = parent {
            if prev.contains(&p) {
                return prev;
            }
            let components = p.components().collect::<Vec<_>>();
            for index in 1..=components.len() {
                let path = &components[..index]
                    .iter()
                    .fold(PathBuf::new(), |mut child, cur| {
                        child.push(PathBuf::from(cur));
                        child
                    });
                prev.push(path.clone());
            }
        }
        prev
    }
}

/// In a single system thread to parse files.
///
/// - connect to ftp server and create all parents not exist on server.
/// - divide file list by cpu nums, then send to child threads.
///
/// ## Arguments
///
/// - `files`: total found files path.
/// - `cpus`: current cpu nums.
/// - `sneder`: message channel for send files.
///
/// ## Error
///
/// The function will failure when create parent folders on ftp server.
pub fn build_worker_task(
    mut files: Vec<PathBuf>,
    cpus: usize,
    sender: Sender<Vec<PathBuf>>,
) -> impl FnMut() {
    move || {
        let rt = runtime::Builder::new_current_thread().build().unwrap();
        let task = async {
            let Args {
                server,
                port,
                local_path,
                remote_path,
                ..
            } = get_args()?;
            let addr = format!("{}:{}", server, port);
            let mut ftp_stream = AsyncFtpStream::connect(addr).await.map_err(|err| {
                eprintln!("Thread main connnect failed {}", err);
                anyhow!("{}", err)
            });
            let _ = connect_and_init(ftp_stream.as_mut(), 0).await;
            let mut ftp_stream = ftp_stream?;

            // All element in files is files, so can use parent.
            // Create all parent folders.
            let all_parents: Vec<_> = files.iter().fold(vec![], fold_parents(local_path));
            for parent in all_parents {
                let mut remote = PathBuf::from(&remote_path);
                remote.push(parent);
                remote_mkdir(&mut ftp_stream, 0, &remote.to_string_lossy()).await?;
            }
            // Total files length
            let len = files.len();
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
                    sender.send(file_data)?;
                    AOk(())
                })
                .collect::<Result<Vec<_>>>();
            AOk((sender, len))
        };
        let (result, len) = rt.block_on(task).expect("start a tokio runtime failed");
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
        };
    }
}

/// Build upload threads.
///
/// This function will be build tokio async runtime in single
/// thread. And connect to ftp server in the runtime.
///
/// ## Arguments
///
/// - `receiver`: file list receiver.
/// - `file_count`: file list length.
/// - `failed_files`: file list for sent failed.
///
/// ## Return
///
/// A std thread handler `JoinHandle<()>`.
pub fn create_thread_task(
    receiver: Receiver<Vec<PathBuf>>,
    file_count: Arc<Mutex<usize>>,
    failed_files: Arc<Mutex<Vec<PathBuf>>>,
) -> impl Fn(usize) -> JoinHandle<()> {
    move |i| {
        let r = receiver.clone();
        let file_count = file_count.clone();
        let failed_files = failed_files.clone();
        let thread_task = move || {
            let rt = runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("create tokio runtime failed");

            let async_task = async {
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
            };
            let async_handle = rt.block_on(async_task);
            if let Err(err) = async_handle {
                eprintln!("Thread {} got error {}", i, err);
            };
        };

        thread::spawn(thread_task)
    }
}
