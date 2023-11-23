use clap::Parser;

/// Cymo: Multi-threaded FTP Upload Tool
///
/// The `Args` struct represents the command-line arguments for the Cymo tool, which is used
/// for multi-threaded FTP file uploads.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "Cymo is a command-line tool for multi-threaded FTP file uploads. It allows you to efficiently upload files and directories to an FTP server using multiple threads, improving upload speed and performance.

You can specify the remote path on the FTP server, the local path to the files or directories to upload, and optional authentication credentials. Cymo simplifies the process of uploading large amounts of data to an FTP server with ease.

Example Usage:
To upload files to an FTP server, use a command like:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com

Or use username and password for authentication:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com -u <username> -p <password>"
)]
pub struct Args {
    /// The remote path on the FTP server where files will be uploaded.
    #[arg(short, long)]
    pub remote_path: String,

    /// The local path to the directory or file that will be uploaded to the FTP server.
    #[arg(short, long)]
    pub local_path: String,

    /// The FTP server address or hostname where the files will be uploaded.
    #[arg(short, long)]
    pub server: String,

    /// The username for authenticating with the FTP server (optional).
    #[arg(short, long)]
    pub username: Option<String>,

    /// The password for authenticating with the FTP server (optional).
    #[arg(short, long)]
    pub password: Option<String>,

    /// Retry times
    #[arg(long)]
    pub retry: Option<u32>,

    /// Remote server port
    #[arg(long, default_value_t = 21)]
    pub port: u32,

    /// Specific thread numbers
    #[arg(short, long)]
    pub thread: Option<usize>,
}
