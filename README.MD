# Simple Download Utility

The `simple_download_utility` crate provides utilities for downloading files with progress tracking and SHA1 validation support.

## Structures

### DownloadProgress

Progress information for a single file download.

```rust,ignore
pub struct DownloadProgress {
	pub bytes_to_download: usize,
	pub bytes_downloaded: usize,
	pub bytes_per_second: usize,
}
```

| Field               | Type    | Description              |
|---------------------|---------|--------------------------|
| `bytes_to_download` | `usize` | Total file size in bytes |
| `bytes_downloaded`  | `usize` | Bytes downloaded so far  |
| `bytes_per_second`  | `usize` | Current download speed   |

### MultiDownloadProgress

Progress information for multiple concurrent file downloads.

```rust,ignore
pub struct MultiDownloadProgress {
	pub bytes_to_download: usize,
	pub bytes_downloaded: usize,
	pub bytes_per_second: usize,
	pub files_downloaded: usize,
	pub files_total: usize,
	pub file_names_downloaded: Vec<String>,
	pub file_names: Vec<String>,
}
```

| Field                   | Type          | Description                         |
|-------------------------|---------------|-------------------------------------|
| `bytes_to_download`     | `usize`       | Total bytes across all files        |
| `bytes_downloaded`      | `usize`       | Total bytes downloaded so far       |
| `bytes_per_second`      | `usize`       | Current aggregate download speed    |
| `files_downloaded`      | `usize`       | Number of files completed           |
| `files_total`           | `usize`       | Total number of files to download   |
| `file_names_downloaded` | `Vec<String>` | Names of completed files            |
| `file_names`            | `Vec<String>` | Names of all files being downloaded |

### FileDownloadArguments

Arguments for downloading a single file in a batch operation.

```rust,ignore
pub struct FileDownloadArguments {
	pub url: String,
	pub sha1: Option<String>,
	pub path: String,
	pub sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
}
```

| Field    | Type                               | Description                             |
|----------|------------------------------------|-----------------------------------------|
| `url`    | `String`                           | URL to download from                    |
| `sha1`   | `Option<String>`                   | Optional SHA1 hash for validation       |
| `path`   | `String`                           | Destination file path                   |
| `sender` | `Option<Sender<DownloadProgress>>` | Optional progress channel for this file |

## Functions

### download_file()

Downloads a single file from a URL to a local path.

