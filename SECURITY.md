# Security Policy — Kryptos Endpoint Privacy Suite

## Current Privilege Model

The daemon must run as **root** because:
1. `nftables` rule modification requires `CAP_NET_ADMIN`
2. `ip link set` operations require `CAP_NET_ADMIN`
3. `sysctl` writes require `CAP_SYS_ADMIN`
4. Binding to low ports (e.g., DNS port 53) requires `CAP_NET_BIND_SERVICE`

### Risks of Root Execution

- Any compromise of the Rust daemon process gives an attacker full root capabilities.
- The `EPS_PASSWORD` environment variable, containing the config decryption password, is visible in `/proc/<pid>/environ` to the root user and, if the kernel is unhardened, to any process with `CAP_SYS_PTRACE`. The `--password` CLI flag is hidden from `ps` output via clap's `hide = true`.
- Child processes (tor, awg) inherit root. tor drops privileges by default (`User debian-tor`), but other binaries may not.

## Binary Integrity Verification

Every external binary (tor, obfs4proxy, awg, syncthing) can be verified by SHA-256 hash before execution.

### How it works

1. A `.hashes` file in the config directory (`/etc/endpoint-privacy/.hashes`) contains expected SHA-256 hashes for each binary.
2. On daemon start with `--verify-signatures` (or `--strict-verification`), all configured hashes are checked before any service starts.
3. Before each `start()` and `restart()` call, the process manager verifies the binary's hash matches the expected value.
4. A mismatch logs the discrepancy and prevents the binary from executing — detects tampering or unexpected upgrades.

### Generating the .hashes file

On the target system, run:
```bash
# After installing the required packages
cd /etc/endpoint-privacy
for bin in /usr/bin/tor /usr/bin/obfs4proxy /usr/bin/awg /usr/bin/syncthing; do
  if [ -f "$bin" ]; then
    hash=$(sha256sum "$bin" | cut -d' ' -f1)
    pkg=$(dpkg -S "$bin" 2>/dev/null | cut -d: -f1)
    pkgver=$(dpkg -l "$pkg" 2>/dev/null | awk '/^ii/ {print $3}')
    source="${pkg}=${pkgver:-unknown}"
    echo "\"$bin\" = { hash = \"$hash\", source = \"$source\" }"
  fi
done > .hashes
chmod 600 .hashes
```

The Makefile also provides `make hashes` to generate this file automatically.

### Policy

- **No `.hashes` file, no `--verify-signatures` flag**: binaries are spawned with only a filesystem existence check (legacy behavior, warning logged).
- **`--verify-signatures` without `.hashes` file**: the daemon warns and starts without verification.
- **`--strict-verification`**: the daemon refuses to start unless every configured binary has an entry in `.hashes` and all hashes match.
- **After a package upgrade**: regenerate `.hashes` with `make hashes` or the script above. The old hashes will cause start failures until updated.

## Supply Chain Security

### Reproducible builds

- `Cargo.lock` is committed to the repository, pinning every dependency to a specific version.
- `cargo audit` checks for known vulnerabilities in the dependency tree (run via `make audit` or CI).
- `cargo deny` enforces license compliance and bans unapproved dependency sources (run via CI or `cargo deny check`).

### CI pipeline

Every push and pull request runs:

| Job | What it checks |
|-----|----------------|
| `security-audit` | `cargo audit` (vulnerabilities) + `cargo deny` (licenses, advisories, sources) |
| `test` | `cargo build --release` + `cargo test` |
| `supply-chain` | Verifies `Cargo.lock` is committed and up-to-date |

### Release signing

Release binaries are GPG-signed with detached ASCII-armored signatures. The `Makefile` provides:

```bash
make release VERSION=x.y.z       # Build release binary
make sign GPG_KEY=your@email.com # Sign binary + create SHA256SUMS.asc
make verify-signature BINARY=bin # Verify a release binary's signature
```

## Proposed Privilege Separation (not yet implemented)

A future iteration should split the daemon into:
1. **Privileged helper** (small, audit-able): exposes only a fixed set of operations via a narrow Unix socket protocol:
   - `load_nftables_rules(rules: String)` — validates and applies nft ruleset
   - `set_sysctl(key: String, value: String)` — validates key is in allowlist
   - `configure_interface(iface: String, action: InterfaceAction)` — up/down/mac
2. **Unprivileged daemon**: owns IPC, DNS proxy, traffic classifier, process lifecycle.
   - Connects to privileged helper for all kernel operations.
   - Runs as `nobody` or a dedicated `kryptos` user.

