use super::nftables::{KillSwitchLevel, NftablesManager};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

const COMMAND_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PanicLevel {
    Off = 0,
    Soft = 1,
    Hard = 2,
    Nuclear = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanicStatus {
    pub level: PanicLevel,
    pub kill_switch_active: bool,
    pub interfaces_down: bool,
    pub dns_flushed: bool,
    pub kernel_caches_purged: bool,
}

async fn run_command(cmd: &mut Command) -> Result<(String, String)> {
    let output = timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), cmd.output())
        .await
        .context("Command timed out")?
        .context("Failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        bail!("exit code {}: {}", output.status, stderr.trim());
    }

    Ok((stdout, stderr))
}

async fn run_command_logged(cmd: &mut Command, label: &str) -> Result<(String, String)> {
    match run_command(cmd).await {
        Ok(result) => Ok(result),
        Err(e) => {
            error!("{} failed: {e}", label);
            Err(e).context(label.to_string())
        }
    }
}

pub struct PanicEngine {
    nftables: Arc<RwLock<NftablesManager>>,
    current_level: PanicLevel,
    interfaces_down: bool,
    dns_flushed: bool,
    kernel_caches_purged: bool,
    dropped_interfaces: Vec<String>,
    excluded_interfaces: Vec<String>,
}

impl PanicEngine {
    pub fn new(nftables: Arc<RwLock<NftablesManager>>) -> Self {
        Self {
            nftables,
            current_level: PanicLevel::Off,
            interfaces_down: false,
            dns_flushed: false,
            kernel_caches_purged: false,
            dropped_interfaces: Vec::new(),
            excluded_interfaces: vec!["lo".into()],
        }
    }

    pub fn with_excluded_interfaces(mut self, excluded: Vec<String>) -> Self {
        self.excluded_interfaces = excluded;
        self
    }

    pub fn current_level(&self) -> PanicLevel {
        self.current_level
    }

    pub async fn activate(&mut self, level: PanicLevel) -> Result<PanicStatus> {
        info!("Panic engine activating at {:?} level", level);

        match level {
            PanicLevel::Off => {
                warn!("activate called with Off level, no action taken");
                self.current_level = PanicLevel::Off;
            }
            PanicLevel::Soft => {
                {
                    let mut nft = self.nftables.write().await;
                    nft.install_kill_switch(KillSwitchLevel::Soft).await?;
                }
                self.current_level = PanicLevel::Soft;
                self.interfaces_down = false;
                self.dns_flushed = false;
                self.kernel_caches_purged = false;
            }
            PanicLevel::Hard => {
                {
                    let mut nft = self.nftables.write().await;
                    nft.install_kill_switch(KillSwitchLevel::Hard).await?;
                }
                self.current_level = PanicLevel::Hard;
                self.interfaces_down = false;
                self.dns_flushed = false;
                self.kernel_caches_purged = false;
            }
            PanicLevel::Nuclear => {
                {
                    let mut nft = self.nftables.write().await;
                    nft.install_kill_switch(KillSwitchLevel::Nuclear).await?;
                }
                self.current_level = PanicLevel::Nuclear;

                match self.drop_all_interfaces().await {
                    Ok(ifaces) => {
                        self.dropped_interfaces = ifaces;
                        self.interfaces_down = true;
                    }
                    Err(e) => {
                        error!("Nuclear partial failure — dropped interfaces: {e}");
                        self.interfaces_down = false;
                    }
                }

                match self.flush_dns_cache().await {
                    Ok(()) => self.dns_flushed = true,
                    Err(e) => {
                        error!("Nuclear partial failure — DNS flush: {e}");
                        self.dns_flushed = false;
                    }
                }

                match self.purge_kernel_caches().await {
                    Ok(()) => self.kernel_caches_purged = true,
                    Err(e) => {
                        error!("Nuclear partial failure — cache purge: {e}");
                        self.kernel_caches_purged = false;
                    }
                }
            }
        }

        Ok(self.status().await)
    }

