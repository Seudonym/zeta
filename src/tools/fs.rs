use color_eyre::eyre::Result;
use std::fs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),
}

#[rig::tool_macro(
    description = "Lists contents of the directory",
    params(directory = "The directory to list files in")
)]
pub async fn list_files(directory: String) -> Result<Vec<String>, FsError> {
    let entries = fs::read_dir(directory)?;
    let entries: Vec<String> = entries
        .filter_map(|res| res.ok())
        .map(|entry| {
            let path = entry.path();
            let path_str = path.to_string_lossy().into_owned();
            if path.is_dir() {
                format!("{}/", path_str)
            } else {
                path_str
            }
        })
        .collect();
    Ok(entries)
}

#[rig::tool_macro(
    description = "Read the contents of a file",
    params(filename = "The absolute path of the file")
)]
pub async fn read_file(file_path: String) -> Result<String, FsError> {
    let bytes = fs::read(file_path)?;
    let res = String::from_utf8_lossy(&bytes).into_owned();
    Ok(res)
}
