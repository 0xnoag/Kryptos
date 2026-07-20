use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, trace, warn};

use crate::daemon::config::DnsConfig;

const MAX_DNS_PACKET: usize = 512;
const DNS_TIMEOUT_SECS: u64 = 5;

/// Resolve a raw DNS query via DNS-over-HTTPS (RFC 8484).
/// Uses ureq to POST to the configured DoH endpoint.
/// Must be called via spawn_blocking since ureq is a blocking client.
fn dns_over_https(doh_url: &str, query: &[u8]) -> Result<Vec<u8>> {
    let response = ureq::post(doh_url)
        .set("Content-Type", "application/dns-message")
        .set("Accept", "application/dns-message")
        .send_bytes(query)
        .map_err(|e| anyhow::anyhow!("DoH request failed: {e}"))?;

    let status = response.status();
    if status != 200 {
        anyhow::bail!("DoH server returned HTTP {status}");
    }

    let mut body = Vec::new();
    use std::io::Read;
    response
        .into_reader()
        .read_to_end(&mut body)
        .context("Failed to read DoH response body")?;

    if body.is_empty() {
        anyhow::bail!("DoH server returned empty response");
    }

    Ok(body)
}

/// Local plain-UDP DNS forwarder.
///
/// **SECURITY NOTICE**: This forwards DNS queries to the upstream resolver
/// over *unencrypted UDP* (port 53). This means DNS queries are visible
/// to the local network and ISP in plaintext, even though they are routed
/// through the local DNS proxy. The `doh_url` config field is reserved for
/// future DoH (DNS-over-HTTPS) implementation and is currently unused.
///
/// To prevent DNS leaks when the kill switch is active:
/// - The nftables rules allow outbound DNS (ports 53, 853) so the proxy can reach
///   the upstream resolver even during Hard kill-switch mode.
/// - All local applications should be pointed to the local bind address
///   (127.0.0.1:53) via systemd-resolved or resolvconf integration.
pub struct DnsHijacker {
    config: DnsConfig,
    upstream_socket: Option<UdpSocket>,
    listener_socket: Option<UdpSocket>,
    cache: Arc<RwLock<lru::LruCache<Vec<u8>, Vec<u8>>>>,
}

