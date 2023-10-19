use std::{fs, path::PathBuf, sync::Arc, thread};

use anyhow::{anyhow, Ok as AOk, Result};
use clap::Parser;
use crossbeam_channel::unbounded;
use futures::future::BoxFuture;
use futures::FutureExt;
use glob::glob;
use suppaftp::AsyncFtpStream;

use tokio::{
    runtime, spawn,
    sync::{Mutex, RwLock},
};

use crate::args::Args;
use crate::eudora::{connect_and_init, upload_files};

mod args;
mod eudora;

/// Recursively reads all the files in a given directory and stores
/// their paths in a shared data structure.
///
/// This function takes two parameters: `files` and `path`. The `files` parameter is an
/// `Arc<Mutex<Vec<PathBuf>>>`, which is a thread-safe reference-counted pointer to a
/// mutex-protected vector of file paths. The `path` parameter is a `PathBuf`,
/// which is a type of owned file or directory path.
///
/// This function returns a `BoxFuture<'static, Result<()>>`, which is a type of heap-allocated
/// asynchronous value that can be executed later and can return either an empty tuple or an error.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use std::sync::{Arc, Mutex};
/// use futures::{future::BoxFuture, executor::block_on};
/// use tokio::fs;
///
/// # [tokio::main
/// async fn main () -> Result< (), Box<dyn std::error::Error>> {
///     // Create a shared data structure to store the file paths
///     let files = Arc::new(Mutex::new(Vec::new()));
///     // Create a path to the current directory
///     let path = PathBuf::from(".");
///     // Call the recursive_read_file function and get the future
///     let future = recursive_read_file(files.clone(), path);
///     // Await the future to complete
///     future.await?;
///     // Print the file paths
///     println!("{:?}", files.lock().unwrap());
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// This function may return an error if:
///
/// - The `path` is neither a file nor a directory.
/// - There is an error while reading the directory entries.
/// - There is an error while creating or executing a task.
fn recursive_read_file(
    files: Arc<Mutex<Vec<PathBuf>>>,
    path: PathBuf,
) -> BoxFuture<'static, Result<()>> {
    async move {
        if path.is_file() {
            let mut files = files.lock().await;
            files.push(path);
            return Ok(());
        }
        if path.is_dir() {
            let dir = fs::read_dir(&path)?;
            let tasks = dir
                .into_iter()
                .map(|path| {
                    let files = files.clone();
                    spawn(async move {
                        recursive_read_file(files, path?.path()).await?;
                        AOk(())
                    })
                })
                .collect::<Vec<_>>();
            for task in tasks {
                let _ = task.await?;
            }
        }
        Ok(())
    }
    .boxed()
}

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
                        upload_files(&mut ftp_stream, i, &path, &current_remote).await?;
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

#[cfg(test)]
mod tests {
    use tokio::fs;

    use super::*;

    #[tokio::test]
    async fn test_recursive_read_file() {
        // Create a temporary test directory
        let temp_dir = tempfile::tempdir().unwrap();
        let test_path = temp_dir.path().to_path_buf();

        // Create some test files and directories
        let file1 = test_path.join("file1.txt");
        let file2 = test_path.join("file2.txt");
        let sub_dir = test_path.join("sub_dir");
        let file3 = sub_dir.join("file3.txt");
        fs::write(&file1, "Test file 1 content").await.unwrap();
        fs::write(&file2, "Test file 2 content").await.unwrap();
        fs::create_dir(&sub_dir).await.unwrap();
        fs::write(&file3, "Test file 3 content").await.unwrap();

        // Create an Arc<Mutex> to hold the collected file paths
        let files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));

        // Call the recursive_read_file function
        recursive_read_file(files.clone(), test_path.clone())
            .await
            .expect("Failed to read files recursively");

        // Lock the files mutex to access the collected paths
        let files = files.lock().await;

        // Check if the collected paths match the expected paths
        assert_eq!(files.len(), 3);
        assert!(files.contains(&file1));
        assert!(files.contains(&file2));
        assert!(files.contains(&file3));

        // Clean up the temporary test directory
        temp_dir.close().unwrap();
    }
}
