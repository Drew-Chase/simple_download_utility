//! ```
//! Module to handle downloading and validating files, with support for progress tracking and multiple file downloads.
//!
//! # Overview
//! This module provides functions to download files from a given URL to a specified path.
//! It also includes facilities for tracking download progress, validating file integrity using hash values,
//! and downloading multiple files concurrently. The module leverages `reqwest` for HTTP requests,
//! `tokio` for asynchronous file I/O, and `serde
#![doc = include_str!("../README.MD")]

use anyhow::{Result, anyhow};
use serde::Serialize;
use sha_file_hashing::{Hashable, SHAError};
use std::borrow::Borrow;
use std::path::Path;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

/// Represents the progress of a file download.
///
/// This struct tracks the total size, the amount downloaded so far,
/// and the current download speed.
///
/// # Fields
///
/// * `bytes_to_download` - The total number of bytes to be downloaded.
/// * `bytes_downloaded` - The number of bytes that have been downloaded so far.
/// * `bytes_per_second` - The current download speed in bytes per second.
///
/// # Example
/// ```
/// use your_crate::DownloadProgress;
///
/// let progress = DownloadProgress {
///     bytes_to_download: 1_000_000,
///     bytes_downloaded: 500_000,
///     bytes_per_second: 200_000,
/// };
///
/// println!("{:?}", progress);
/// ```
#[derive(Debug, Serialize)]
pub struct DownloadProgress {
    pub bytes_to_download: usize,
    pub bytes_downloaded: usize,
    pub bytes_per_second: usize,
}

/// A structure representing the progress of multiple file downloads.
///
/// This structure tracks various metrics related to the download process, including
/// the number of files and bytes downloaded, the download speed, and the names of
/// the files being downloaded or already downloaded.
pub struct MultiDownloadProgress {
    pub bytes_to_download: usize,
    pub bytes_downloaded: usize,
    pub bytes_per_second: usize,
    pub files_downloaded: usize,
    pub files_total: usize,
    pub file_names_downloaded: Vec<String>,
    pub file_names: Vec<String>,
}

/// Represents the arguments required to download a file, along with optional verification and progress updates.
///
/// # Fields
///
/// * `url` - The URL of the file to be downloaded. This is a required field and must be a valid URL string.
///
/// * `sha1` - An optional SHA-1 checksum string used to verify the integrity of the downloaded file.
///            If provided, the downloaded file's checksum will be compared with this value.
///
/// * `path` - The filesystem path where the downloaded file will be saved. This is a required field and must
///            point to a valid location accessible to the program.
///
/// * `sender` - An optional sender channel (`tokio::sync::mpsc::Sender`) for sending updates on the download progress.
///              If provided, progress updates will be communicated through this channel.
///
/// # Example
///
/// ```
/// use tokio::sync::mpsc;
/// use crate::FileDownloadArguments;
/// use crate::DownloadProgress;
///
/// let (tx, rx) = mpsc::channel(100);
///
/// let file_args = FileDownloadArguments {
///     url: "https://example.com/file.zip".to_string(),
///     sha1: Some("d3486ae9136e7856bc42212385ea797094475802".to_string()),
///     path: "./downloads/file.zip".to_string(),
///     sender: Some(tx),
/// };
///
/// // Use `file_args` to initiate the file download
/// ```
#[derive(Debug)]
pub struct FileDownloadArguments {
    pub url: String,
    pub sha1: Option<String>,
    pub path: String,
    pub sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
}

/// Downloads a file from a given URL to a specified local path asynchronously.
///
/// # Parameters
/// - `url`: The URL of the file to download. This can be any type that implements `AsRef<str>`.
/// - `path`: The destination path where the downloaded file will be saved. This can be any type that implements `AsRef<Path>`.
/// - `sender`: An optional `tokio::sync::mpsc::Sender` to send download progress updates.
///     - If provided, the function will send `DownloadProgress` messages through this channel,
///       allowing the caller to monitor the download progress in real time.
///     - If `None`, no progress updates will be sent.
///
/// # Returns
/// - On success, returns `Ok(())`.
/// - On failure, returns an `Err` with the appropriate error details.
///
/// # Example
/// ```rust
/// use tokio::sync::mpsc;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let url = "https://example.com/file.zip";
///     let path = Path::new("file.zip");
///
///     // Set up an optional channel for monitoring progress
///     let (sender, mut receiver) = mpsc::channel(100);
///
///     // Spawn a task to handle progress updates
///     tokio::spawn(async move {
///         while let Some(progress) = receiver.recv().await {
///             println!("Download progress: {:?}", progress);
///         }
///     });
///
///     // Download the file
///     download_file(url, path, Some(sender)).await?;
///
///     Ok(())
/// }
/// ```
///
/// # Notes
/// - This function internally uses a `reqwest::Client` for making HTTP requests.
/// - To customize the HTTP client or add specific configurations, consider using `download_file_with_client` instead.
pub async fn download_file(
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    download_file_with_client(reqwest::Client::new(), url, path, sender).await
}

