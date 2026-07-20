use crate::daemon::engine::{ProcessManager, ServiceInfo, ServiceName};
use crate::firewall::panic::{PanicEngine, PanicLevel, PanicStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

const MAX_PAYLOAD_BYTES: usize = 65536;
const MAX_SERVICE_NAME_LEN: usize = 32;

/// Connection rate limiting: max 30 connections per window
const RATE_LIMIT_WINDOW_SECS: u64 = 10;
const RATE_LIMIT_MAX_CONNECTIONS: u32 = 30;
const PER_CONNECTION_DELAY_MS: u64 = 50;

/// Nuclear confirmation required string
const NUCLEAR_CONFIRMATION: &str = "CONFIRM_NUCLEAR_I_AM_SURE";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcRequest {
    GetStatus,
    StartService {
        service: String,
    },
    StopService {
        service: String,
    },
    RestartService {
        service: String,
    },
    SetPanicLevel {
        level: String,
        confirmation: Option<String>,
    },
    GetPanicStatus,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcResponse {
    Ok {
        message: String,
    },
    Error {
        message: String,
    },
    Status {
        services: Vec<ServiceInfo>,
        panic: PanicStatus,
    },
    PanicStatus(PanicStatus),
}

pub struct IpcServer {
    listener: UnixListener,
    process_manager: Arc<RwLock<ProcessManager>>,
    panic_engine: Arc<RwLock<PanicEngine>>,
    shutdown_signal: tokio::sync::watch::Sender<bool>,
    connection_count: Arc<std::sync::atomic::AtomicU32>,
    rate_limit_reset: Arc<tokio::sync::Mutex<tokio::time::Instant>>,
}

impl IpcServer {
    pub fn new(
        socket_path: &str,
        process_manager: Arc<RwLock<ProcessManager>>,
        panic_engine: Arc<RwLock<PanicEngine>>,
        shutdown_signal: tokio::sync::watch::Sender<bool>,
    ) -> Result<Self> {
        let sock_path = Path::new(socket_path);
        if sock_path.exists() {
            #[cfg(unix)]
            {
                let meta = std::fs::symlink_metadata(socket_path)
                    .context("Failed to read socket metadata")?;
                if meta.file_type().is_symlink() {
                    anyhow::bail!("Socket path is a symlink, refusing to remove");
                }
                use std::os::unix::fs::FileTypeExt;
                if !meta.file_type().is_socket() {
                    anyhow::bail!("Socket path exists but is not a Unix socket");
                }
            }
            std::fs::remove_file(socket_path).context("Failed to remove existing socket file")?;
        }

        let dir = sock_path
            .parent()
            .context("Socket path has no parent directory")?;
        std::fs::create_dir_all(dir).context("Failed to create socket directory")?;

        let listener = UnixListener::bind(socket_path).context("Failed to bind Unix socket")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o700))
                .context("Failed to set socket permissions to 0700")?;
        }

        info!("IPC server listening on {}", socket_path);
        Ok(Self {
            listener,
            process_manager,
            panic_engine,
            shutdown_signal,
            connection_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            rate_limit_reset: Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now())),
        })
    }

    pub async fn run(&self) -> Result<()> {
        let mut shutdown_rx = self.shutdown_signal.subscribe();

        loop {
            tokio::select! {
                accept_result = self.listener.accept() => {
                    match accept_result {
                        Ok((stream, _addr)) => {
                            // IPC-02: Rate limiting check
                            if !self.check_rate_limit().await {
                                error!("IPC rate limit exceeded, dropping connection");
                                // Wait briefly to avoid tight loop on repeated rejections
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }

                            // IPC-01: Peer credential verification (must be root)
                            if !Self::verify_peer_credentials(&stream) {
                                error!("IPC connection rejected: peer is not root");
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }

                            let pm = self.process_manager.clone();
                            let pe = self.panic_engine.clone();
                            let shutdown_tx = self.shutdown_signal.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(stream, pm, pe, shutdown_tx).await {
                                    error!("IPC client handler error: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept IPC connection: {e}");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("IPC server received shutdown signal, stopping accept loop");
                    break;
                }
            }
        }

        Ok(())
    }

    /// IPC-02: Connection rate limiting using a sliding window
    async fn check_rate_limit(&self) -> bool {
        let mut reset_time = self.rate_limit_reset.lock().await;
        let now = tokio::time::Instant::now();

        // Reset the counter if the window has expired
        if now.duration_since(*reset_time) > Duration::from_secs(RATE_LIMIT_WINDOW_SECS) {
            self.connection_count
                .store(0, std::sync::atomic::Ordering::Relaxed);
            *reset_time = now;
        }

        let count = self
            .connection_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        count < RATE_LIMIT_MAX_CONNECTIONS
    }

    /// IPC-01: Verify the peer process is running as root (UID 0)
    #[cfg(unix)]
    fn verify_peer_credentials(stream: &UnixStream) -> bool {
        // Try to get peer credentials via SO_PEERCRED
        match stream.peer_cred() {
            Ok(cred) => {
                let uid = cred.uid();
                if uid != 0 {
                    warn!("IPC connection rejected from non-root UID {uid}");
                    false
                } else {
                    true
                }
            }
            Err(e) => {
                // If we can't verify, log and reject by default (fail secure)
                error!("Failed to get IPC peer credentials: {e}");
                false
            }
        }
    }

    #[cfg(not(unix))]
    fn verify_peer_credentials(_stream: &UnixStream) -> bool {
        true
    }

    async fn handle_client(
        mut stream: UnixStream,
        process_manager: Arc<RwLock<ProcessManager>>,
        panic_engine: Arc<RwLock<PanicEngine>>,
        shutdown_tx: tokio::sync::watch::Sender<bool>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }

            if line.len() > MAX_PAYLOAD_BYTES {
                let resp = IpcResponse::Error {
                    message: format!("Request too large (max {} bytes)", MAX_PAYLOAD_BYTES),
                };
                let mut buf = serde_json::to_vec(&resp)?;
                buf.push(b'\n');
                writer.write_all(&buf).await?;
                continue;
            }

            let request: IpcRequest = match serde_json::from_str(line.trim()) {
                Ok(req) => req,
                Err(e) => {
                    let resp = IpcResponse::Error {
                        message: format!("Invalid request: {e}"),
                    };
                    let mut buf = serde_json::to_vec(&resp)?;
                    buf.push(b'\n');
                    writer.write_all(&buf).await?;
                    continue;
                }
            };

            if let Err(validation_err) = validate_request(&request) {
                let resp = IpcResponse::Error {
                    message: validation_err,
                };
                let mut buf = serde_json::to_vec(&resp)?;
                buf.push(b'\n');
                writer.write_all(&buf).await?;
                continue;
            }

            let response =
                Self::process_request(request, &process_manager, &panic_engine, &shutdown_tx).await;
            let mut buf = serde_json::to_vec(&response)?;
            buf.push(b'\n');
            writer.write_all(&buf).await?;

            // V-14: Enforce minimum inter-request gap to prevent lock starvation
            tokio::time::sleep(Duration::from_millis(PER_CONNECTION_DELAY_MS)).await;
        }

        Ok(())
    }

    async fn process_request(
        request: IpcRequest,
        pm: &Arc<RwLock<ProcessManager>>,
        pe: &Arc<RwLock<PanicEngine>>,
        shutdown_tx: &tokio::sync::watch::Sender<bool>,
    ) -> IpcResponse {
        match request {
            IpcRequest::GetStatus => {
                let pm = pm.read().await;
                let pe = pe.read().await;
                IpcResponse::Status {
                    services: pm.all_status().await,
                    panic: pe.status().await,
                }
            }
            IpcRequest::StartService { service } => {
                let name = match service_name_from_str(&service) {
                    Some(n) => n,
                    None => {
                        return IpcResponse::Error {
                            message: format!("Unknown service: {service}"),
                        }
                    }
                };
                let mut pm = pm.write().await;
                match pm.start(name).await {
                    Ok(()) => IpcResponse::Ok {
                        message: format!("{service} started"),
                    },
                    Err(e) => IpcResponse::Error {
                        message: format!("Failed to start {service}: {e}"),
                    },
                }
            }
            IpcRequest::StopService { service } => {
                let name = match service_name_from_str(&service) {
                    Some(n) => n,
                    None => {
                        return IpcResponse::Error {
                            message: format!("Unknown service: {service}"),
                        }
                    }
                };
                let mut pm = pm.write().await;
                match pm.stop(name).await {
                    Ok(()) => IpcResponse::Ok {
                        message: format!("{service} stopped"),
                    },
                    Err(e) => IpcResponse::Error {
                        message: format!("Failed to stop {service}: {e}"),
                    },
                }
            }
            IpcRequest::RestartService { service } => {
                let name = match service_name_from_str(&service) {
                    Some(n) => n,
                    None => {
                        return IpcResponse::Error {
                            message: format!("Unknown service: {service}"),
                        }
                    }
                };
                let mut pm = pm.write().await;
                if let Err(e) = pm.stop(name).await {
                    warn!("Stop failed during restart of {}: {e}", service);
                }
                match pm.start(name).await {
                    Ok(()) => IpcResponse::Ok {
                        message: format!("{service} restarted"),
                    },
                    Err(e) => IpcResponse::Error {
                        message: format!("Failed to restart {service}: {e}"),
                    },
                }
            }
            IpcRequest::SetPanicLevel {
                level,
                confirmation,
            } => {
                let normalized = level.to_lowercase();
                let mut pe = pe.write().await;

                if normalized == "off" {
                    return match pe.deactivate().await {
                        Ok(status) => IpcResponse::PanicStatus(status),
                        Err(e) => IpcResponse::Error {
                            message: format!("Panic deactivation failed: {e}"),
                        },
                    };
                }

                let panic_level = match normalized.as_str() {
                    "soft" => PanicLevel::Soft,
                    "hard" => PanicLevel::Hard,
                    "nuclear" => PanicLevel::Nuclear,
                    _ => {
                        return IpcResponse::Error {
                            message: format!(
                                "Invalid panic level: {level}. Use off, soft, hard, or nuclear"
                            ),
                        }
                    }
                };

                // T-05: Nuclear panic requires explicit confirmation
                if panic_level == PanicLevel::Nuclear {
                    let confirmed = confirmation.as_deref() == Some(NUCLEAR_CONFIRMATION);
                    if !confirmed {
                        return IpcResponse::Error {
                            message: format!(
                                "Nuclear panic requires confirmation field set to {:?}",
                                NUCLEAR_CONFIRMATION
                            ),
                        };
                    }
                }

                match pe.activate(panic_level).await {
                    Ok(status) => IpcResponse::PanicStatus(status),
                    Err(e) => IpcResponse::Error {
                        message: format!("Panic activation failed: {e}"),
                    },
                }
            }
            IpcRequest::GetPanicStatus => {
                let pe = pe.read().await;
                IpcResponse::PanicStatus(pe.status().await)
            }
            IpcRequest::Shutdown => {
                let _ = shutdown_tx.send(true);
                IpcResponse::Ok {
                    message: "Shutting down".into(),
                }
            }
        }
    }
}

