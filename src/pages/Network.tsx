import { useDaemon } from "../lib/daemon-context";
import { Route, Wifi, Shield, GitBranch, Globe } from "lucide-react";

export function Network() {
  const { services, panic } = useDaemon();

  const torRunning = services.find((s) => s.name === "Tor")?.status === "Running";
  const awgRunning = services.find((s) => s.name === "AmneziaWG")?.status === "Running";
  const panicLevel = panic?.level ?? "Off";
  const killSwitchActive = panic?.kill_switch_active ?? false;

  return (
    <div className="space-y-5 max-w-[1280px]">
      <div>
        <h1 className="text-lg font-semibold text-[#e2e8f0]">Network</h1>
        <p className="text-caption mt-0.5">
          Split routing: TCP &rarr; Tor · UDP &rarr; AmneziaWG · DNS &rarr; Local forwarder
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* TCP path */}
        <div className={`card ${torRunning ? "border-[#5eead4]/20" : ""}`}>
          <div className="card-header">
            <div className={`p-1.5 rounded-md ${torRunning ? "bg-[#064e3b]" : "bg-[#232738]"}`}>
              <Route className={`w-4 h-4 ${torRunning ? "text-[#6ee7b7]" : "text-[#64748b]"}`} />
            </div>
            <span className="card-title">TCP path: Tor</span>
            {torRunning && <span className="status-dot-ok ml-auto" />}
          </div>
          <div className="mt-3 space-y-1.5">
            <div className="data-row">
              <span className="data-row-label">Destination</span>
              <span className="data-row-value">All TCP (port 6)</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Proxy</span>
              <span className="data-row-value">SOCKS5 127.0.0.1:9050</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Obfuscation</span>
              <span className="data-row-value">{torRunning ? "obfs4 active" : "Inactive"}</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">TransPort</span>
              <span className="data-row-value">fwmark 0x0f0f → table 100</span>
            </div>
          </div>
        </div>

        {/* UDP path */}
        <div className={`card ${awgRunning ? "border-[#5eead4]/20" : ""}`}>
          <div className="card-header">
            <div className={`p-1.5 rounded-md ${awgRunning ? "bg-[#064e3b]" : "bg-[#232738]"}`}>
              <Wifi className={`w-4 h-4 ${awgRunning ? "text-[#6ee7b7]" : "text-[#64748b]"}`} />
            </div>
            <span className="card-title">UDP path: AmneziaWG</span>
            {awgRunning && <span className="status-dot-ok ml-auto" />}
          </div>
          <div className="mt-3 space-y-1.5">
            <div className="data-row">
              <span className="data-row-label">Destination</span>
              <span className="data-row-value">All UDP (port 17)</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Tunnel</span>
              <span className="data-row-value">awg0</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Listen port</span>
              <span className="data-row-value">UDP 51820</span>
            </div>
            <div className="data-row">
              <span className="data-row-label">Status</span>
              <span className="data-row-value">{awgRunning ? "Tunneled" : "Direct"}</span>
            </div>
          </div>
        </div>
      </div>

      {/* DNS section */}
      <div className="card">
        <div className="card-header">
          <Globe className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">DNS configuration</span>
        </div>
        <div className="mt-3 space-y-1.5">
          <div className="data-row">
            <span className="data-row-label">Listener</span>
            <span className="data-row-value">127.0.0.1:53 (UDP)</span>
          </div>
          <div className="data-row">
            <span className="data-row-label">Upstream</span>
            <span className="data-row-value">1.1.1.1:53 (plain UDP)</span>
          </div>
          <div className="data-row">
            <span className="data-row-label">System DNS</span>
            <span className="data-row-value">resolvectl → lo</span>
          </div>
          <div className="data-row">
            <span className="data-row-label">Encryption</span>
            <span className="data-row-value text-[#fbbf24]">Plain UDP (DoH planned)</span>
          </div>
        </div>
        {/* DNS leak warning */}
        <div className="mt-3 p-2 rounded-md bg-[#451a03] border border-[#fbbf24]/20">
          <span className="text-[11px] text-[#fbbf24]">
            DNS queries are forwarded over plain UDP and visible to the ISP/network. Hard mode
            restricts outbound DNS to the configured upstream resolver IP only.
          </span>
        </div>
      </div>

      {/* Kill switch state */}
      <div className="card">
        <div className="card-header">
          <Shield className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">Firewall state</span>
          {killSwitchActive && (
            <span className="ml-auto text-[11px] text-[#f87171]">Kill switch: {panicLevel}</span>
          )}
        </div>
        <div className="mt-3 space-y-1.5">
          <div className="data-row">
            <span className="data-row-label">Kill switch</span>
            <span className={`data-row-value ${killSwitchActive ? "text-[#f87171]" : ""}`}>
              {killSwitchActive ? `Active (${panicLevel})` : "Inactive"}
            </span>
          </div>
          <div className="data-row">
            <span className="data-row-label">IPv6</span>
            <span className="data-row-value text-[#6ee7b7]">Blocked (sysctl)</span>
          </div>
          <div className="data-row">
            <span className="data-row-label">Routing</span>
            <span className="data-row-value">fwmark policy routing</span>
          </div>
          <div className="data-row">
            <span className="data-row-label">nftables</span>
            <span className="data-row-value">table inet endpoint_privacy</span>
          </div>
        </div>
      </div>

      {/* Traffic classifier */}
      <div className="card">
        <div className="card-header">
          <GitBranch className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">Traffic classifier rules</span>
        </div>
        <div className="mt-3 space-y-1">
          <div className="data-row rounded-md bg-[#161922]">
            <div className="flex items-center gap-2">
              <span className="status-dot-ok" />
              <span className="data-row-label">TCP</span>
            </div>
            <span className="data-row-value text-[#6ee7b7]">&rarr; Tor SOCKS5 (127.0.0.1:9050)</span>
          </div>
          <div className="data-row rounded-md bg-[#161922]">
            <div className="flex items-center gap-2">
              <span className="status-dot-info" />
              <span className="data-row-label">UDP</span>
            </div>
            <span className="data-row-value text-[#67e8f9]">&rarr; AmneziaWG tunnel (awg0)</span>
          </div>
          <div className="data-row rounded-md bg-[#161922]">
            <div className="flex items-center gap-2">
              <span className="status-dot-warn" />
              <span className="data-row-label">DNS</span>
            </div>
            <span className="data-row-value text-[#fbbf24]">&rarr; Local forwarder (127.0.0.1:53)</span>
          </div>
          <div className="data-row rounded-md bg-[#161922]">
            <div className="flex items-center gap-2">
              <span className="status-dot-muted" />
              <span className="data-row-label">Local net</span>
            </div>
            <span className="data-row-value text-[#64748b]">&rarr; Direct (bypass, CIDR-restricted)</span>
          </div>
        </div>
      </div>
    </div>
  );
}
