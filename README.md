# Cymo: Multi-threaded FTP Upload Tool

Cymo is a command-line tool for multithreaded FTP file uploads. It allows you to efficiently upload files and directories to an FTP server using multiple threads, improving upload speed and performance.

## Features

- Multithreaded FTP uploads: Speed up the file upload process by utilizing multiple threads.
- Specify the remote path: Upload files to a specific path on the FTP server.
- Supports authentication: Provide username and password for FTP server authentication (optional).
- User-friendly and easy to use: Simplifies the process of uploading large amounts of data to an FTP server.

## Usage

```bash
# To upload files to an FTP server, use a command like:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com

# Or use username and password for authentication:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com -u <username> -p <password>
```

## Options:

- `-r, --remote-path:` The remote path on the FTP server where files will be uploaded.
- `-l, --local-path:` The local path to the directory or file that will be uploaded to the FTP server.
- `-s, --server:` The FTP server address or hostname where the files will be uploaded.
- `-u, --username:` The username for authenticating with the FTP server (optional).
- `-p, --password:` The password for authenticating with the FTP server (optional).