This reduces the attack surface: a bug in JSON parsing or service management cannot directly modify firewall rules.

## Known Limitations

| Issue | Status | Impact |
|-------|--------|--------|
| **DNS over plain UDP** | DNS is forwarded to upstream via unencrypted UDP (port 53). The `doh_url` config field exists but is unimplemented. | DNS queries are visible in plaintext to ISP/local network. This is a known leak. |
| **IPv6 leak window** | IPv6 is disabled via sysctl on daemon start, but there is a race between boot and daemon launch. Daemon now calls `block_ipv6_leaks()` at startup. | Brief window where IPv6 traffic bypasses tunnels. Mitigated by systemd ordering (daemon starts early). |
| **No nftables rule persistence across reboot** | nftables rules from the kill switch survive daemon crash (they're in-kernel) but NOT a system reboot. | After reboot, there is a window with no firewall protection until the daemon starts and applies rules. Mitigated by systemd `ExecStartPre` to restore rules. |
| **Plain-text stdout/stderr logs** | Child process output is captured and logged via `tracing`. These logs may contain sensitive information (URLs, IPs, error details). | Logs should be treated as sensitive. Configure log rotation and restrict access (`chmod 600`). |
| **MAC spoofing races** | `ip link set down` while traffic is in flight causes transient connection drops. No synchronization with active sockets. | Expected behavior: brief network interruption during MAC change. Long-running TCP connections may reset. |
| **Hard mode DNS port 53/853 restriction** | Outbound DNS exceptions in Hard kill-switch mode are now restricted to the configured upstream resolver IP (instead of allowing ANY IP on those ports). | Prevents apps with hardcoded DNS resolvers from bypassing the local DNS proxy. |
| **Interface name validation** | `add_allowed_interface()` now validates interface name characters (alphanumeric, hyphens, underscores, plus). | Prevents nftables rule-injection through interface names. |
| **IPC deadlock eliminated** | Outer `Arc<RwLock<>>` removed; lock ordering is now consistent. Shutdown is reachable via ctrl-c (was permanently blocked by IPC accept loop). | Daemon can now gracefully shut down and zeroize encryption keys on exit. |
| **Classifier is informational only** | The `TrafficClassifier` does not enforce routing decisions — it only annotates packet types. Actual routing enforcement depends on nftables + routing table configuration. | No enforcement gap if nftables rules are correctly applied. |
| **Binary hashes are user-managed** | SHA-256 hash verification requires a `.hashes` file in the config directory. No built-in defaults are compiled in. | Admin must generate hashes after package installs/upgrades. Without `--verify-signatures`, hashes are not checked. |

## How to Report a Vulnerability

**Do not open public GitHub issues for security vulnerabilities.**

Instead, send a description of the finding to:

**Email**: `0xnoag@proton.me` (PGP key available on keyserver)

Please include:
- A clear description of the vulnerability
- Steps to reproduce
- Possible impact and exploitation scenario
- Suggested fix (if known)

You can expect:
- Initial acknowledgment within 72 hours
- A fix timeline within 7 days for moderate-severity issues
- Coordinated disclosure: we will work with you to determine a release date for the fix

## Components NOT Yet Independently Audited

- **Full Rust daemon**: This is a pre-release codebase that has not undergone a third-party security audit.
- **AmneziaWG integration**: The `awg` binary is from a third-party project and has its own security posture.
- **HTTP API (axum)**: The read-only HTTP API and its Bearer token authentication have not been formally audited. The implementation follows standard axum patterns but may contain logic errors.
- **Unix socket JSON-RPC**: Parameter validation exists but the full IPC surface has not been fuzz-tested.
- **Encrypted config**: The AES-256-GCM + Argon2 scheme follows best practices but has not been formally verified.

## Secure Deployment Checklist

- [ ] Run the daemon as a systemd service with `ProtectSystem=strict`, `NoNewPrivileges=true`, `CapabilityBoundingSet=CAP_NET_ADMIN CAP_SYS_ADMIN CAP_NET_BIND_SERVICE`
- [ ] Set `EPS_PASSWORD` via a systemd credential (inaccessible to unprivileged users) rather than an environment variable
- [ ] Restrict `/etc/endpoint-privacy/` to `root:root 0700`
- [ ] Restrict `/run/endpoint-privacy/` to `root:root 0700`
- [ ] Confirm HTTP API binds to `127.0.0.1` only (default) — never expose to `0.0.0.0`
- [ ] Use `logrotate` for daemon logs with `create 0600 root root`
- [ ] Pin `tor`, `obfs4proxy`, `amneziawg`, `syncthing` to specific, known-good versions via apt pinning
