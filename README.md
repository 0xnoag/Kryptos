# Kryptos — Endpoint Privacy Suite

**Kryptos** is a high-performance, low-level endpoint privacy daemon for Kali Linux that integrates Tor (with obfs4), AmneziaWG (obfuscated WireGuard), and Syncthing into a unified security stack with a kernel-level kill switch, intelligent split routing, and a browser-based web UI.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Browser (Web UI — React + Vite)             │
│           http://localhost:8080 (read-only)              │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP (Bearer token + Origin check)
┌──────────────────────▼──────────────────────────────────┐
│                 Rust Daemon (tokio async)                 │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐           │
│  │  axum    │  │  Unix    │  │ Shared      │           │
│  │ HTTP API │  │ Socket   │  │ Business    │           │
│  │(read-only)│  │IPC (full)│  │ Logic       │           │
│  └────┬─────┘  └────┬─────┘  └──────┬──────┘           │
│       │              │               │                   │
│  ┌────▼──────────────▼───────────────▼────────────────┐ │
│  │  Process Manager  │  nftables Kill Switch          │ │
│  │  Traffic Classifier  │  DNS Forwarder  │  Config   │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

> **Security documentation**: See [THREAT_MODEL.md](THREAT_MODEL.md) for adversary scope and [SECURITY.md](SECURITY.md) for current privilege model, known limitations, and vulnerability reporting.

## Security Model: HTTP API

All state-changing operations (start/stop services, panic levels, shutdown) are exposed **only** via the Unix socket (`0700` permissions), never via HTTP. The HTTP API is **read-only by design**.

- **Bearer token**: randomly generated per daemon start, delivered only via initial HTML response (`<meta name="api-token">`). Not recoverable via any API endpoint.
- **Origin check**: defense-in-depth against browser-based DNS rebinding attacks. NOT a substitute for the Bearer token — a non-browser client can spoof Origin freely.
- **Token lifetime**: no expiry. A daemon restart invalidates all previous tokens; the frontend auto-reloads on HTTP 401.
- **127.0.0.1 bind only**: the HTTP server never binds to 0.0.0.0.

## Current Status / Known Issues

| Feature | Status | Notes |
|---------|--------|-------|
| nftables kill switch | ✅ Implemented | 3 levels (soft/hard/nuclear), atomic rule loading |
| Panic engine | ✅ Implemented | Real state tracking, interface allowlist |
| Process lifecycle management | ✅ Implemented | Exponential backoff, max-restart circuit breaker |
| Encrypted config | ✅ Implemented | AES-256-GCM + Argon2id, zeroized on drop |
| Unix socket IPC | ✅ Implemented | `0700` permissions, sole mutation channel |
| Tor service management | ✅ Implemented | |
| AmneziaWG service management | ✅ Implemented | |
| obfs4proxy management | ✅ Implemented | |
| Syncthing management | ✅ Implemented | |
| MAC spoofing | ✅ Implemented | Locally-administered bit set correctly |
| Traffic classifier | ✅ Implemented | Informational only — enforcement via nftables |
| Routing / sysctl helpers | ✅ Implemented | IPv6 blocking, policy routing |
| Browser web UI | ✅ Implemented | Read-only, 5 pages, 2s polling, Bearer auth |
| DNS encryption (DoH/DoT) | ❌ Not implemented | DNS is **plain UDP** — known leak. Config has `doh_url` field reserved for future use. |
| nftables rule persistence across reboot | ❌ Not implemented | Rules survive daemon crash (kernel state) but not reboot. Requires systemd `ExecStartPre` or a oneshot service. |
| Privilege separation | ❌ Not implemented | Daemon runs entirely as root. See SECURITY.md for proposed split. |
| IPv6 tunnel support | ❌ Not implemented | IPv6 is disabled via sysctl at daemon start. Nftables kill switch drops IPv6 by default. |
| Automatic service restart on crash | ✅ Implemented | Exponential backoff (2s base, 60s max), max-restarts configurable per service |
| Graceful shutdown with kill switch | ✅ Implemented | Nuclear panic activated on shutdown if configured |

### Boot / Reboot Ordering