/// Downloads a file from the specified URL, saves it to the given path, and validates its hash.
///
/// # Parameters
/// - `url`: The URL of the file to download. Must be a type that implements `AsRef<str>`.
/// - `path`: The local filesystem path where the downloaded file will be saved. Must implement `AsRef<Path>`.
/// - `hash`: The expected hash of the downloaded file used for validation. Must implement `AsRef<str>`.
/// - `sender`: An optional `tokio::sync::mpsc::Sender` to report download progress. Each progress update is sent as a `DownloadProgress` object.
///
/// # Returns
/// - `Ok(())` if the file is successfully downloaded, saved, and validated against the provided hash.
/// - `Err(_)` if an error occurs during the download, file saving, or hash validation process.
///
/// # Errors
/// This function will return an error if:
/// - The download fails (e.g., due to network errors).
/// - Writing the file to the specified path fails.
/// - The file's hash does not match the provided expected hash.
///
/// # Notes
/// - The function internally uses a default `reqwest::Client` for the download.
/// - If a sender is provided, consumers can use it to track download progress.
///
/// # Example
/// ```
/// use tokio::sync::mpsc;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let url = "https://example.com/file.txt";
///     let path = Path::new("file.txt");
///     let hash = "expected_hash_here";
///
///     let (progress_sender, mut progress_receiver) = mpsc::channel(100);
///
///     // Spawn a task to print download progress
///     tokio::spawn(async move {
///         while let Some(progress) = progress_receiver.recv().await {
///             println!("Downloaded: {}%", progress.percent);
///         }
///     });
///
///     download_and_validate_file(url, path, hash, Some(progress_sender)).await?;
///     println!("File downloaded and validated successfully.");
///     Ok(())
/// }
/// ```
pub async fn download_and_validate_file(
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    hash: impl AsRef<str>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    download_and_validate_file_with_client(reqwest::Client::new(), url, path, hash, sender).await
}

/// Downloads a file from the specified URL, saves it to the given path, and validates its checksum.
///
/// This function performs an asynchronous file download using the provided `reqwest::Client`.
/// After downloading, it validates the file's contents against the expected hash.
/// If the validation succeeds, the function resolves successfully; otherwise, it returns an error.
///
/// # Arguments
///
/// * `client` - A borrowable instance of a `reqwest::Client` used to perform the HTTP request.
/// * `url` - The URL of the file to download.
/// * `path` - The local file path where the downloaded file should be saved.
/// * `hash` - The expected hash value for validating the file's integrity.
/// * `sender` - An optional `tokio::sync::mpsc::Sender` to send real-time download progress updates.
///   This allows monitoring download progress asynchronously.
///
/// # Returns
///
/// * `Ok(())` - If the file was downloaded and successfully validated against the hash.
/// * `Err(anyhow::Error)` - If any error occurs during downloading, writing to disk, or validation.
///
/// # Errors
///
/// This function returns an error in the following scenarios:
/// - Network issues while downloading the file.
/// - File I/O errors while saving the file.
/// - Validation failure if the computed hash of the file does not match the expected hash.
/// - Any unexpected issues during validation (e.g., unsupported hash algorithms).
///
/// # Example
///
/// ```rust
/// use tokio::sync::mpsc;
/// use reqwest::Client;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let client = Client::new();
///     let url = "https://example.com/file.txt";
///     let path = Path::new("downloaded_file.txt");
///     let hash = "expected_sha256_hash";
///     let (progress_sender, mut progress_receiver) = mpsc::channel(100);
///
///     tokio::spawn(async move {
///         while let Some(progress) = progress_receiver.recv().await {
///             println!("Download progress: {:?}", progress);
///         }
///     });
///
///     download_and_validate_file_with_client(client, url, path, hash, Some(progress_sender)).await?;
///     println!("File downloaded and validated successfully");
///
///     Ok(())
/// }
/// ```
///
/// # Notes
///
/// The `path.validate(hash)` function is assumed to validate the file against the given
/// hash. If the function fails validation or encounters other issues, appropriate errors
/// are returned.
///
/// Ensure that the provided `hash` matches the hash type supported by the validation
/// logic (e.g., SHA256).
pub async fn download_and_validate_file_with_client(
    client: impl Borrow<reqwest::Client>,
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    hash: impl AsRef<str>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    let path = path.as_ref();
    download_file_with_client(client, url, path, sender).await?;

    match path.validate(hash) {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!(SHAError::FailedValidation(format!(
            "Failed to validate file: {}",
            path.display()
        )))),
        Err(e) => Err(anyhow!(e)),
    }
}

