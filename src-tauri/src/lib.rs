pub mod daemon;
pub mod firewall;
pub mod network;
pub mod utils;
pub mod crypto;
pub mod security;

use daemon::config::DaemonConfig;
use daemon::engine::ProcessManager;
use firewall::nftables::{KillSwitchStatus, NftablesManager};
use firewall::panic::PanicEngine;
use security::verify::BinaryVerifier;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub struct EndpointPrivacyDaemon {
    pub config: DaemonConfig,
    pub process_manager: Arc<RwLock<ProcessManager>>,
    pub nftables_manager: Arc<RwLock<NftablesManager>>,
    pub panic_engine: Arc<RwLock<PanicEngine>>,
    pub binary_verifier: Option<BinaryVerifier>,
}

impl EndpointPrivacyDaemon {
    pub async fn new(
        config_dir: &str,
        password: &str,
        strict_verification: bool,
    ) -> anyhow::Result<Self> {
        let config_mgr = daemon::config::ConfigManager::new(config_dir, password)?;
        let config = config_mgr.load()?;
        info!("Configuration loaded from {}", config_dir);

        let nftables_manager = Arc::new(RwLock::new({
            let mut nft = NftablesManager::new();
            // Restrict outbound DNS exceptions to the configured upstream resolver
            nft.set_dns_upstream(&config.dns.upstream);
            nft
        }));
        let panic_engine = Arc::new(RwLock::new(PanicEngine::new(nftables_manager.clone())));

        {
            let nft = nftables_manager.read().await;
            match nft.check_status().await {
                Ok(KillSwitchStatus::Active(rules)) => {
                    warn!(
                        "Pre-existing nftables kill-switch rules detected on startup:\n{}",
                        rules
                    );
                }
                Ok(KillSwitchStatus::Partial(rules)) => {
                    warn!(
                        "Partial pre-existing nftables rules detected:\n{}",
                        rules
                    );
                }
                Ok(KillSwitchStatus::Inactive) => {
                    info!("No pre-existing nftables kill-switch rules found");
                }
                Ok(KillSwitchStatus::Unavailable) => {
                    warn!("nftables binary not available — kill switch cannot be activated");
                }
                Err(e) => {
                    warn!("Failed to check nftables status on startup: {e}");
                }
            }
        }

        // Load binary hash verifier from config directory .hashes file
        let hashes_path = Path::new(config_dir).join(".hashes");
        let verifier = match BinaryVerifier::from_hashes_file(&hashes_path, strict_verification) {
            Ok(v) => {
                let hash_count = v.hash_count();
                info!(
                    "Loaded {} binary hash(es) from {} (strict: {strict_verification})",
                    hash_count,
                    hashes_path.display()
                );
                Some(v)
            }
            Err(e) => {
                warn!("Failed to load .hashes file ({}): {e}", hashes_path.display());
                if strict_verification {
                    return Err(e);
                }
                None
            }
        };

        let mut pm = ProcessManager::new();
        if let Some(ref v) = verifier {
            daemon::engine::set_verifier(&mut pm, v.clone());
        }
        pm.register_defaults(
            Some(config.tor.config_path.clone()),
            Some(config.amneziawg.config_path.clone()),
        );

        let process_manager = Arc::new(RwLock::new(pm));

        Ok(Self {
            config,
            process_manager,
            nftables_manager,
            panic_engine,
            binary_verifier: verifier,
        })
    }
}
