//! Bash tool — command execution with optional OS-native sandbox.
//!
//! When the `sandbox` feature is enabled and `sandboxed=true`, commands execute
//! inside an [`oa_sandbox`] isolation boundary (macOS Seatbelt / Linux
//! Landlock+Seccomp / Windows `AppContainer`). Otherwise falls back to direct
//! `sh -c` execution.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::server::{ContentItem, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BashParams {
    /// The bash command to execute.
    pub command: String,
    /// Execution timeout in seconds (default: 120).
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

const fn default_timeout() -> u64 {
    120
}

#[must_use]
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "bash".to_owned(),
        description: "Execute a bash command in the workspace directory.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)",
                    "default": 120
                }
            },
            "required": ["command"]
        }),
    }
}

/// Execute the bash tool.
///
/// When `sandboxed` is `true` and the `sandbox` feature is compiled in,
/// the command runs inside an OS-native sandbox via [`oa_sandbox`].
/// Otherwise falls back to direct `sh -c` execution.
pub fn execute(
    workspace: &Path,
    sandboxed: bool,
    arguments: serde_json::Value,
) -> Result<ToolCallResult> {
    let params: BashParams =
        serde_json::from_value(arguments).context("invalid bash parameters")?;

    if sandboxed {
        #[cfg(feature = "sandbox")]
        {
            return execute_sandboxed(workspace, &params);
        }

        #[cfg(not(feature = "sandbox"))]
        {
            tracing::warn!("sandbox feature not compiled in, falling back to direct execution");
        }
    }

    execute_direct(workspace, &params)
}

// ---------------------------------------------------------------------------
// Sandboxed execution (oa-sandbox)
// ---------------------------------------------------------------------------

/// Execute a command inside the OS-native sandbox.
#[cfg(feature = "sandbox")]
fn execute_sandboxed(workspace: &Path, params: &BashParams) -> Result<ToolCallResult> {
    use oa_sandbox::config::{
        BackendPreference, OutputFormat, ResourceLimits, SandboxConfig, SecurityLevel,
    };

    tracing::info!(command = %params.command, "executing in sandbox");

    let config = SandboxConfig {
        security_level: SecurityLevel::L1Sandbox,
        command: "sh".to_owned(),
        args: vec!["-c".to_owned(), params.command.clone()],
        workspace: workspace.to_path_buf(),
        mounts: vec![],
        resource_limits: ResourceLimits {
            timeout_secs: Some(params.timeout),
            ..ResourceLimits::default()
        },
        network_policy: None, // use L1 default (Restricted)
        env_vars: std::collections::HashMap::new(),
        format: OutputFormat::Json,
        backend: BackendPreference::Auto,
    };

    // Select best available backend (native → Docker fallback).
    let runner = oa_sandbox::select_runner(&config)
        .map_err(|e| anyhow::anyhow!("sandbox backend selection failed: {e}"))?;

    tracing::debug!(backend = runner.name(), "sandbox backend selected");

    let output = runner
        .run(&config)
        .map_err(|e| anyhow::anyhow!("sandbox execution failed: {e}"))?;

    // Convert SandboxOutput → ToolCallResult.
    let mut text = String::new();
    if !output.stdout.is_empty() {
        text.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("STDERR:\n");
        text.push_str(&output.stderr);
    }
    if let Some(ref error_msg) = output.error {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("SANDBOX ERROR:\n");
        text.push_str(error_msg);
    }

    // Append metadata footer.
    let _ = write!(
        text,
        "\n(exit code: {}, sandbox: {}, duration: {}ms)",
        output.exit_code, output.sandbox_backend, output.duration_ms
    );

    if text.trim().is_empty() {
        text = format!(
            "(exit code: {}, sandbox: {})",
            output.exit_code, output.sandbox_backend
        );
    }

    Ok(ToolCallResult {
        content: vec![ContentItem {
            content_type: "text".to_owned(),
            text,
        }],
        is_error: output.exit_code != 0,
    })
}

// ---------------------------------------------------------------------------
// Direct execution (no sandbox)
// ---------------------------------------------------------------------------

/// Direct execution without sandbox.
fn execute_direct(workspace: &Path, params: &BashParams) -> Result<ToolCallResult> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&params.command)
        .current_dir(workspace)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn: {}", params.command))?;

    let timeout = std::time::Duration::from_secs(params.timeout);
    let start = std::time::Instant::now();

    // Poll with short sleeps until process exits or timeout.
    let status = loop {
        match child.try_wait().context("failed to check process status")? {
            Some(status) => break status,
            None if start.elapsed() >= timeout => {
                // Kill the process on timeout.
                let _ = child.kill();
                let _ = child.wait();
                return Ok(ToolCallResult {
                    content: vec![ContentItem {
                        content_type: "text".to_owned(),
                        text: format!(
                            "Command timed out after {}s: {}",
                            params.timeout, params.command
                        ),
                    }],
                    is_error: true,
                });
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };

    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        std::io::Read::read_to_end(&mut out, &mut stdout_buf).ok();
    }
    if let Some(mut err) = child.stderr.take() {
        std::io::Read::read_to_end(&mut err, &mut stderr_buf).ok();
    }

    let stdout = String::from_utf8_lossy(&stdout_buf);
    let stderr = String::from_utf8_lossy(&stderr_buf);
    let exit_code = status.code().unwrap_or(-1);

    let mut text = String::new();
    if !stdout.is_empty() {
        text.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("STDERR:\n");
        text.push_str(&stderr);
    }
    if text.is_empty() {
        text = format!("(exit code: {exit_code})");
    } else {
        let _ = write!(text, "\n(exit code: {exit_code})");
    }

    Ok(ToolCallResult {
        content: vec![ContentItem {
            content_type: "text".to_owned(),
            text,
        }],
        is_error: exit_code != 0,
    })
}
