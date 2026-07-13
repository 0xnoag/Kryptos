use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RoutingTable {
    Main,
    Tor(u16),
    AmneziaWG(u16),
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub table: RoutingTable,
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub priority: u32,
}

pub struct RouteManager;

impl RouteManager {
    pub async fn add_default_via_tor(tor_socks_ip: &str) -> Result<()> {
        let _ = Command::new("ip")
            .args(["route", "add", "1.0.0.0/8", "via", "127.0.0.1"])
            .output()
            .await;

        let output = Command::new("iptables")
            .args([
                "-t", "nat",
                "-A", "OUTPUT",
                "-p", "tcp",
                "--syn",
                "-m", "owner", "!",
                "--uid-owner", "debian-tor",
                "-j", "REDIRECT",
                "--to-ports", "9040",
            ])
            .output()
            .await
            .context("Failed to add iptables TPROXY rule for Tor")?;

        if !output.status.success() {
            warn!("iptables Tor redirect rule failed: {}",
                String::from_utf8_lossy(&output.stderr));
        }

        info!("Tor transparent proxy routing configured");
        Ok(())
    }

    pub async fn add_route_via_interface(destination: &str, interface: &str) -> Result<()> {
        let output = Command::new("ip")
            .args(["route", "add", destination, "dev", interface])
            .output()
            .await
            .with_context(|| format!("Failed to add route to {} via {}", destination, interface))?;

        if !output.status.success() {
            bail!("Route add failed: {}",
                String::from_utf8_lossy(&output.stderr));
        }

        info!("Route added: {} via {}", destination, interface);
        Ok(())
    }

    pub async fn add_policy_routing(
        mark: u32,
        table_id: u16,
        table_name: &str,
    ) -> Result<()> {
        let _ = Command::new("ip")
            .args(["rule", "add", &format!("fwmark {}", mark), "table", &table_id.to_string()])
            .output()
            .await;

        let output = Command::new("ip")
            .args(["route", "add", "default", "dev", table_name, "table", &table_id.to_string()])
            .output()
            .await;

        if let Ok(out) = output {
            if !out.status.success() {
                warn!("Policy routing setup for {} may be incomplete", table_name);
            }
        }

        info!("Policy routing configured for {} (table {})", table_name, table_id);
        Ok(())
    }

    pub async fn flush_routes() -> Result<()> {
        let tables = ["main", "local", "100", "200"];

        for table in &tables {
            let output = Command::new("ip")
                .args(["route", "flush", "table", table])
                .output()
                .await?;

            if !output.status.success() {
                warn!("Failed to flush table {}: {}", table,
                    String::from_utf8_lossy(&output.stderr));
            }
        }

        let output = Command::new("ip")
            .args(["rule", "flush"])
            .output()
            .await?;

        if output.status.success() {
            info!("All routing rules flushed");
        } else {
            warn!("Failed to flush routing rules");
        }

        Ok(())
    }

    pub async fn sysctl_set(net_key: &str, value: &str) -> Result<()> {
        let output = Command::new("sysctl")
            .args(["-w", &format!("net.{}={}", net_key, value)])
            .output()
            .await
            .with_context(|| format!("Failed to set sysctl net.{}={}", net_key, value))?;

        if !output.status.success() {
            bail!("sysctl failed: {}",
                String::from_utf8_lossy(&output.stderr));
        }

        info!("sysctl net.{} = {}", net_key, value);
        Ok(())
    }

    pub async fn configure_ip_forward(enable: bool) -> Result<()> {
        let val = if enable { "1" } else { "0" };
        Self::sysctl_set("ipv4.ip_forward", val).await?;
        Self::sysctl_set("ipv6.conf.all.forwarding", val).await?;
        Ok(())
    }

    pub async fn enable_udp_tunneling() -> Result<()> {
        Self::sysctl_set("ipv4.conf.all.src_valid_mark", "1").await?;
        Self::sysctl_set("ipv6.conf.all.disable_ipv6", "0").await?;
        Ok(())
    }
}
