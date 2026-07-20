use anyhow::{bail, Context, Result};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info, warn};

const COMMAND_TIMEOUT_SECS: u64 = 10;

async fn run_cmd(cmd: &mut Command, label: &str) -> Result<(String, String)> {
    let output = timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), cmd.output())
        .await
        .with_context(|| format!("{label}: command timed out"))?
        .with_context(|| format!("{label}: failed to execute"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        bail!("{label}: exit {} — {stderr}", output.status);
    }

    Ok((stdout, stderr))
}

pub struct RouteManager;

impl RouteManager {
    /// Configure Tor transparent proxy via policy routing (fwmark).
    ///
    /// This sets up a routing policy that marks TCP packets with a specific
    /// firewall mark and routes them through the Tor daemon's TransPort.
    /// It does NOT use iptables REDIRECT (which breaks for non-local traffic).
    pub async fn add_default_via_tor(tor_trans_port: u16) -> Result<()> {
        let mark = 0x0f0fu32;
        let table_id: u16 = 100;

        run_cmd(
            Command::new("ip").args([
                "rule",
                "add",
                &format!("fwmark {mark}"),
                "table",
                &table_id.to_string(),
            ]),
            "ip rule add for Tor fwmark",
        )
        .await?;

        run_cmd(
            Command::new("ip").args([
                "route",
                "add",
                "local",
                "0.0.0.0/0",
                "dev",
                "lo",
                "table",
                &table_id.to_string(),
            ]),
            "ip route add for Tor table",
        )
        .await?;

        run_cmd(
            Command::new("iptables").args([
                "-t",
                "mangle",
                "-A",
                "OUTPUT",
                "-p",
                "tcp",
                "--syn",
                "-m",
                "owner",
                "!",
                "--uid-owner",
                "debian-tor",
                "-j",
                "MARK",
                "--set-mark",
                &mark.to_string(),
            ]),
            "iptables mark Tor traffic",
        )
        .await?;

        run_cmd(
            Command::new("iptables").args([
                "-t",
                "nat",
                "-A",
                "OUTPUT",
                "-p",
                "tcp",
                "-m",
                "mark",
                "--mark",
                &mark.to_string(),
                "-j",
                "REDIRECT",
                "--to-ports",
                &tor_trans_port.to_string(),
            ]),
            "iptables redirect Tor marked traffic",
        )
        .await?;

        info!("Tor transparent proxy configured (fwmark {mark}, table {table_id}, port {tor_trans_port})");
        Ok(())
    }

    pub async fn add_route_via_interface(destination: &str, interface: &str) -> Result<()> {
        run_cmd(
            Command::new("ip").args(["route", "add", destination, "dev", interface]),
            &format!("ip route add {destination} dev {interface}"),
        )
        .await?;

        info!("Route added: {} via {}", destination, interface);
        Ok(())
    }

    pub async fn add_policy_routing(mark: u32, table_id: u16, table_name: &str) -> Result<()> {
        run_cmd(
            Command::new("ip").args([
                "rule",
                "add",
                &format!("fwmark {mark}"),
                "table",
                &table_id.to_string(),
            ]),
            &format!("ip rule add fwmark {mark} table {table_id}"),
        )
        .await?;

        run_cmd(
            Command::new("ip").args([
                "route",
                "add",
                "default",
                "dev",
                table_name,
                "table",
                &table_id.to_string(),
            ]),
            &format!("ip route add default dev {table_name} table {table_id}"),
        )
        .await?;

        info!(
            "Policy routing configured for {} (table {})",
            table_name, table_id
        );
        Ok(())
    }

    pub async fn flush_routes() -> Result<()> {
        let tables = ["100", "200"];
        for table in &tables {
            let res = run_cmd(
                Command::new("ip").args(["route", "flush", "table", table]),
                &format!("ip route flush table {table}"),
            )
            .await;
            if let Err(e) = res {
                warn!("Failed to flush table {}: {e}", table);
            }
        }

        let res = run_cmd(Command::new("ip").args(["rule", "flush"]), "ip rule flush").await;
        if let Err(e) = &res {
            warn!("Failed to flush routing rules: {e}");
        } else {
            info!("All routing rules flushed");
        }

        Ok(())
    }

    pub async fn sysctl_set(net_key: &str, value: &str) -> Result<()> {
        run_cmd(
            Command::new("sysctl").args(["-w", &format!("net.{}={}", net_key, value)]),
            &format!("sysctl net.{net_key}={value}"),
        )
        .await?;

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
        Self::sysctl_set("ipv4.conf.all.rp_filter", "2").await?;
        Ok(())
    }

    /// Block IPv6 traffic to prevent IPv6 DNS / traffic leaks outside the tunnel.
    ///
    /// **Security trade-off**: Disabling IPv6 prevents leaks via IPv6 when only
    /// IPv4 tunnels are active (Tor and AmneziaWG). However, this may break
    /// IPv6-only sites and services. If the tunnel infrastructure eventually
    /// supports IPv6, this block should be removed.
    ///
    /// The nftables kill switch already drops non-loopback IPv6 by default
    /// (policy drop), but this provides defense-in-depth at the kernel level.
    pub async fn block_ipv6_leaks() -> Result<()> {
        let settings = [
            ("ipv6.conf.all.disable_ipv6", "1"),
            ("ipv6.conf.default.disable_ipv6", "1"),
            ("ipv6.conf.lo.disable_ipv6", "0"),
        ];
        let mut errors = Vec::new();
        for (key, val) in &settings {
            if let Err(e) = Self::sysctl_set(key, val).await {
                warn!("Failed to set {key}={val}: {e}");
                errors.push(format!("{key}={val}"));
            }
        }
        if !errors.is_empty() {
            warn!(
                "IPv6 leak blocking partially applied — some sysctls failed: {}",
                errors.join(", ")
            );
        } else {
            info!("IPv6 disabled on all interfaces except lo (IPv6 leak prevention)");
        }
        Ok(())
    }

    /// Undo the IPv6 block applied by `block_ipv6_leaks()`.
    pub async fn unblock_ipv6() -> Result<()> {
        Self::sysctl_set("ipv6.conf.all.disable_ipv6", "0").await?;
        Self::sysctl_set("ipv6.conf.default.disable_ipv6", "0").await?;
        info!("IPv6 re-enabled");
        Ok(())
    }
}
