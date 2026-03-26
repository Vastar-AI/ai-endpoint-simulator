use std::fs::{self, File};
use std::io::{self, Read};
use log::info;
use rand::seq::SliceRandom;
use crate::ResponseSimulator;

pub fn read_file_content(file_path: &str) -> io::Result<String> {
    info!("Reading file content from {}", file_path);
    let mut file = File::open(file_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

pub fn read_random_markdown_file(folder_path: &str) -> io::Result<String> {
    let paths = fs::read_dir(folder_path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "md"))
        .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();
    let random_file = paths.choose(&mut rng).expect("No markdown files found");

    read_file_content(random_file.path().to_str().unwrap())
}

/// Async version of read_random_markdown_file using spawn_blocking
/// to avoid blocking the async runtime
pub async fn read_random_markdown_file_async(folder_path: &str) -> io::Result<String> {
    let folder_path = folder_path.to_string();

    tokio::task::spawn_blocking(move || {
        read_random_markdown_file(&folder_path)
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Task join error: {}", e)))?
}

/// Async version of read_file_content using tokio::fs
pub async fn read_file_content_async(file_path: &str) -> io::Result<String> {
    info!("Reading file content async from {}", file_path);
    tokio::fs::read_to_string(file_path).await
}

pub(crate) fn format_response_from_db(response: &ResponseSimulator) -> String {
    info!("Formatting response from database");
    let mut formatted_response = format!(
        "**Pertanyaan:**\n{}\n\n**Jawaban:**\n{}",
        response.pertanyaan, response.jawaban
    );

    if !response.referensi.is_empty() {
        formatted_response.push_str(&format!("\n\n**Referensi:**\n{}", response.referensi));
    }

    formatted_response.replace("\\n", "\n")
}

pub fn select_random_response_from_db(responses: &[ResponseSimulator]) -> &ResponseSimulator {
    info!("Selecting random response from database");
    let mut rng = rand::thread_rng();
    responses.choose(&mut rng).expect("No responses available")
}
