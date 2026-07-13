pub mod daemon;
pub mod firewall;
pub mod network;
pub mod utils;
pub mod crypto;

use daemon::config::{ConfigManager, DaemonConfig};
use daemon::engine::ProcessManager;
use daemon::ipc::IpcServer;
use firewall::nftables::NftablesManager;
use firewall::panic::PanicEngine;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct EndpointPrivacyDaemon {
    pub config: DaemonConfig,
    pub process_manager: Arc<RwLock<ProcessManager>>,
    pub nftables_manager: Arc<RwLock<NftablesManager>>,
    pub panic_engine: Arc<RwLock<PanicEngine>>,
    ipc_server: Option<IpcServer>,
}

impl EndpointPrivacyDaemon {
    pub async fn new(config_dir: &str, password: &str) -> anyhow::Result<Self> {
        let config_mgr = ConfigManager::new(config_dir, password)?;
        let config = config_mgr.load()?;
        info!("Configuration loaded from {}", config_dir);

        let nftables_manager = Arc::new(RwLock::new(NftablesManager::new()));
        let panic_engine = Arc::new(RwLock::new(PanicEngine::new(nftables_manager.clone())));

        let mut pm = ProcessManager::new();
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
            ipc_server: None,
        })
    }

    pub async fn start_ipc_server(&mut self) -> anyhow::Result<()> {
        let ipc = IpcServer::new(
            &self.config.ipc_socket_path,
            self.process_manager.clone(),
            self.panic_engine.clone(),
        )?;
        self.ipc_server = Some(ipc);
        info!("IPC server started on {}", self.config.ipc_socket_path);
        Ok(())
    }

    pub async fn run_ipc(&self) -> anyhow::Result<()> {
        if let Some(ref ipc) = self.ipc_server {
            ipc.run().await
        } else {
            anyhow::bail!("IPC server not initialized");
        }
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        info!("Shutting down Endpoint Privacy Suite");

        if self.config.kill_switch_on_exit {
            let mut panic = self.panic_engine.write().await;
            let _ = panic.activate(firewall::panic::PanicLevel::Nuclear).await;
        }

        let mut pm = self.process_manager.write().await;
        pm.stop_all().await?;

        info!("Shutdown complete");
        Ok(())
    }
}
