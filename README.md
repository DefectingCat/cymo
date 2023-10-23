# Cymo: Multithreaded FTP Upload Tool

Cymo is a command-line tool for multithreaded FTP file uploads.

## Features

- Multithreaded.
- Asynchronous.
- TCP Stream.

## 安装

```bash

```

## 食用

```bash
# To upload files to an FTP server, use a command like:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com

# Or use username and password for authentication:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com -u <username> -p <password>
```

## 参数:

- `-r, --remote-path:` The remote path on the FTP server where files will be uploaded.
- `-l, --local-path:` The local path to the directory or file that will be uploaded to the FTP server.
- `-s, --server:` The FTP server address or hostname where the files will be uploaded.
- `-u, --username:` The username for authenticating with the FTP server (optional).
- `-p, --password:` The password for authenticating with the FTP server (optional).
