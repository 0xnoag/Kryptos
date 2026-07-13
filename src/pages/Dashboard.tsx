import { useDaemon } from "../lib/daemon-context";
import { Activity, Shield, Radio, Route, Globe } from "lucide-react";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

export function Dashboard() {
  const { services, panic } = useDaemon();

  const torSvc = services.find((s) => s.name === "Tor");
  const awgSvc = services.find((s) => s.name === "AmneziaWG");
  const syncthingSvc = services.find((s) => s.name === "Syncthing");
  const torRunning = torSvc?.status === "Running";
  const awgRunning = awgSvc?.status === "Running";
  const syncthingRunning = syncthingSvc?.status === "Running";
  const runningCount = services.filter((s) => s.status === "Running").length;
  const totalCount = services.length;
  const panicLevel = panic?.level ?? "Off";

  return (
    <div className="space-y-5 max-w-[1280px]">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold text-[#e2e8f0]">Dashboard</h1>
          <p className="text-caption mt-0.5">Endpoint Privacy Suite status and traffic flow</p>
        </div>
        <div className="flex items-center gap-3">
          {panic?.kill_switch_active && (
            <span className="flex items-center gap-1.5 text-[11px] text-[#f87171]">
              <span className="status-dot-critical" />
              Kill switch: {panicLevel}
            </span>
          )}
          <span className="text-[11px] text-[#64748b]">
            Updated {new Date().toLocaleTimeString()}
          </span>
        </div>
      </div>

      {/* Top row: quick stats */}
      <div className="grid grid-cols-4 gap-4">
        <div className={`card ${panic?.kill_switch_active && panicLevel === "Nuclear" ? "border-[#f87171]/30" : ""}`}>
          <span className="text-label">Kill switch</span>
          <div className="flex items-center gap-2 mt-1">
            <Shield className={`w-4 h-4 ${
              panic?.kill_switch_active ? "text-[#f87171]" : "text-[#2a2e3d]"
            }`} />
            <span className="text-data font-semibold">{panicLevel}</span>
          </div>
        </div>
        <div className="card">
          <span className="text-label">Services</span>
          <div className="flex items-center gap-2 mt-1">
            <Radio className="w-4 h-4 text-[#2a2e3d]" />
            <span className="text-data font-semibold">
              {runningCount}/{totalCount}
            </span>
            <span className="text-caption">running</span>
          </div>
        </div>
        <div className="card">
          <span className="text-label">TCP path</span>
          <div className="flex items-center gap-2 mt-1">
            <span
              className={torRunning ? "status-dot-ok" : "status-dot-muted"}
            />
            <span className="text-data font-semibold">
              {torRunning ? "Tor" : "Direct"}
            </span>
          </div>
        </div>
        <div className="card">
          <span className="text-label">UDP path</span>
          <div className="flex items-center gap-2 mt-1">
            <span
              className={awgRunning ? "status-dot-ok" : "status-dot-muted"}
            />
            <span className="text-data font-semibold">
              {awgRunning ? "AmneziaWG" : "Direct"}
            </span>
          </div>
        </div>
      </div>

      {/* Traffic topology */}
      <div className="card">
        <div className="card-header">
          <Route className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">Traffic flow</span>
        </div>
        <div className="pt-4 flex items-center justify-center gap-2">
          {/* Application traffic */}
          <div className="topology-node">
            <Activity className="w-5 h-5 text-[#94a3b8]" />
            <span className="topology-node-label">Applications</span>
            <span className="topology-node-value">TCP · UDP · DNS</span>
          </div>

          <div className={`topology-arrow ${torRunning || awgRunning ? "topology-arrow-active" : ""}`}>
            ▶
          </div>

          {/* Classifier */}
          <div className="topology-node">
            <Route className="w-5 h-5 text-[#94a3b8]" />
            <span className="topology-node-label">Classifier</span>
            <span className="topology-node-value">nftables fwmark</span>
          </div>

          <div className="flex flex-col items-center gap-1">
            <div className="flex items-center gap-1">
              <span className={`topology-arrow ${torRunning ? "topology-arrow-active" : ""}`}>▶</span>
              <div className={`topology-node ${torRunning ? "border-[#5eead4]/30" : ""}`}>
                <span className="topology-node-label">TCP</span>
                <span className="topology-node-value text-[11px]">Tor · obfs4</span>
                {torSvc && (
                  <span className="text-[10px] text-[#64748b]">
                    {torSvc.status === "Running" ? formatUptime(torSvc.uptime_secs) : torSvc.status}
                  </span>
                )}
              </div>
            </div>
            <div className="flex items-center gap-1">
              <span className={`topology-arrow ${awgRunning ? "topology-arrow-active" : ""}`}>▶</span>
              <div className={`topology-node ${awgRunning ? "border-[#5eead4]/30" : ""}`}>
                <span className="topology-node-label">UDP</span>
                <span className="topology-node-value text-[11px]">AmneziaWG</span>
                {awgSvc && (
                  <span className="text-[10px] text-[#64748b]">
                    {awgSvc.status === "Running" ? formatUptime(awgSvc.uptime_secs) : awgSvc.status}
                  </span>
                )}
              </div>
            </div>
          </div>

          <div className={`topology-arrow ${torRunning || awgRunning ? "topology-arrow-active" : ""}`}>
            ▶
          </div>

          {/* Internet */}
          <div className="topology-node border-[#2a2e3d]">
            <Globe className="w-5 h-5 text-[#64748b]" />
            <span className="topology-node-label">Internet</span>
            <span className="text-[10px] text-[#64748b]">
              {torRunning || awgRunning ? "Tunneled" : "Direct"}
            </span>
          </div>
        </div>

        {/* Kill switch indicator */}
        {panic?.kill_switch_active && (
          <div className="mt-4 pt-3 border-divider flex items-center justify-center gap-2">
            <span className="status-dot-critical" />
            <span className="text-[11px] text-[#f87171]">
              Kill switch active at {panicLevel} level
              {panic.interfaces_down ? " · Interfaces down" : ""}
              {panic.dns_flushed ? " · DNS flushed" : ""}
              {panic.kernel_caches_purged ? " · Caches purged" : ""}
            </span>
          </div>
        )}
      </div>

      {/* Service details */}
      <div className="card">
        <div className="card-header">
          <Radio className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">Services</span>
          <span className="ml-auto text-caption">
            {runningCount} of {totalCount} active
          </span>
        </div>
        <div className="pt-3 space-y-1">
          {services.map((svc) => {
            let dotClass = "status-dot-muted";
            let statusLabel = svc.status;
            if (svc.status === "Running") {
              dotClass = "status-dot-ok";
              statusLabel = `Running · ${formatUptime(svc.uptime_secs)}`;
            } else if (svc.status === "Failed") {
              dotClass = "status-dot-critical";
              statusLabel = "Failed";
            } else if (svc.status === "Starting" || svc.status === "Restarting") {
              dotClass = "status-dot-warn";
            }
            return (
              <div key={svc.name} className="data-row">
                <div className="flex items-center gap-3">
                  <span className={dotClass} />
                  <span className="font-medium text-[#e2e8f0]">{svc.name}</span>
                  <span className="text-caption">{statusLabel}</span>
                </div>
                <div className="flex items-center gap-3">
                  {svc.restart_count > 0 && (
                    <span className="text-caption">Restarts: {svc.restart_count}</span>
                  )}
                </div>
              </div>
            );
          })}
          {services.length === 0 && (
            <div className="py-8 text-center text-caption">
              No services registered. Ensure daemon is connected.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
