use std::path::PathBuf;
use std::sync::Arc;
use std::{fs, thread};

use anyhow::{Ok as AOk, Result};
use clap::Parser;
use futures::future::BoxFuture;
use futures::FutureExt;
use glob::glob;
use suppaftp::AsyncFtpStream;
use tokio::spawn;
use tokio::sync::Mutex;

use crate::args::Args;

mod args;

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
/// # [tokio::main]
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

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        remote_path,
        local_path,
        server,
        username,
        password,
    } = Args::parse();

    let local_path = glob(&local_path)?;
    let files = Arc::new(Mutex::new(vec![]));
    let tasks = local_path
        .into_iter()
        .map(|path| {
            let files = files.clone();
            spawn(async move {
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
            })
        })
        .collect::<Vec<_>>();

    for task in tasks {
        let _ = task.await?;
    }

    let files = files.lock().await;
    let len = files.len();
    println!("Find {} file(s)", len);

    let cpus = thread::available_parallelism()?.get();
    let div = (len as f64 / cpus as f64).ceil() as usize;
    dbg!(&div);

    let mut ftp_clients = vec![];
    for i in 1..=cpus {
        let mut ftp_stream = AsyncFtpStream::connect(format!("{}:21", server)).await?;
        println!("Thread {} connect to {} success", i, &server);
        if let (Some(username), Some(password)) = (&username, &password) {
            ftp_stream.login(username, password).await?;
            println!("Thread {} login {} success", i, &server);
        }
        ftp_stream.cwd(&remote_path).await?;
        println!(
            "Thread {} current directory: {}",
            i,
            ftp_stream.pwd().await?
        );
        let ftp_stream = Arc::new(Mutex::new(ftp_stream));
        ftp_clients.push(ftp_stream);
    }
    dbg!(ftp_clients.len());

    let tasks = files
        .iter()
        .enumerate()
        .map(|(i, file)| {})
        .collect::<Vec<_>>();

    // let _ = files
    //     .map(|file| {
    //         let filename = file
    //             .file_name()
    //             .ok_or_else(|| anyhow!("Cannot read target name {:?}", file))?
    //             .to_str()
    //             .ok_or_else(|| anyhow!("Cannot read target name {:?}", file))?;
    //         let file = fs::read(file)?;
    //
    //         dbg!(id);
    //
    //         Ok(())
    //     })
    //     .collect::<Result<Vec<_>>>();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

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
