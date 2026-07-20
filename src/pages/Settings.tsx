import { Settings as SettingsIcon, Lock, Terminal } from "lucide-react";

const sysDeps = [
  { bin: "tor", pkg: "tor" },
  { bin: "obfs4proxy", pkg: "obfs4proxy" },
  { bin: "awg", pkg: "amneziawg-tools" },
  { bin: "syncthing", pkg: "syncthing" },
  { bin: "nft", pkg: "nftables" },
  { bin: "ip", pkg: "iproute2" },
  { bin: "resolvectl", pkg: "systemd-resolved" },
  { bin: "sysctl", pkg: "procps" },
];

const configPaths = [
  ["DAEMON CONFIG", "/etc/endpoint-privacy/config.enc"],
  ["PASSWORD FILE", "/etc/endpoint-privacy/password"],
  ["SALT FILE", "/etc/endpoint-privacy/.salt"],
  ["HASHES FILE", "/etc/endpoint-privacy/.hashes"],
  ["IPC SOCKET", "/run/endpoint-privacy/ipc.sock"],
  ["TOR CONFIG", "/etc/tor/torrc"],
  ["TOR DATA", "/var/lib/tor"],
  ["AWG CONFIG", "/etc/amneziawg/awg0.conf"],
  ["SYNCTHING HOME", "/etc/syncthing"],
];

export function Settings() {
  return (
    <div className="space-y-3 max-w-[1280px]">
      <div>
        <div className="page-title">SETTINGS</div>
        <div className="page-subtitle">CONFIGURATION // SYSTEM INFORMATION // INTEGRITY STATUS</div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {/* Encryption */}
        <div className="card">
          <div className="card-header">
            <Lock className="w-3 h-3 text-[#4ade80]" />
            <span className="card-title">ENCRYPTION</span>
          </div>
          <div className="mt-2 data-grid">
            <div className="data-row">
              <span className="data-label">ALGORITHM</span>
              <span className="data-value">AES-256-GCM</span>
            </div>
            <div className="data-row">
              <span className="data-label">KEY DERIVATION</span>
              <span className="data-value">Argon2id (64 MiB, 3 iter, 4 lanes)</span>
            </div>
            <div className="data-row">
              <span className="data-label">CONFIG STORAGE</span>
              <span className="data-value">/etc/endpoint-privacy/config.enc</span>
            </div>
            <div className="data-row">
              <span className="data-label">AUTH METHOD</span>
              <span className="data-value">EPS_PASSWORD env / password file</span>
            </div>
          </div>
        </div>

        {/* Shutdown behavior */}
        <div className="card">
          <div className="card-header">
            <Terminal className="w-3 h-3 text-[#4ade80]" />
            <span className="card-title">SHUTDOWN BEHAVIOR</span>
          </div>
          <div className="mt-2 data-grid">
            <div className="data-row">
              <span className="data-label">KILL SWITCH ON EXIT</span>
              <span className="data-value value-ok">ENABLED</span>
            </div>
            <div className="data-row">
              <span className="data-label">SHUTDOWN ACTION</span>
              <span className="data-value value-ok">NUCLEAR &rarr; STOP ALL</span>
            </div>
            <div className="data-row">
              <span className="data-label">KEY ZEROIZATION</span>
              <span className="data-value value-ok">ON DROP</span>
            </div>
            <div className="data-row">
              <span className="data-label">SIGNAL HANDLING</span>
              <span className="data-value">SIGINT + SIGTERM</span>
            </div>
          </div>
        </div>
      </div>

      {/* Configuration paths */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">CONFIGURATION PATHS</span>
        </div>
        <div className="mt-2 data-grid">
          {configPaths.map(([label, path]) => (
            <div key={label} className="data-row">
              <span className="data-label">{label}</span>
              <code className="text-[10px] font-mono text-[#4ade80]">{path}</code>
            </div>
          ))}
        </div>
      </div>

      {/* System dependencies */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">SYSTEM DEPENDENCIES</span>
        </div>
        <div className="mt-2 grid grid-cols-2 md:grid-cols-4 gap-1">
          {sysDeps.map(({ bin, pkg }) => (
            <div
              key={bin}
              className="flex items-center gap-2 px-2 py-1"
              style={{ background: "#111318" }}
            >
              <span className="indicator-off" />
              <span className="text-[10px] font-mono text-[#c8ccd4]">{bin}</span>
              <span className="text-[8px] font-mono text-[#6b7280] ml-auto">{pkg}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Security notes */}
      <div className="card border-[#ef4444]/20">
        <div className="card-header">
          <SettingsIcon className="w-3 h-3 text-[#ef4444]" />
          <span className="card-title text-[#ef4444]">SECURITY NOTICES</span>
        </div>
        <div className="mt-2 text-[9px] font-mono text-[#6b7280] space-y-1 leading-relaxed">
          <p>
            (!) DNS queries forwarded over plain UDP unless DoH is configured.
            Plaintext DNS visible to ISP and local network.
          </p>
          <p>
            (!) nftables rules survive daemon crash (kernel state) but not system reboot.
            Brief unprotected window exists until daemon starts after boot.
          </p>
          <p>
            (!) Daemon runs as root. Privilege separation documented but not yet implemented.
            Password is read from EPS_PASSWORD env var or password file only.
          </p>
        </div>
      </div>

      {/* Footer */}
      <div className="text-[8px] font-mono text-[#6b7280] text-center pt-2">
        KRYPTOS v0.1.0 // ENDPOINT PRIVACY SUITE // BUILD {new Date().toISOString().split("T")[0]}
      </div>
    </div>
  );
}
