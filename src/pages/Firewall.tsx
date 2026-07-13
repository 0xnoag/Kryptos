import { useDaemon } from "../lib/daemon-context";
import {
  Shield,
  ShieldOff,
  AlertTriangle,
  SkipForward,
  Ban,
} from "lucide-react";

const panicDescriptions: Record<string, { desc: string; icon: typeof Shield; color: string }> = {
  Off: {
    desc: "No firewall restrictions. All traffic flows normally.",
    icon: ShieldOff,
    color: "text-gray-500",
  },
  Soft: {
    desc: "Blocks new connections outside tunnel interfaces. Established connections persist. DNS (53/853) still allowed for resolution.",
    icon: SkipForward,
    color: "text-yellow-500",
  },
  Hard: {
    desc: "Blocks all traffic except on tunnel interfaces (tun+, wg+, obfs+). No DNS exceptions. Full kill switch active.",
    icon: Shield,
    color: "text-orange-500",
  },
  Nuclear: {
    desc: "Complete blackout. Only loopback allowed. All interfaces taken down. DNS cache flushed. System memory purged.",
    icon: Ban,
    color: "text-danger-500",
  },
};

export function Firewall() {
  const { panic, setPanicLevel } = useDaemon();

  const current = panic
    ? panicDescriptions[panic.level] ?? panicDescriptions["Off"]
    : panicDescriptions["Off"];
  const Icon = current.icon;

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-bold text-white">Firewall & Kill Switch</h1>
        <p className="text-gray-400 mt-1">
          nftables-based panic engine with zero-leak guarantee
        </p>
      </div>

      <div className="card">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Icon className={`w-8 h-8 ${current.color}`} />
            <div>
              <h2 className="text-xl font-bold text-white capitalize">
                {panic?.level ?? "Off"}
              </h2>
              <p className="text-sm text-gray-400 mt-1">{current.desc}</p>
            </div>
          </div>
          {panic?.kill_switch_active && (
            <div className="flex items-center gap-2 bg-danger-900/30 text-danger-400 px-3 py-1.5 rounded-lg text-sm">
              <AlertTriangle className="w-4 h-4" />
              Active
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {(["Off", "Soft", "Hard", "Nuclear"] as const).map((level) => {
          const info = panicDescriptions[level];
          const LIcon = info.icon;
          const isActive = panic?.level === level;

          return (
            <button
              key={level}
              onClick={() => setPanicLevel(level.toLowerCase())}
              disabled={isActive}
              className={`card text-left transition-all hover:border-gray-600 ${
                isActive ? "ring-2 ring-privacy-500" : ""
              } ${level === "Nuclear" ? "border-danger-800" : ""}`}
            >
              <LIcon
                className={`w-8 h-8 mb-3 ${
                  isActive ? info.color : "text-gray-600"
                }`}
              />
              <h3 className="font-semibold text-white">{level}</h3>
              <p className="text-xs text-gray-500 mt-1">{info.desc}</p>
            </button>
          );
        })}
      </div>

      <div className="card">
        <h3 className="text-lg font-semibold text-white mb-4">
          Active nftables Rules
        </h3>
        <div className="bg-gray-950 rounded-lg p-4 font-mono text-sm text-gray-400">
          <pre className="overflow-x-auto">
{`table inet endpoint_privacy {
  chain privacy_input {
    type filter hook input priority 0; policy ${panic?.kill_switch_active ? "drop" : "accept"};
    iif "lo" accept
    ct state established,related accept
    iifname { "tun+", "wg0", "wg+", "obfs+" } accept
    reject with icmpx type admin-prohibited
  }
  chain privacy_output {
    type filter hook output priority 0; policy ${panic?.kill_switch_active ? "drop" : "accept"};
    oif "lo" accept
    ct state established,related accept
    oifname { "tun+", "wg0", "wg+", "obfs+" } accept
    reject with icmpx type admin-prohibited
  }
}`}
          </pre>
        </div>
      </div>
    </div>
  );
}
