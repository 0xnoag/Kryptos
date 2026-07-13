import { useDaemon } from "../lib/daemon-context";
import {
  Wifi,
  Route,
  GitBranch,
  Shield,
  Dns,
} from "lucide-react";

export function Network() {
  const { services } = useDaemon();

  const torRunning = services.find((s) => s.name === "Tor")?.status === "Running";
  const awgRunning = services.find((s) => s.name === "AmneziaWG")?.status === "Running";

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-white">Network Routing</h1>
        <p className="text-gray-400 mt-1">
          Intelligent split routing: TCP &rarr; Tor · UDP &rarr; AmneziaWG
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className={`card border ${torRunning ? "border-privacy-600/30" : "border-gray-800"}`}>
          <div className="flex items-center gap-3 mb-4">
            <div className={`p-2 rounded-lg ${torRunning ? "bg-privacy-600/20" : "bg-gray-800"}`}>
              <Route className={`w-5 h-5 ${torRunning ? "text-privacy-400" : "text-gray-600"}`} />
            </div>
            <div>
              <h3 className="font-semibold text-white">TCP Path: Tor</h3>
              <p className="text-xs text-gray-500">
                SOCKS5 proxy on 127.0.0.1:9050
              </p>
            </div>
          </div>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between text-gray-400">
              <span>All TCP traffic</span>
              <span className={torRunning ? "text-privacy-400" : "text-gray-600"}>
                {torRunning ? "Routed via Tor" : "Direct"}
              </span>
            </div>
            <div className="flex justify-between text-gray-400">
              <span>DNS over Tor</span>
              <span className="text-gray-500">127.0.0.1:5353</span>
            </div>
            <div className="flex justify-between text-gray-400">
              <span>obfs4 transport</span>
              <span className={torRunning ? "text-privacy-400" : "text-gray-600"}>
                {torRunning ? "Active" : "Inactive"}
              </span>
            </div>
          </div>
        </div>

        <div className={`card border ${awgRunning ? "border-privacy-600/30" : "border-gray-800"}`}>
          <div className="flex items-center gap-3 mb-4">
            <div className={`p-2 rounded-lg ${awgRunning ? "bg-privacy-600/20" : "bg-gray-800"}`}>
              <Wifi className={`w-5 h-5 ${awgRunning ? "text-privacy-400" : "text-gray-600"}`} />
            </div>
            <div>
              <h3 className="font-semibold text-white">UDP Path: AmneziaWG</h3>
              <p className="text-xs text-gray-500">
                Obfuscated WireGuard tunnel on awg0
              </p>
            </div>
          </div>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between text-gray-400">
              <span>VoIP / Gaming UDP</span>
              <span className={awgRunning ? "text-privacy-400" : "text-gray-600"}>
                {awgRunning ? "Tunneled" : "Direct"}
              </span>
            </div>
            <div className="flex justify-between text-gray-400">
              <span>Streaming ports</span>
              <span className={awgRunning ? "text-privacy-400" : "text-gray-600"}>
                {awgRunning ? "Tunneled" : "Direct"}
              </span>
            </div>
            <div className="flex justify-between text-gray-400">
              <span>Jitter reduction</span>
              <span className={awgRunning ? "text-privacy-400" : "text-gray-600"}>
                {awgRunning ? "Active" : "N/A"}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="panel-title">
          <GitBranch className="w-5 h-5" />
          Traffic Classifier Rules
        </div>
        <div className="mt-4 space-y-3">
          <div className="flex items-center justify-between p-3 bg-gray-950 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 rounded-full bg-privacy-500" />
              <span className="text-sm text-gray-300">TCP (port 6)</span>
            </div>
            <span className="text-sm text-privacy-400 font-mono">
              &rarr; Tor SOCKS5 127.0.0.1:9050
            </span>
          </div>
          <div className="flex items-center justify-between p-3 bg-gray-950 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 rounded-full bg-blue-500" />
              <span className="text-sm text-gray-300">UDP (port 17)</span>
            </div>
            <span className="text-sm text-blue-400 font-mono">
              &rarr; AmneziaWG tunnel awg0
            </span>
          </div>
          <div className="flex items-center justify-between p-3 bg-gray-950 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 rounded-full bg-yellow-500" />
              <span className="text-sm text-gray-300">DNS (port 53/853)</span>
            </div>
            <span className="text-sm text-yellow-400 font-mono">
              &rarr; Local DoH 127.0.0.1:53
            </span>
          </div>
          <div className="flex items-center justify-between p-3 bg-gray-950 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 rounded-full bg-gray-600" />
              <span className="text-sm text-gray-300">Local network (10/8, 172.16/12, 192.168/16)</span>
            </div>
            <span className="text-sm text-gray-500 font-mono">
              &rarr; Direct (bypass)
            </span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="panel-title">
          <Shield className="w-5 h-5" />
          MAC Spoofing
        </div>
        <p className="text-gray-400 text-sm mt-2">
          Randomized MAC address rotation for all non-excluded interfaces.
        </p>
        <div className="mt-4 flex items-center gap-4">
          <button className="btn-primary">Randomize Now</button>
          <span className="text-xs text-gray-500">
            Rotation interval: 10 minutes
          </span>
        </div>
      </div>
    </div>
  );
}