fn validate_request(request: &IpcRequest) -> Result<(), String> {
    match request {
        IpcRequest::StartService { service }
        | IpcRequest::StopService { service }
        | IpcRequest::RestartService { service } => {
            if service.is_empty() {
                return Err("Service name cannot be empty".into());
            }
            if service.len() > MAX_SERVICE_NAME_LEN {
                return Err(format!(
                    "Service name too long (max {} characters)",
                    MAX_SERVICE_NAME_LEN
                ));
            }
            if !service
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return Err("Service name contains invalid characters".into());
            }
        }
        IpcRequest::SetPanicLevel { level, .. } => {
            if level.is_empty() {
                return Err("Panic level cannot be empty".into());
            }
            if level.len() > 16 {
                return Err("Panic level too long".into());
            }
        }
        _ => {}
    }
    Ok(())
}

fn service_name_from_str(s: &str) -> Option<ServiceName> {
    match s.to_lowercase().as_str() {
        "tor" => Some(ServiceName::Tor),
        "obfs4proxy" | "obfs4" => Some(ServiceName::Obfs4Proxy),
        "amneziawg" | "awg" => Some(ServiceName::AmneziaWG),
        "syncthing" => Some(ServiceName::Syncthing),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_request_rejects_empty_service() {
        let req = IpcRequest::StartService { service: "".into() };
        assert!(
            validate_request(&req).is_err(),
            "empty service name rejected"
        );
    }

    #[test]
    fn test_validate_request_rejects_long_service() {
        let long = "a".repeat(MAX_SERVICE_NAME_LEN + 1);
        let req = IpcRequest::StartService { service: long };
        assert!(
            validate_request(&req).is_err(),
            "long service name rejected"
        );
    }

    #[test]
    fn test_validate_request_rejects_invalid_chars() {
        let req = IpcRequest::StartService {
            service: "tor; rm -rf /".into(),
        };
        assert!(validate_request(&req).is_err(), "shell chars rejected");

        let req = IpcRequest::StartService {
            service: "tor\n".into(),
        };
        assert!(validate_request(&req).is_err(), "newlines rejected");

        let req = IpcRequest::StartService {
            service: "../tor".into(),
        };
        assert!(validate_request(&req).is_err(), "path traversal rejected");
    }

    #[test]
    fn test_validate_request_accepts_valid_service() {
        let req = IpcRequest::StartService {
            service: "tor".into(),
        };
        assert!(validate_request(&req).is_ok(), "valid service accepted");

        let req = IpcRequest::StartService {
            service: "obfs4proxy".into(),
        };
        assert!(validate_request(&req).is_ok(), "obfs4proxy accepted");
    }

    #[test]
    fn test_validate_panic_level_rejects_empty() {
        let req = IpcRequest::SetPanicLevel {
            level: "".into(),
            confirmation: None,
        };
        assert!(
            validate_request(&req).is_err(),
            "empty panic level rejected"
        );
    }

    #[test]
    fn test_validate_panic_level_rejects_long() {
        let req = IpcRequest::SetPanicLevel {
            level: "super-duper-extra-nuclear-mode".into(),
            confirmation: None,
        };
        assert!(validate_request(&req).is_err(), "long panic level rejected");
    }

    #[test]
    fn test_service_name_from_str_maps_correctly() {
        assert_eq!(service_name_from_str("tor"), Some(ServiceName::Tor));
        assert_eq!(service_name_from_str("TOR"), Some(ServiceName::Tor));
        assert_eq!(
            service_name_from_str("obfs4proxy"),
            Some(ServiceName::Obfs4Proxy)
        );
        assert_eq!(service_name_from_str("awg"), Some(ServiceName::AmneziaWG));
        assert_eq!(
            service_name_from_str("syncthing"),
            Some(ServiceName::Syncthing)
        );
        assert_eq!(service_name_from_str("nonexistent"), None);
    }

    #[test]
    fn test_ipc_response_serialization() {
        let resp = IpcResponse::Error {
            message: "test error".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("test error"), "error serialized");
    }
}
