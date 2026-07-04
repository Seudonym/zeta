use color_eyre::eyre::Result;
use rig::tool::ToolDyn;
use std::{fs, process::Stdio};
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command failed: {0}")]
    CommandFailed(String),
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

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push_str("\n... [output truncated]");
        out
    }
}

#[rig::tool_macro(
    description = "Search for strings or regex patterns across files in a directory",
    params(
        pattern = "The regex or literal pattern to search for",
        path = "Optional directory to search in (defaults to current directory)",
        filetype = "Optional file type filter (e.g., 'py', 'rs', 'js')",
    )
)]
pub async fn grep(
    pattern: String,
    path: Option<String>,
    filetype: Option<String>,
) -> Result<String, FsError> {
    let path = path.filter(|s| !s.is_empty());
    let filetype = filetype.filter(|s| !s.is_empty());

    let mut cmd = Command::new("rg");
    cmd.arg("--line-number")
        .arg("--color=never")
        .arg("--column")
        .arg("--max-columns=500")
        .arg("--max-count=200"); // cap matches per file so output stays sane

    if let Some(t) = filetype {
        cmd.arg("-g").arg(format!("*.{}", t)); // filtype fuckery with ripgrep
    }

    cmd.arg("--").arg(&pattern);
    cmd.arg(path.unwrap_or_else(|| ".".to_string()));

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    // rg exit codes: 0 = match found, 1 = no match, 2 = error
    match output.status.code() {
        Some(0) => Ok(truncate(&String::from_utf8_lossy(&output.stdout), 20_000)),
        Some(1) => Ok("No matches found.".to_string()),
        _ => Err(FsError::CommandFailed(format!(
            "ripgrep failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

#[rig::tool_macro(
    description = "Find files or directories matching a name pattern",
    params(
        pattern = "The filename or partial name to search for",
        path = "Optional directory to search in (defaults to current directory)",
        filetype = "Optional file extension filter (e.g., 'py', 'rs', 'js')"
    )
)]
pub async fn find_files(
    pattern: String,
    path: Option<String>,
    filetype: Option<String>,
) -> Result<String, FsError> {
    let path = path.filter(|s| !s.is_empty());
    let filetype = filetype.filter(|s| !s.is_empty());

    let mut cmd = Command::new("fd");
    cmd.arg("--color=never");
    if let Some(ext) = filetype {
        cmd.arg("-e").arg(ext);
    }
    cmd.arg("--").arg(&pattern);
    cmd.arg(path.unwrap_or_else(|| ".".to_string()));

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    // fd exit codes: 0 = success (includes no matchse), otherwise = error
    if output.status.success() {
        let results = String::from_utf8_lossy(&output.stdout);
        let trimmed = results.trim();
        if trimmed.is_empty() {
            Ok("No matching files found.".to_string())
        } else {
            Ok(truncate(trimmed, 20_000))
        }
    } else {
        Err(FsError::CommandFailed(format!(
            "fd failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    return vec![
        Box::new(ListFiles),
        Box::new(ReadFile),
        Box::new(Grep),
        Box::new(FindFiles),
    ];
}
