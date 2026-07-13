# Kryptos — Endpoint Privacy Suite

**Kryptos** is a high-performance, low-level endpoint privacy daemon for Kali Linux that integrates Tor (with obfs4), AmneziaWG (obfuscated WireGuard), and Syncthing into a unified security stack with a kernel-level kill switch, intelligent split routing, and a desktop UI built on Tauri + React.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Desktop UI                      │
│  (React + Vite + TailwindCSS → Webview)                 │
└──────────────────────┬──────────────────────────────────┘
                       │ Unix Socket (JSON-RPC)
┌──────────────────────▼──────────────────────────────────┐
│               Rust Daemon (tokio async)                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  │
│  │   Tor    │  │ obfs4    │  │AmneziaWG │  │Syncth. │  │
│  │ + obfs4  │  │ Bridge   │  │ Tunnel   │  │  P2P   │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──┬─────┘  │
│       │              │             │           │         │
│  ┌────▼──────────────▼─────────────▼───────────▼──────┐ │
│  │           Process Lifecycle Manager                │ │
│  │   (auto-restart, health monitoring, max limits)    │ │
│  └────────────────────────┬──────────────────────────┘ │
│                           │                             │
│  ┌────────────────────────▼──────────────────────────┐ │
│  │            nftables Kill Switch                    │ │
│  │   Soft │ Hard │ Nuclear (zero-leak guarantee)     │ │
│  └───────────────────────────────────────────────────┘ │
│                           │                             │
│  ┌────────────────────────▼──────────────────────────┐ │
│  │   Traffic Classifier (Split Routing)               │ │
│  │   TCP → Tor · UDP → AmneziaWG · DNS → Local DoH  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust (tokio async, nix, libc) |
| Frontend | React 18 + Vite + TailwindCSS |
| Desktop | Tauri 2 (Rust-wrapped webview) |
| Messaging | Unix Socket JSON-RPC (IPC) |
| Firewall | nftables via netlink (`nft -f -`) |
| Config Encryption | AES-256-GCM + Argon2 KDF |
| Core Engines | tor, obfs4proxy, amneziawg-wireguard, syncthing |

---

## Features

### 1. Kernel-Level Kill Switch (`firewall/nftables.rs`)
Three panic levels that manipulate nftables at the kernel level via raw ruleset injection:

- **Soft** — Blocks new connections outside tunnel interfaces. Established connections persist. DNS (53/853) still resolves.
- **Hard** — Blocks all traffic except through `tun+`, `wg+`, `obfs+` interfaces. Full VPN kill switch.
- **Nuclear** — Complete blackout. Only loopback survives. Drops all interfaces, flushes DNS cache, purges system memory (drop_caches, swapoff).

### 2. Intelligent Split Routing (`network/classifier.rs`)
Traffic is classified by protocol and port:

| Traffic Type | Route |
|-------------|-------|
| TCP (all) | → Tor SOCKS5 (`127.0.0.1:9050`) |
| UDP (VoIP, gaming, streaming) | → AmneziaWG tunnel (`awg0`) |
| DNS (53/853) | → Local DoH resolver (`127.0.0.1:53`) |
| Local network | → Direct (bypass) |

VoIP/gaming ports auto-detected: 3478-3481 (STUN/TURN), 5000-6000, 1194-1195 (OpenVPN), 27015-27030, 4380, 16384-32767.

### 3. Process Lifecycle (`daemon/engine.rs`)
Manages four core binaries with automatic restart on failure:

- **tor** — TCP anonymization with obfs4 pluggable transport
- **obfs4proxy** — Traffic obfuscation bridge
- **awg** — AmneziaWG obfuscated WireGuard tunnel
- **syncthing** — P2P encrypted file synchronization

Restart limits prevent infinite crash loops (configurable per service).

### 4. Encrypted Configuration (`daemon/config.rs`)
All settings stored encrypted at rest:

- AES-256-GCM encryption with random 12-byte nonces
- Argon2 password-based key derivation (32-byte salt)
- Zeroize secure memory wiping on drop
- Password supplied via `EPS_PASSWORD` env variable or CLI flag

### 5. DNS Hijacking (`network/dns.rs`)
- Local UDP DNS listener on `127.0.0.1:53`
- Forwards queries to upstream (Cloudflare 1.1.1.1 by default)
- LRU cache (configurable, default 4096 entries)
- Overrides system DNS via `resolvectl`

### 6. MAC Spoofing (`network/mac.rs`)
- Generates random MACs with locally administered bit set
- Rotates on configurable interval (default 10 minutes)
- Excludes loopback and user-specified interfaces
- Uses `ip link` commands to apply