1. **Kernel boots** — no nftables rules active, all traffic allowed
2. **systemd starts network** — brief window of unprotected connectivity
3. **kryptos-daemon.service starts** (should be ordered early via `After=network-pre.target` with `Wants=network.target`):
   - Detects any pre-existing nftables state
   - Applies kill switch rules (if configured for autostart)
   - Starts tunnel services (tor, awg)
4. **nftables rules are in-kernel** — they survive daemon crash or panic, but NOT reboot
5. On **clean shutdown**: daemon activates nuclear kill switch, then stops tunnels

> ⚠️ Nftables rules are not persisted to disk. After a reboot, there is a window before the daemon starts where no firewall rules are active. Use systemd ordering (`After=network-pre.target`) and consider a `network-pre` oneshot service to restore nftables rules early in boot.

### Stack
|-------|-----------|
| Backend | Rust (tokio async, axum, nix, libc) |
| Frontend | React 18 + Vite + TailwindCSS |
| API | Axum HTTP (read-only) + Unix socket JSON-RPC (mutations) |
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
| DNS (53/853) | → Local DNS forwarder (`127.0.0.1:53`, plain UDP upstream) |
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

### 5. DNS Forwarding (`network/dns.rs`)
- Local DNS listener on `127.0.0.1:53`
- Forwards queries to upstream resolver over **plain UDP** (port 53)
- LRU cache (configurable, default 4096 entries)
- Overrides system DNS via `resolvectl`

> ⚠️ **DNS is NOT encrypted**. Queries are forwarded over plain UDP to the upstream resolver (default: `1.1.1.1`). DNS-over-HTTPS (DoH) is not yet implemented — the `doh_url` config field is reserved for future use. Until DoH is implemented, DNS queries are visible in plaintext to the ISP and local network. The kill switch ensures all *other* traffic is tunneled, but DNS itself leaks.

### 6. MAC Spoofing (`network/mac.rs`)
- Generates random MACs with locally administered bit set
- Rotates on configurable interval (default 10 minutes)
- Excludes loopback and user-specified interfaces
- Uses `ip link` commands to apply

### 7. Unix Socket IPC (`daemon/ipc.rs`) — Mutation Channel
JSON-RPC over Unix socket at `/run/endpoint-privacy/ipc.sock` (`0700` permissions):

| Request | Description |
|---------|-------------|
| `GetStatus` | All services + panic status |
| `StartService` | Start tor, awg, obfs4, syncthing |
| `StopService` | Graceful stop with SIGTERM |
| `RestartService` | Stop then start |
| `SetPanicLevel` | off / soft / hard / nuclear |
| `GetPanicStatus` | Current panic state |
| `Shutdown` | Graceful daemon shutdown |

### 8. HTTP API (`daemon/http_api.rs`) — Read-Only Channel
REST over HTTP on `127.0.0.1:PORT` (default 8080), Bearer token required:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Serves web UI (index.html with token injected) |
| `/api/status` | GET | All services + panic status (JSON) |
| `/api/panic` | GET | Current panic status (JSON) |
| `/*` | GET | SPA fallback (static files or index.html) |

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

### Quick Install (Recommended)

Clone and install everything — daemon, systemd service, desktop icon, and CLI — in one step:

```bash
git clone https://github.com/0xnoag/Kryptos.git
cd Kryptos
sudo make install
```

This runs `install/install.sh` which:
1. Builds the Rust daemon (release mode)
2. Builds the frontend (as `kali` user)
3. Copies daemon to `/usr/local/lib/kryptos/endpoint-privacy-suite`
4. Installs CLI to `/usr/local/bin/kryptos`
5. Installs systemd service (`kryptos.service`)
6. Generates a random 48-character password and saves it to `/etc/endpoint-privacy/env`
7. Installs desktop entry for all users
8. Enables and starts the systemd service

To rebuild from scratch after a pull:

```bash
sudo make fresh
```

(Stops daemon, uninstalls, cleans build artifacts, rebuilds, installs, restarts.)

### Manual Build

```bash
# Build the daemon
cd src-tauri
cargo build --release
cd ..

# Build the frontend
npm install
npm run build
```

