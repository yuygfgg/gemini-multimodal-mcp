use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::AppError;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
pub const VIDEO_TIMEOUT: Duration = Duration::from_secs(600);

pub async fn run(prompt: &str, model: &str, deadline: Duration) -> Result<String, AppError> {
    let output = run_with_timeout(command(prompt, model), deadline).await?;
    output_text(output)
}

fn command(prompt: &str, model: &str) -> Command {
    let mut cmd = Command::new("agy");
    cmd.arg("--print").arg(prompt).arg("--model").arg(model);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    cmd
}

async fn run_with_timeout(mut cmd: Command, deadline: Duration) -> Result<Output, AppError> {
    let result = timeout(deadline, async {
        let child = cmd.spawn()?;
        child.wait_with_output().await
    })
    .await;

    let output = match result {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(AppError::Io(e)),
        Err(_) => return Err(AppError::Timeout(deadline)),
    };
    Ok(output)
}

fn output_text(output: Output) -> Result<String, AppError> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        match output.status.code() {
            Some(code) => {
                return Err(AppError::AgyExit {
                    status: code,
                    stderr,
                });
            }
            None => return Err(AppError::AgySignaled),
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(AppError::EmptyOutput)
    } else {
        Ok(stderr)
    }
}
