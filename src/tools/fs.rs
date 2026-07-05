use color_eyre::eyre::Result;
use rig::tool::ToolDyn;
use std::fs;
use thiserror::Error;

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    return vec![Box::new(ReadFile)];
}

#[derive(Debug, Error)]
pub enum FsError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),
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