/// Asynchronously downloads a file from the specified URL and writes it to the provided file path.
///
/// This function uses the `reqwest` client to fetch the file's content and supports download
/// progress reporting via an optional `tokio::sync::mpsc::Sender`.
///
/// # Arguments
///
/// * `client` - An instance of a `reqwest::Client`, or a type that can be borrowed as a `reqwest::Client`.
/// * `url` - The URL of the file to download. It can be provided as any type that implements the `AsRef<str>` trait.
/// * `path` - The file path where the downloaded content will be saved. It can be provided as any type that implements the `AsRef<Path>` trait.
/// * `sender` - An optional `tokio::sync::mpsc::Sender` to send progress updates during the download. If provided, progress updates will be sent as instances of `DownloadProgress`.
///
/// # Returns
///
/// Returns a `Result<()>` which:
/// - On success, indicates the file was successfully downloaded and written to the specified path.
/// - On failure, contains the error that occurred during the operation.
///
/// # Progress Reporting
///
/// If a `sender` is provided, progress updates are sent as `DownloadProgress` instances. These updates
/// include:
/// - `bytes_to_download`: The total size of the file in bytes (if known).
/// - `bytes_downloaded`: The number of bytes downloaded so far.
/// - `bytes_per_second`: The current download speed in bytes per second.
///
/// # Behavior
///
/// - If the parent directories of the provided path do not exist, they will be created automatically.
/// - If progress reporting is enabled, the function streams the file content while updating progress.
///   Otherwise, the file is downloaded in one operation.
/// - If the file already exists at the specified path, it will be overwritten.
///
/// # Example
///
/// ```rust
/// use reqwest::Client;
/// use tokio::sync::mpsc;
/// use std::path::Path;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = Client::new();
///     let url = "https://example.com/file.zip";
///     let path = Path::new("downloads/file.zip");
///
///     let (tx, mut rx) = mpsc::channel(10);
///
///     // Spawn a task to print progress updates
///     tokio::spawn(async move {
///         while let Some(progress) = rx.recv().await {
///             println!(
///                 "Downloaded: {} / {} bytes, Speed: {} bytes/sec",
///                 progress.bytes_downloaded,
///                 progress.bytes_to_download,
///                 progress.bytes_per_second
///             );
///         }
///     });
///
///     download_file_with_client(client, url, path, Some(tx)).await?;
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// - The file path is invalid or cannot be created.
/// - The file cannot be written to the specified path.
/// - Network errors occur during the download.
/// - Progress updates fail to be sent to the channel (if progress reporting is enabled).
pub async fn download_file_with_client(
    client: impl Borrow<reqwest::Client>,
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    let client = client.borrow();
    let url = url.as_ref();
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(path).await?;

    if let Some(sender) = sender {
        let response = client.get(url).send().await?;
        let total_size = response.content_length().unwrap_or(0) as usize;

        let mut stream = response.bytes_stream();
        let mut downloaded = 0;
        let start_time = Instant::now();

        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;

            downloaded += chunk.len();
            let elapsed = start_time.elapsed().as_secs_f64();
            let bytes_per_second = if elapsed > 0.0 {
                (downloaded as f64 / elapsed) as usize
            } else {
                0
            };

            let progress = DownloadProgress {
                bytes_to_download: total_size,
                bytes_downloaded: downloaded,
                bytes_per_second,
            };

            let _ = sender.send(progress).await;
        }
    } else {
        let result = client.get(url).send().await?;
        let bytes = result.bytes().await?;
        file.write_all(bytes.iter().as_slice()).await?;
    }

    file.flush().await?;
    Ok(())
}

