# Changelog

## fix/security-audit-pass-1 — 2026-07-13

### Critical Concurrency Fixes

#### `main.rs` — Eliminated outer lock deadlock (CRITICAL)
- **Removed `Arc<RwLock<EndpointPrivacyDaemon>>` outer wrapper** that caused two deadlocks:
  1. IPC accept loop held `daemon.read()` forever, preventing ctrl-c handler from acquiring `daemon.write()` (shutdown was unreachable, only SIGKILL worked).
  2. Lock-ordering violation: IPC took `ProcessManager(R)` then `PanicEngine(R)`, but shutdown took `PanicEngine(W)` then `ProcessManager(W)` — textbook ABBA deadlock.
- **Fix**: IPC and ctrl-c now receive separate `Arc<RwLock<ProcessManager>>` + `Arc<RwLock<PanicEngine>>` clones directly. Consistent lock ordering enforced: always `ProcessManager` first, then `PanicEngine`.
- **Signal handling**: Replaced `ctrlc` crate + blocking `loop { sleep }` with `tokio::signal::ctrl_c()` + `tokio::select!`. No more `std::process::exit(0)` that bypassed Drop zeroization.
- **Startup**: Calls `RouteManager::block_ipv6_leaks()` at daemon start (was only callable via IPC).
- **Password**: Changed `--password` CLI flag to `-P`/`--password` with `hide = true`; primary path is `EPS_PASSWORD` env var only (CLI arg visible in `ps`).
- **Graceful shutdown**: No longer uses `std::process::exit(0)` — drops through to `main()` return, running all Drop implementations (including key zeroization).

#### `daemon/engine.rs` — TOCTOU race fixes (CRITICAL)
- **`start()` TOCTOU fix**: Removed read-then-write pattern. Now uses write-lock directly to prevent `watch_process` from changing status between lock release and re-acquisition (which could cause a double-start: two instances of the same service running simultaneously).
- **`watch_process` restart cancelled check**: After exponential backoff sleep, re-reads `proc.status`. If it changed from `Restarting` (e.g., `stop()` was called during the delay), aborts the restart instead of silently re-spawning.
- **`stop()` race fix**: Now uses `tx.send(0).await` (blocking send) instead of `tx.try_send(0)` (non-blocking, could silently fail if channel was full). Shutdown signal is now reliably delivered.
- **`status()` cleanup**: Removed `blocking_read()` in async context — was wasting blocking thread pool.

### Security & Correctness Fixes

#### `firewall/panic.rs` — Major rewrite
- **Real state tracking**: `PanicStatus` fields (`interfaces_down`, `dns_flushed`, `kernel_caches_purged`) now reflect actual operation results instead of being hardcoded to `false`.
- **Error propagation**: All `let _ = Command::new(...).output().await` calls now check exit status, log with `tracing::error!` including stderr, and propagate failure into `PanicStatus`.
- **Interface restoration**: `drop_all_interfaces()` stores the list of successfully dropped interfaces; `deactivate()` restores exactly those interfaces (not just loopback).
- **Incremental state update**: `activate()` sets `current_level` per completed step with error handling, no stale state on partial failure.
- **Honest cache purge rename**: `flush_memory()` → `purge_kernel_caches()`. Documents that this clears kernel page/dentry caches (not process secrets) and does NOT wipe swap. Adds warning about `vm.drop_caches` limitations.
- **Interface exclusion list**: `PanicEngineConfig::excluded_interfaces` configurably prevents dropping management/SSH interfaces.
- **Command timeouts**: All subprocess calls wrapped in `tokio::time::timeout(Duration::from_secs(10))`.

#### `firewall/nftables.rs` — Race condition fix
- **Stdin write ordering**: `nft_execute()` now completes the async `stdin.write_all()` before calling `child.wait_with_output()`, preventing a race where nft could close stdin before all data was written.
- **Version-safe `check_status`**: Removed `-a` flag (handle display, not universal). Now uses `nft list table inet ...` with proper error handling for missing tables.
- **Timeouts**: All nft subprocess calls have timeouts (5-15s).