---

## Usage

### CLI (`kryptos`)

After installation, control everything with the `kryptos` command (no sudo needed for most commands):

| Command | Description |
|---------|-------------|
| `kryptos status` | Show daemon + all service status |
| `kryptos start` | Start the daemon (systemd) |
| `kryptos stop` | Stop the daemon |
| `kryptos restart` | Restart the daemon |
| `kryptos service start <Service>` | Start a service |
| `kryptos service stop <Service>` | Stop a service |
| `kryptos panic <Level>` | Set kill switch level |
| `kryptos ui` | Open web UI in browser |
| `kryptos explain` | Beginner-friendly explanation of all features |

Services: `Tor`, `AmneziaWG`, `Syncthing`, `Obfs4Proxy`

Panic levels: `Off` | `Soft` | `Hard` | `Nuclear`

### Web UI

The UI provides:
- Real-time service status with auto-refresh (2s polling)
- Visual nftables ruleset display
- Split routing classifier overview
- Configuration viewer

Open it with: `kryptos ui`

> **Note**: The web UI is **read-only**. All state-changing operations (start/stop services, panic levels) must be done via the `kryptos` CLI.

### Daemon Flags (low-level)

```bash
endpoint-privacy-suite --help

Usage: endpoint-privacy-suite [OPTIONS]

Options:
  -c, --config-dir <DIR>      Config directory [default: /etc/endpoint-privacy]
  -f, --foreground             Run in foreground
      --verify-signatures      Verify SHA-256 hashes of external binaries
      --strict-verification    Fail if any binary hash is missing
      --http-port <PORT>       Web UI port [default: 8080]
  -h, --help                   Print help
```

### Manual IPC (advanced)

All mutations go through the Unix socket directly:

```bash
# Start a service
echo '{"type":"StartService","payload":"Tor"}' | sudo nc -U /run/endpoint-privacy/ipc.sock

# Set kill switch
echo '{"type":"SetPanic","payload":"Nuclear"}' | sudo nc -U /run/endpoint-privacy/ipc.sock

# Get full status
echo '{"type":"GetStatus"}' | sudo nc -U /run/endpoint-privacy/ipc.sock
```

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
│   ├── ipc.rs               # Unix socket JSON-RPC server (mutations)
│   └── http_api.rs          # Axum HTTP server (read-only + static files)
├── network/
│   ├── classifier.rs        # TCP/UDP/DNS traffic classifier
│   ├── dns.rs               # Local DNS forwarder (plain UDP, DoH planned)
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
│   ├── Dashboard.tsx        # Status cards
│   ├── Services.tsx         # Service status (read-only)
│   ├── Firewall.tsx         # Kill switch display (read-only)
│   ├── Network.tsx          # Split routing display
│   └── Settings.tsx         # Configuration viewer
└── lib/
    ├── ipc.ts               # HTTP fetch to axum API
    └── daemon-context.tsx   # React state + polling
```

---

## Security Model

1. **Defense in Depth** — Compromise of one layer does not expose others
2. **Least Privilege** — Daemon runs as root (required for nftables), child processes drop privileges where possible. **Known gap**: privilege separation is not yet implemented — see [SECURITY.md](SECURITY.md).
3. **Memory Safety** — Rust with `unsafe` only for syscalls (documented in code)
4. **No Shell Injection** — All system commands use `Command::new()` with pre-split args, never `sh -c`
5. **Encrypted at Rest** — All config files use AES-256-GCM + Argon2id with explicit parameters. Keys are zeroized on drop.
6. **Zero-Leak Guarantee** — Kill switch installs nftables rules before any tunnel is brought up. Rules survive daemon crash (kernel state persists).
7. **Secure IPC** — Unix socket with `0700` permissions. Sole mutation channel. All state-changing operations are gated here.
8. **Read-Only HTTP API** — Axum server bound to `127.0.0.1`, Bearer token authentication, Origin header check (DNS rebinding protection). No mutation endpoints exposed.
9. **DNS is NOT encrypted** — Queries are forwarded as plain UDP. DoH is planned.

---

## License

MIT License — see [LICENSE](LICENSE) for details.
