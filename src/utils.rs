use crate::{
    args::Args,
    eudora::{connect_and_init, get_args, remote_mkdir},
};
use anyhow::{anyhow, Ok as AOk, Result};
use crossbeam_channel::Sender;
use std::{path::PathBuf, sync::Arc};
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
    files: Arc<tokio::sync::Mutex<Vec<PathBuf>>>,
    cpus: usize,
    sender: Sender<Vec<PathBuf>>,
) -> impl Fn() {
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

            let mut files = files.lock().await;
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
