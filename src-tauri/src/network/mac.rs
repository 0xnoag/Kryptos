use anyhow::{Context, Result};
use rand::Rng;
use tokio::process::Command;
use tracing::{info, warn};

pub struct MacSpoofer {
    interval_secs: u64,
    exclude_interfaces: Vec<String>,
    running: bool,
}

impl MacSpoofer {
    pub fn new(interval_secs: u64, exclude_interfaces: Vec<String>) -> Self {
        Self {
            interval_secs,
            exclude_interfaces,
            running: false,
        }
    }

    pub fn generate_random_mac() -> String {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 6] = rng.gen();
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            bytes[0] & 0xFE | 0x02,
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5]
        )
    }

    pub async fn list_interfaces() -> Result<Vec<String>> {
        let output = Command::new("ip")
            .args(["-o", "link", "show"])
            .output()
            .await
            .context("Failed to list network interfaces")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut interfaces = Vec::new();

        for line in stdout.lines() {
            if let Some(idx) = line.find(':') {
                if let Some(end) = line[idx + 1..].find(':') {
                    let name = line[idx + 2..idx + 1 + end].trim().to_string();
                    if name != "lo" {
                        interfaces.push(name);
                    }
                }
            }
        }

        Ok(interfaces)
    }

    pub async fn spoof_interface(iface: &str, mac: &str) -> Result<()> {
        let bring_down = Command::new("ip")
            .args(["link", "set", "dev", iface, "down"])
            .output()
            .await
            .context("Failed to bring interface down")?;

        if !bring_down.status.success() {
            warn!("Failed to bring {} down: {}", iface,
                String::from_utf8_lossy(&bring_down.stderr));
        }

        let set_mac = Command::new("ip")
            .args(["link", "set", "dev", iface, "address", mac])
            .output()
            .await
            .context("Failed to set MAC address")?;

        if !set_mac.status.success() {
            warn!("Failed to set MAC on {}: {}", iface,
                String::from_utf8_lossy(&set_mac.stderr));
            let _ = Command::new("ip")
                .args(["link", "set", "dev", iface, "up"])
                .output()
                .await;
            anyhow::bail!("Failed to spoof MAC on {}: {}", iface,
                String::from_utf8_lossy(&set_mac.stderr));
        }

        let bring_up = Command::new("ip")
            .args(["link", "set", "dev", iface, "up"])
            .output()
            .await
            .context("Failed to bring interface up")?;

        if !bring_up.status.success() {
            warn!("Failed to bring {} up after MAC change", iface);
        }

        info!("MAC spoofed on {} to {}", iface, mac);
        Ok(())
    }

    pub async fn randomize_all(&self) -> Result<()> {
        let interfaces = Self::list_interfaces().await?;
        for iface in &interfaces {
            if self.exclude_interfaces.contains(iface) {
                continue;
            }
            let mac = Self::generate_random_mac();
            if let Err(e) = Self::spoof_interface(iface, &mac).await {
                warn!("Failed to spoof {}: {e}", iface);
            }
        }
        Ok(())
    }

    pub async fn restore_interface_original(iface: &str) -> Result<()> {
        match Command::new("ethtool").args(["-P", iface]).output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(mac) = stdout.split_whitespace().last() {
                    if mac.len() == 17 && mac.chars().filter(|&c| c == ':').count() == 5 {
                        Self::spoof_interface(iface, mac).await?;
                    } else {
                        warn!("Could not parse permanent MAC from ethtool output for {}: '{}'", iface, stdout.trim());
                    }
                }
            }
            Ok(output) => {
                warn!("ethtool -P {} failed (exit: {}): {}", iface, output.status, String::from_utf8_lossy(&output.stderr).trim());
            }
            Err(e) => {
                warn!("ethtool not available, cannot restore MAC for {}: {e}", iface);
            }
        }
        Ok(())
    }
}
