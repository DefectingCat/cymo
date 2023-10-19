use anyhow::Result;
use suppaftp::AsyncFtpStream;

pub async fn connect_and_init(
    ftp_stream: &mut AsyncFtpStream,
    i: usize,
    server: &str,
    username: Option<&String>,
    password: Option<&String>,
    remote_path: &str,
) -> Result<String> {
    println!("Thread {} connect to {} success", i, server);
    dbg!(username);
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
