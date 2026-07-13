import { useDaemon } from "../lib/daemon-context";
import { Shield, ShieldOff, Ban, ArrowUpDown } from "lucide-react";

const LEVELS = [
  {
    key: "Off",
    label: "Off",
    desc: "No firewall restrictions. All traffic flows normally through the system routing table.",
    icon: ShieldOff,
    color: "text-[#64748b]",
    border: "border-[#2a2e3d]",
    dot: "status-dot-muted",
  },
  {
    key: "Soft",
    label: "Soft",
    desc: "New non-tunnel connections blocked. Established flows persist. Outbound DNS restricted to tunnel interfaces.",
    icon: ArrowUpDown,
    color: "text-[#fbbf24]",
    border: "border-[#fbbf24]/20",
    dot: "status-dot-warn",
  },
  {
    key: "Hard",
    label: "Hard",
    desc: "All traffic blocked except via tunnel interfaces (tun+, wg+, obfs+). DNS exceptions restricted to configured upstream IP only.",
    icon: Shield,
    color: "text-[#f87171]",
    border: "border-[#f87171]/20",
    dot: "status-dot-critical",
  },
  {
    key: "Nuclear",
    label: "Nuclear",
    desc: "Complete isolation: loopback only. All interfaces taken down. DNS/ARP caches flushed. Kernel page caches purged.",
    icon: Ban,
    color: "text-[#f87171]",
    border: "border-[#f87171]/40",
    dot: "status-dot-critical",
  },
] as const;

export function Firewall() {
  const { panic, setPanicLevel } = useDaemon();

  const currentLevel = panic?.level ?? "Off";
  const current = LEVELS.find((l) => l.key === currentLevel) ?? LEVELS[0];

  return (
    <div className="space-y-5 max-w-[1280px]">
      <div>
        <h1 className="text-lg font-semibold text-[#e2e8f0]">Firewall</h1>
        <p className="text-caption mt-0.5">Kill switch and nftables rule management</p>
      </div>

      {/* Current state */}
      <div className="card">
        <div className="card-header">
          <current.icon className={`w-4 h-4 ${current.color}`} />
          <span className="card-title">Kill switch: {currentLevel}</span>
          {panic?.kill_switch_active && (
            <span className="ml-auto flex items-center gap-1.5 text-[11px] text-[#f87171]">
              <span className="status-dot-critical" />
              Active
            </span>
          )}
        </div>
        <p className="mt-3 text-[13px] text-[#94a3b8] leading-relaxed">
          {current.desc}
        </p>
        {panic && currentLevel !== "Off" && (
          <div className="mt-3 flex flex-wrap gap-3">
            {panic.interfaces_down && (
              <span className="text-[11px] text-[#6ee7b7]">Interfaces: down</span>
            )}
            {panic.dns_flushed && (
              <span className="text-[11px] text-[#6ee7b7]">DNS cache: flushed</span>
            )}
            {panic.kernel_caches_purged && (
              <span className="text-[11px] text-[#6ee7b7]">Kernel caches: purged</span>
            )}
          </div>
        )}
      </div>

      {/* Level selection */}
      <div className="grid grid-cols-4 gap-3">
        {LEVELS.map((level) => {
          const Icon = level.icon;
          const isActive = currentLevel === level.key;

          return (
            <button
              key={level.key}
              onClick={() => setPanicLevel(level.key.toLowerCase())}
              disabled={isActive}
              className={`card text-left transition-all duration-150 cursor-pointer
                ${isActive ? level.border + " ring-1 ring-[#5eead4]/30" : "hover:border-[#5eead4]/20"}
                ${level.key === "Nuclear" ? "border-[#f87171]/20" : ""}`}
            >
              <div className="flex items-center gap-2 mb-2">
                <Icon className={`w-4 h-4 ${isActive ? level.color : "text-[#2a2e3d]"}`} />
                {isActive && <span className={level.dot} />}
              </div>
              <span className="text-sm font-semibold text-[#e2e8f0]">{level.label}</span>
              <p className="text-[11px] text-[#64748b] mt-1 leading-relaxed">
                {level.desc}
              </p>
            </button>
          );
        })}
      </div>

      {/* Policy detail */}
      <div className="card">
        <div className="card-header">
          <Shield className="w-4 h-4 text-[#5eead4]" />
          <span className="card-title">nftables chains</span>
        </div>
        <div className="mt-3 space-y-3">
          {(["privacy_input", "privacy_output", "privacy_forward"] as const).map((chain) => {
            const policy = currentLevel === "Off" ? "accept" : "drop";
            return (
              <div key={chain} className="surface-input rounded-md p-3">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-label">{chain.replace("privacy_", "")}</span>
                  <span className="text-[11px] font-mono text-[#64748b]">
                    policy {policy}
                  </span>
                  {panic?.kill_switch_active && policy === "drop" && (
                    <span className="status-dot-critical" />
                  )}
                </div>
                <div className="font-mono text-[11px] text-[#64748b] space-y-0.5">
                  {chain === "privacy_input" && (
                    <>
                      <div>iif "lo" accept</div>
                      <div>ct state established,related accept</div>
                      <div>{'iifname { "tun+", "wg0", "wg+", "obfs+" } accept'}</div>
                      <div>reject with icmpx type admin-prohibited</div>
                    </>
                  )}
                  {chain === "privacy_output" && (
                    <>
                      <div>oif "lo" accept</div>
                      <div>ct state established,related accept</div>
                      <div>{'oifname { "tun+", "wg0", "wg+", "obfs+" } accept'}</div>
                      {currentLevel === "Hard" && (
                        <div className="text-[#5eead4]">{'ip daddr upstream-ip udp/tcp dport { 53, 853 } accept'}</div>
                      )}
                      <div>reject with icmpx type admin-prohibited</div>
                    </>
                  )}
                  {chain === "privacy_forward" && (
                    <>
                      <div>{'iifname { "tun+", "wg0", "wg+", "obfs+" } oifname != "lo" accept'}</div>
                      <div>reject with icmpx type admin-prohibited</div>
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
