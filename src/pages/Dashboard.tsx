import { useDaemon } from "../lib/daemon-context";
import { Route, Radio } from "lucide-react";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

export function Dashboard() {
  const { services, panic } = useDaemon();

  const tor = services.find((s) => s.name === "Tor");
  const awg = services.find((s) => s.name === "AmneziaWG");
  const torOk = tor?.status === "Running";
  const awgOk = awg?.status === "Running";
  const runningCount = services.filter((s) => s.status === "Running").length;
  const totalCount = services.length;
  const ks = panic?.kill_switch_active ?? false;
  const pl = panic?.level ?? "OFF";

  return (
    <div className="space-y-3 max-w-[1280px]">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="page-title">DASHBOARD</div>
          <div className="page-subtitle">SYSTEM STATUS // {new Date().toLocaleTimeString()}Z</div>
        </div>
        <div className="flex items-center gap-2">
          {ks && (
            <span className="text-[9px] font-mono text-[#ef4444] tracking-wider uppercase border border-[#ef4444]/30 px-1.5 py-0.5">
              KILL SWITCH: {pl}
            </span>
          )}
        </div>
      </div>

      {/* Quick status row */}
      <div className="grid grid-cols-4 gap-3">
        <div className="card">
          <div className="card-number">SYS-01</div>
          <div className="mt-1 flex items-center gap-2">
            <span className={`indicator ${ks ? "indicator-critical" : "indicator-off"}`} />
            <span className="text-[11px] font-mono text-[#c8ccd4]">KILL SWITCH</span>
          </div>
          <div className="text-[15px] font-mono mt-1">{pl}</div>
        </div>
        <div className="card">
          <div className="card-number">SYS-02</div>
          <div className="mt-1 flex items-center gap-2">
            <span className={`indicator ${runningCount > 0 ? "indicator-ok" : "indicator-off"}`} />
            <span className="text-[11px] font-mono text-[#c8ccd4]">SERVICES</span>
          </div>
          <div className="text-[15px] font-mono mt-1">{runningCount}/{totalCount}</div>
        </div>
        <div className="card">
          <div className="card-number">SYS-03</div>
          <div className="mt-1 flex items-center gap-2">
            <span className={`indicator ${torOk ? "indicator-ok" : "indicator-off"}`} />
            <span className="text-[11px] font-mono text-[#c8ccd4]">TCP PATH</span>
          </div>
          <div className="text-[15px] font-mono mt-1">{torOk ? "TOR" : "DIRECT"}</div>
        </div>
        <div className="card">
          <div className="card-number">SYS-04</div>
          <div className="mt-1 flex items-center gap-2">
            <span className={`indicator ${awgOk ? "indicator-ok" : "indicator-off"}`} />
            <span className="text-[11px] font-mono text-[#c8ccd4]">UDP PATH</span>
          </div>
          <div className="text-[15px] font-mono mt-1">{awgOk ? "AWG" : "DIRECT"}</div>
        </div>
      </div>

      {/* Traffic topology */}
      <div className="card">
        <div className="card-header">
          <Route className="w-3 h-3 text-[#4ade80]" />
          <span className="card-title">TRAFFIC FLOW</span>
        </div>
        <div className="mt-2 flex items-center justify-center gap-1 py-2 text-[10px] font-mono">
          <div className="border border-[#2a2f3f] px-2 py-1 text-center min-w-[80px]">
            <div className="text-[#6b7280]">APPS</div>
            <div className="text-[#c8ccd4]">TCP/UDP</div>
          </div>
          <div className={`${torOk || awgOk ? "text-[#4ade80]" : "text-[#2a2f3f]"}`}>
            &rarr;
          </div>
          <div className="border border-[#2a2f3f] px-2 py-1 text-center min-w-[80px]">
            <div className="text-[#6b7280]">CLASSIFY</div>
            <div className="text-[#c8ccd4]">nftables</div>
          </div>
          <div className="flex flex-col gap-1">
            <div className="flex items-center gap-1">
              <span className={`${torOk ? "text-[#4ade80]" : "text-[#2a2f3f]"}`}>&rarr;</span>
              <div className={`border px-2 py-1 text-center min-w-[100px] ${torOk ? "border-[#4ade80]/30" : "border-[#2a2f3f]"}`}>
                <div className="text-[#6b7280]">TCP</div>
                <div className="text-[#c8ccd4]">Tor</div>
                {tor && <div className="text-[#6b7280] text-[9px]">{tor.status === "Running" ? formatUptime(tor.uptime_secs) : tor.status}</div>}
              </div>
            </div>
            <div className="flex items-center gap-1">
              <span className={`${awgOk ? "text-[#4ade80]" : "text-[#2a2f3f]"}`}>&rarr;</span>
              <div className={`border px-2 py-1 text-center min-w-[100px] ${awgOk ? "border-[#4ade80]/30" : "border-[#2a2f3f]"}`}>
                <div className="text-[#6b7280]">UDP</div>
                <div className="text-[#c8ccd4]">AWG</div>
                {awg && <div className="text-[#6b7280] text-[9px]">{awg.status === "Running" ? formatUptime(awg.uptime_secs) : awg.status}</div>}
              </div>
            </div>
          </div>
          <span className={`${torOk || awgOk ? "text-[#4ade80]" : "text-[#2a2f3f]"}`}>
            &rarr;
          </span>
          <div className="border border-[#2a2f3f] px-2 py-1 text-center min-w-[80px]">
            <div className="text-[#6b7280]">INTERNET</div>
            <div className="text-[#c8ccd4]">{torOk || awgOk ? "TUNNELED" : "DIRECT"}</div>
          </div>
        </div>
        {ks && (
          <div className="mt-2 pt-2 border-t border-[#2a2f3f] flex items-center justify-center gap-2">
            <span className="indicator-critical" />
            <span className="text-[9px] font-mono text-[#ef4444]">
              KILL SWITCH: {pl}{panic?.interfaces_down ? " | IF DOWN" : ""}{panic?.dns_flushed ? " | DNS FLUSH" : ""}{panic?.kernel_caches_purged ? " | CACHE PURGE" : ""}
            </span>
          </div>
        )}
      </div>

      {/* Service details */}
      <div className="card">
        <div className="card-header">
          <Radio className="w-3 h-3 text-[#4ade80]" />
          <span className="card-title">SERVICES</span>
          <span className="ml-auto text-[9px] font-mono text-[#6b7280]">{runningCount}/{totalCount} ACTIVE</span>
        </div>
        <div className="mt-1">
          <table className="proc-table">
            <thead>
              <tr>
                <th></th>
                <th>PROCESS</th>
                <th>STATUS</th>
                <th>UPTIME</th>
                <th>RESTARTS</th>
              </tr>
            </thead>
            <tbody>
              {services.map((svc) => {
                let indicator = "indicator-off";
                let statusText = svc.status;
                if (svc.status === "Running") { indicator = "indicator-ok"; statusText = "RUNNING"; }
                else if (svc.status === "Failed") { indicator = "indicator-critical"; statusText = "FAILED"; }
                else if (svc.status === "Starting" || svc.status === "Restarting") { indicator = "indicator-warn"; statusText = svc.status.toUpperCase(); }
                return (
                  <tr key={svc.name}>
                    <td className="w-4"><span className={`indicator ${indicator}`} /></td>
                    <td>{svc.name}</td>
                    <td>{statusText}</td>
                    <td className="text-[#6b7280]">{svc.uptime_secs > 0 ? formatUptime(svc.uptime_secs) : "-"}</td>
                    <td className="text-[#6b7280]">{svc.restart_count > 0 ? svc.restart_count : "-"}</td>
                  </tr>
                );
              })}
              {services.length === 0 && (
                <tr>
                  <td colSpan={5} className="text-center py-4 text-[#6b7280]">NO SERVICES REGISTERED</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Footer */}
      <div className="text-[8px] font-mono text-[#6b7280] text-center pt-2">
        KRYPTOS // ENDPOINT PRIVACY SUITE // {new Date().toISOString().split("T")[0]}
      </div>
    </div>
  );
}
