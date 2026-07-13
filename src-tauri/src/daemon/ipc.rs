use crate::daemon::engine::{ProcessManager, ServiceInfo, ServiceName, ServiceStatus};
use crate::firewall::panic::{PanicEngine, PanicLevel, PanicStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

const MAX_PAYLOAD_BYTES: usize = 65536;
const MAX_SERVICE_NAME_LEN: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcRequest {
    GetStatus,
    StartService { service: String },
    StopService { service: String },
    RestartService { service: String },
    SetPanicLevel { level: String },
    GetPanicStatus,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum IpcResponse {
    Ok { message: String },
    Error { message: String },
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
}

impl IpcServer {
    pub fn new(
        socket_path: &str,
        process_manager: Arc<RwLock<ProcessManager>>,
        panic_engine: Arc<RwLock<PanicEngine>>,
    ) -> Result<Self> {
        let sock_path = Path::new(socket_path);
        if sock_path.exists() {
            std::fs::remove_file(socket_path)
                .context("Failed to remove existing socket file")?;
        }

        let dir = sock_path.parent().unwrap();
        std::fs::create_dir_all(dir)
            .context("Failed to create socket directory")?;

        let listener = UnixListener::bind(socket_path)
            .context("Failed to bind Unix socket")?;

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
        })
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let pm = self.process_manager.clone();
                    let pe = self.panic_engine.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, pm, pe).await {
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
    }

    async fn handle_client(
        mut stream: UnixStream,
        process_manager: Arc<RwLock<ProcessManager>>,
        panic_engine: Arc<RwLock<PanicEngine>>,
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

            let response = Self::process_request(request, &process_manager, &panic_engine).await;
            let mut buf = serde_json::to_vec(&response)?;
            buf.push(b'\n');
            writer.write_all(&buf).await?;
        }

        Ok(())
    }

    async fn process_request(
        request: IpcRequest,
        pm: &Arc<RwLock<ProcessManager>>,
        pe: &Arc<RwLock<PanicEngine>>,
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
                let _ = pm.stop(name).await;
                match pm.start(name).await {
                    Ok(()) => IpcResponse::Ok {
                        message: format!("{service} restarted"),
                    },
                    Err(e) => IpcResponse::Error {
                        message: format!("Failed to restart {service}: {e}"),
                    },
                }
            }
            IpcRequest::SetPanicLevel { level } => {
                let panic_level = match level.to_lowercase().as_str() {
                    "off" => PanicLevel::Off,
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
                let mut pe = pe.write().await;
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
            IpcRequest::Shutdown => IpcResponse::Ok {
                message: "Shutting down".into(),
            },
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
            if !service.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
                return Err("Service name contains invalid characters".into());
            }
        }
        IpcRequest::SetPanicLevel { level } => {
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
