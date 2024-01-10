# Changelog

## [0.2.9] - 2024-01-10

### Fixed

-   Upload failed when local params too long.

## [0.2.8] - 2023-11-23

### Added

-   Auto thread number detect.
-   Upload speed calculate.
-   Improve performance for build file list.

### Fixed

-   Remove useless cpu count.
-   Remove dependencies doc for `cargo doc`.

## [0.2.7] - 2023-11-23

### Fixed

-   Create remote directory failed when other threads created it.
-   Change back to base remote directory failed.
-   WASM file regconize as text file.
-   Count local path parent length error.

## [0.2.6] - 2023-11-23

### Added

-   Set thread number from args.

### Fixed

-   File count error when connect failed.
-   Refactor files collector in target directory with `Walkdir`.

## [0.2.5] - 2023-11-21

### Added

-   Maximum retry times setting.
-   Count upload failed files.
-   Skip failed file when retry failed.
-   List all upload failed files.

## [0.2.4] - 2023-11-01

### Added

-   Count total uploaded files.
-   Retry when upload failed.

### Fixed

-   Upload single file failed.
-   Upload empty or less 8 bytes file failed.

## [0.2.3] - 2023-10-30

### Added

-   Detect local file mime type and set is binary or text file to FTP server.

### Fixed

-   Panic when local files not exist.

## [0.2.2] - 2023-10-24

### Fixed

-   Each threads to be uploaded file number calculate error

## [0.2.1] - 2023-10-23

### Added

-   Sort local file list by directory name
-   Improve performance
-   Improve binary size

### Fixed

-   Remote directory detection
-   Change same remote directory repeatedly

## [0.2.0] - 2023-10-20

### Added

-   Asynchronous read file.
-   Asynchronous ftp stream.
-   File stream to upload.
-   File path pattern support.

## [0.0.1] - 2023-10-17

### Added

-   Initial release of the multithreaded FTP client.
-   Multithreaded asynchronous read target directory.
