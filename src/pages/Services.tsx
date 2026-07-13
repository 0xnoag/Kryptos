import { useDaemon } from "../lib/daemon-context";
import { Play, Square, RotateCcw, Radio } from "lucide-react";

const serviceLabels: Record<string, string> = {
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
    <div className="space-y-5 max-w-[1280px]">
      <div>
        <h1 className="text-lg font-semibold text-[#e2e8f0]">Services</h1>
        <p className="text-caption mt-0.5">Manage the core privacy engine processes</p>
      </div>

      <div className="space-y-3">
        {services.map((svc) => {
          let dotClass = "status-dot-muted";
          let statusLabel = "Stopped";
          if (svc.status === "Running") {
            dotClass = "status-dot-ok";
            statusLabel = "Running";
          } else if (svc.status === "Failed") {
            dotClass = "status-dot-critical";
            statusLabel = "Failed";
          } else if (svc.status === "Starting") {
            dotClass = "status-dot-warn";
            statusLabel = "Starting";
          } else if (svc.status === "Restarting") {
            dotClass = "status-dot-warn";
            statusLabel = "Restarting";
          }

          return (
            <div key={svc.name} className="card">
              <div className="flex items-start justify-between">
                <div className="flex items-start gap-3">
                  <div className={`p-2 rounded-md ${
                    svc.status === "Running"
                      ? "bg-[#064e3b]"
                      : "bg-[#232738]"
                  }`}>
                    <Radio className={`w-4 h-4 ${
                      svc.status === "Running"
                        ? "text-[#6ee7b7]"
                        : "text-[#64748b]"
                    }`} />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <h3 className="text-sm font-semibold text-[#e2e8f0]">
                        {svc.name}
                      </h3>
                      <span className={dotClass} />
                      <span className="text-[11px] text-[#64748b]">{statusLabel}</span>
                    </div>
                    <p className="text-[11px] text-[#64748b] mt-0.5">
                      {serviceLabels[svc.name] ?? ""}
                    </p>
                    <div className="flex items-center gap-3 mt-1.5">
                      {svc.uptime_secs > 0 && (
                        <span className="text-caption font-mono">
                          {formatUptime(svc.uptime_secs)}
                        </span>
                      )}
                      {svc.restart_count > 0 && (
                        <span className="text-caption">
                          Restarts: {svc.restart_count}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  {svc.status !== "Running" ? (
                    <button
                      onClick={() => startService(svc.name)}
                      className="btn-primary flex items-center gap-1.5"
                      disabled={svc.status === "Starting" || svc.status === "Restarting"}
                    >
                      <Play className="w-3 h-3" />
                      Start
                    </button>
                  ) : (
                    <>
                      <button
                        onClick={() => restartService(svc.name)}
                        className="btn-secondary flex items-center gap-1.5"
                      >
                        <RotateCcw className="w-3 h-3" />
                        Restart
                      </button>
                      <button
                        onClick={() => stopService(svc.name)}
                        className="btn-critical flex items-center gap-1.5"
                      >
                        <Square className="w-3 h-3" />
                        Stop
                      </button>
                    </>
                  )}
                </div>
              </div>
            </div>
          );
        })}

        {services.length === 0 && (
          <div className="card py-8 text-center">
            <Radio className="w-8 h-8 text-[#2a2e3d] mx-auto mb-2" />
            <p className="text-[13px] text-[#64748b]">No services registered</p>
            <p className="text-caption mt-1">Daemon may not be connected</p>
          </div>
        )}
      </div>
    </div>
  );
}
