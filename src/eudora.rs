use std::path::{Path, PathBuf};

use std::time::Duration;

use anyhow::{anyhow, Result};
use async_recursion::async_recursion;

use suppaftp::types::{FileType, FormatControl};
use suppaftp::AsyncFtpStream;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use tokio::io;
use tokio::time::sleep;
use tokio_util::compat::{FuturesAsyncWriteCompatExt, TokioAsyncReadCompatExt};
use walkdir::DirEntry;

use crate::args::Args;
use crate::{ARG, PARAM_PATH, REMOTE_PATH};

pub fn get_args<'a>() -> Result<&'a Args> {
    ARG.get().ok_or(anyhow!("Parse args error"))
}

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Connects to an FTP server and changes to a target directory, and returns the current remote directory name.
///
/// This function takes a mutable reference to an `AsyncFtpStream`, which is used to perform
/// asynchronous FTP operations. It also takes the index of the thread that is calling the function,
/// the address of the server, the optional username and password for authentication, and the remote
/// path to change to.
///
/// The function prints some messages to indicate the progress of the connection, login, and directory
/// change. It also prints the welcome message from the server, if any. It returns a `Result<String>`
/// that contains the current remote directory name, or an error if any of the FTP operations fail.
///
/// # Arguments
///
/// * `ftp_stream` - A mutable reference to an `AsyncFtpStream` that is used to communicate with the
///   server.
/// * `i` - The index of the thread that is calling the function.
/// * `server` - A reference to a string that contains the address of the server.
/// * `username` - An optional reference to a string that contains the username for authentication.
/// * `password` - An optional reference to a string that contains the password for authentication.
/// * `remote_path` - A reference to a string that contains the remote path to change to.
///
/// # Errors
///
/// This function may return an error if any of the FTP operations fail, such as connecting, logging
/// in, or changing directory. The error will contain the details of the failure.
pub async fn connect_and_init(
    ftp_stream: Result<&mut AsyncFtpStream, &mut anyhow::Error>,
    i: usize,
) -> Result<()> {
    let Args {
        username,
        password,
        remote_path,
        server,
        ..
    } = get_args()?;
    let ftp_stream = ftp_stream.map_err(|err| anyhow!("{}", err))?;
    println!("Thread {} connect to {} success", i, server);
    if let (Some(username), Some(password)) = (&username, &password) {
        ftp_stream.login(username, password).await?;
        println!("Thread {} login {} success", i, &server);
    }
    ftp_stream.cwd(&remote_path).await?;
    let current_remote = ftp_stream.pwd().await?;
    println!("Thread {} current directory: {}", i, &current_remote);
    if let Some(welcome) = ftp_stream.get_welcome_msg() {
        println!("{}", welcome);
    }
    Ok(())
}

/// Changes the remote directory on the FTP server to match the local directory.
///
/// This function takes a mutable reference to an `AsyncFtpStream`, an index `i` that identifies the thread, a reference to a `Path` that represents the local directory, and a reference to a `str` that represents the current remote directory. It returns a `Result<()>` that indicates whether the operation was successful or not.
///
/// This function first extracts the components of the local directory and skips the first one, which is assumed to be the root directory. It then appends these components to the current remote directory and tries to change to it using the `cwd` method of the `AsyncFtpStream`. If the remote directory does not exist, it creates it using the `mkdir` method and then changes to it. It prints a message to indicate the success of the operation.
pub async fn change_remote(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    parents: &Path,
    current_remote: &str,
) -> Result<()> {
    // Collect path from params
    let param_path = PARAM_PATH.get().ok_or(anyhow!("Parse args error"))?;
    if param_path.is_file() {
        return Ok(());
    }

    // Skip folders from params
    let parents = parents
        .components()
        .collect::<Vec<_>>()
        .into_iter()
        .skip(param_path.parent().iter().len())
        .collect::<Vec<_>>();

    // The final remote path
    let remote_path = REMOTE_PATH.get().ok_or(anyhow!(""))?;
    let mut remote = PathBuf::from(&remote_path);
    remote.push(parents.iter().collect::<PathBuf>());
    // If path is same, do not change directory
    if remote.to_string_lossy() == current_remote {
        return Ok(());
    }

    let len = parents.len();
    if len == 0 {
        remote_mkdir(ftp_stream, i, &remote_path.to_string_lossy()).await?;
        return Ok(());
    }
    let local_path = &parents.iter().collect::<PathBuf>();
    // Current remote directory
    let mut remote = PathBuf::from(&remote_path);
    remote.push(local_path);
    let remote = remote.to_string_lossy();
    // Current local directory
    if local_path.to_string_lossy().len() != 0 {
        // Create or change to it.
        remote_mkdir(ftp_stream, i, &remote).await?;
    }
    Ok(())
}

