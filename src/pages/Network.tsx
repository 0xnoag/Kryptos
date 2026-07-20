import { useDaemon } from "../lib/daemon-context";
import { Route } from "lucide-react";

export function Network() {
  const { services, panic } = useDaemon();

  const tor = services.find((s) => s.name === "Tor");
  const awg = services.find((s) => s.name === "AmneziaWG");
  const torOk = tor?.status === "Running";
  const awgOk = awg?.status === "Running";
  const ks = panic?.kill_switch_active ?? false;
  const pl = panic?.level ?? "OFF";

  return (
    <div className="space-y-3 max-w-[1280px]">
      <div>
        <div className="page-title">NETWORK</div>
        <div className="page-subtitle">ROUTING TABLE // TRAFFIC PATH CONFIGURATION</div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {/* TCP path */}
        <div className={`card ${torOk ? "border-[#4ade80]/20" : ""}`}>
          <div className="card-header">
            <span className="text-[11px] font-mono text-[#4ade80]">TCP</span>
            <span className="card-title">PATH: TOR</span>
            {torOk && <span className="indicator-ok ml-auto" />}
          </div>
          <div className="mt-2 data-grid">
            <div className="data-row">
              <span className="data-label">DESTINATION</span>
              <span className="data-value">ALL TCP</span>
            </div>
            <div className="data-row">
              <span className="data-label">PROXY</span>
              <span className="data-value">SOCKS5 127.0.0.1:9050</span>
            </div>
            <div className="data-row">
              <span className="data-label">OBFUSCATION</span>
              <span className={`data-value ${torOk ? "value-ok" : "value-dim"}`}>
                {torOk ? "obfs4 ACTIVE" : "INACTIVE"}
              </span>
            </div>
            <div className="data-row">
              <span className="data-label">TRANSPORT</span>
              <span className="data-value">fwmark 0x0f0f &rarr; table 100</span>
            </div>
            <div className="data-row">
              <span className="data-label">CONTROL</span>
              <span className="data-value">127.0.0.1:9051</span>
            </div>
          </div>
        </div>

        {/* UDP path */}
        <div className={`card ${awgOk ? "border-[#4ade80]/20" : ""}`}>
          <div className="card-header">
            <span className="text-[11px] font-mono text-[#22d3ee]">UDP</span>
            <span className="card-title">PATH: AMNEZIAWG</span>
            {awgOk && <span className="indicator-ok ml-auto" />}
          </div>
          <div className="mt-2 data-grid">
            <div className="data-row">
              <span className="data-label">DESTINATION</span>
              <span className="data-value">ALL UDP</span>
            </div>
            <div className="data-row">
              <span className="data-label">TUNNEL</span>
              <span className="data-value">awg0</span>
            </div>
            <div className="data-row">
              <span className="data-label">LISTEN PORT</span>
              <span className="data-value">UDP 51820</span>
            </div>
            <div className="data-row">
              <span className="data-label">STATUS</span>
              <span className={`data-value ${awgOk ? "value-ok" : "value-dim"}`}>
                {awgOk ? "TUNNELED" : "DIRECT"}
              </span>
            </div>
            <div className="data-row">
              <span className="data-label">CONFIG</span>
              <span className="data-value">/etc/amneziawg/awg0.conf</span>
            </div>
          </div>
        </div>
      </div>

      {/* DNS section */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">DNS CONFIGURATION</span>
        </div>
        <div className="mt-2 data-grid">
          <div className="data-row">
            <span className="data-label">LISTENER</span>
            <span className="data-value">127.0.0.1:53</span>
          </div>
          <div className="data-row">
            <span className="data-label">UPSTREAM</span>
            <span className="data-value">1.1.1.1:53</span>
          </div>
          <div className="data-row">
            <span className="data-label">DOH ENDPOINT</span>
            <span className="data-value">cloudflare-dns.com/dns-query</span>
          </div>
          <div className="data-row">
            <span className="data-label">ENCRYPTION</span>
            <span className={`data-value ${true ? "value-warn" : "value-ok"}`}>
              DoH CONFIGURED
            </span>
          </div>
          <div className="data-row">
            <span className="data-label">SYSTEM DNS</span>
            <span className="data-value">resolvectl &rarr; lo</span>
          </div>
        </div>
      </div>

      {/* Firewall state */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">FIREWALL STATE</span>
          {ks && <span className="ml-auto text-[9px] font-mono text-[#ef4444]">KS: {pl}</span>}
        </div>
        <div className="mt-2 data-grid">
          <div className="data-row">
            <span className="data-label">KILL SWITCH</span>
            <span className={`data-value ${ks ? "value-critical" : "value-dim"}`}>
              {ks ? `ACTIVE (${pl})` : "INACTIVE"}
            </span>
          </div>
          <div className="data-row">
            <span className="data-label">IPv6</span>
            <span className="data-value value-ok">BLOCKED</span>
          </div>
          <div className="data-row">
            <span className="data-label">ROUTING</span>
            <span className="data-value">fwmark policy routing</span>
          </div>
          <div className="data-row">
            <span className="data-label">NFTABLES TABLE</span>
            <span className="data-value">inet endpoint_privacy</span>
          </div>
        </div>
      </div>

      {/* Traffic classifier */}
      <div className="card">
        <div className="card-header">
          <Route className="w-3 h-3 text-[#4ade80]" />
          <span className="card-title">TRAFFIC CLASSIFIER RULES</span>
        </div>
        <div className="mt-2 space-y-px">
          <div className="data-row" style={{ background: "#111318" }}>
            <div className="flex items-center gap-2">
              <span className="indicator-ok" />
              <span className="data-label">TCP</span>
            </div>
            <span className="text-[10px] font-mono text-[#4ade80]">&rarr; Tor SOCKS5</span>
          </div>
          <div className="data-row" style={{ background: "#111318" }}>
            <div className="flex items-center gap-2">
              <span className="indicator-info" />
              <span className="data-label">UDP</span>
            </div>
            <span className="text-[10px] font-mono text-[#22d3ee]">&rarr; AmneziaWG (awg0)</span>
          </div>
          <div className="data-row" style={{ background: "#111318" }}>
            <div className="flex items-center gap-2">
              <span className="indicator-warn" />
              <span className="data-label">DNS</span>
            </div>
            <span className="text-[10px] font-mono text-[#f59e0b]">&rarr; Local fwd (127.0.0.1:53)</span>
          </div>
          <div className="data-row" style={{ background: "#111318" }}>
            <div className="flex items-center gap-2">
              <span className="indicator-off" />
              <span className="data-label">LOCAL</span>
            </div>
            <span className="text-[10px] font-mono text-[#6b7280]">&rarr; Direct (CIDR-restricted)</span>
          </div>
        </div>
      </div>
    </div>
  );
}
