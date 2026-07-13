use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

const NFT_BINARY: &str = "/usr/sbin/nft";
const TABLE_NAME: &str = "endpoint_privacy";
const CHAIN_INPUT: &str = "privacy_input";
const CHAIN_OUTPUT: &str = "privacy_output";
const CHAIN_FORWARD: &str = "privacy_forward";

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
        let nft_path = self.binary_path.clone();
        let mut cmd = Command::new(&nft_path);
        cmd.args(["-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn nft process")?;

        let stdin = child.stdin.take().context("Failed to open nft stdin")?;
        let rules_owned = rules.to_string();

        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut stdin = stdin;
            if let Err(e) = stdin.write_all(rules_owned.as_bytes()).await {
                error!("Failed to write nft rules to stdin: {e}");
            }
        });

        let output = child.wait_with_output().await?;

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
        cmd.status().await.map(|s| s.success()).unwrap_or(false)
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
        cmd.args(["-a", "list", "table", "inet", TABLE_NAME])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let output = cmd.output().await?;

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
