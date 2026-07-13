# Threat Model — Kryptos Endpoint Privacy Suite

## In Scope

Kryptos is designed to protect against the following adversaries:

| Adversary | Capability | Protected? |
|-----------|-----------|-----------|
| Passive ISP / upstream observer | Sees all unencrypted traffic metadata (IPs, ports, DNS queries, packet sizes, timing) | Yes — all traffic is tunneled through Tor (TCP) or AmneziaWG (UDP). DNS queries go through a local forwarder but are still plain UDP to upstream. |
| Local network observer (LAN, Wi-Fi hotspot) | Sees all traffic between device and gateway | Yes — same as above. MAC spoofing prevents device fingerprinting on Wi-Fi. |
| Censorship firewall (DPI-based) | Deep packet inspection to detect Tor, WireGuard, VPN protocols | Partial — obfs4 obfuscates Tor traffic. AmneziaWG uses obfuscated WireGuard protocol. Plain DNS queries may still be detected/interfered with. |
| Local network operator (DHCP, DNS) | Can serve malicious DNS responses, intercept local traffic | Partial — DNS queries to upstream are plain UDP, but the kill switch ensures all traffic goes through tunnels. Malicious DNS would still result in tunneled connections. |
| Remote website / service | Sees the IP of the exit node or tunnel endpoint | Yes — Tor exit nodes and AmneziaWG servers hide your real IP. |
| Post-reboot / crash state | Traffic flows before daemon starts | Partial — systemd ordering ensures tunnels start before unprotected traffic can leak, but nftables rules are NOT persisted. On crash, the kernel retains nftables rules (they survive daemon death), preserving the kill switch. |

## Out of Scope

The following adversaries or scenarios are **NOT** protected:

| Threat | Reason |
|--------|--------|
| **Compromised endpoint (malware, remote access)** | If the attacker has code execution on the machine, they can read memory (including passwords from `EPS_PASSWORD`), disable the daemon, install a rootkit, or exfiltrate data before it reaches the tunnel. Kryptos is a transit privacy tool, not an endpoint security suite. |
| **Global passive adversary (state-level)** | Traffic correlation attacks, timing analysis, and global observation of Tor entry/exit pairs can deanonymize users regardless of obfuscation. Kryptos does not add guard node selection, traffic padding, or any timing mitigation. |
| **Physical access with daemon unlocked** | If the machine is seized while the daemon is running and the IPC socket is accessible, an attacker can read the daemon state. The config is encrypted, but the key is in memory. |
| **Physical access with daemon locked/shut down** | The config file is AES-256-GCM encrypted with Argon2 KDF. Physical access enables offline brute-force of the password. Use a strong password (>80 bits entropy). |
| **Side-channel attacks (power analysis, cache timing)** | No hardware-level side-channel protections. |
| **Traffic confirmation attacks** | A website can confirm traffic is coming from Tor by checking for Tor exit node IPs. Kryptos cannot prevent this. |
| **DNS via JavaScript / WebRTC** | Browser-level DNS leaks (WebRTC STUN requests, JS DNS queries) are not intercepted. They would resolve via the attacker's configured DNS or STUN servers. The kill switch blocks them if they don't go through the tunnel. |
| **IPv6 traffic before daemon blocks it** | The daemon blocks IPv6 on startup via sysctl, but there is a race window between boot and daemon start when IPv6 traffic could leak. |

## Definitions

- **Leak**: Any packet whose source IP is the real public IP of the device and whose destination is outside the local network, which does not traverse either the Tor or AmneziaGW tunnel interfaces.
- **Kill switch**: A set of nftables rules that drop all packets not matching allowed conditions (loopback, established connections, tunnel interfaces). The rules are applied atomically and persist in the kernel regardless of daemon process health.
- **Panic level**: An escalation of the kill switch from permissive (soft, allows DNS) to total (nuclear, all interfaces down).

## Assumptions

1. The operating system kernel is trusted and uncompromised.
2. `nftables`, `ip`, `sysctl`, and tunnel binaries (`tor`, `awg`, `obfs4proxy`, `syncthing`) are from trusted package sources and have not been tampered with.
3. The system clock is accurate (required for Tor consensus validation).
4. The user's password for config encryption has sufficient entropy.
