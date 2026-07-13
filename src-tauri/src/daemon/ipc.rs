use crate::firewall::panic::{PanicEngine, PanicLevel, PanicStatus};
use crate::daemon::engine::{ProcessManager, ServiceName, ServiceInfo, ServiceStatus};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

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

impl From<anyhow::Error> for IpcResponse {
    fn from(e: anyhow::Error) -> Self {
        IpcResponse::Error {
            message: format!("{:#}", e),
        }
    }
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
        if std::path::Path::new(socket_path).exists() {
            std::fs::remove_file(socket_path)?;
        }

        let dir = std::path::Path::new(socket_path).parent().unwrap();
        std::fs::create_dir_all(dir)?;

        let listener = UnixListener::bind(socket_path)?;
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

fn service_name_from_str(s: &str) -> Option<ServiceName> {
    match s.to_lowercase().as_str() {
        "tor" => Some(ServiceName::Tor),
        "obfs4proxy" | "obfs4" => Some(ServiceName::Obfs4Proxy),
        "amneziawg" | "awg" => Some(ServiceName::AmneziaWG),
        "syncthing" => Some(ServiceName::Syncthing),
        _ => None,
    }
}