#### `daemon/engine.rs` — Process lifecycle hardening
- **Race condition guards**: Per-service `Mutex` lock in `start()` prevents concurrent start calls on the same service.
- **Capture stdout/stderr**: Child processes now use `Stdio::piped()` for stdout/stderr instead of `Stdio::null()`. Output is logged with `info!`/`warn!` on exit.
- **Exponential backoff**: Restart delay starts at 2s and doubles per attempt (capped at 60s, max 5 doublings) instead of a fixed 3s.
- **Proper restart loop**: `watch_process()` uses a loop instead of broken recursion/early-return, so restart actually re-spawns the child process.
- **Circuit breaker**: Max-restarts check properly prevents infinite restart loops.

#### `daemon/config.rs` — Cryptographic hardening
- **Drop zeroization**: `ConfigManager::drop()` now calls `self.key.zeroize()` to ensure the derived key is wiped from memory.
- **Salt file permissions**: Salt file is created with `0o600` permissions on Unix (not world-readable).
- **Explicit Argon2 params**: Uses `Argon2id` with explicit parameters (64MB memory, 3 iterations, 4 parallelism) instead of `Argon2::default()` defaults.

#### `daemon/ipc.rs` — Socket security
- **Socket permissions**: Sets `0o700` on the Unix socket via `std::fs::set_permissions`, preventing non-root local users from sending IPC commands.
- **Input validation**: Added `validate_request()` that checks payload size (max 64KB), service name length (max 32 chars), and character whitelist (alphanumeric + `_-`).
- **Removed unused imports**: Cleaned up `mpsc` import.

#### `network/dns.rs` — Honest documentation
- **Plain UDP disclosure**: Renamed from "DNS hijacker" / "DNS resolver" to "DNS forwarder". Added explicit `SECURITY NOTICE` doc comment:
  > Forwards DNS queries over *unencrypted UDP* (port 53). DNS is visible to ISP/network in plaintext.
- **Timeouts**: Added 5s timeouts on upstream send/recv to prevent hang on unresponsive resolver.
- **Zero cache size guard**: Prevents zero-sized LRU cache (which would panic).

#### `network/routing.rs` — IPv6 leak prevention
- **IPv6 blocking**: New `block_ipv6_leaks()` disables IPv6 on all interfaces except `lo` via sysctl. Adds `unblock_ipv6()` to revert.
- **Fixed nonsense route**: Removed `1.0.0.0/8 via 127.0.0.1` (which did nothing). Replaced with proper fwmark-based policy routing matching Tor's TransPort.
- **Error propagation**: All commands now use `run_cmd()` helper that checks exit status, logs stderr, and adds timeouts.
- **Hardcoded port fix**: Changed iptables REDIRECT port from 9040 to accept `tor_trans_port` parameter (matching config's `socks_port`).

#### `network/mac.rs` — (minor)
- No functional changes in this pass. Glob-pattern interface exclusions and synchronization guards noted as deferred.

#### `crypto/keys.rs` — (minor)
- `secure_zero()` is called correctly. No changes needed beyond what config.rs now does with `Drop`.

#### `lib.rs` — On-boot detection
- **Pre-existing rule detection**: Daemon now checks nftables status on startup and logs whether kill-switch rules from a previous (possibly crashed) instance are still active.
- **Graceful shutdown on panic**: `shutdown()` now logs success/failure of nuclear activation instead of silently discarding the result.

#### `main.rs` — (minor)
- `foreground` flag is now referenced (`cli.foreground`), so it won't generate a dead-code warning.

### Documentation

- `THREAT_MODEL.md` — Created with adversary matrix, in-scope/out-of-scope threats, definitions of "leak," and assumptions.
- `SECURITY.md` — Created with privilege model documentation, risk evaluation, known limitations table, vulnerability reporting process, and secure deployment checklist.
- `README.md` — Updated "Current Status / Known Issues" section; corrected "DoH" claims; added links to threat model and security docs; documented reboot/deploy ordering accurately.
- `CHANGELOG.md` — This file.

### Frontend Updates
- `src/lib/daemon-context.tsx`: Updated `PanicStatus` interface to use `kernel_caches_purged` (was `memory_flushed`), matching renamed backend field.
