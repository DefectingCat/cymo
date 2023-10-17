use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Ok as AOk, Result};
use clap::Parser;
use futures::future::BoxFuture;
use futures::FutureExt;
use glob::glob;
use suppaftp::FtpStream;
use tokio::spawn;
use tokio::sync::Mutex;

use crate::args::Args;

mod args;
mod utils;

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
    println!("Find {} file(s)", files.lock().await.len());

    let mut ftp_stream = FtpStream::connect(format!("{}:21", server))?;
    if let (Some(username), Some(password)) = (username, password) {
        ftp_stream.login(username, password)?;
        println!("Login {} success", server);
    }
    ftp_stream.cwd(remote_path)?;
    println!("Current directory: {}", ftp_stream.pwd()?);

    let files = files.lock().await;
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
