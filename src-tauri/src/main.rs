use clap::Parser;
use endpoint_privacy_suite::EndpointPrivacyDaemon;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "endpoint-privacy-daemon")]
#[command(about = "High-performance Endpoint Privacy Suite Daemon for Kali Linux")]
struct Cli {
    #[arg(short, long, default_value = "/etc/endpoint-privacy")]
    config_dir: String,

    #[arg(short, long, env = "EPS_PASSWORD")]
    password: String,

    #[arg(short, long)]
    foreground: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,endpoint_privacy_suite=debug")))
        .init();

    let cli = Cli::parse();

    if !nix::unistd::getuid().is_root() {
        eprintln!("ERROR: Endpoint Privacy Suite must be run as root.");
        eprintln!("Please run with: sudo {}", std::env::args().next().unwrap_or_default());
        std::process::exit(1);
    }

    let mut daemon = EndpointPrivacyDaemon::new(&cli.config_dir, &cli.password).await?;
    daemon.start_ipc_server().await?;

    let daemon = Arc::new(tokio::sync::RwLock::new(daemon));

    let daemon_clone = daemon.clone();
    tokio::spawn(async move {
        let daemon = daemon_clone.read().await;
        if let Err(e) = daemon.run_ipc().await {
            tracing::error!("IPC server error: {e}");
        }
    });

    let daemon_clone2 = daemon.clone();
    ctrlc::set_handler(move || {
        tracing::info!("Received shutdown signal");
        let daemon = daemon_clone2.clone();
        tokio::spawn(async move {
            let mut daemon = daemon.write().await;
            let _ = daemon.shutdown().await;
            std::process::exit(0);
        });
    }).expect("Failed to set Ctrl+C handler");

    tracing::info!("Endpoint Privacy Suite daemon running");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
