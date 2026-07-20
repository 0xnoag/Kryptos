use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use argon2::{Argon2, Params as Argon2Params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;
use zeroize::Zeroize;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TrafficMode {
    #[serde(rename = "split")]
    Split,
    #[serde(rename = "tor_only")]
    TorOnly,
}

impl Default for TrafficMode {
    fn default() -> Self {
        TrafficMode::Split
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DaemonConfig {
    pub version: u32,
    pub autostart_services: Vec<String>,
    pub kill_switch_on_exit: bool,
    pub default_panic_level: String,
    pub traffic_mode: TrafficMode,
    pub dns: DnsConfig,
    pub tor: TorConfig,
    pub amneziawg: AmneziaWGConfig,
    pub syncthing: SyncthingConfig,
    pub mac_spoofing: MacSpoofConfig,
    pub ipc_socket_path: String,
    pub data_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DnsConfig {
    pub upstream: String,
    pub doh_url: String,
    pub bind_address: String,
    pub bind_port: u16,
    pub cache_size: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TorConfig {
    pub binary_path: String,
    pub config_path: String,
    pub socks_port: u16,
    pub control_port: u16,
    pub data_dir: String,
    pub bridges: Vec<String>,
}

impl std::fmt::Debug for TorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorConfig")
            .field("binary_path", &self.binary_path)
            .field("config_path", &self.config_path)
            .field("socks_port", &self.socks_port)
            .field("control_port", &self.control_port)
            .field("data_dir", &self.data_dir)
            .field("bridges", &format!("[{} entries]", self.bridges.len()))
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AmneziaWGConfig {
    pub binary_path: String,
    pub config_path: String,
    pub tunnel_name: String,
    pub listen_port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncthingConfig {
    pub binary_path: String,
    pub home: String,
    pub gui_address: String,
    pub gui_port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MacSpoofConfig {
    pub enabled: bool,
    pub rotation_interval_secs: u64,
    pub exclude_interfaces: Vec<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            version: 1,
            autostart_services: vec!["tor".into()],
            kill_switch_on_exit: true,
            default_panic_level: "soft".into(),
            traffic_mode: TrafficMode::default(),
            dns: DnsConfig {
                upstream: "1.1.1.1".into(),
                doh_url: "https://cloudflare-dns.com/dns-query".into(),
                bind_address: "127.0.0.1".into(),
                bind_port: 53,
                cache_size: 4096,
            },
            tor: TorConfig {
                binary_path: "/usr/bin/tor".into(),
                config_path: "/etc/tor/torrc".into(),
                socks_port: 9050,
                control_port: 9051,
                data_dir: "/var/lib/tor".into(),
                bridges: vec![],
            },
            amneziawg: AmneziaWGConfig {
                binary_path: "/usr/bin/awg".into(),
                config_path: "/etc/amneziawg/awg0.conf".into(),
                tunnel_name: "awg0".into(),
                listen_port: 51820,
            },
            syncthing: SyncthingConfig {
                binary_path: "/usr/bin/syncthing".into(),
                home: "/etc/syncthing".into(),
                gui_address: "127.0.0.1".into(),
                gui_port: 8384,
            },
            mac_spoofing: MacSpoofConfig {
                enabled: false,
                rotation_interval_secs: 600,
                exclude_interfaces: vec!["lo".into()],
            },
            ipc_socket_path: "/run/endpoint-privacy/ipc.sock".into(),
            data_dir: "/etc/endpoint-privacy".into(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    key: [u8; 32],
}

impl ConfigManager {
    pub fn new(config_dir: &str, password: &str) -> Result<Self> {
        let config_dir = PathBuf::from(config_dir);
        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config_path = config_dir.join("config.enc");
        let key = Self::derive_key(password, &config_dir)?;

        Ok(Self { config_path, key })
    }

    fn derive_key(password: &str, salt_dir: &Path) -> Result<[u8; 32]> {
        let salt_path = salt_dir.join(".salt");
        let salt = if salt_path.exists() {
            std::fs::read(&salt_path).context("Failed to read salt file")?
        } else {
            let mut salt = vec![0u8; 32];
            use rand::RngCore;
            rand::rngs::OsRng.fill_bytes(&mut salt);
            std::fs::write(&salt_path, &salt).context("Failed to write salt file")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&salt_path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    if let Err(e) = std::fs::set_permissions(&salt_path, perms) {
                        warn!("Failed to set permissions on {}: {e}", salt_path.display());
                    }
                }
            }
            salt
        };

        let mut key = [0u8; 32];
        let params = Argon2Params::new(65536, 3, 4, Some(32))
            .map_err(|e| anyhow::anyhow!("Failed to create Argon2 params: {e}"))?;

        if let Err(e) = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
            .hash_password_into(password.as_bytes(), &salt, &mut key)
        {
            key.zeroize();
            anyhow::bail!("Argon2 key derivation failed: {e}");
        }

        Ok(key)
    }

    pub fn load(&self) -> Result<DaemonConfig> {
        if !self.config_path.exists() {
            let config = DaemonConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        let encrypted =
            std::fs::read(&self.config_path).context("Failed to read encrypted config")?;

        if encrypted.len() < 12 {
            anyhow::bail!("Encrypted config file is too short ({} bytes) — expected at least 12 bytes for nonce", encrypted.len());
        }

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| anyhow::anyhow!("Invalid key length: {e}"))?;

        let nonce = Nonce::from_slice(&encrypted[..12]);
        let plaintext = cipher
            .decrypt(nonce, &encrypted[12..])
            .map_err(|_| anyhow::anyhow!("Decryption failed — wrong password or corrupted file"))?;

        let config: DaemonConfig = toml::from_str(
            std::str::from_utf8(&plaintext).context("Config is not valid UTF-8")?,
        )
        .context("Failed to deserialize config")?;

        Ok(config)
    }

    pub fn save(&self, config: &DaemonConfig) -> Result<()> {
        let plaintext = toml::to_string_pretty(config).context("Failed to serialize config")?;

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| anyhow::anyhow!("Invalid key length: {e}"))?;

        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;

        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.config_path)
                .context("Failed to write encrypted config")?;
            file.write_all(&output)
                .context("Failed to write encrypted config")?;
            file.sync_all().ok();
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&self.config_path, &output).context("Failed to write encrypted config")?;
        }

        Ok(())
    }
}

impl Drop for ConfigManager {
    fn drop(&mut self) {

        self.key.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tor_config_debug_hides_bridges() {
        let config = TorConfig {
            binary_path: "/usr/bin/tor".into(),
            config_path: "/etc/tor/torrc".into(),
            socks_port: 9050,
            control_port: 9051,
            data_dir: "/var/lib/tor".into(),
            bridges: vec![
                "obfs4 192.0.2.1:443 cert=deadbeef iat-mode=0".into(),
                "obfs4 192.0.2.2:443 cert=cafebabe iat-mode=0".into(),
            ],
        };

        let debug_str = format!("{:?}", config);
        assert!(
            !debug_str.contains("deadbeef"),
            "Debug should not expose bridge cert"
        );
        assert!(
            !debug_str.contains("cafebabe"),
            "Debug should not expose bridge cert"
        );
        assert!(
            debug_str.contains("[2 entries]"),
            "Debug should show entry count"
        );
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = DaemonConfig::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.dns.upstream, "1.1.1.1");
        assert!(config.kill_switch_on_exit);
    }

    #[test]
    fn test_argon2_params_use_strong_defaults() {
        // The parameters used in ConfigManager::derive_key
        let params = Argon2Params::new(65536, 3, 4, Some(32)).expect("valid Argon2 params");
        // Verify the params are reasonable (non-trivial)
        assert!(
            params.m_cost() >= 65536,
            "memory cost should be at least 64 MiB, got {}",
            params.m_cost()
        );
        assert!(
            params.t_cost() >= 3,
            "time cost should be at least 3, got {}",
            params.t_cost()
        );
        assert!(
            params.p_cost() >= 1,
            "parallelism should be at least 1, got {}",
            params.p_cost()
        );
    }

    #[test]
    fn test_config_roundtrip() {
        let config = DaemonConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: DaemonConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.version, config.version);
        assert_eq!(deserialized.dns.upstream, config.dns.upstream);
    }
}
