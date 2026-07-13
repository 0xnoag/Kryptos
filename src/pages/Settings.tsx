import { Settings as SettingsIcon, Lock, RefreshCw, FileText, Package } from "lucide-react";

export function Settings() {
  return (
    <div className="space-y-5 max-w-[1280px]">
      <div>
        <h1 className="text-lg font-semibold text-[#e2e8f0]">Settings</h1>
        <p className="text-caption mt-0.5">Daemon configuration and system information</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Encryption */}
        <div className="card">
          <div className="card-header">
            <Lock className="w-4 h-4 text-[#5eead4]" />
            <span className="card-title">Encryption</span>
          </div>
          <div className="mt-3 space-y-1.5">
            <div className="data-row">
              <span className="data-row-label">Algorithm</span>
              <span className="data-row-value">AES-256-GCM</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">KDF</span>
              <span className="data-row-value">Argon2id (64 MiB, 3 iter, 4 lanes)</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Config file</span>
              <span className="data-row-value">/etc/endpoint-privacy/config.enc</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Auth method</span>
              <span className="data-row-value">EPS_PASSWORD env var</span>
            </div>
          </div>
        </div>

        {/* Kill switch on exit */}
        <div className="card">
          <div className="card-header">
            <RefreshCw className="w-4 h-4 text-[#5eead4]" />
            <span className="card-title">Shutdown behavior</span>
          </div>
          <div className="mt-3 space-y-1.5">
            <div className="data-row">
              <span className="data-row-label">Kill switch on exit</span>
              <span className="data-row-value text-[#6ee7b7]">Enabled</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Shutdown action</span>
              <span className="data-row-value">Nuclear panic → stop all</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Key zeroization</span>
              <span className="data-row-value text-[#6ee7b7]">On drop</span>
            </div>
          </div>
        </div>
      </div>

      {/* Configuration paths */}
      <div className="card">
        <div className="card-header">
          <FileText className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">Configuration paths</span>
        </div>
        <div className="mt-3 space-y-1">
          {[
            ["Daemon config", "/etc/endpoint-privacy/config.enc"],
            ["Salt file", "/etc/endpoint-privacy/.salt"],
            ["IPC socket", "/run/endpoint-privacy/ipc.sock"],
            ["Tor config", "/etc/tor/torrc"],
            ["Tor data", "/var/lib/tor"],
            ["AmneziaWG config", "/etc/amneziawg/awg0.conf"],
            ["Syncthing home", "/etc/syncthing"],
            ["Logs", "/var/log/endpoint-privacy/"],
          ].map(([label, path]) => (
            <div key={label} className="data-row">
              <span className="data-row-label">{label}</span>
              <code className="text-[11px] font-mono text-[#5eead4]">{path}</code>
            </div>
          ))}
        </div>
      </div>

      {/* System dependencies */}
      <div className="card">
        <div className="card-header">
          <Package className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">System dependencies</span>
        </div>
        <div className="mt-3 grid grid-cols-2 md:grid-cols-4 gap-2">
          {[
            { bin: "tor", pkg: "tor" },
            { bin: "obfs4proxy", pkg: "obfs4proxy" },
            { bin: "awg", pkg: "amneziawg-tools" },
            { bin: "syncthing", pkg: "syncthing" },
            { bin: "nft", pkg: "nftables" },
            { bin: "ip", pkg: "iproute2" },
            { bin: "resolvectl", pkg: "systemd-resolved" },
            { bin: "sysctl", pkg: "procps" },
          ].map(({ bin, pkg }) => (
            <div
              key={bin}
              className="flex items-center gap-2 p-2 rounded-md bg-[#161922]"
            >
              <span className="status-dot-muted" />
              <span className="text-[12px] text-[#94a3b8] font-mono">{bin}</span>
              <span className="text-[10px] text-[#64748b] ml-auto">{pkg}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Security notes */}
      <div className="card border-[#f87171]/20">
        <div className="card-header">
          <SettingsIcon className="w-4 h-4 text-[#f87171]" />
          <span className="card-title text-[#f87171]">Security notes</span>
        </div>
        <div className="mt-3 space-y-2 text-[12px] text-[#94a3b8] leading-relaxed">
          <p>
            DNS is forwarded over plain UDP (DoH config field reserved). DNS queries are visible
            to the ISP and local network. The kill switch restricts outbound DNS to the configured
            upstream resolver IP only.
          </p>
          <p>
            nftables rules survive daemon crash (kernel state) but not reboot. After a system
            restart, there is a brief unprotected window until the daemon starts.
          </p>
          <p>
            The daemon runs as root. Privilege separation (privileged helper + unprivileged daemon)
            is documented in SECURITY.md but not yet implemented.
          </p>
        </div>
      </div>
    </div>
  );
}