```rust,ignore
pub async fn download_file(
	url: impl AsRef<str>,
	path: impl AsRef<Path>,
	sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `url` - URL to download from
- `path` - Destination file path (parent directories are created automatically)
- `sender` - Optional channel for receiving download progress updates

**Returns:** `Result<()>` - Success or error

### download_file_with_client()

Downloads a single file using a provided HTTP client.

```rust,ignore
pub async fn download_file_with_client(
	client: impl Borrow<reqwest::Client>,
	url: impl AsRef<str>,
	path: impl AsRef<Path>,
	sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `client` - HTTP client to use (accepts `reqwest::Client`, `&reqwest::Client`, or `Arc<reqwest::Client>`)
- `url` - URL to download from
- `path` - Destination file path (parent directories are created automatically)
- `sender` - Optional channel for receiving download progress updates

**Returns:** `Result<()>` - Success or error

### download_and_validate_file()

Downloads a file and validates its SHA1 hash.

```rust,ignore
pub async fn download_and_validate_file(
	url: impl AsRef<str>,
	path: impl AsRef<Path>,
	hash: impl AsRef<str>,
	sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `url` - URL to download from
- `path` - Destination file path
- `hash` - Expected SHA1 hash
- `sender` - Optional channel for receiving download progress updates

**Returns:** `Result<()>` - Success or error if hash doesn't match

### download_and_validate_file_with_client()

Downloads a file using a provided HTTP client and validates its SHA1 hash.

```rust,ignore
pub async fn download_and_validate_file_with_client(
	client: impl Borrow<reqwest::Client>,
	url: impl AsRef<str>,
	path: impl AsRef<Path>,
	hash: impl AsRef<str>,
	sender: Option<tokio::sync::mpsc::Sender<DownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `client` - HTTP client to use (accepts `reqwest::Client`, `&reqwest::Client`, or `Arc<reqwest::Client>`)
- `url` - URL to download from
- `path` - Destination file path
- `hash` - Expected SHA1 hash
- `sender` - Optional channel for receiving download progress updates

**Returns:** `Result<()>` - Success or error if hash doesn't match

### download_multiple_files()

Downloads multiple files concurrently with progress tracking.

```rust,ignore
pub async fn download_multiple_files(
	items: Vec<FileDownloadArguments>,
	parallel: u16,
	sender: Option<tokio::sync::mpsc::Sender<MultiDownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `items` - List of files to download
- `parallel` - Maximum number of concurrent downloads
- `sender` - Optional channel for aggregate progress updates

**Returns:** `Result<()>` - Success or first error encountered

### download_multiple_files_with_client()

Downloads multiple files concurrently using a provided HTTP client.

```rust,ignore
pub async fn download_multiple_files_with_client(
	client: reqwest::Client,
	items: Vec<FileDownloadArguments>,
	parallel: u16,
	sender: Option<tokio::sync::mpsc::Sender<MultiDownloadProgress>>,
) -> Result<()>
```

**Parameters:**

- `client` - HTTP client to use (will be wrapped in `Arc` internally for sharing across concurrent downloads)
- `items` - List of files to download
- `parallel` - Maximum number of concurrent downloads
- `sender` - Optional channel for aggregate progress updates

**Returns:** `Result<()>` - Success or first error encountered

## Examples

### Download Single File Without Progress

```rust,no_run
use simple_download_utility::download_file;

#[tokio::main]
async fn main() {
	let url = "https://example.com/file.jar";
	let path = "downloads/file.jar";

	download_file(url, path, None).await.expect("Download failed");
}
```

### Download Single File With Progress

```rust,no_run
use simple_download_utility::{download_file, DownloadProgress};

#[tokio::main]
async fn main() {
	let url = "https://example.com/file.jar";
	let path = "downloads/file.jar";

	// Create a channel for progress updates
	let (sender, mut receiver) = tokio::sync::mpsc::channel::<DownloadProgress>(10);

	// Start the download task
	let download_task = download_file(url, path, Some(sender));

	// Handle progress updates in a separate task
	tokio::spawn(async move {
		while let Some(progress) = receiver.recv().await {
			let percent = (progress.bytes_downloaded as f32 / progress.bytes_to_download as f32) * 100.0;
			let mb_per_sec = progress.bytes_per_second as f32 / 1024.0 / 1024.0;
			println!("{:.2}% - {:.2} MB/s", percent, mb_per_sec);
		}
	});

	download_task.await.expect("Download failed");
}
```

### Download With SHA1 Validation

```rust,no_run
use simple_download_utility::download_and_validate_file;

#[tokio::main]
async fn main() {
	let url = "https://example.com/file.jar";
	let path = "downloads/file.jar";
	let expected_hash = "abc123def456...";

	download_and_validate_file(url, path, expected_hash, None)
		.await
		.expect("Download or validation failed");
}
```

### Download With Custom Client

Use the `_with_client` variants to share a configured HTTP client across multiple downloads:

```rust,no_run
use simple_download_utility::download_file_with_client;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
	// Create a configured client
	let client = reqwest::Client::builder()
		.timeout(Duration::from_secs(30))
		.user_agent("my-app/1.0")
		.build()
		.expect("Failed to create client");

	// Wrap in Arc to share across multiple downloads
	let client = Arc::new(client);

	// Use the same client for multiple downloads
	download_file_with_client(client.clone(), "https://example.com/file1.jar", "downloads/file1.jar", None)
		.await
		.expect("Download failed");

	download_file_with_client(client.clone(), "https://example.com/file2.jar", "downloads/file2.jar", None)
		.await
		.expect("Download failed");
}
```

### Download Multiple Files

```rust,no_run
use simple_download_utility::{download_multiple_files, FileDownloadArguments, MultiDownloadProgress};

#[tokio::main]
async fn main() {
	let files = vec![
		FileDownloadArguments {
			url: "https://example.com/file1.jar".to_string(),
			sha1: Some("hash1...".to_string()),
			path: "downloads/file1.jar".to_string(),
			sender: None,
		},
		FileDownloadArguments {
			url: "https://example.com/file2.jar".to_string(),
			sha1: Some("hash2...".to_string()),
			path: "downloads/file2.jar".to_string(),
			sender: None,
		},
	];

	// Create a channel for aggregate progress
	let (sender, mut receiver) = tokio::sync::mpsc::channel::<MultiDownloadProgress>(16);

	// Start downloads with 10 concurrent connections
	let download_task = download_multiple_files(files, 10, Some(sender));

	// Handle progress updates
	tokio::spawn(async move {
		while let Some(progress) = receiver.recv().await {
			let percent = (progress.files_downloaded as f32 / progress.files_total as f32) * 100.0;
			let mb_per_sec = progress.bytes_per_second as f32 / 1024.0 / 1024.0;
			println!("{:.2}% ({}/{} files) - {:.2} MB/s",
			         percent,
			         progress.files_downloaded,
			         progress.files_total,
			         mb_per_sec
			);
		}
	});

	download_task.await.expect("Downloads failed");
}
```

## Progress Tracking Pattern

The download functions use Tokio's `mpsc` channels for progress reporting. This allows non-blocking progress updates while downloads continue:

```rust,ignore
// Create a buffered channel
let (sender, mut receiver) = tokio::sync::mpsc::channel::<DownloadProgress>(buffer_size);

// Start download with sender
let download_task = download_file(url, path, Some(sender));

// Handle progress in separate task
let progress_task = tokio::spawn( async move {
while let Some(progress) = receiver.recv().await {
// Process progress update
}
});

// Wait for download to complete
download_task.await?;

// Progress task will end when channel closes
```

The buffer size determines how many progress updates can queue before backpressure occurs. A buffer of 10-16 is typically sufficient.
