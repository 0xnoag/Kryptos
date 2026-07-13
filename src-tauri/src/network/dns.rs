use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::daemon::config::DnsConfig;

const DNS_HEADER_SIZE: usize = 12;
const MAX_DNS_PACKET: usize = 512;

pub struct DnsHijacker {
    config: DnsConfig,
    upstream_socket: Option<UdpSocket>,
    listener_socket: Option<UdpSocket>,
    cache: Arc<RwLock<lru::LruCache<Vec<u8>, Vec<u8>>>>,
}

impl DnsHijacker {
    pub fn new(config: DnsConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(config.cache_size).unwrap_or(std::num::NonZeroUsize::new(4096).unwrap()),
            ))),
            config,
            upstream_socket: None,
            listener_socket: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let bind_addr: SocketAddr = format!("{}:{}", self.config.bind_address, self.config.bind_port)
            .parse()
            .context("Invalid DNS bind address")?;

        let listener = UdpSocket::bind(bind_addr).await?;
        info!("DNS hijacker listening on {}", bind_addr);

        let upstream = UdpSocket::bind("0.0.0.0:0").await?;
        let upstream_addr: SocketAddr = format!("{}:53", self.config.upstream)
            .parse()
            .context("Invalid upstream DNS address")?;

        self.listener_socket = Some(listener);
        self.upstream_socket = Some(upstream);

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        let listener = self
            .listener_socket
            .as_ref()
            .context("DNS hijacker not started")?;
        let upstream = self
            .upstream_socket
            .as_ref()
            .context("DNS hijacker not started")?;
        let upstream_addr: SocketAddr = format!("{}:53", self.config.upstream)
            .parse()
            .context("Invalid upstream DNS address")?;
        let cache = self.cache.clone();

        let mut buf = vec![0u8; MAX_DNS_PACKET];

        loop {
            match listener.recv_from(&mut buf).await {
                Ok((n, src)) => {
                    let query = buf[..n].to_vec();
                    let cache_hit = {
                        let mut cache = cache.write().await;
                        cache.get(&query).cloned()
                    };

                    let cache_clone = cache.clone();
                    let upstream_clone = upstream.clone();
                    let upstream_addr_clone = upstream_addr;

                    if let Some(cached_response) = cache_hit {
                        let _ = listener.send_to(&cached_response, src).await;
                        info!("DNS cache hit for {}", src);
                    } else {
                        tokio::spawn(async move {
                            match upstream_clone
                                .send_to(&query, upstream_addr_clone)
                                .await
                            {
                                Ok(_) => {
                                    let mut resp = vec![0u8; MAX_DNS_PACKET];
                                    match upstream_clone.recv_from(&mut resp).await {
                                        Ok((rn, _)) => {
                                            let response = resp[..rn].to_vec();
                                            let mut cache = cache_clone.write().await;
                                            cache.put(query, response.clone());
                                            let _ = listener.send_to(&response, src).await;
                                        }
                                        Err(e) => {
                                            error!("DNS upstream recv error: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("DNS upstream send error: {e}");
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("DNS listener recv error: {e}");
                }
            }
        }
    }

    pub async fn apply_system_dns(&self) -> Result<()> {
        let address = format!("{}:{}", self.config.bind_address, self.config.bind_port);
        let mut cmd = tokio::process::Command::new("resolvectl");
        cmd.args(["dns", "lo", &address]);
        let output = cmd.output().await?;
        if output.status.success() {
            info!("System DNS set to local hijacker at {}", address);
        } else {
            warn!(
                "Failed to set system DNS: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut cmd = tokio::process::Command::new("resolvectl");
        cmd.args(["domain", "lo", "~."]);
        let _ = cmd.output().await;

        Ok(())
    }

    pub async fn restore_system_dns(&self) -> Result<()> {
        let mut cmd = tokio::process::Command::new("resolvectl");
        cmd.args(["dns", "lo", &self.config.upstream]);
        let _ = cmd.output().await;
        info!("System DNS restored to upstream {}", self.config.upstream);
        Ok(())
    }
}
