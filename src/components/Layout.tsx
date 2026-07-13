import { Outlet, NavLink } from "react-router-dom";
import { useDaemon } from "../lib/daemon-context";
import {
  Activity,
  Radio,
  Gauge,
  Route,
  Terminal,
  CircleDot,
} from "lucide-react";

const navItems = [
  { to: "/", icon: Gauge, label: "Dashboard" },
  { to: "/services", icon: Radio, label: "Services" },
  { to: "/firewall", icon: Activity, label: "Firewall" },
  { to: "/network", icon: Route, label: "Network" },
  { to: "/settings", icon: Terminal, label: "Settings" },
];

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

export function Layout() {
  const { connected, panic, services } = useDaemon();

  const runningCount = services.filter((s) => s.status === "Running").length;
  const totalCount = services.length;
  const panicLevel = panic?.level ?? "Off";

  let statusDotClass = "status-dot-muted";
  let statusText = "Disconnected";
  if (connected) {
    if (panicLevel === "Nuclear") {
      statusDotClass = "status-dot-critical";
      statusText = "Nuclear panic";
    } else if (panicLevel === "Hard" || panicLevel === "Soft") {
      statusDotClass = "status-dot-warn";
      statusText = `${panicLevel} kill switch`;
    } else if (runningCount > 0) {
      statusDotClass = "status-dot-ok";
      statusText = `${runningCount}/${totalCount} services`;
    } else {
      statusDotClass = "status-dot-muted";
      statusText = "Connected, idle";
    }
  }

  return (
    <div className="min-h-screen surface-page flex flex-col">
      {/* Persistent top status bar */}
      <div className="status-bar flex-shrink-0">
        <div className="flex items-center gap-2 mr-4">
          <Activity className="w-4 h-4 text-[#5eead4]" />
          <span className="text-xs font-semibold text-[#e2e8f0] tracking-wider uppercase">
            Kryptos
          </span>
        </div>

        <div className="h-3 w-px bg-[#2a2e3d]" />

        <div className="status-bar-item">
          <span className={`${statusDotClass}`} />
          <span className="text-[#e2e8f0]">{statusText}</span>
        </div>

        {panic?.kill_switch_active && (
          <div className="status-bar-item">
            <span className="status-dot-critical" />
            <span className="text-[#f87171]">Kill switch: {panicLevel}</span>
          </div>
        )}

        <div className="ml-auto flex items-center gap-4">
          {panic && (
            <span className="text-caption">
              Panic: {panicLevel}
            </span>
          )}
          <span className="text-caption">
            {runningCount}/{totalCount} running
          </span>
        </div>
      </div>

      {/* Body: sidebar + content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside className="w-48 border-r border-[#2a2e3d] flex flex-col flex-shrink-0 bg-[#1a1d27]">
          <nav className="flex-1 p-3 space-y-0.5">
            {navItems.map(({ to, icon: Icon, label }) => (
              <NavLink
                key={to}
                to={to}
                end={to === "/"}
                className={({ isActive }) =>
                  `flex items-center gap-3 px-3 py-2 rounded-md transition-colors duration-150 text-sm ${
                    isActive
                      ? "bg-[#5eead4]/10 text-[#5eead4] border border-[#5eead4]/20"
                      : "text-[#64748b] hover:text-[#e2e8f0] hover:bg-[#232738]"
                  }`
                }
              >
                <Icon className="w-4 h-4" />
                <span className="font-medium">{label}</span>
              </NavLink>
            ))}
          </nav>

          <div className="p-3 border-t border-[#2a2e3d]">
            <div className="flex items-center gap-2 text-caption">
              <CircleDot
                className={`w-3 h-3 ${
                  connected ? "text-[#6ee7b7]" : "text-[#f87171]"
                }`}
              />
              <span>{connected ? "Daemon connected" : "Disconnected"}</span>
            </div>
          </div>
        </aside>

        {/* Content */}
        <main className="flex-1 overflow-auto p-5">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
