import { useDaemon } from "../lib/daemon-context";
import {
  Shield,
  Globe,
  Lock,
  Activity,
  AlertTriangle,
  CheckCircle,
  XCircle,
} from "lucide-react";

export function Dashboard() {
  const { services, panic, setPanicLevel } = useDaemon();

  const runningCount = services.filter((s) => s.status === "Running").length;
  const totalCount = services.length;
  const allRunning = runningCount === totalCount && totalCount > 0;

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-white">Dashboard</h1>
        <p className="text-gray-400 mt-1">Endpoint Privacy Suite Overview</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Protection Status</p>
              <div className="flex items-center gap-2 mt-1">
                {allRunning ? (
                  <>
                    <CheckCircle className="w-5 h-5 text-privacy-500" />
                    <span className="text-2xl font-bold text-privacy-500">
                      Active
                    </span>
                  </>
                ) : (
                  <>
                    <XCircle className="w-5 h-5 text-danger-500" />
                    <span className="text-2xl font-bold text-danger-500">
                      Inactive
                    </span>
                  </>
                )}
              </div>
            </div>
            <Shield
              className={`w-12 h-12 ${
                allRunning ? "text-privacy-500" : "text-gray-700"
              }`}
            />
          </div>
        </div>

        <div className="card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Services Running</p>
              <p className="text-2xl font-bold text-white mt-1">
                {runningCount} / {totalCount}
              </p>
            </div>
            <Activity className="w-12 h-12 text-gray-700" />
          </div>
          <div className="meter-bar mt-4">
            <div
              className="meter-fill"
              style={{
                width: `${totalCount > 0 ? (runningCount / totalCount) * 100 : 0}%`,
              }}
            />
          </div>
        </div>

        <div className="card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Panic Level</p>
              <p className="text-2xl font-bold text-white mt-1 capitalize">
                {panic?.level ?? "Off"}
              </p>
            </div>
            <Lock
              className={`w-12 h-12 ${
                panic?.kill_switch_active
                  ? "text-danger-500"
                  : "text-gray-700"
              }`}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="panel-title">
          <AlertTriangle className="w-5 h-5 text-danger-400" />
          Panic Button
        </div>
        <p className="text-gray-400 text-sm">
          Activate the kill switch to immediately block all non-tunnel traffic.
          Nuclear mode also drops all interfaces and flushes DNS cache.
        </p>
        <div className="flex flex-wrap gap-3 mt-4">
          <button
            onClick={() => setPanicLevel("off")}
            className="btn-outline"
            disabled={panic?.level === "Off"}
          >
            Disarm
          </button>
          <button
            onClick={() => setPanicLevel("soft")}
            className="btn-outline"
            disabled={panic?.level === "Soft"}
          >
            Soft
          </button>
          <button
            onClick={() => setPanicLevel("hard")}
            className="btn-outline"
            disabled={panic?.level === "Hard"}
          >
            Hard
          </button>
          <button
            onClick={() => setPanicLevel("nuclear")}
            className="btn-danger"
            disabled={panic?.level === "Nuclear"}
          >
            Nuclear
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {services.map((svc) => (
          <div key={svc.name} className="card">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <span
                  className={`w-3 h-3 rounded-full ${
                    svc.status === "Running"
                      ? "bg-privacy-500"
                      : svc.status === "Failed"
                        ? "bg-danger-500"
                        : "bg-gray-600"
                  }`}
                />
                <div>
                  <p className="text-white font-medium capitalize">
                    {svc.name}
                  </p>
                  <p className="text-xs text-gray-500">
                    {svc.status === "Running"
                      ? `PID ${svc.pid} · ${Math.floor(svc.uptime_secs / 60)}m uptime`
                      : svc.status === "Failed"
                        ? "Error state"
                        : "Stopped"}
                  </p>
                </div>
              </div>
              {svc.status === "Running" && (
                <Globe className="w-5 h-5 text-privacy-500" />
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
