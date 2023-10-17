use clap::Parser;
use std::path::PathBuf;

/// Cymo: Multi-threaded FTP Upload Tool
///
/// The `Args` struct represents the command-line arguments for the Cymo tool, which is used
/// for multi-threaded FTP file uploads.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use cymo_tool::Args;
///
/// let args = Args {
///     remote_path: PathBuf::from("/remote/ftp/directory"),
///     local_path: "/local/directory".to_string(),
/// };
///
/// println!("Remote Path: {:?}", args.remote_path);
/// println!("Local Path: {:?}", args.local_path);
/// ```
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The remote path on the FTP server where files will be uploaded.
    #[arg(short, long)]
    pub remote_path: PathBuf,

    /// The local path to the directory or file that will be uploaded to the FTP server.
    #[arg(short, long)]
    pub local_path: String,
}
