use crate::security::verify::BinaryVerifier;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch, Mutex, RwLock};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    shutdown_tx: Option<mpsc::Sender<u8>>,
}

pub struct ProcessManager {
    services: HashMap<ServiceName, Arc<RwLock<ManagedProcess>>>,
    start_locks: HashMap<ServiceName, Arc<Mutex<()>>>,
    base_restart_delay: Duration,
    max_restart_delay: Duration,
    verifier: Option<BinaryVerifier>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            start_locks: HashMap::new(),
            base_restart_delay: Duration::from_secs(2),
            max_restart_delay: Duration::from_secs(60),
            verifier: None,
        }
    }

    pub fn with_verifier(verifier: BinaryVerifier) -> Self {
        Self {
            services: HashMap::new(),
            start_locks: HashMap::new(),
            base_restart_delay: Duration::from_secs(2),
            max_restart_delay: Duration::from_secs(60),
            verifier: Some(verifier),
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
        self.start_locks.insert(name, Arc::new(Mutex::new(())));
        info!(
            "Registered service: {} at {}",
            name.display_name(),
            binary_path
        );
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
        let lock = self
            .start_locks
            .get(&name)
            .ok_or_else(|| anyhow::anyhow!("Service {} not registered", name.display_name()))?
            .clone();

        let _guard = lock.lock().await;

        let proc_arc = self
            .services
            .get(&name)
            .ok_or_else(|| anyhow::anyhow!("Service {} not registered", name.display_name()))?;

        // Acquire write-lock directly to avoid TOCTOU race
        let mut proc = proc_arc.write().await;

        if proc.status == ServiceStatus::Running || proc.status == ServiceStatus::Starting {
            info!("{} is already {:?}", name.display_name(), proc.status);
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

        if let Some(ref verifier) = self.verifier {
            verifier.verify(binary).context(format!(
                "Integrity check failed for {} at {}",
                name.display_name(),
                binary.display()
            ))?;
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
        let base_delay = self.base_restart_delay;
        let max_delay = self.max_restart_delay;

        let child = Command::new(&binary_clone)
            .args(&args_clone)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context(format!("Failed to spawn {}", name_clone.display_name()))?;

        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("No PID for spawned process"))?;

        if let Some(ref tx) = status_tx {
            let _ = tx.send(ServiceStatus::Running);
        }
        proc.status = ServiceStatus::Running;
        proc.started_at = Some(std::time::Instant::now());
        info!(
            "{} started (PID: {}, restart limit: {})",
            name_clone.display_name(),
            pid,
            max_restarts
        );

        let proc_watch = proc_arc.clone();
        let verifier = self.verifier.clone();
        tokio::spawn(async move {
            Self::watch_process(
                name_clone,
                child,
                status_tx,
                shutdown_rx,
                proc_watch,
                base_delay,
                max_delay,
                max_restarts,
                verifier,
            )
            .await;
        });

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn watch_process(
        name: ServiceName,
        child: Child,
        status_tx: Option<watch::Sender<ServiceStatus>>,
        mut shutdown_rx: mpsc::Receiver<u8>,
        proc_arc: Arc<RwLock<ManagedProcess>>,
        base_delay: Duration,
        max_delay: Duration,
        max_restarts: u32,
        verifier: Option<BinaryVerifier>,
    ) {
        let mut current_child: Option<Child> = Some(child);

        loop {
            let mut child = match current_child.take() {
                Some(c) => c,
                None => break,
            };

            let output = tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("{} shutdown requested, killing", name.display_name());
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    if let Some(ref tx) = status_tx {
                        let _ = tx.send(ServiceStatus::Stopped);
                    }
                    let mut proc = proc_arc.write().await;
                    proc.status = ServiceStatus::Stopped;
                    return;
                }
                status = child.wait() => {
                    use tokio::io::AsyncReadExt;
                    let mut stdout_buf = Vec::new();
                    let mut stderr_buf = Vec::new();
                    if let Some(ref mut out) = child.stdout {
                        let _ = out.read_to_end(&mut stdout_buf).await;
                    }
                    if let Some(ref mut err) = child.stderr {
                        let _ = err.read_to_end(&mut stderr_buf).await;
                    }
                    status.map(|s| std::process::Output {
                        status: s,
                        stdout: stdout_buf,
                        stderr: stderr_buf,
                    })
                }
            };

            match output {
                Ok(output) => {
                    let stderr_str = String::from_utf8_lossy(&output.stderr);
                    let stdout_str = String::from_utf8_lossy(&output.stdout);

                    if !stdout_str.trim().is_empty() {
                        for line in stdout_str.lines() {
                            info!("{} [stdout]: {}", name.display_name(), line);
                        }
                    }
                    if !stderr_str.trim().is_empty() {
                        for line in stderr_str.lines() {
                            warn!("{} [stderr]: {}", name.display_name(), line);
                        }
                    }

                    let exit_code = output.status;
                    warn!("{} exited with status: {}", name.display_name(), exit_code);

                    let mut proc = proc_arc.write().await;
                    proc.restart_count += 1;
                    let attempt = proc.restart_count;

                    // Re-check: if service was stopped while we were processing output, abort restart
                    if proc.status == ServiceStatus::Stopped
                        || proc.status == ServiceStatus::Stopping
                    {
                        info!("{} was stopped during restart window", name.display_name());
                        return;
                    }

                    if attempt <= max_restarts {
                        let exp = std::cmp::min(attempt.saturating_sub(1), 5);
                        let multiplier = 2u64.pow(exp);
                        let delay_secs = base_delay.as_secs_f64() * (multiplier as f64);
                        let delay =
                            Duration::from_secs_f64(delay_secs.min(max_delay.as_secs_f64()));

                        proc.status = ServiceStatus::Restarting;
                        drop(proc);

                        info!(
                            "Restarting {} in {:.1}s (attempt {}/{})",
                            name.display_name(),
                            delay.as_secs_f64(),
                            attempt,
                            max_restarts,
                        );
                        tokio::time::sleep(delay).await;

                        // Re-acquire and verify service hasn't been stopped during sleep
                        let proc = proc_arc.read().await;
                        if proc.status != ServiceStatus::Restarting {
                            info!(
                                "{} restart cancelled (status changed to {:?} during delay)",
                                name.display_name(),
                                proc.status
                            );
                            return;
                        }
                        let binary = proc.binary_path.clone();
                        let args = proc.args.clone();
                        let name_for_verify = proc.name;
                        drop(proc);

                        // Verify binary hash before restart spawn
                        if let Some(ref verifier) = verifier {
                            if let Err(e) = verifier.verify(&binary) {
                                error!(
                                    "Integrity check failed on restart for {} at {}: {e}",
                                    name_for_verify.display_name(),
                                    binary.display()
                                );
                                let status =
                                    ServiceStatus::Failed(format!("Integrity check failed: {e}"));
                                if let Some(ref tx) = status_tx {
                                    let _ = tx.send(status.clone());
                                }
                                let mut proc = proc_arc.write().await;
                                proc.status = status;
                                break;
                            }
                        }

                        match Command::new(&binary)
                            .args(&args)
                            .stdin(Stdio::null())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .kill_on_drop(true)
                            .spawn()
                        {
                            Ok(new_child) => {
                                current_child = Some(new_child);
                                continue;
                            }
                            Err(e) => {
                                error!("Failed to restart {}: {e}", name.display_name());
                                let status = ServiceStatus::Failed(format!("Restart failed: {e}"));
                                if let Some(ref tx) = status_tx {
                                    let _ = tx.send(status.clone());
                                }
                                let mut proc = proc_arc.write().await;
                                proc.status = status;
                                break;
                            }
                        }
                    } else {
                        let status = ServiceStatus::Failed(format!(
                            "Exceeded max restarts ({max_restarts}), last exit: {exit_code}"
                        ));
                        if let Some(ref tx) = status_tx {
                            let _ = tx.send(status.clone());
                        }
                        let mut proc = proc_arc.write().await;
                        proc.status = status;
                        break;
                    }
                }
                Err(e) => {
                    error!("{} wait error: {}", name.display_name(), e);
                    let status = ServiceStatus::Failed(format!("Wait error: {e}"));
                    if let Some(ref tx) = status_tx {
                        let _ = tx.send(status.clone());
                    }
                    let mut proc = proc_arc.write().await;
                    proc.status = status;
                    break;
                }
            }
        }
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
            // Use blocking send to ensure delivery; unwrap is safe because
            // watch_process holds the receiver end for the entire loop
            if tx.send(0).await.is_err() {
                warn!(
                    "{} shutdown signal not received (watchdog already exited)",
                    name.display_name()
                );
            }
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
            .map(|p| {
                match p.try_read() {
                Ok(guard) => guard.status.clone(),
                Err(_) => ServiceStatus::Stopped,
                }
            })
            .unwrap_or(ServiceStatus::Stopped)
    }

    pub async fn all_status(&self) -> Vec<ServiceInfo> {
        let now = std::time::Instant::now();
        let mut infos = Vec::new();
        for proc_arc in self.services.values() {
            let proc = proc_arc.read().await;
            infos.push(ServiceInfo {
                name: proc.name,
                status: proc.status.clone(),
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

/// Set the binary verifier on an existing ProcessManager.
/// Used during daemon initialization, before registering services.
pub fn set_verifier(pm: &mut ProcessManager, verifier: BinaryVerifier) {
    pm.verifier = Some(verifier);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pm() -> ProcessManager {
        let mut pm = ProcessManager::new();
        pm.register_service(
            ServiceName::Tor,
            "/usr/bin/tor",
            vec!["-f".into(), "/etc/tor/torrc".into()],
            5,
        );
        pm.register_service(
            ServiceName::Syncthing,
            "/usr/bin/syncthing",
            vec!["--no-browser".into()],
            3,
        );
        pm
    }

    #[tokio::test]
    async fn test_start_rejects_unregistered_service() {
        let mut pm = ProcessManager::new();
        let result = pm.start(ServiceName::Tor).await;
        assert!(result.is_err(), "unregistered service should fail");
    }

    #[tokio::test]
    async fn test_stop_unstarted_service_is_noop() {
        let mut pm = test_pm();
        // Stop a service that was never started
        let result = pm.stop(ServiceName::Syncthing).await;
        assert!(result.is_ok(), "stopping unstarted service should succeed");
    }

    #[tokio::test]
    async fn test_stop_all_clears_all() {
        let mut pm = test_pm();
        let result = pm.stop_all().await;
        assert!(result.is_ok(), "stop_all should succeed");

        let mut proc = pm.services.get(&ServiceName::Tor).unwrap().write().await;
        assert_eq!(proc.status, ServiceStatus::Stopped);
    }

    #[tokio::test]
    async fn test_all_status_returns_all_services() {
        let pm = test_pm();
        let statuses = pm.all_status().await;
        assert_eq!(statuses.len(), 2, "should have 2 registered services");
        assert!(statuses.iter().any(|s| s.name == ServiceName::Tor));
        assert!(statuses.iter().any(|s| s.name == ServiceName::Syncthing));
    }

    #[tokio::test]
    async fn test_is_any_running_returns_false_initially() {
        let pm = test_pm();
        assert!(
            !pm.is_any_running().await,
            "no services should be running initially"
        );
    }

    #[tokio::test]
    async fn test_start_locks_prevent_double_start() {
        let mut pm = test_pm();
        let lock1 = pm.start_locks.get(&ServiceName::Tor).unwrap().clone();
        let lock2 = pm.start_locks.get(&ServiceName::Tor).unwrap().clone();

        let _guard1 = lock1.lock().await;
        // Second lock attempt would block -- just verify the lock exists
        assert!(
            lock2.try_lock().is_err(),
            "concurrent start should be blocked"
        );
    }

    #[test]
    fn test_service_name_display() {
        assert_eq!(ServiceName::Tor.display_name(), "Tor");
        assert_eq!(ServiceName::Obfs4Proxy.display_name(), "obfs4proxy");
        assert_eq!(ServiceName::AmneziaWG.display_name(), "AmneziaWG");
        assert_eq!(ServiceName::Syncthing.display_name(), "Syncthing");
    }

    #[test]
    fn test_service_name_binary() {
        assert_eq!(ServiceName::Tor.binary_name(), "tor");
        assert_eq!(ServiceName::AmneziaWG.binary_name(), "awg");
    }

    #[test]
    fn test_service_status_serialization() {
        let status = ServiceStatus::Failed("test error".into());
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("test error"));

        let deserialized: ServiceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{:?}", deserialized), format!("{:?}", status));
    }
}
