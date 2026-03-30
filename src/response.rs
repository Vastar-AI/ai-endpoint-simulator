use std::fs;
use std::io;
use log::info;
use rand::seq::SliceRandom;
use crate::embedded_responses;

pub fn read_file_content(file_path: &str) -> io::Result<String> {
    info!("Reading file content from {}", file_path);
    std::fs::read_to_string(file_path)
}

pub fn read_random_markdown_file(folder_path: &str) -> io::Result<String> {
    // Try filesystem first
    if let Ok(paths) = fs::read_dir(folder_path) {
        let md_files: Vec<_> = paths
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "md"))
            .collect();

        if !md_files.is_empty() {
            let mut rng = rand::thread_rng();
            let random_file = md_files.choose(&mut rng).unwrap();
            return read_file_content(random_file.path().to_str().unwrap());
        }
    }

    // Fallback to embedded responses
    info!("Using embedded response data (no zresponse/ folder found)");
    let mut rng = rand::thread_rng();
    let content = embedded_responses::EMBEDDED_RESPONSES
        .choose(&mut rng)
        .expect("No embedded responses");
    Ok(content.to_string())
}

pub async fn read_random_markdown_file_async(folder_path: &str) -> io::Result<String> {
    let folder_path = folder_path.to_string();
    tokio::task::spawn_blocking(move || read_random_markdown_file(&folder_path))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Task join error: {}", e)))?
}
