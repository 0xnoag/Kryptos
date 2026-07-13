import { useDaemon } from "../lib/daemon-context";
import { Play, Square, RotateCcw, Server, Activity } from "lucide-react";

const serviceLabels: Record<string, string> = {
  Tor: "TCP Anonymization via Tor + obfs4",
  Obfs4Proxy: "Traffic Obfuscation Proxy",
  AmneziaWG: "Obfuscated WireGuard UDP Tunnel",
  Syncthing: "P2P Encrypted File Sync",
};

export function Services() {
  const { services, startService, stopService, restartService } = useDaemon();

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-white">Services</h1>
        <p className="text-gray-400 mt-1">
          Manage the core privacy engine binaries
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6">
        {services.map((svc) => (
          <div key={svc.name} className="card">
            <div className="flex items-start justify-between">
              <div className="flex items-start gap-4">
                <div
                  className={`p-3 rounded-lg ${
                    svc.status === "Running"
                      ? "bg-privacy-600/20 text-privacy-400"
                      : "bg-gray-800 text-gray-500"
                  }`}
                >
                  <Server className="w-6 h-6" />
                </div>
                <div>
                  <h3 className="text-lg font-semibold text-white capitalize">
                    {svc.name}
                  </h3>
                  <p className="text-sm text-gray-400 mt-1">
                    {serviceLabels[svc.name] ?? ""}
                  </p>
                  <div className="flex items-center gap-4 mt-3 text-sm">
                    <div className="flex items-center gap-2">
                      <span
                        className={`w-2.5 h-2.5 rounded-full ${
                          svc.status === "Running"
                            ? "bg-privacy-500"
                            : svc.status === "Failed"
                              ? "bg-danger-500"
                              : svc.status === "Starting"
                                ? "bg-yellow-500 animate-pulse"
                                : "bg-gray-600"
                        }`}
                      />
                      <span className="text-gray-400">{svc.status}</span>
                    </div>
                    {svc.pid && (
                      <span className="text-gray-500">PID: {svc.pid}</span>
                    )}
                    {svc.uptime_secs > 0 && (
                      <span className="text-gray-500">
                        Uptime: {Math.floor(svc.uptime_secs / 60)}m{" "}
                        {svc.uptime_secs % 60}s
                      </span>
                    )}
                    {svc.restart_count > 0 && (
                      <span className="text-gray-500">
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
                    className="btn-primary flex items-center gap-2"
                    disabled={svc.status === "Starting"}
                  >
                    <Play className="w-4 h-4" />
                    Start
                  </button>
                ) : (
                  <>
                    <button
                      onClick={() => restartService(svc.name)}
                      className="btn-outline flex items-center gap-2"
                    >
                      <RotateCcw className="w-4 h-4" />
                      Restart
                    </button>
                    <button
                      onClick={() => stopService(svc.name)}
                      className="btn-danger flex items-center gap-2"
                    >
                      <Square className="w-4 h-4" />
                      Stop
                    </button>
                  </>
                )}
              </div>
            </div>

            {svc.status === "Running" && (
              <div className="mt-4 pt-4 border-t border-gray-800">
                <div className="flex items-center gap-2 text-sm text-gray-500">
                  <Activity className="w-4 h-4" />
                  <span>Traffic flowing through encrypted tunnel</span>
                </div>
              </div>
            )}
          </div>
        ))}

        {services.length === 0 && (
          <div className="card text-center py-12">
            <Server className="w-12 h-12 text-gray-700 mx-auto mb-4" />
            <p className="text-gray-400">No services registered</p>
            <p className="text-gray-600 text-sm mt-1">
              Daemon may not be connected
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
