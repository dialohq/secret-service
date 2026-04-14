use anyhow::{Result, bail};
use std::process::Command;
use tracing::{Level, info, span};

pub fn run_command(description: &str, shell_cmd: &str) -> Result<String> {
    let span = span!(Level::INFO, "cmd", command = description);
    let _enter = span.enter();

    info!(?description, "Executing command");

    let output = Command::new("bash").arg("-c").arg(shell_cmd).output()?;

    if !output.stderr.is_empty() {
        let out = String::from_utf8_lossy(&output.stderr);
        info!(?out, "Command stderr is not empty");
    }

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // nitpick: can have a different error message for when
        // command was terminated by a signal for some reason. Then
        // exit code is `None`.
        bail!(
            "Command failed with exit code: {:?}\nSTDOUT: {}\nSTDERR: {}",
            output.status.code(),
            stdout,
            stderr,
        );
    }

    let result = String::from_utf8(output.stdout)?;
    info!(len = result.len(), "Got command output");
    Ok(result)
}
