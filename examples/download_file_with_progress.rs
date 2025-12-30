use std::path::Path;
use simple_download_utility::DownloadProgress;

#[tokio::main]
async fn main() {
	let url = "https://mirror.pilotfiber.com/ubuntu-iso/24.04.3/ubuntu-24.04.3-desktop-amd64.iso";
	// Define the output path for the downloaded client JAR file
	let output = Path::new("target/examples/ubuntu.iso");

	// Create a channel for receiving download progress updates
	// Buffer size of 10 allows some backpressure handling
	let (sender, mut receiver) = tokio::sync::mpsc::channel::<DownloadProgress>(10);

	// Start the download task, saving the file to the specified path
	let task = simple_download_utility::download_file(url, output, Some(sender));
	// Spawn a separate task to handle progress updates asynchronously
	tokio::spawn(async move {
		// Continuously receive and display progress until the channel closes
		while let Some(progress) = receiver.recv().await {
			// Calculate download completion percentage
			let percent = (progress.bytes_downloaded as f32 / progress.bytes_to_download as f32) * 100.0;
			// Convert download speed from bytes/sec to megabytes/sec
			let mb_per_sec = progress.bytes_per_second as f32 / 1024.0 / 1024.0;

			println!("Download progress: {:.2}% ({}/{} bytes) {:.2} MB/s", percent, progress.bytes_downloaded, progress.bytes_to_download, mb_per_sec);
		}
	});

	// Await the download task completion
	task.await.unwrap();
}
