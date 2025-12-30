use simple_download_utility::{download_multiple_files, FileDownloadArguments, MultiDownloadProgress};

#[tokio::main]
async fn main() {
    // Define multiple files to download
    let files = vec![
        FileDownloadArguments {
            url: "https://www.rust-lang.org/logos/rust-logo-512x512.png".to_string(),
            sha1: None,
            path: "target/examples/rust-logo.png".to_string(),
            sender: None,
        },
        FileDownloadArguments {
            url: "https://raw.githubusercontent.com/rust-lang/rust/master/README.md".to_string(),
            sha1: None,
            path: "target/examples/rust-readme.md".to_string(),
            sender: None,
        },
        FileDownloadArguments {
            url: "https://raw.githubusercontent.com/rust-lang/rust/master/CONTRIBUTING.md".to_string(),
            sha1: None,
            path: "target/examples/rust-contributing.md".to_string(),
            sender: None,
        },
        FileDownloadArguments {
            url: "https://raw.githubusercontent.com/rust-lang/rust/master/LICENSE-MIT".to_string(),
            sha1: None,
            path: "target/examples/license-mit.txt".to_string(),
            sender: None,
        },
    ];

    // Create a channel for receiving multi-download progress updates
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<MultiDownloadProgress>(10);

    // Spawn a task to handle progress updates
    tokio::spawn(async move {
        while let Some(progress) = receiver.recv().await {
            // Calculate overall completion percentage
            let percent = if progress.bytes_to_download > 0 {
                (progress.bytes_downloaded as f32 / progress.bytes_to_download as f32) * 100.0
            } else {
                0.0
            };

            // Convert download speed to MB/s
            let mb_per_sec = progress.bytes_per_second as f32 / 1024.0 / 1024.0;

            println!(
                "Overall: {:.2}% | Files: {}/{} | Downloaded: {}/{} bytes | Speed: {:.2} MB/s",
                percent,
                progress.files_downloaded,
                progress.files_total,
                progress.bytes_downloaded,
                progress.bytes_to_download,
                mb_per_sec
            );

            // Show which files have been downloaded
            if !progress.file_names_downloaded.is_empty() {
                println!("  Completed: {:?}", progress.file_names_downloaded);
            }
        }
    });

    // Start downloading multiple files with 3 parallel downloads
    println!("Starting download of {} files...\n", files.len());

    match download_multiple_files(files, 3, Some(sender)).await {
        Ok(_) => println!("\nAll files downloaded successfully!"),
        Err(e) => eprintln!("\nError downloading files: {}", e),
    }
}