    pub async fn deactivate(&mut self) -> Result<PanicStatus> {
        info!("Panic engine deactivating");

        {
            let mut nft = self.nftables.write().await;
            nft.remove_kill_switch().await?;
        }

        if self.interfaces_down && !self.dropped_interfaces.is_empty() {
            let ifaces = self.dropped_interfaces.clone();
            for iface in &ifaces {
                let mut cmd = Command::new("ip");
                cmd.args(["link", "set", "dev", iface, "up"]);
                if let Err(e) =
                    run_command_logged(&mut cmd, &format!("restore interface {iface}")).await
                {
                    error!("Failed to restore interface {iface}: {e}");
                } else {
                    info!("Restored interface {iface}");
                }
            }
            self.dropped_interfaces.clear();
            self.interfaces_down = false;
        }

        self.current_level = PanicLevel::Off;
        self.dns_flushed = false;
        self.kernel_caches_purged = false;

        Ok(self.status().await)
    }

    pub async fn status(&self) -> PanicStatus {
        let nft = self.nftables.read().await;
        PanicStatus {
            level: self.current_level,
            kill_switch_active: nft.is_active(),
            interfaces_down: self.interfaces_down,
            dns_flushed: self.dns_flushed,
            kernel_caches_purged: self.kernel_caches_purged,
        }
    }

    async fn drop_all_interfaces(&self) -> Result<Vec<String>> {
        info!(
            "Dropping all network interfaces (except excluded: {:?})",
            self.excluded_interfaces
        );
        let mut dropped = Vec::new();

        let entries = tokio::fs::read_dir("/sys/class/net")
            .await
            .context("Failed to read /sys/class/net")?;

        use tokio_stream::wrappers::ReadDirStream;
        use tokio_stream::StreamExt;
        let mut stream = ReadDirStream::new(entries);

        while let Some(entry) = stream.next().await {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to read directory entry: {e}");
                    continue;
                }
            };

            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();

            if self
                .excluded_interfaces
                .iter()
                .any(|ex| name_str == *ex || name_str.starts_with(ex))
            {
                info!("Skipping excluded interface: {name_str}");
                continue;
            }

            warn!("Taking interface {name_str} down");
            let mut cmd = Command::new("ip");
            cmd.args(["link", "set", "dev", &name_str, "down"]);

            match run_command_logged(&mut cmd, &format!("ip link set {name_str} down")).await {
                Ok(_) => {
                    dropped.push(name_str);
                }
                Err(e) => {
                    error!("Failed to bring {name_str} down: {e}");
                }
            }
        }

        Ok(dropped)
    }

    async fn flush_dns_cache(&self) -> Result<()> {
        info!("Flushing DNS cache");
        let commands: [(&str, &[&str]); 3] = [
            ("systemd-resolve", &["--flush-caches"]),
            ("resolvectl", &["flush-caches"]),
            ("systemctl", &["restart", "systemd-resolved"]),
        ];

        for (cmd_name, args) in &commands {
            let mut cmd = Command::new(cmd_name);
            cmd.args(args);
            match run_command_logged(&mut cmd, &format!("{cmd_name} flush")).await {
                Ok(_) => info!("DNS cache flushed via {cmd_name}"),
                Err(e) => warn!("DNS flush method {cmd_name} failed: {e} (this is expected if the service is not installed)"),
            }
        }

        Ok(())
    }

    async fn purge_kernel_caches(&self) -> Result<()> {
        info!("Purging kernel page/dentry/inode caches");

        let mut cmd = Command::new("sysctl");
        cmd.args(["-w", "vm.drop_caches=3"]);
        match run_command_logged(&mut cmd, "sysctl vm.drop_caches").await {
            Ok(_) => info!("Kernel caches dropped"),
            Err(e) => warn!("Failed to drop kernel caches: {e}"),
        }

        let mut cmd = Command::new("sysctl");
        cmd.args(["-w", "vm.page-cluster=0"]);
        match run_command_logged(&mut cmd, "sysctl vm.page-cluster").await {
            Ok(_) => info!("Kernel page-cluster set to 0"),
            Err(e) => warn!("Failed to set page-cluster: {e}"),
        }

        Ok(())
    }
}
