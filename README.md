# Cymo: Multithreaded FTP Upload Tool

Cymo is a command-line tool for multithreaded FTP file uploads.

## Features

-   Multithreaded.
-   Asynchronous.
-   TCP Stream.

## 安装

### 预构建

下载预先构建好的二进制文件：

```bash
chmod +x ./cymo
move ./cymo /usr/local/bin # optional
```

### 从源码构建

```bash
cargo make install
```

## 食用

```bash
Cymo: Multi-threaded FTP Upload Tool

Usage: cymo [OPTIONS] --remote-path <REMOTE_PATH> --local-path <LOCAL_PATH> --server <SERVER>

Options:
  -r, --remote-path <REMOTE_PATH>  The remote path on the FTP server where files will be uploaded
  -l, --local-path <LOCAL_PATH>    The local path to the directory or file that will be uploaded to the FTP server
  -s, --server <SERVER>            The FTP server address or hostname where the files will be uploaded
  -u, --username <USERNAME>        The username for authenticating with the FTP server (optional)
  -p, --password <PASSWORD>        The password for authenticating with the FTP server (optional)
      --retry <RETRY>              Retry times
      --port <PORT>                Remote server port [default: 21]
  -t, --thread <THREAD>            Specific thread numbers
  -h, --help                       Print help (see more with '--help')
  -V, --version                    Print version
```

```bash
# To upload files to an FTP server, use a command like:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com

# Or use username and password for authentication:
$ cymo -r /ftp/upload -l /local/files -s ftp.example.com -u <username> -p <password>
```

## 参数:

-   `-r, --remote-path:` The remote path on the FTP server where files will be uploaded.
-   `-l, --local-path:` The local path to the directory or file that will be uploaded to the FTP server.
-   `-s, --server:` The FTP server address or hostname where the files will be uploaded.
-   `-u, --username:` The username for authenticating with the FTP server (optional).
-   `-p, --password:` The password for authenticating with the FTP server (optional).
