use super::nftables::{KillSwitchLevel, NftablesManager};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

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
    pub memory_flushed: bool,
}

pub struct PanicEngine {
    nftables: Arc<RwLock<NftablesManager>>,
    current_level: PanicLevel,
}

impl PanicEngine {
    pub fn new(nftables: Arc<RwLock<NftablesManager>>) -> Self {
        Self {
            nftables,
            current_level: PanicLevel::Off,
        }
    }

    pub fn current_level(&self) -> PanicLevel {
        self.current_level
    }

    pub async fn activate(&mut self, level: PanicLevel) -> Result<PanicStatus> {
        info!("Panic engine activating at {:?} level", level);

        match level {
            PanicLevel::Soft => {
                let mut nft = self.nftables.write().await;
                nft.install_kill_switch(KillSwitchLevel::Soft).await?;
            }
            PanicLevel::Hard => {
                let mut nft = self.nftables.write().await;
                nft.install_kill_switch(KillSwitchLevel::Hard).await?;
            }
            PanicLevel::Nuclear => {
                let mut nft = self.nftables.write().await;
                nft.install_kill_switch(KillSwitchLevel::Nuclear).await?;
                self.drop_all_interfaces().await?;
                self.flush_dns_cache().await?;
                self.flush_memory().await?;
            }
            PanicLevel::Off => {
                warn!("activate called with Off level, no action taken");
            }
        }

        self.current_level = level;
        Ok(self.status().await)
    }

    pub async fn deactivate(&mut self) -> Result<PanicStatus> {
        info!("Panic engine deactivating");

        let mut nft = self.nftables.write().await;
        nft.remove_kill_switch().await?;

        if self.current_level == PanicLevel::Nuclear {
            self.bring_loopback_up().await?;
        }

        self.current_level = PanicLevel::Off;
        Ok(self.status().await)
    }

    pub async fn status(&self) -> PanicStatus {
        let nft = self.nftables.read().await;
        PanicStatus {
            level: self.current_level,
            kill_switch_active: nft.is_active(),
            interfaces_down: false,
            dns_flushed: false,
            memory_flushed: false,
        }
    }

    async fn drop_all_interfaces(&self) -> Result<()> {
        info!("Dropping all network interfaces (except loopback)");

        let entries = tokio::fs::read_dir("/sys/class/net").await?;
        use tokio_stream::StreamExt;
        use tokio_stream::wrappers::ReadDirStream;

        let mut stream = ReadDirStream::new(entries);
        while let Some(entry) = stream.next().await {
            if let Ok(entry) = entry {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str == "lo" {
                    continue;
                }
                warn!("Taking interface {} down", name_str);
                let _ = Command::new("ip")
                    .args(["link", "set", "dev", &name_str, "down"])
                    .output()
                    .await;
            }
        }
        Ok(())
    }

    async fn bring_loopback_up(&self) -> Result<()> {
        info!("Bringing loopback interface back up");
        let _ = Command::new("ip")
            .args(["link", "set", "dev", "lo", "up"])
            .output()
            .await;
        Ok(())
    }

    async fn flush_dns_cache(&self) -> Result<()> {
        info!("Flushing DNS cache");
        let commands = [
            ("systemd-resolve", &["--flush-caches" as &str]),
            ("resolvectl", &["flush-caches"]),
            ("systemctl", &["restart", "systemd-resolved"]),
        ];
        for (cmd, args) in &commands {
            let _ = Command::new(cmd).args(args).output().await;
        }
        Ok(())
    }

    async fn flush_memory(&self) -> Result<()> {
        info!("Purging sensitive memory");
        let _ = Command::new("sysctl")
            .args(["-w", "vm.drop_caches=3"])
            .output()
            .await;
        let _ = Command::new("sysctl")
            .args(["-w", "vm.page-cluster=0"])
            .output()
            .await;
        let _ = Command::new("swapoff")
            .arg("-a")
            .output()
            .await;
        Ok(())
    }
}
