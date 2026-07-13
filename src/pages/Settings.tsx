import { Settings as SettingsIcon, Lock, FileText, RefreshCw } from "lucide-react";

export function Settings() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-white">Settings</h1>
        <p className="text-gray-400 mt-1">
          Daemon configuration and system preferences
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="card">
          <div className="panel-title">
            <Lock className="w-5 h-5 text-privacy-400" />
            Daemon Authentication
          </div>
          <p className="text-gray-400 text-sm mt-2">
            Config files are encrypted with AES-256-GCM + Argon2 KDF.
            Password is provided at daemon startup via the
            <code className="text-privacy-400 mx-1">EPS_PASSWORD</code>
            environment variable.
          </p>
          <div className="mt-4 flex items-center gap-2 text-sm text-gray-500">
            <div className="w-2 h-2 rounded-full bg-privacy-500" />
            Encrypted config at /etc/endpoint-privacy/config.enc
          </div>
        </div>

        <div className="card">
          <div className="panel-title">
            <RefreshCw className="w-5 h-5" />
            Kill Switch on Exit
          </div>
          <p className="text-gray-400 text-sm mt-2">
            When enabled, the daemon will automatically activate the nuclear panic
            level on shutdown to ensure zero traffic leaks.
          </p>
          <div className="mt-4 flex items-center gap-3">
            <button className="btn-primary">Enabled</button>
            <span className="text-xs text-gray-500">
              Recommended: ON
            </span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="panel-title">
          <FileText className="w-5 h-5" />
          Configuration Paths
        </div>
        <div className="mt-4 space-y-3 text-sm">
          {[
            ["Daemon Config", "/etc/endpoint-privacy/config.enc"],
            ["IPC Socket", "/run/endpoint-privacy/ipc.sock"],
            ["Tor Config", "/etc/tor/torrc"],
            ["Tor Data", "/var/lib/tor"],
            ["AmneziaWG Config", "/etc/amneziawg/awg0.conf"],
            ["Syncthing Home", "/etc/syncthing"],
            ["Logs", "/var/log/endpoint-privacy/"],
          ].map(([label, path]) => (
            <div
              key={label}
              className="flex items-center justify-between p-2 bg-gray-950 rounded-lg"
            >
              <span className="text-gray-400">{label}</span>
              <code className="text-privacy-400 font-mono text-xs">{path}</code>
            </div>
          ))}
        </div>
      </div>

      <div className="card">
        <div className="panel-title">
          <SettingsIcon className="w-5 h-5" />
          System Dependencies
        </div>
        <p className="text-gray-400 text-sm mt-2">
          Required binaries that must be installed on the system:
        </p>
        <div className="mt-4 grid grid-cols-2 md:grid-cols-4 gap-3">
          {[
            { name: "tor", pkg: "tor" },
            { name: "obfs4proxy", pkg: "obfs4proxy" },
            { name: "awg", pkg: "amneziawg" },
            { name: "syncthing", pkg: "syncthing" },
            { name: "nft", pkg: "nftables" },
            { name: "ip", pkg: "iproute2" },
            { name: "resolvectl", pkg: "systemd-resolved" },
            { name: "sysctl", pkg: "procps" },
          ].map(({ name, pkg }) => (
            <div
              key={name}
              className="flex items-center gap-2 p-2 bg-gray-950 rounded-lg"
            >
              <div className="w-2 h-2 rounded-full bg-privacy-500" />
              <span className="text-gray-300">{name}</span>
              <span className="text-gray-600 text-xs ml-auto">{pkg}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="card border-danger-800">
        <h3 className="text-lg font-semibold text-danger-400 mb-2">
          Danger Zone
        </h3>
        <p className="text-gray-400 text-sm">
          These actions will reset all daemon state and firewall rules.
        </p>
        <div className="flex gap-3 mt-4">
          <button className="btn-danger">Reset Firewall Rules</button>
          <button className="btn-danger">Purge All Config</button>
        </div>
      </div>
    </div>
  );
}
