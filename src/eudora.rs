use anyhow::{anyhow, Result};

use std::path::{Path, PathBuf};
use suppaftp::AsyncFtpStream;
use tokio::fs::File;
use tokio::io;
use tokio_util::compat::{FuturesAsyncWriteCompatExt, TokioAsyncReadCompatExt};

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
/// # Examples
///
/// ```rust
/// use anyhow::Result;
/// use suppaftp::AsyncFtpStream;
///
/// async fn example() -> Result<()> {
///     let mut ftp_stream = AsyncFtpStream::connect("127.0.0.1:21").await?;
///     let current_remote = connect_and_init(&mut ftp_stream, 0, "127.0.0.1", Some("anonymous"), Some("anonymous"), "/home/user").await?;
///     println!("Current remote directory: {}", current_remote);
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// This function may return an error if any of the FTP operations fail, such as connecting, logging
/// in, or changing directory. The error will contain the details of the failure.
pub async fn connect_and_init(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    server: &str,
    username: Option<&String>,
    password: Option<&String>,
    remote_path: &str,
) -> Result<String> {
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
    Ok(current_remote)
}

/// Changes the remote directory on the FTP server to match the local directory.
///
/// This function takes a mutable reference to an `AsyncFtpStream`, an index `i` that identifies the thread, a reference to a `Path` that represents the local directory, and a reference to a `str` that represents the current remote directory. It returns a `Result<()>` that indicates whether the operation was successful or not.
///
/// This function first extracts the components of the local directory and skips the first one, which is assumed to be the root directory. It then appends these components to the current remote directory and tries to change to it using the `cwd` method of the `AsyncFtpStream`. If the remote directory does not exist, it creates it using the `mkdir` method and then changes to it. It prints a message to indicate the success of the operation.
///
/// # Examples
///
/// ```no_run
/// use async_ftp::AsyncFtpStream;
/// use std::path::Path;
///
/// let mut ftp_stream = AsyncFtpStream::connect("127.0.0.1:21").await?;
/// let i = 0;
/// let path = Path::new("/home/user/foo/bar/baz.txt");
/// let current_remote = "/var/www/html";
/// change_remote(&mut ftp_stream, i, path.parent().unwrap(), current_remote).await?;
/// ```
pub async fn change_remote(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    parents: &Path,
    current_remote: &str,
) -> Result<()> {
    let parents = parents
        .components()
        .collect::<Vec<_>>()
        .into_iter()
        .skip(1)
        .collect::<Vec<_>>();
    // .components()
    // .collect::<Vec<_>>();
    let len = parents.len();
    for index in 0..=len {
        let local_path = &parents[..index]
            .iter()
            .fold(PathBuf::new(), |mut prev, cur| {
                prev.push(PathBuf::from(cur));
                prev
            });
        // Current remote directory
        let mut remote = PathBuf::from(&current_remote);
        remote.push(local_path);
        let remote = remote.to_string_lossy();
        // Current local directory
        if local_path.to_string_lossy().len() != 0 {
            // Create or change to it.
            remote_mkdir(ftp_stream, i, &remote).await?;
        }
    }
    Ok(())
}

async fn remote_mkdir(ftp_stream: &mut AsyncFtpStream, i: usize, remote: &str) -> Result<()> {
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
/// # Examples
///
/// ```no_run
/// use async_ftp::AsyncFtpStream;
/// use std::path::Path;
///
/// let mut ftp_stream = AsyncFtpStream::connect("127.0.0.1:21").await?;
/// let i = 0;
/// let path = Path::new("/home/user/foo/bar/baz.txt");
/// let current_remote = "/var/www/html";
/// upload_files(&mut ftp_stream, i, &path, current_remote).await?;
/// ```
pub async fn upload_files(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    path: &Path,
    current_remote: &str,
) -> Result<()> {
    // Current local file filename
    let filename = path
        .file_name()
        .ok_or(anyhow!(""))?
        .to_str()
        .ok_or(anyhow!(""))?;
    // Current local file parent directories
    let parents = path.parent();
    // Check remote directory exists
    // And change into it.
    if let Some(parents) = parents {
        change_remote(ftp_stream, i, parents, current_remote).await?;
    }
    // Upload files
    let mut local = File::open(&path).await?;
    let mut remote = ftp_stream.put_with_stream(filename).await?.compat_write();
    io::copy(&mut local, &mut remote).await?;
    ftp_stream.finalize_put_stream(remote.compat()).await?;
    println!("Thread {} upload file {:?} success", i, &path);
    Ok(())
}
