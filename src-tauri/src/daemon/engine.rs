use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch, RwLock};
use std::sync::Arc;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceName {
    Tor,
    Obfs4Proxy,
    AmneziaWG,
    Syncthing,
}

impl ServiceName {
    pub fn binary_name(&self) -> &'static str {
        match self {
            ServiceName::Tor => "tor",
            ServiceName::Obfs4Proxy => "obfs4proxy",
            ServiceName::AmneziaWG => "awg",
            ServiceName::Syncthing => "syncthing",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ServiceName::Tor => "Tor",
            ServiceName::Obfs4Proxy => "obfs4proxy",
            ServiceName::AmneziaWG => "AmneziaWG",
            ServiceName::Syncthing => "Syncthing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed(String),
    Restarting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: ServiceName,
    pub status: ServiceStatus,
    pub uptime_secs: u64,
    pub restart_count: u32,
    pub pid: Option<u32>,
}

struct ManagedProcess {
    name: ServiceName,
    binary_path: PathBuf,
    args: Vec<String>,
    status: ServiceStatus,
    restart_count: u32,
    max_restarts: u32,
    started_at: Option<std::time::Instant>,
    status_tx: Option<watch::Sender<ServiceStatus>>,
    status_rx: watch::Receiver<ServiceStatus>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

pub struct ProcessManager {
    services: HashMap<ServiceName, Arc<RwLock<ManagedProcess>>>,
    restart_delay: Duration,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            restart_delay: Duration::from_secs(3),
        }
    }

    pub fn register_service(
        &mut self,
        name: ServiceName,
        binary_path: &str,
        args: Vec<String>,
        max_restarts: u32,
    ) {
        let (status_tx, status_rx) = watch::channel(ServiceStatus::Stopped);
        let proc = ManagedProcess {
            name,
            binary_path: PathBuf::from(binary_path),
            args,
            status: ServiceStatus::Stopped,
            restart_count: 0,
            max_restarts,
            started_at: None,
            status_tx: Some(status_tx),
            status_rx,
            shutdown_tx: None,
        };
        self.services.insert(name, Arc::new(RwLock::new(proc)));
        info!("Registered service: {} at {}", name.display_name(), binary_path);
    }

    pub fn register_defaults(&mut self, tor_config: Option<String>, awg_config: Option<String>) {
        let tor_args = match tor_config {
            Some(path) => vec!["-f".into(), path],
            None => vec![],
        };
        self.register_service(ServiceName::Tor, "/usr/bin/tor", tor_args, 5);
        self.register_service(ServiceName::Obfs4Proxy, "/usr/bin/obfs4proxy", vec![], 5);

        let awg_args = match awg_config {
            Some(path) => vec!["-c".into(), path],
            None => vec!["-c".into(), "/etc/amneziawg/awg0.conf".into()],
        };
        self.register_service(ServiceName::AmneziaWG, "/usr/bin/awg", awg_args, 3);

        self.register_service(
            ServiceName::Syncthing,
            "/usr/bin/syncthing",
            vec!["--no-browser".into(), "--no-restart".into()],
            3,
        );
    }

    pub async fn start(&mut self, name: ServiceName) -> Result<()> {
        let proc_arc = self
            .services
            .get(&name)
            .ok_or_else(|| anyhow::anyhow!("Service {} not registered", name.display_name()))?;

        let mut proc = proc_arc.write().await;

        if proc.status == ServiceStatus::Running {
            info!("{} is already running", name.display_name());
            return Ok(());
        }

        let binary = &proc.binary_path;
        if !binary.exists() {
            bail!(
                "Binary not found: {} — install {}",
                binary.display(),
                name.binary_name()
            );
        }

        proc.status = ServiceStatus::Starting;
        proc.restart_count = 0;
        proc.started_at = None;

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<u8>(1);
        proc.shutdown_tx = Some(shutdown_tx);

        let binary_clone = proc.binary_path.clone();
        let args_clone = proc.args.clone();
        let max_restarts = proc.max_restarts;
        let name_clone = name;
        let status_tx = proc.status_tx.clone();
        let restart_delay = self.restart_delay;

        let child = Command::new(&binary_clone)
            .args(&args_clone)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context(format!("Failed to spawn {}", name_clone.display_name()))?;

        let pid = child.id().ok_or_else(|| anyhow::anyhow!("No PID for spawned process"))?;

        if let Some(ref tx) = status_tx {
            let _ = tx.send(ServiceStatus::Running);
        }
        proc.status = ServiceStatus::Running;
        proc.started_at = Some(std::time::Instant::now());
        info!("{} started (PID: {}, restart limit: {})", name_clone.display_name(), pid, max_restarts);

        let proc_watch = proc_arc.clone();
        tokio::spawn(async move {
            Self::watch_process(name_clone, child, status_tx, shutdown_rx, proc_watch, restart_delay).await;
        });

        Ok(())
    }

