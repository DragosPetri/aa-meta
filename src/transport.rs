use std::process::Command;
use std::time::Duration;

use crate::error::AttachMetaError;
use crate::protocol::manifest::CommandMapping;

pub fn invoke(
    mapping: &CommandMapping,
    extra_args: &[String],
) -> std::result::Result<serde_json::Value, AttachMetaError> {
    let argv = &mapping.argv;
    if argv.is_empty() {
        return Err(AttachMetaError::ManifestError(
            "command argv is empty".to_string(),
        ));
    }

    let binary = &argv[0];
    let mut cmd = Command::new(binary);
    cmd.args(&argv[1..]);
    cmd.args(extra_args);
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| AttachMetaError::TransportError(format!("failed to spawn '{binary}': {e}")))?;

    let output = if let Some(timeout_ms) = mapping.timeout_ms {
        wait_with_timeout(child, Duration::from_millis(timeout_ms), binary)?
    } else {
        child.wait_with_output().map_err(|e| {
            AttachMetaError::TransportError(format!("failed to wait for '{binary}': {e}"))
        })?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        return Err(AttachMetaError::TransportError(format!(
            "'{binary}' exited with code {code}: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(AttachMetaError::TransportError(format!(
            "'{binary}' produced empty stdout"
        )));
    }

    serde_json::from_str(stdout.trim()).map_err(|e| {
        AttachMetaError::TransportError(format!("'{binary}' produced invalid JSON: {e}"))
    })
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
    binary: &str,
) -> std::result::Result<std::process::Output, AttachMetaError> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = {
                    let mut buf = Vec::new();
                    if let Some(mut out) = child.stdout.take() {
                        std::io::Read::read_to_end(&mut out, &mut buf).ok();
                    }
                    buf
                };
                let stderr = {
                    let mut buf = Vec::new();
                    if let Some(mut err) = child.stderr.take() {
                        std::io::Read::read_to_end(&mut err, &mut buf).ok();
                    }
                    buf
                };
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AttachMetaError::TransportError(format!(
                        "'{binary}' timed out after {}ms",
                        timeout.as_millis()
                    )));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(AttachMetaError::TransportError(format!(
                    "failed to wait for '{binary}': {e}"
                )));
            }
        }
    }
}
