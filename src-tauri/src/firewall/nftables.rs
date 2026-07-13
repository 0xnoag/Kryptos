use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

const NFT_BINARY: &str = "/usr/sbin/nft";
const TABLE_NAME: &str = "endpoint_privacy";
const COMMAND_TIMEOUT_SECS: u64 = 15;

const NFT_FLUSH: &str = "flush ruleset";

const NFT_KILL_SWITCH: &str = r#"
table inet endpoint_privacy {
    chain privacy_input {
        type filter hook input priority 0; policy drop;
        iif "lo" accept
        ct state established,related accept
        iifname { "tun+", "wg0", "wg+", "obfs+" } accept
        ip protocol icmp accept
        ip6 protocol icmpv6 accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_output {
        type filter hook output priority 0; policy drop;
        oif "lo" accept
        ct state established,related accept
        oifname { "tun+", "wg0", "wg+", "obfs+" } accept
        udp dport { 53, 853 } accept
        tcp dport { 53, 853 } accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_forward {
        type filter hook forward priority 0; policy drop;
        iifname { "tun+", "wg0", "wg+", "obfs+" } oifname != "lo" accept
        reject with icmpx type admin-prohibited
    }
}
"#;

const NFT_KILL_SWITCH_SOFT: &str = r#"
table inet endpoint_privacy {
    chain privacy_input {
        type filter hook input priority 0; policy drop;
        iif "lo" accept
        ct state established,related accept
        iifname { "tun+", "wg0", "wg+", "obfs+" } accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_output {
        type filter hook output priority 0; policy drop;
        oif "lo" accept
        ct state established,related accept
        oifname { "tun+", "wg0", "wg+", "obfs+" } accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_forward {
        type filter hook forward priority 0; policy drop;
        reject with icmpx type admin-prohibited
    }
}
"#;

const NFT_NUCLEAR: &str = r#"
table inet endpoint_privacy {
    chain privacy_input {
        type filter hook input priority 0; policy drop;
        iif "lo" accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_output {
        type filter hook output priority 0; policy drop;
        oif "lo" accept
        reject with icmpx type admin-prohibited
    }
    chain privacy_forward {
        type filter hook forward priority 0; policy drop;
        reject with icmpx type admin-prohibited
    }
}
"#;

pub enum KillSwitchLevel {
    Soft,
    Hard,
    Nuclear,
}

pub struct NftablesManager {
    binary_path: String,
    active: bool,
    allowed_interfaces: HashSet<String>,
}

impl NftablesManager {
    pub fn new() -> Self {
        Self {
            binary_path: NFT_BINARY.to_string(),
            active: false,
            allowed_interfaces: HashSet::new(),
        }
    }

    pub fn with_binary(mut self, path: &str) -> Self {
        self.binary_path = path.to_string();
        self
    }

    pub fn add_allowed_interface(&mut self, iface: &str) {
        self.allowed_interfaces.insert(iface.to_string());
    }

    pub fn remove_allowed_interface(&mut self, iface: &str) {
        self.allowed_interfaces.remove(iface);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    async fn nft_execute(&self, rules: &str) -> Result<String> {
        let nft_path = &self.binary_path;
        let mut cmd = Command::new(nft_path);
        cmd.args(["-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn nft process")?;

        let stdin = child.stdin.take().context("Failed to open nft stdin")?;
        let rules_owned = rules.to_string();

        let write_result = {
            let mut stdin = stdin;
            timeout(Duration::from_secs(5), stdin.write_all(rules_owned.as_bytes()))
                .await
                .context("Timeout writing nft rules to stdin")?
                .context("Failed to write nft rules to stdin")
        };

        if let Err(e) = write_result {
            error!("Failed to write nft rules: {e}");
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(e);
        }

        drop(stdin);

        let output = timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), child.wait_with_output())
            .await
            .context("nft command timed out")?
            .context("nft process failed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("nft failed (exit: {}): {}", output.status, stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("nft executed successfully: {}", stdout.trim());
        Ok(stdout)
    }

    async fn nft_is_available(&self) -> bool {
        let path = Path::new(&self.binary_path);
        if !path.exists() {
            warn!("nft binary not found at {}", self.binary_path);
            return false;
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let status = timeout(Duration::from_secs(5), cmd.status())
            .await;

        match status {
            Ok(Ok(s)) if s.success() => true,
            Ok(Ok(s)) => {
                warn!("nft --version returned non-zero: {s}");
                false
            }
            Ok(Err(e)) => {
                warn!("Failed to check nft version: {e}");
                false
            }
            Err(_) => {
                warn!("nft --version timed out");
                false
            }
        }
    }

    pub async fn flush_ruleset(&self) -> Result<()> {
        self.nft_execute(NFT_FLUSH).await?;
        info!("nftables ruleset flushed");
        Ok(())
    }

    pub async fn install_kill_switch(&mut self, level: KillSwitchLevel) -> Result<()> {
        if !self.nft_is_available().await {
            bail!("nftables is not available on this system");
        }

        let rules = match level {
            KillSwitchLevel::Soft => self.build_soft_rules(),
            KillSwitchLevel::Hard => self.build_hard_rules(),
            KillSwitchLevel::Nuclear => NFT_NUCLEAR.to_string(),
        };

        self.nft_execute(&rules).await?;
        self.active = true;
        info!("Kill switch installed at {:?} level", level);
        Ok(())
    }

    pub async fn remove_kill_switch(&mut self) -> Result<()> {
        self.flush_ruleset().await?;
        self.active = false;
        info!("Kill switch removed");
        Ok(())
    }

    pub async fn check_status(&self) -> Result<KillSwitchStatus> {
        if !self.nft_is_available().await {
            return Ok(KillSwitchStatus::Unavailable);
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(["list", "table", "inet", TABLE_NAME])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(Duration::from_secs(10), cmd.output())
            .await
            .context("nft list command timed out")?
            .context("nft list command failed")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(TABLE_NAME) {
                let chain_count = stdout.matches("chain ").count();
                return match chain_count {
                    3 => Ok(KillSwitchStatus::Active(stdout.to_string())),
                    _ => Ok(KillSwitchStatus::Partial(stdout.to_string())),
                };
            }
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such file or directory") || stderr.contains("does not exist") {
                return Ok(KillSwitchStatus::Inactive);
            }
            warn!("nft list returned non-zero (table may not exist): {stderr}");
        }

        Ok(KillSwitchStatus::Inactive)
    }

    fn build_soft_rules(&self) -> String {
        if self.allowed_interfaces.is_empty() {
            return NFT_KILL_SWITCH_SOFT.to_string();
        }
        let mut iface_list: Vec<String> = self
            .allowed_interfaces
            .iter()
            .map(|i| format!("\"{}\"", i))
            .collect();
        iface_list.push("\"tun+\"".to_string());
        iface_list.push("\"wg0\"".to_string());
        iface_list.push("\"wg+\"".to_string());
        iface_list.push("\"obfs+\"".to_string());
        let ifaces = iface_list.join(", ");

        format!(
            r#"
table inet endpoint_privacy {{
    chain privacy_input {{
        type filter hook input priority 0; policy drop;
        iif "lo" accept
        ct state established,related accept
        iifname {{ {ifaces} }} accept
        ip protocol icmp accept
        ip6 protocol icmpv6 accept
        reject with icmpx type admin-prohibited
    }}
    chain privacy_output {{
        type filter hook output priority 0; policy drop;
        oif "lo" accept
        ct state established,related accept
        oifname {{ {ifaces} }} accept
        udp dport {{ 53, 853 }} accept
        tcp dport {{ 53, 853 }} accept
        reject with icmpx type admin-prohibited
    }}
    chain privacy_forward {{
        type filter hook forward priority 0; policy drop;
        iifname {{ {ifaces} }} oifname != "lo" accept
        reject with icmpx type admin-prohibited
    }}
}}
"#
        )
    }

    fn build_hard_rules(&self) -> String {
        if self.allowed_interfaces.is_empty() {
            return NFT_KILL_SWITCH.to_string();
        }
        let mut iface_list: Vec<String> = self
            .allowed_interfaces
            .iter()
            .map(|i| format!("\"{}\"", i))
            .collect();
        iface_list.push("\"tun+\"".to_string());
        iface_list.push("\"wg0\"".to_string());
        iface_list.push("\"wg+\"".to_string());
        iface_list.push("\"obfs+\"".to_string());
        let ifaces = iface_list.join(", ");

        format!(
            r#"
table inet endpoint_privacy {{
    chain privacy_input {{
        type filter hook input priority 0; policy drop;
        iif "lo" accept
        ct state established,related accept
        iifname {{ {ifaces} }} accept
        reject with icmpx type admin-prohibited
    }}
    chain privacy_output {{
        type filter hook output priority 0; policy drop;
        oif "lo" accept
        ct state established,related accept
        oifname {{ {ifaces} }} accept
        reject with icmpx type admin-prohibited
    }}
    chain privacy_forward {{
        type filter hook forward priority 0; policy drop;
        reject with icmpx type admin-prohibited
    }}
}}
"#
        )
    }
}

pub enum KillSwitchStatus {
    Active(String),
    Partial(String),
    Inactive,
    Unavailable,
}

impl Default for NftablesManager {
    fn default() -> Self {
        Self::new()
    }
}
