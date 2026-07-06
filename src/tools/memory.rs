use color_eyre::eyre::Result;
use directories::ProjectDirs;
use rig::tool::ToolDyn;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;
use tokio::process::Command;

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(SearchMemories),
        Box::new(InspectMemory),
        Box::new(WriteMemory),
    ]
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Failed to access memory file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Could not determine cache directory")]
    NoProjectDir,
}

fn get_memory_dir() -> Result<PathBuf, MemoryError> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "zeta") {
        let memory_dir = proj_dirs.data_dir().join("memory");
        fs::create_dir_all(&memory_dir)?;
        Ok(memory_dir)
    } else {
        Err(MemoryError::NoProjectDir)
    }
}

#[derive(Serialize)]
pub struct SearchResult {
    pub matches: Vec<String>,
}

#[rig::tool_macro(
    description = "Search the memory files by keyword",
    params(query = "A list of queries to search for")
)]
pub async fn search_memories(query: Vec<String>) -> Result<SearchResult, MemoryError> {
    let memory_dir = get_memory_dir()?;
    if !memory_dir.exists() {
        return Ok(SearchResult { matches: vec![] });
    }

    let output = Command::new("fd")
        .arg("--color")
        .arg("never")
        .arg("--ignore-case")
        .arg(query.join("|"))
        .arg(&memory_dir)
        .arg("-X")
        .arg("printf")
        .arg(r"%s\n")
        .arg("{/}")
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let matches = stdout.lines().map(|s| s.to_string()).collect();
    Ok(SearchResult { matches })
}

#[rig::tool_macro(
    description = "Read the contents of a specific memory file",
    params(memory_name = "The name of the memory file")
)]
pub async fn inspect_memory(memory_name: String) -> Result<String, MemoryError> {
    let memory_dir = get_memory_dir()?;
    let memory_path = memory_dir.join(memory_name);

    if !memory_dir.exists() || !memory_path.exists() {
        return Ok("Memory file is empty or does not exist yet.".to_string());
    }

    let content = fs::read_to_string(memory_path)?;
    Ok(content)
}

#[rig::tool_macro(
    description = "Write or overwrite a memory file with new content",
    params(
        memory_name = "The name of the memory file that will be saved as a text file",
        content = "The text content to remember"
    )
)]
pub async fn write_memory(memory_name: String, content: String) -> Result<String, MemoryError> {
    let memory_dir = get_memory_dir()?;
    let memory_path = memory_dir.join(memory_name);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(false)
        .open(&memory_path)?;

    let formatted_content = if content.ends_with('\n') {
        content
    } else {
        format!("{}\n", content)
    };

    file.write_all(formatted_content.as_bytes())?;
    Ok("Successfully appended to memory.".to_string())
}