pub async fn remote_mkdir(ftp_stream: &mut AsyncFtpStream, i: usize, remote: &str) -> Result<()> {
    // Create or change to it.
    match ftp_stream.cwd(&remote).await {
        Ok(_) => {
            let remote = ftp_stream.pwd().await?;
            println!("Thread {} change directory to {} success", i, remote);
        }
        Err(_) => {
            ftp_stream.mkdir(&remote).await?;
            println!("Thread {} create directory {} success", i, remote);
            ftp_stream.cwd(&remote).await?;
            println!("Thread {} change directory to {} success", i, remote);
        }
    }
    Ok(())
}

/// Uploads a local file to the FTP server.
///
/// This function takes a mutable reference to an `AsyncFtpStream`, an index `i` that identifies the thread, a reference to a `Path` that represents the local file, and a reference to a `str` that represents the current remote directory. It returns a `Result<()>` that indicates whether the operation was successful or not.
///
/// This function first extracts the file name and the parent directories of the local file. It then calls the `change_remote` function to ensure that the remote directory exists and matches the local directory. It then opens the local file using `File::open` and creates a data stream for uploading using `put_with_stream`. It copies the bytes from the local file to the data stream using `io::copy` and finalizes the upload using `finalize_put_stream`. It prints a message to indicate the success of the operation.
///
pub async fn upload_files(ftp_stream: &mut AsyncFtpStream, i: usize, path: &Path) -> Result<()> {
    // Current local file filename
    let filename = path
        .file_name()
        .ok_or(anyhow!("read file name failed"))?
        .to_str()
        .ok_or(anyhow!("read file name failed"))?;

    let current_remote = ftp_stream.pwd().await?;
    // Current local file parent directories
    let parents = path.parent();
    // Check remote directory exists
    // And change into it.
    if let Some(parents) = parents {
        change_remote(ftp_stream, i, parents, &current_remote).await?;
    }
    // Upload files
    // https://docs.rs/suppaftp/latest/suppaftp/types/enum.FileType.html#
    // TODO replace file
    let mut local = File::open(&path).await?;
    // Detect file type
    let mut magic_number = [0u8; 16];
    if local.read_exact(&mut magic_number).await.is_ok() {
        let is_text = String::from_utf8(magic_number.into());
        if is_text.is_ok() {
            ftp_stream
                .transfer_type(FileType::Ascii(FormatControl::Default))
                .await?;
        } else {
            ftp_stream.transfer_type(FileType::Binary).await?;
        }
    };

    let mut local = File::open(&path).await?;
    // Stream file content to ftp server
    let mut remote = ftp_stream.put_with_stream(filename).await?.compat_write();
    io::copy(&mut local, &mut remote).await?;
    ftp_stream.finalize_put_stream(remote.compat()).await?;
    Ok(())
}

/// TODO show file upload speed
#[async_recursion(?Send)]
pub async fn upload(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    path: &Path,
    retry_times: u32,
) -> Result<()> {
    let Args { retry, .. } = get_args()?;
    return match upload_files(ftp_stream, i, path).await {
        Ok(res) => Ok(res),
        Err(err) => match retry {
            Some(times) => {
                if retry_times >= *times {
                    return Err(err);
                }
                sleep_with_seconds(3, format!("Thread {} file {:?}", i, path).into()).await;
                upload(ftp_stream, i, path, retry_times + 1).await
            }
            None => Err(err),
        },
    };
}

/// Sleep current thread and print count
///
/// Argments:
///
/// - `duration`: duration for sleep, seconds
async fn sleep_with_seconds(duration: usize, message: Option<String>) {
    let message = message.map(|m| format!("{} ", m)).unwrap_or("".into());
    for i in 1..=duration {
        println!("{}will retry in {}s", message, duration - i);
        sleep(Duration::from_secs(1)).await;
    }
}
