use color_eyre::eyre::Result;
use rig::tool::ToolDyn;
use serde::Serialize;
use thiserror::Error;
use tokio::process::Command;

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    return vec![Box::new(Bash)];
}

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("Failed to execute command: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command exited with status {status}")]
    Exit {
        status: std::process::ExitStatus,
        stderr: String,
    },
}

#[derive(Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[rig::tool_macro(
    description = "Run a bash command in the current directory",
    params(command = "The bash command to run")
)]
pub async fn bash(command: String) -> Result<BashOutput, ShellError> {
    let output = Command::new("bash").arg("-c").arg(command).output().await?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        return Err(ShellError::Exit {
            status: output.status,
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    Ok(BashOutput {
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
    })
}
