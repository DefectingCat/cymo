use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use glob::glob;
use rayon::prelude::*;

use crate::args::Args;
use crate::utils::map_mutex_err;

mod args;
mod utils;

/// Recursively reads all files in the target directory and stores their paths in the `files` vector.
///
/// This function traverses the specified directory and its subdirectories, adding the paths
/// of all files found to the `files` vector. It uses parallel processing to improve performance.
///
/// # Arguments
///
/// * `files`: An `Arc<Mutex<Vec<PathBuf>>>` that holds the vector of file paths. It is shared
/// across multiple threads and safely modified using a mutex.
/// * `path`: The path of the directory to start the recursive traversal.
///
/// # Returns
///
/// Returns a `Result` indicating success or an error if any occurred during the traversal.
///
/// # Example
///
/// ```rust
/// use std::sync::{Arc, Mutex};
/// use std::path::PathBuf;
/// use anyhow::Result;
/// use rayon::prelude::*;
///
/// let files = Arc::new(Mutex::new(Vec::new()));
/// let path = PathBuf::from("/path/to/start/directory");
///
/// if let Err(err) = recursive_read_file(files.clone(), path) {
///     eprintln!("Error: {:?}", err);
/// }
///
/// // Access the collected file paths
/// let collected_files = files.lock().unwrap();
/// println!("Collected files: {:?}", *collected_files);
/// ```
///
/// In this example, the `recursive_read_file` function is used to recursively collect file paths
/// from a starting directory, and the results are accessed from the `files` vector after the
/// function completes.
fn recursive_read_file(files: Arc<Mutex<Vec<PathBuf>>>, path: PathBuf) -> Result<()> {
    if path.is_file() {
        let mut files = files.lock().map_err(map_mutex_err)?;
        files.push(path);
        return Ok(());
    }
    if path.is_dir() {
        let dir = fs::read_dir(&path)?;
        let _ = dir
            .par_bridge()
            .into_par_iter()
            .map(|path| recursive_read_file(files.clone(), path?.path()))
            .collect::<Vec<_>>();
    }
    Ok(())
}

fn main() -> Result<()> {
    let Args {
        remote_path,
        local_path,
    } = Args::parse();

    let local_path = glob(&local_path)?;
    let files = Arc::new(Mutex::new(vec![]));
    let _ = local_path
        .par_bridge()
        .into_par_iter()
        .map(|path| {
            use anyhow::Ok as AOk;
            match path {
                Ok(path) => recursive_read_file(files.clone(), path),
                Err(err) => {
                    eprintln!("Read file failed {}", err);
                    AOk(())
                }
            }
        })
        .collect::<Result<Vec<_>>>();

    println!(
        "Find {} file(s)",
        files.lock().map_err(map_mutex_err)?.len()
    );

    // let local_files =  local_path.iter().map(|path| {
    //       dbg!(path);
    //   });

    Ok(())
}
