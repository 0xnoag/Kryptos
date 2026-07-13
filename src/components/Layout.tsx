import { Outlet, NavLink, useLocation } from "react-router-dom";
import { useDaemon } from "../lib/daemon-context";
import {
  Shield,
  Activity,
  Wifi,
  Settings,
  Server,
  AlertTriangle,
} from "lucide-react";

const navItems = [
  { to: "/", icon: Shield, label: "Dashboard" },
  { to: "/services", icon: Server, label: "Services" },
  { to: "/firewall", icon: AlertTriangle, label: "Firewall" },
  { to: "/network", icon: Wifi, label: "Network" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

export function Layout() {
  const { connected, panic } = useDaemon();

  return (
    <div className="min-h-screen bg-gray-950 flex">
      <aside className="w-64 bg-gray-900 border-r border-gray-800 flex flex-col">
        <div className="p-6 border-b border-gray-800">
          <div className="flex items-center gap-3">
            <Shield className="w-8 h-8 text-privacy-500" />
            <div>
              <h1 className="text-lg font-bold text-white tracking-tight">
                Privacy Suite
              </h1>
              <div className="flex items-center gap-2 mt-0.5">
                <span
                  className={`w-2 h-2 rounded-full ${
                    connected ? "bg-privacy-500" : "bg-danger-500"
                  }`}
                />
                <span className="text-xs text-gray-500">
                  {connected ? "Daemon Connected" : "Disconnected"}
                </span>
              </div>
            </div>
          </div>
        </div>

        <nav className="flex-1 p-4 space-y-1">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              end={to === "/"}
              className={({ isActive }) =>
                `flex items-center gap-3 px-4 py-2.5 rounded-lg transition-colors ${
                  isActive
                    ? "bg-privacy-600/20 text-privacy-400 border border-privacy-600/30"
                    : "text-gray-400 hover:text-gray-200 hover:bg-gray-800"
                }`
              }
            >
              <Icon className="w-5 h-5" />
              <span className="font-medium">{label}</span>
            </NavLink>
          ))}
        </nav>

        {panic && panic.kill_switch_active && (
          <div className="p-4 border-t border-danger-800 bg-danger-900/30">
            <div className="flex items-center gap-2 text-danger-400">
              <AlertTriangle className="w-4 h-4" />
              <span className="text-xs font-medium uppercase tracking-wider">
                Panic: {panic.level}
              </span>
            </div>
          </div>
        )}
      </aside>

      <main className="flex-1 overflow-auto">
        <div className="p-8">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
