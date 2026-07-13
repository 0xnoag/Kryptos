use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use argon2::{Argon2, Params as Argon2Params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zeroize::Zeroize;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DaemonConfig {
    pub version: u32,
    pub autostart_services: Vec<String>,
    pub kill_switch_on_exit: bool,
    pub default_panic_level: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TorConfig {
    pub binary_path: String,
    pub config_path: String,
    pub socks_port: u16,
    pub control_port: u16,
    pub data_dir: String,
    pub bridges: Vec<String>,
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
        std::fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;

        let config_path = config_dir.join("config.enc");
        let key = Self::derive_key(password, &config_dir)?;

        Ok(Self { config_path, key })
    }

    fn derive_key(password: &str, salt_dir: &PathBuf) -> Result<[u8; 32]> {
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
                    let _ = std::fs::set_permissions(&salt_path, perms);
                }
            }
            salt
        };

        let mut key = [0u8; 32];
        let params = Argon2Params::new(
            65536,
            3,
            4,
            Some(32),
        ).context("Failed to create Argon2 params")?;

        Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            params,
        )
            .hash_password_into(password.as_bytes(), &salt, &mut key)
            .context("Argon2 key derivation failed")?;

        Ok(key)
    }

    pub fn load(&self) -> Result<DaemonConfig> {
        if !self.config_path.exists() {
            let config = DaemonConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        let encrypted = std::fs::read(&self.config_path)
            .context("Failed to read encrypted config")?;

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| anyhow::anyhow!("Invalid key length: {e}"))?;

        let nonce = Nonce::from_slice(&encrypted[..12]);
        let plaintext = cipher
            .decrypt(nonce, &encrypted[12..])
            .map_err(|_| anyhow::anyhow!("Decryption failed — wrong password or corrupted file"))?;

        let config: DaemonConfig = toml::from_slice(&plaintext)
            .context("Failed to deserialize config")?;

        Ok(config)
    }

    pub fn save(&self, config: &DaemonConfig) -> Result<()> {
        let plaintext = toml::to_string_pretty(config)
            .context("Failed to serialize config")?;

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

        std::fs::write(&self.config_path, &output)
            .context("Failed to write encrypted config")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&self.config_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&self.config_path, perms);
            }
        }

        Ok(())
    }
}

impl Drop for ConfigManager {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
    }
}