    async fn watch_process(
        name: ServiceName,
        mut child: Child,
        status_tx: Option<watch::Sender<ServiceStatus>>,
        mut shutdown_rx: mpsc::Receiver<u8>,
        proc_arc: Arc<RwLock<ManagedProcess>>,
        restart_delay: Duration,
    ) {
        let exit_status = tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("{} shutdown requested, killing", name.display_name());
                let _ = child.kill().await;
                let status = child.wait().await;
                if let Some(ref tx) = status_tx {
                    let _ = tx.send(ServiceStatus::Stopped);
                }
                let mut proc = proc_arc.write().await;
                proc.status = ServiceStatus::Stopped;
                return;
            }
            status = child.wait() => {
                status
            }
        };

        let status = match exit_status {
            Ok(s) => {
                warn!("{} exited with status: {}", name.display_name(), s);
                let mut proc = proc_arc.write().await;
                proc.restart_count += 1;
                if proc.restart_count <= proc.max_restarts {
                    proc.status = ServiceStatus::Restarting;
                    drop(proc);
                    tokio::time::sleep(restart_delay).await;
                    info!("Restarting {} (attempt {}/{})", name.display_name(),
                        proc_arc.read().await.restart_count, proc_arc.read().await.max_restarts);
                    drop(proc_arc);
                    return;
                }
                ServiceStatus::Failed(format!("Exited with {}", s))
            }
            Err(e) => {
                error!("{} wait error: {}", name.display_name(), e);
                ServiceStatus::Failed(format!("Wait error: {e}"))
            }
        };

        if let Some(ref tx) = status_tx {
            let _ = tx.send(status);
        }
        let mut proc = proc_arc.write().await;
        proc.status = status;
    }

    pub async fn stop(&mut self, name: ServiceName) -> Result<()> {
        let proc_arc = self
            .services
            .get(&name)
            .ok_or_else(|| anyhow::anyhow!("Service {} not registered", name.display_name()))?;

        let mut proc = proc_arc.write().await;

        if proc.status == ServiceStatus::Stopped {
            return Ok(());
        }

        proc.status = ServiceStatus::Stopping;

        if let Some(tx) = proc.shutdown_tx.take() {
            let _ = tx.try_send(0);
        }

        proc.status = ServiceStatus::Stopped;
        proc.started_at = None;
        info!("{} stopped", name.display_name());
        Ok(())
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let names: Vec<ServiceName> = self.services.keys().copied().collect();
        for name in names {
            self.stop(name).await?;
        }
        info!("All services stopped");
        Ok(())
    }

    pub async fn status(&self, name: ServiceName) -> ServiceStatus {
        self.services
            .get(&name)
            .map(|p| p.blocking_read().status)
            .unwrap_or(ServiceStatus::Stopped)
    }

    pub async fn all_status(&self) -> Vec<ServiceInfo> {
        let now = std::time::Instant::now();
        let mut infos = Vec::new();
        for proc_arc in self.services.values() {
            let proc = proc_arc.read().await;
            infos.push(ServiceInfo {
                name: proc.name,
                status: proc.status,
                uptime_secs: proc
                    .started_at
                    .map(|t| now.duration_since(t).as_secs())
                    .unwrap_or(0),
                restart_count: proc.restart_count,
                pid: None,
            });
        }
        infos
    }

    pub async fn is_any_running(&self) -> bool {
        for proc_arc in self.services.values() {
            let proc = proc_arc.read().await;
            if proc.status == ServiceStatus::Running {
                return true;
            }
        }
        false
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}
