#![doc = include_str!("../README.MD")]

use anyhow::{anyhow, Result};
use serde::Serialize;
use sha_file_hashing::{Hashable, SHAError};
use std::borrow::Borrow;
use std::path::Path;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize)]
pub struct DownloadProgress {
    pub bytes_to_download: usize,
    pub bytes_downloaded: usize,
    pub bytes_per_second: usize,
}

pub struct MultiDownloadProgress {
    pub bytes_to_download: usize,
    pub bytes_downloaded: usize,
    pub bytes_per_second: usize,
    pub files_downloaded: usize,
    pub files_total: usize,
    pub file_names_downloaded: Vec<String>,
    pub file_names: Vec<String>,
}

#[derive(Debug)]
pub struct FileDownloadArguments {
    pub url: String,
    pub sha1: Option<String>,
    pub path: String,
    pub sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
}

pub async fn download_file(
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    download_file_with_client(reqwest::Client::new(), url, path, sender).await
}

pub async fn download_and_validate_file(
    url: impl AsRef<str>,
    path: impl AsRef<Path>,
    hash: impl AsRef<str>,
    sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()> {
    download_and_validate_file_with_client(reqwest::Client::new(), url, path, hash, sender).await
}

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
        let result = reqwest::get(url).await?;
        let bytes = result.bytes().await?;
        file.write_all(bytes.iter().as_slice()).await?;
    }

    file.flush().await?;
    Ok(())
}

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
                    download_and_validate_file_with_client(client, &item.url, &item.path, sha1, Some(internal_tx)).await
                } else {
                    download_file_with_client(client, &item.url, &item.path, Some(internal_tx)).await
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

pub async fn download_multiple_files(
    items: Vec<FileDownloadArguments>,
    parallel: u16,
    sender: Option<tokio::sync::mpsc::Sender<MultiDownloadProgress>>,
) -> Result<()> {
    let client = reqwest::Client::new();
    download_multiple_files_with_client(client, items, parallel, sender).await
}