/// Downloads multiple files concurrently using the given HTTP client, with configurable parallelism and progress reporting.
///
/// # Arguments
///
/// * `client` - A `reqwest::Client` instance to be used for making HTTP requests.
/// * `items` - A vector of `FileDownloadArguments` containing the URL, file path, optional checksum (SHA1), and optional progress sender for each file to be downloaded.
/// * `parallel` - Maximum number of concurrent downloads allowed.
/// * `sender` - An optional `tokio::sync::mpsc::Sender<MultiDownloadProgress>` to report the overall download progress across all files.
///
/// # Errors
///
/// This function may return an error in the following cases:
/// * Errors related to creating directories for file paths that do not exist.
/// * Errors related to the `download_file_with_client` or `download_and_validate_file_with_client` utilities (e.g., HTTP errors, checksum mismatches).
/// * Errors from the internal asynchronous tasks or communication channels.
///
/// # Returns
///
/// On successful completion, the function returns `Ok(())`. If any error occurs during the download of files, the function will return the first encountered error.
///
/// # Progress Reporting
///
/// If a `sender` is provided, the function will send periodic updates encapsulated in a `MultiDownloadProgress` instance.
/// This includes information such as:
/// * Total bytes to download across all files.
/// * Total bytes already downloaded.
/// * Estimated download speed in bytes per second.
/// * Count of successfully completed downloads.
/// * Names of files downloaded successfully.
/// * Names of all files being downloaded.
///
/// # Implementation Notes
///
/// 1. The function uses the `futures_util::stream` to handle concurrent downloads while adhering to the specified parallelism level.
/// 2. Each file's progress is tracked individually using a channel (`internal_tx`) which is processed to update the shared `ProgressState`.
/// 3. The shared progress state is protected with a `tokio::sync::Mutex` to ensure safe access across asynchronous tasks.
/// 4. The function ensures that parent directories for file paths are created, and it validates downloaded files against their provided SHA1 checksum if applicable.
///
/// # Example
///
/// ```rust
/// use tokio::sync::mpsc;
/// use reqwest::Client;
///
/// #[tokio::main]
/// async fn main() {
///     let client = Client::new();
///     let items = vec![
///         FileDownloadArguments {
///             url: "https://example.com/file1".to_string(),
///             path: "/path/to/file1".to_string(),
///             sha1: None,
///             sender: None,
///         },
///         FileDownloadArguments {
///             url: "https://example.com/file2".to_string(),
///             path: "/path/to/file2".to_string(),
///             sha1: Some("abc123...".to_string()),
///             sender: None,
///         },
///     ];
///     let parallel = 4;
///     let (progress_sender, mut progress_receiver) = mpsc::channel(32);
///
///     // Spawn a task to receive progress updates
///     tokio::spawn(async move {
///         while let Some(progress) = progress_receiver.recv().await {
///             println!("Progress: {:?}", progress);
///         }
///     });
///
///     let result = download_multiple_files_with_client(client, items, parallel, Some(progress_sender)).await;
///
///     if let Err(e) = result {
///         eprintln!("Error occurred: {:?}", e);
///     } else {
///         println!("All files downloaded successfully!");
///     }
/// }
/// ```
pub async fn download_multiple_files_with_client(
    client: reqwest::Client,
    items: Vec<FileDownloadArguments>,
    parallel: u16,
    sender: Option<tokio::sync::mpsc::Sender<MultiDownloadProgress>>,
) -> Result<()> {
    use futures_util::StreamExt;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let files_total = items.len();
    let file_names: Vec<String> = items
        .iter()
        .map(|item| {
            Path::new(&item.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| item.path.clone())
        })
        .collect();

    struct ProgressState {
        file_progress: Vec<(usize, usize)>, // (bytes_to_download, bytes_downloaded) per file
        files_downloaded: usize,
        file_names_downloaded: Vec<String>,
    }

    let progress_state = Arc::new(Mutex::new(ProgressState {
        file_progress: vec![(0, 0); files_total],
        files_downloaded: 0,
        file_names_downloaded: Vec::new(),
    }));

    let start_time = Arc::new(Instant::now());
    let sender = sender.map(Arc::new);
    let file_names = Arc::new(file_names);
    let client = Arc::new(client);

    let results: Vec<Result<()>> = futures_util::stream::iter(items.into_iter().enumerate())
        .map(|(idx, item)| {
            let progress_state = Arc::clone(&progress_state);
            let sender = sender.clone();
            let file_names = Arc::clone(&file_names);
            let start_time = Arc::clone(&start_time);
            let client = Arc::clone(&client);

            async move {
                let path = Path::new(&item.path);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                let (internal_tx, mut internal_rx) =
                    tokio::sync::mpsc::channel::<DownloadProgress>(32);
                let original_sender = item.sender;

                let progress_task = {
                    let progress_state = Arc::clone(&progress_state);
                    let sender = sender.clone();
                    let file_names = Arc::clone(&file_names);
                    let start_time = Arc::clone(&start_time);

                    tokio::spawn(async move {
                        while let Some(progress) = internal_rx.recv().await {
                            {
                                let mut state = progress_state.lock().await;
                                state.file_progress[idx] =
                                    (progress.bytes_to_download, progress.bytes_downloaded);

                                let total_bytes: usize =
                                    state.file_progress.iter().map(|(t, _)| *t).sum();
                                let downloaded_bytes: usize =
                                    state.file_progress.iter().map(|(_, d)| *d).sum();

                                let elapsed = start_time.elapsed().as_secs_f64();
                                let bytes_per_second = if elapsed > 0.0 {
                                    (downloaded_bytes as f64 / elapsed) as usize
                                } else {
                                    0
                                };

                                if let Some(ref sender) = sender {
                                    let multi_progress = MultiDownloadProgress {
                                        bytes_to_download: total_bytes,
                                        bytes_downloaded: downloaded_bytes,
                                        bytes_per_second,
                                        files_downloaded: state.files_downloaded,
                                        files_total,
                                        file_names_downloaded: state.file_names_downloaded.clone(),
                                        file_names: file_names.to_vec(),
                                    };
                                    let _ = sender.send(multi_progress).await;
                                }
                            }

                            if let Some(ref original) = original_sender {
                                let _ = original.send(progress).await;
                            }
                        }
                    })
                };

                let result = if let Some(sha1) = &item.sha1 {
                    download_and_validate_file_with_client(
                        client,
                        &item.url,
                        &item.path,
                        sha1,
                        Some(internal_tx),
                    )
                    .await
                } else {
                    download_file_with_client(client, &item.url, &item.path, Some(internal_tx))
                        .await
                };

                let _ = progress_task.await;

                if result.is_ok() {
                    let mut state = progress_state.lock().await;
                    state.files_downloaded += 1;
                    state.file_names_downloaded.push(file_names[idx].clone());

                    if let Some(ref sender) = sender {
                        let total_bytes: usize = state.file_progress.iter().map(|(t, _)| *t).sum();
                        let downloaded_bytes: usize =
                            state.file_progress.iter().map(|(_, d)| *d).sum();

                        let elapsed = start_time.elapsed().as_secs_f64();
                        let bytes_per_second = if elapsed > 0.0 {
                            (downloaded_bytes as f64 / elapsed) as usize
                        } else {
                            0
                        };

                        let multi_progress = MultiDownloadProgress {
                            bytes_to_download: total_bytes,
                            bytes_downloaded: downloaded_bytes,
                            bytes_per_second,
                            files_downloaded: state.files_downloaded,
                            files_total,
                            file_names_downloaded: state.file_names_downloaded.clone(),
                            file_names: file_names.to_vec(),
                        };
                        let _ = sender.send(multi_progress).await;
                    }
                }

                result
            }
        })
        .buffer_unordered(parallel as usize)
        .collect()
        .await;

    for result in results {
        result?;
    }

    Ok(())
}