### 7. Secure IPC (`daemon/ipc.rs`)
JSON-RPC over Unix socket at `/run/endpoint-privacy/ipc.sock`:

| Request | Description |
|---------|-------------|
| `GetStatus` | All services + panic status |
| `StartService` | Start tor, awg, obfs4, syncthing |
| `StopService` | Graceful stop with SIGTERM |
| `RestartService` | Stop then start |
| `SetPanicLevel` | off / soft / hard / nuclear |
| `GetPanicStatus` | Current panic state |
| `Shutdown` | Graceful daemon shutdown |

---

## Installation

### Prerequisites (Kali Linux)

```bash
# Core binaries
sudo apt install -y tor obfs4proxy syncthing nftables iproute2 systemd-resolved

# AmneziaWG (from GitHub releases or build from source)
# https://github.com/amnezia-vpn/amneziawg-wireguard

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js for frontend
sudo apt install -y nodejs npm
```

### Build from Source

```bash
# Clone
git clone https://github.com/0xnoag/Kryptos.git
cd Kryptos

# Build the daemon
cd src-tauri
cargo build --release
cd ..

# Build the frontend
npm install
npm run build

# Or build the full Tauri desktop app
npx tauri build
```

### Deploy Daemon

```bash
# Create config directory
sudo mkdir -p /etc/endpoint-privacy

# Run daemon (root required for nftables + routing)
sudo EPS_PASSWORD="your-strong-password" ./target/release/endpoint-privacy-daemon

# Or as a systemd service
sudo cp ./contrib/endpoint-privacy.service /etc/systemd/system/
sudo systemctl enable --now endpoint-privacy
```

---

## Usage

### CLI Flags

```bash
endpoint-privacy-daemon --help

Usage: endpoint-privacy-daemon [OPTIONS] --password <PASSWORD>

Options:
  -c, --config-dir <DIR>   Config directory [default: /etc/endpoint-privacy]
  -p, --password <PASS>    Encryption password (also via EPS_PASSWORD env)
  -f, --foreground         Run in foreground
  -h, --help               Print help
```

### Desktop UI

```bash
# Via Tauri
npx tauri dev
```

The UI provides:
- Real-time service status with auto-refresh (2s polling)
- Start / stop / restart controls for each engine
- One-click panic level activation
- Visual nftables ruleset display
- Split routing classifier overview
- MAC spoofing trigger
- Configuration viewer

---

## Project Structure

```
src-tauri/src/
├── main.rs                  # CLI entry, root check, signal handling
├── lib.rs                   # Daemon orchestrator, module wiring
├── firewall/
│   ├── nftables.rs          # nftables rule construction & execution
│   └── panic.rs             # Panic engine (4 levels)
├── daemon/
│   ├── engine.rs            # Process lifecycle manager
│   ├── config.rs            # Encrypted config (AES-256-GCM + Argon2)
│   └── ipc.rs               # Unix socket JSON-RPC server
├── network/
│   ├── classifier.rs        # TCP/UDP/DNS traffic classifier
│   ├── dns.rs               # Local DoH resolver with cache
│   ├── mac.rs               # MAC address spoofing
│   └── routing.rs           # Policy routing + sysctl
├── crypto/
│   └── keys.rs              # Key derivation + encrypt/decrypt
└── utils/
    └── command.rs           # Secure subprocess execution

src/
├── App.tsx                  # Router (5 views)
├── components/Layout.tsx    # Sidebar navigation
├── pages/
│   ├── Dashboard.tsx        # Status cards + panic controls
│   ├── Services.tsx         # Service management
│   ├── Firewall.tsx         # Kill switch panel
│   ├── Network.tsx          # Split routing display
│   └── Settings.tsx         # Configuration viewer
└── lib/
    ├── ipc.ts               # Tauri IPC bridge
    └── daemon-context.tsx   # React state + polling
```

---

## Security Model

1. **Defense in Depth** — Compromise of one layer does not expose others
2. **Least Privilege** — Daemon runs as root (required for nftables), child processes drop privileges where possible
3. **Memory Safety** — Rust with `unsafe` only for syscalls (documented in code)
4. **No Shell Injection** — All system commands use `Command::new()` with pre-split args, never `sh -c`
5. **Encrypted at Rest** — All config files use AES-256-GCM with Argon2-derived keys
6. **Zero-Leak Guarantee** — Kill switch installs nftables rules before any tunnel is brought up
7. **Secure IPC** — Unix socket with `0700` permissions, no network exposure

---

## License

MIT License — see [LICENSE](LICENSE) for details.
