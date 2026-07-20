use anyhow::Context;
use clap::Parser;
use endpoint_privacy_suite::firewall::panic::PanicLevel;
use endpoint_privacy_suite::{daemon, network, security};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[cfg(unix)]
async fn signal_sigterm() {
    let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Failed to register SIGTERM handler");
    stream.recv().await;
}

#[cfg(not(unix))]
async fn signal_sigterm() {
    use std::future::pending;
    pending().await
}

#[derive(Parser)]
#[command(name = "endpoint-privacy-daemon")]
#[command(about = "High-performance Endpoint Privacy Suite Daemon for Kali Linux")]
struct Cli {
    #[arg(short, long, default_value = "/etc/endpoint-privacy")]
    config_dir: String,

    // Password is read from EPS_PASSWORD env var or config_dir/password file only.
    // Never from CLI args (prevents /proc/PID/cmdline leakage).
    #[arg(short, long, help = "Run in foreground (default: daemonize)")]
    foreground: bool,

    #[arg(
        long = "verify-signatures",
        help = "Verify SHA-256 hashes of external binaries (tor, obfs4proxy, awg, syncthing) against .hashes file before starting"
    )]
    verify_signatures: bool,

    #[arg(
        long = "strict-verification",
        help = "Fail if any binary hash is missing from .hashes file (implies --verify-signatures)"
    )]
    strict_verification: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,endpoint_privacy_suite=debug")),
        )
        .init();

    let cli = Cli::parse();

    if !nix::unistd::getuid().is_root() {
        eprintln!("ERROR: Endpoint Privacy Suite must be run as root.");
        eprintln!(
            "Please run with: sudo {}",
            std::env::args().next().unwrap_or_default()
        );
        std::process::exit(1);
    }

    let strict = cli.strict_verification;
    let do_verify = cli.strict_verification || cli.verify_signatures;

    // Pre-flight: verify all configured binary hashes before starting daemon
    let hashes_path = std::path::Path::new(&cli.config_dir).join(".hashes");
    let _verifier = if do_verify {
        match security::verify::BinaryVerifier::from_hashes_file(&hashes_path, strict) {
            Ok(v) => {
                tracing::info!("Binary hash verification enabled (strict: {strict})");
                let failures = v.verify_all();
                if !failures.is_empty() {
                    for (path, reason) in &failures {
                        tracing::error!("Pre-flight integrity failure — {path}: {reason}");
                    }
                    anyhow::bail!(
                        "{} binary integrity check(s) failed — refusing to start",
                        failures.len()
                    );
                }
                Some(v)
            }
            Err(e) => {
                tracing::error!("Failed to load .hashes file for verification: {e}");
                if strict {
                    anyhow::bail!("Strict verification requested but .hashes file is invalid");
                }
                None
            }
        }
    } else {
        None
    };

    let password = load_password(&cli.config_dir)?;
    let daemon =
        endpoint_privacy_suite::EndpointPrivacyDaemon::new(&cli.config_dir, &password, strict)
            .await?;
    // Zeroize password after use
    drop(password);

    let pm = daemon.process_manager.clone();
    let pe = daemon.panic_engine.clone();
    let kill_on_exit = daemon.config.kill_switch_on_exit;

    // Create shutdown channel for graceful IPC-initiated shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    // Start IPC server with cloned Arcs (no outer lock)
    let ipc = daemon::ipc::IpcServer::new(
        &daemon.config.ipc_socket_path,
        pm.clone(),
        pe.clone(),
        shutdown_tx,
    )?;

    let ipc_handle = tokio::spawn(async move {
        if let Err(e) = ipc.run().await {
            tracing::error!("IPC server error: {e}");
        }
    });

    // Apply IPv6 leak block at startup
    if let Err(e) = network::routing::RouteManager::block_ipv6_leaks().await {
        tracing::warn!("Failed to block IPv6 leaks on startup: {e}");
    }

    tracing::info!(
        "Endpoint Privacy Suite daemon running (foreground: {})",
        cli.foreground
    );

    // Wait for shutdown signal (SIGINT, SIGTERM, IPC shutdown request, or IPC exit)
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C (SIGINT) shutdown signal");
        }
        _ = signal_sigterm() => {
            tracing::info!("Received SIGTERM shutdown signal");
        }
        _ = async { shutdown_rx.changed().await } => {
            if *shutdown_rx.borrow() {
                tracing::info!("Received IPC Shutdown request, initiating graceful shutdown");
            } else {
                tracing::info!("Shutdown channel closed, initiating shutdown");
            }
        }
        _ = async { ipc_handle.await.unwrap_or(()) } => {
            tracing::info!("IPC server exited, shutting down");
        }
    }

    // Consistent lock ordering: ProcessManager first, then PanicEngine
    if kill_on_exit {
        let mut pe_lock = pe.write().await;
        match pe_lock.activate(PanicLevel::Nuclear).await {
            Ok(_) => tracing::info!("Nuclear kill switch activated on shutdown"),
            Err(e) => tracing::warn!("Failed to activate kill switch on shutdown: {e}"),
        }
    }

    {
        let mut pm_lock = pm.write().await;
        let _ = pm_lock.stop_all().await;
    }

    tracing::info!("Shutdown complete");
    Ok(())
}

/// Load encryption password from EPS_PASSWORD env var or {config_dir}/password file.
/// Never from CLI args to avoid /proc/PID/cmdline leakage.
fn load_password(config_dir: &str) -> anyhow::Result<String> {
    if let Ok(pw) = std::env::var("EPS_PASSWORD") {
        if !pw.is_empty() {
            return Ok(pw);
        }
    }
    let pw_path = PathBuf::from(config_dir).join("password");
    if pw_path.exists() {
        let pw = std::fs::read_to_string(&pw_path)
            .context(format!(
                "Failed to read password file: {}",
                pw_path.display()
            ))?
            .trim()
            .to_string();
        if !pw.is_empty() {
            return Ok(pw);
        }
    }
    anyhow::bail!(
        "No password found. Set EPS_PASSWORD env var or create {}",
        pw_path.display()
    );
}