impl DnsHijacker {
    pub fn new(config: DnsConfig) -> Self {
        let cache_size = if config.cache_size == 0 {
            4096
        } else {
            config.cache_size
        };
        Self {
            cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_size)
                    .unwrap_or_else(|| std::num::NonZeroUsize::new(4096).unwrap()),
            ))),
            config,
            upstream_socket: None,
            listener_socket: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let bind_addr: SocketAddr =
            format!("{}:{}", self.config.bind_address, self.config.bind_port)
                .parse()
                .context("Invalid DNS bind address")?;

        let listener = UdpSocket::bind(bind_addr).await.with_context(|| {
            format!("Failed to bind DNS listener to {bind_addr} (try running as root)")
        })?;

        info!(
            "DNS forwarder listening on {} (plain UDP to {})",
            bind_addr, self.config.upstream
        );

        let upstream = UdpSocket::bind("127.0.0.1:0").await?;

        self.listener_socket = Some(listener);
        self.upstream_socket = Some(upstream);

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        let listener = self
            .listener_socket
            .as_ref()
            .context("DNS forwarder not started")?;
        let upstream = self
            .upstream_socket
            .as_ref()
            .context("DNS forwarder not started")?;
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

                    if let Some(cached_response) = cache_hit {
                        if let Err(e) = listener.send_to(&cached_response, src).await {
                            trace!("Failed to send cached DNS response to {}: {e}", src);
                        }
                        trace!("DNS cache hit for {}", src);
                    } else {
                        let cache_clone = cache.clone();
                        let upstream_clone = upstream.clone();
                        let upstream_addr_clone = upstream_addr;
                        let listener_for_spawn = match listener.try_clone() {
                            Ok(s) => s,
                            Err(e) => {
                                warn!("DNS listener clone failed (FD exhaustion?): {e}");
                                continue;
                            }
                        };
                        let doh_url = self.config.doh_url.clone();
                        let use_doh = !doh_url.is_empty();

                        tokio::spawn(async move {
                            let response = if use_doh {
                                match timeout(
                                    Duration::from_secs(DNS_TIMEOUT_SECS),
                                    tokio::task::spawn_blocking({
                                        let doh_url = doh_url.clone();
                                        let query = query.clone();
                                        move || dns_over_https(&doh_url, &query)
                                    }),
                                )
                                .await
                                {
                                    Ok(Ok(Ok(data))) => Some(data),
                                    Ok(Ok(Err(e))) => {
                                        warn!("DoH request failed: {e}");
                                        None
                                    }
                                    Ok(Err(_)) => {
                                        warn!("DoH blocking task panicked");
                                        None
                                    }
                                    Err(_) => {
                                        warn!("DoH request timed out after {DNS_TIMEOUT_SECS}s");
                                        None
                                    }
                                }
                            } else {
                                // Fallback to plain UDP
                                match timeout(
                                    Duration::from_secs(DNS_TIMEOUT_SECS),
                                    upstream_clone.send_to(&query, upstream_addr_clone),
                                )
                                .await
                                {
                                    Ok(Ok(_)) => {
                                        let mut resp = vec![0u8; MAX_DNS_PACKET];
                                        match timeout(
                                            Duration::from_secs(DNS_TIMEOUT_SECS),
                                            upstream_clone.recv_from(&mut resp),
                                        )
                                        .await
                                        {
                                            Ok(Ok((rn, _))) => Some(resp[..rn].to_vec()),
                                            Ok(Err(e)) => {
                                                warn!("DNS upstream recv error: {e}");
                                                None
                                            }
                                            Err(_) => {
                                                warn!("DNS upstream recv timed out after {DNS_TIMEOUT_SECS}s");
                                                None
                                            }
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        warn!("DNS upstream send error: {e}");
                                        None
                                    }
                                    Err(_) => {
                                        warn!(
                                            "DNS upstream send timed out after {DNS_TIMEOUT_SECS}s"
                                        );
                                        None
                                    }
                                }
                            };

                            if let Some(data) = response {
                                let mut cache = cache_clone.write().await;
                                cache.put(query, data.clone());
                                if let Err(e) = listener_for_spawn.send_to(&data, src).await {
                                    trace!("Failed to send DNS response to {}: {e}", src);
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
            info!("System DNS set to local forwarder at {}", address);
        } else {
            warn!(
                "Failed to set system DNS: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut cmd = tokio::process::Command::new("resolvectl");
        cmd.args(["domain", "lo", "~."]);
        if let Err(e) = cmd.output().await {
            warn!("Failed to set resolvectl domain: {e}");
        }

        Ok(())
    }

    pub async fn restore_system_dns(&self) -> Result<()> {
        let mut cmd = tokio::process::Command::new("resolvectl");
        cmd.args(["dns", "lo", &self.config.upstream]);
        if let Err(e) = cmd.output().await {
            warn!("Failed to restore system DNS: {e}");
        }
        info!("System DNS restored to upstream {}", self.config.upstream);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_config_defaults() {
        let config = crate::daemon::config::DnsConfig {
            upstream: "1.1.1.1".into(),
            doh_url: "https://cloudflare-dns.com/dns-query".into(),
            bind_address: "127.0.0.1".into(),
            bind_port: 53,
            cache_size: 4096,
        };
        assert_eq!(config.upstream, "1.1.1.1");
        assert_eq!(config.bind_port, 53);
    }

    #[test]
    fn test_dns_hijacker_initializes_cache() {
        let config = crate::daemon::config::DnsConfig {
            upstream: "1.1.1.1".into(),
            doh_url: String::new(),
            bind_address: "127.0.0.1".into(),
            bind_port: 53,
            cache_size: 0, // should fall back to default 4096
        };
        let hijacker = DnsHijacker::new(config);
        // Cache should be initialized (we can't inspect it directly, but construction should not panic)
        assert!(hijacker.upstream_socket.is_none());
        assert!(hijacker.listener_socket.is_none());
    }
}
