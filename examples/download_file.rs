use std::path::Path;

#[tokio::main]
async fn main() {
    let url = "https://mirror.pilotfiber.com/ubuntu-iso/24.04.3/ubuntu-24.04.3-desktop-amd64.iso";
    // Define the output path for the downloaded client JAR file
    let output = Path::new("target/examples/ubuntu.iso");

    // Start the download task, saving the file to the specified path
    simple_download_utility::download_file(url, output, None)
        .await
        .unwrap();
}
