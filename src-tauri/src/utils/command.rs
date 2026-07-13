use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

pub struct SecureCommand {
    program: String,
    args: Vec<String>,
    stdin: Option<String>,
    timeout_secs: Option<u64>,
}

impl SecureCommand {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            stdin: None,
            timeout_secs: None,
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        for arg in args {
            self.args.push(arg.to_string());
        }
        self
    }

    pub fn stdin(mut self, data: &str) -> Self {
        self.stdin = Some(data.to_string());
        self
    }

    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    pub fn build(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    pub async fn execute(&self) -> Result<(String, String)> {
        debug!("Executing: {} {}", self.program, self.args.join(" "));
        let mut cmd = self.build();

        let output = if let Some(secs) = self.timeout_secs {
            tokio::time::timeout(std::time::Duration::from_secs(secs), cmd.output())
                .await
                .context("Command timed out")??
        } else {
            cmd.output().await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            anyhow::bail!(
                "Command '{}' failed (exit: {}): {}",
                self.program,
                output.status,
                stderr.trim()
            );
        }

        Ok((stdout, stderr))
    }

    pub async fn execute_with_stdin(&self, data: &str) -> Result<(String, String)> {
        debug!("Executing with stdin: {} {}", self.program, self.args.join(" "));
        let mut cmd = self.build();

        let mut child = cmd.spawn().context("Failed to spawn command")?;

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(data.as_bytes()).await?;
        }

        let output = if let Some(secs) = self.timeout_secs {
            tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output())
                .await
                .context("Command timed out")??
        } else {
            child.wait_with_output().await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            anyhow::bail!(
                "Command '{}' failed (exit: {}): {}",
                self.program,
                output.status,
                stderr.trim()
            );
        }

        Ok((stdout, stderr))
    }

    pub async fn check_exists(program: &str) -> bool {
        Command::new("which")
            .arg(program)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub async fn require_binaries(binaries: &[&str]) -> Result<()> {
        let mut missing = Vec::new();
        for bin in binaries {
            if !Self::check_exists(bin).await {
                missing.push(*bin);
            }
        }
        if !missing.is_empty() {
            anyhow::bail!("Missing required binaries: {}. Install with: apt install {}", 
                missing.join(", "), missing.join(" "));
        }
        Ok(())
    }
}