/// Downloads multiple files concurrently using the given parameters.
///
/// # Arguments
///
/// * `items` - A vector of `FileDownloadArguments` structs, where each struct contains
///   the necessary details for downloading a single file (e.g., URL, destination path, etc.).
/// * `parallel` - The maximum number of concurrent downloads allowed. This defines the
///   level of concurrency for downloading files.
/// * `sender` - An optional `tokio::sync::mpsc::Sender` channel used to send
///   `MultiDownloadProgress` updates back to the caller. If `None`, no progress updates are sent.
///
/// # Returns
///
/// A `Result` which is:
/// * `Ok(())` if all the files are successfully downloaded.
/// * An `Err` if any part of the download process fails.
///
/// # Note
///
/// This function creates a new `reqwest::Client` instance and delegates
/// the actual download task to `download_multiple_files_with_client`. If you
/// need a custom configured client, consider directly using the `download_multiple_files_with_client`
/// function.
///
/// # Example
///
/// ```no_run
/// use tokio::sync::mpsc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let download_items = vec![/* populate with FileDownloadArguments instances */];
/// let (sender, mut receiver) = mpsc::channel(100);
///
/// tokio::spawn(async move {
///     while let Some(progress) = receiver.recv().await {
///         println!("Download progress: {:?}", progress);
///     }
/// });
///
/// download_multiple_files(download_items, 4, Some(sender)).await?;
/// # Ok(())
/// # }
/// ```
pub async fn download_multiple_files(
    items: Vec<FileDownloadArguments>,
    parallel: u16,
    sender: Option<tokio::sync::mpsc::Sender<MultiDownloadProgress>>,
) -> Result<()> {
    let client = reqwest::Client::new();
    download_multiple_files_with_client(client, items, parallel, sender).await
}
