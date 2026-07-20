import { useDaemon } from "../lib/daemon-context";

const serviceDescs: Record<string, string> = {
  Tor: "TCP anonymization via Tor with obfs4 bridging",
  Obfs4Proxy: "Traffic obfuscation proxy for Tor bridges",
  AmneziaWG: "Obfuscated WireGuard UDP tunnel",
  Syncthing: "P2P encrypted file synchronization",
};

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

export function Services() {
  const { services, startService, stopService, restartService } = useDaemon();

  return (
    <div className="space-y-3 max-w-[1280px]">
      <div>
        <div className="page-title">SERVICES</div>
        <div className="page-subtitle">PROCESS CONTROL // SERVICE MANAGEMENT</div>
      </div>

      <table className="proc-table">
        <thead>
          <tr>
            <th></th>
            <th>PROCESS</th>
            <th>STATUS</th>
            <th>UPTIME</th>
            <th>PID</th>
            <th>RESTARTS</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {services.map((svc) => {
            let indicator = "indicator-off";
            let statusClass = "value-dim";
            if (svc.status === "Running") { indicator = "indicator-ok"; statusClass = "value-ok"; }
            else if (svc.status === "Failed") { indicator = "indicator-critical"; statusClass = "value-critical"; }
            else if (svc.status === "Starting" || svc.status === "Restarting") { indicator = "indicator-warn"; statusClass = "value-warn"; }

            return (
              <tr key={svc.name}>
                <td className="w-4"><span className={`indicator ${indicator}`} /></td>
                <td className="font-semibold">{svc.name}</td>
                <td className={statusClass}>{svc.status.toUpperCase()}</td>
                <td className="text-[#6b7280]">{svc.uptime_secs > 0 ? formatUptime(svc.uptime_secs) : "-"}</td>
                <td className="text-[#6b7280]">{svc.pid ?? "-"}</td>
                <td className="text-[#6b7280]">{svc.restart_count > 0 ? svc.restart_count : "-"}</td>
                <td className="text-right">
                  {svc.status !== "Running" ? (
                    <button
                      onClick={() => startService(svc.name)}
                      className="btn-primary"
                      disabled={svc.status === "Starting" || svc.status === "Restarting"}
                    >
                      START
                    </button>
                  ) : (
                    <div className="flex gap-1">
                      <button
                        onClick={() => restartService(svc.name)}
                        className="btn-ghost"
                      >
                        RESTART
                      </button>
                      <button
                        onClick={() => stopService(svc.name)}
                        className="btn"
                      >
                        STOP
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>

      {services.length === 0 && (
        <div className="card py-4 text-center">
          <div className="text-[11px] font-mono text-[#6b7280]">NO SERVICES REGISTERED</div>
          <div className="text-[9px] font-mono text-[#6b7280] mt-1">DAEMON MAY NOT BE CONNECTED</div>
        </div>
      )}

      {/* Process descriptions */}
      {services.length > 0 && (
        <div className="card">
          <div className="card-header">
            <span className="card-title">PROCESS INFORMATION</span>
          </div>
          <div className="mt-2 space-y-px">
            {services.map((svc) => (
              <div key={svc.name} className="data-row">
                <span className="data-label">{svc.name}</span>
                <span className="text-[10px] text-[#6b7280] font-mono">
                  {serviceDescs[svc.name] ?? ""}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
