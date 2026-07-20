import { Outlet, NavLink } from "react-router-dom";
import { useDaemon } from "../lib/daemon-context";
import { Activity, Radio, Gauge, Route, Terminal } from "lucide-react";

const navItems = [
  { to: "/", icon: Gauge, label: "Dashboard" },
  { to: "/services", icon: Radio, label: "Services" },
  { to: "/firewall", icon: Activity, label: "Firewall" },
  { to: "/network", icon: Route, label: "Network" },
  { to: "/settings", icon: Terminal, label: "Settings" },
];

export function Layout() {
  const { connected, panic, services, error } = useDaemon();

  const runningCount = services.filter((s) => s.status === "Running").length;
  const totalCount = services.length;
  const panicLevel = panic?.level ?? "OFF";

  let statusIndicator = "indicator-off";
  let statusLabel = "DISCONNECTED";
  if (connected) {
    if (panicLevel === "NUCLEAR") {
      statusIndicator = "indicator-critical";
      statusLabel = "NUCLEAR";
    } else if (panicLevel === "HARD" || panicLevel === "SOFT") {
      statusIndicator = "indicator-warn";
      statusLabel = `${panicLevel} KS`;
    } else if (runningCount > 0) {
      statusIndicator = "indicator-ok";
      statusLabel = `${runningCount}/${totalCount}`;
    } else {
      statusIndicator = "indicator-off";
      statusLabel = "IDLE";
    }
  } else if (error) {
    statusIndicator = "indicator-critical";
    statusLabel = "ERR";
  }

  return (
    <div className="min-h-screen flex flex-col" style={{ background: "#0a0b0e" }}>
      {/* Classification banner */}
      <div className="classification">
        <span>TOP SECRET // ENDPOINT PRIVACY // KRYPTOS</span>
      </div>

      {/* Status bar */}
      <div className="status-bar">
        <span className="font-mono text-[10px] text-[#4ade80] tracking-widest uppercase">
          KRYPTOS
        </span>
        <span className="text-[#2a2f3f]">|</span>
        <div className="status-bar-item">
          <span className={statusIndicator} />
          <span>{statusLabel}</span>
        </div>
        {panic?.kill_switch_active && (
          <div className="status-bar-item">
            <span className="indicator-critical" />
            <span className="text-[#ef4444]">KS:{panicLevel}</span>
          </div>
        )}
        <div className="ml-auto flex items-center gap-3 text-[#6b7280]">
          <span className="text-[9px] font-mono">
            {runningCount}/{totalCount} svc
          </span>
          <span className="text-[9px] font-mono">
            {new Date().toLocaleTimeString()}
          </span>
        </div>
      </div>

      {/* Body */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside className="sidebar">
          <div className="sidebar-label">Navigation</div>
          <nav className="flex-1 space-y-px px-2">
            {navItems.map(({ to, icon: Icon, label }) => (
              <NavLink
                key={to}
                to={to}
                end={to === "/"}
                className={({ isActive }) =>
                  `sidebar-link ${isActive ? "sidebar-link-active" : ""}`
                }
              >
                <Icon className="w-3 h-3" />
                <span>{label}</span>
              </NavLink>
            ))}
          </nav>
          <div className="px-3 py-2 border-t border-[#2a2f3f]">
            <span className={`indicator ${connected ? "indicator-ok" : "indicator-critical"}`} />
            <span className="ml-2 text-[9px] font-mono text-[#6b7280]">
              {connected ? "DAEMON OK" : "DAEMON DOWN"}
            </span>
          </div>
        </aside>

        {/* Content */}
        <main className="flex-1 overflow-auto p-4" style={{ background: "#0a0b0e" }}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
