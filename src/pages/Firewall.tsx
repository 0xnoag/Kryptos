import { useState } from "react";
import { useDaemon } from "../lib/daemon-context";

const LEVELS = [
  { key: "Off", label: "OFF", desc: "No firewall restrictions. All traffic flows normally.", color: "#6b7280", dot: "indicator-off" },
  { key: "Soft", label: "SOFT", desc: "New non-tunnel connections blocked. Established flows persist.", color: "#f59e0b", dot: "indicator-warn" },
  { key: "Hard", label: "HARD", desc: "All traffic blocked except tunnels. DNS restricted to upstream IP.", color: "#ef4444", dot: "indicator-critical" },
  { key: "Nuclear", label: "NUCLEAR", desc: "Complete isolation: loopback only. Interfaces down. Caches purged.", color: "#ef4444", dot: "indicator-critical" },
] as const;

const NUCLEAR_CONFIRM = "CONFIRM_NUCLEAR_I_AM_SURE";

export function Firewall() {
  const { panic, setPanicLevel } = useDaemon();
  const [confirmText, setConfirmText] = useState("");

  const currentKey = panic?.level ?? "Off";
  const current = LEVELS.find((l) => l.key === currentKey) ?? LEVELS[0];

  const [pendingNuclear, setPendingNuclear] = useState(false);

  const handleSet = (key: string) => {
    if (key.toLowerCase() === "nuclear") {
      if (currentKey === "Nuclear") return; // already active
      setPendingNuclear(true);
      return;
    }
    setPendingNuclear(false);
    setConfirmText("");
    setPanicLevel(key.toLowerCase());
  };

  const confirmNuclear = () => {
    if (confirmText !== NUCLEAR_CONFIRM) return;
    setPanicLevel("nuclear", NUCLEAR_CONFIRM);
    setConfirmText("");
    setPendingNuclear(false);
  };

  return (
    <div className="space-y-3 max-w-[1280px]">
      <div>
        <div className="page-title">FIREWALL</div>
        <div className="page-subtitle">KILL SWITCH // NFTABLES RULE MANAGEMENT</div>
      </div>

      {/* Current state */}
      <div className="card">
        <div className="card-header">
          <span className={`indicator ${current.dot}`} />
          <span className="card-title">KILL SWITCH: {current.label}</span>
          {panic?.kill_switch_active && (
            <span className="ml-auto text-[9px] font-mono text-[#ef4444] tracking-wider">ACTIVE</span>
          )}
        </div>
        <div className="mt-2 text-[10px] font-mono text-[#6b7280]">{current.desc}</div>
        {panic && currentKey !== "Off" && (
          <div className="mt-2 flex flex-wrap gap-2">
            {panic.interfaces_down && <span className="text-[9px] font-mono text-[#4ade80]">IF DOWN</span>}
            {panic.dns_flushed && <span className="text-[9px] font-mono text-[#4ade80]">DNS FLUSH</span>}
            {panic.kernel_caches_purged && <span className="text-[9px] font-mono text-[#4ade80]">CACHE PURGE</span>}
          </div>
        )}
      </div>

      {/* Level selection */}
      <div className="grid grid-cols-4 gap-2">
        {LEVELS.map((level) => {
          const isActive = currentKey === level.key;
          return (
            <button
              key={level.key}
              onClick={() => handleSet(level.key)}
              disabled={isActive}
              className={`level-btn ${isActive ? "level-btn-active" : ""}`}
            >
              <div className="flex items-center gap-1.5">
                <span className={level.dot} />
                <span className="text-[11px] font-mono font-semibold" style={{ color: isActive ? level.color : "#6b7280" }}>
                  {level.label}
                </span>
              </div>
              <div className="text-[8px] font-mono text-[#6b7280] leading-relaxed">
                {level.desc}
              </div>
            </button>
          );
        })}
      </div>

      {/* Nuclear confirmation prompt */}
      {(pendingNuclear || currentKey === "Nuclear") && (
        <div className="card border-[#ef4444]/30">
          <div className="card-header">
            <span className="indicator-critical" />
            <span className="card-title text-[#ef4444]">
              {pendingNuclear ? "CONFIRM NUCLEAR ACTIVATION" : "NUCLEAR IS ACTIVE"}
            </span>
          </div>
          <div className="mt-2 text-[9px] font-mono text-[#6b7280]">
            {pendingNuclear
              ? "Type the confirmation string to activate Nuclear mode:"
              : "Type the confirmation string and click DECONFIRM to return to OFF:"}
          </div>
          <div className="mt-1 flex gap-2">
            <input
              type="text"
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder={NUCLEAR_CONFIRM}
              className="flex-1 px-2 py-1 text-[11px] font-mono border border-[#2a2f3f]"
              style={{ background: "#111318", color: "#c8ccd4" }}
            />
            {pendingNuclear ? (
              <button onClick={confirmNuclear} className="btn-primary" disabled={confirmText !== NUCLEAR_CONFIRM}>
                CONFIRM
              </button>
            ) : (
              <button
                onClick={() => { setPanicLevel("off"); setConfirmText(""); setPendingNuclear(false); }}
                className="btn"
                disabled={confirmText !== NUCLEAR_CONFIRM}
              >
                DECONFIRM
              </button>
            )}
          </div>
          {pendingNuclear && (
            <div className="mt-1 flex gap-1">
              <button onClick={() => setPendingNuclear(false)} className="btn-ghost">
                CANCEL
              </button>
            </div>
          )}
        </div>
      )}

      {/* nftables chains */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">NFTABLES CHAINS</span>
          <span className="ml-auto text-[9px] font-mono text-[#6b7280]">TABLE: INET ENDPOINT_PRIVACY</span>
        </div>
        <div className="mt-2 space-y-2">
          {(["privacy_input", "privacy_output", "privacy_forward"] as const).map((chain) => {
            const policy = currentKey === "Off" ? "accept" : "drop";
            return (
              <div key={chain} className="p-2" style={{ background: "#111318" }}>
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[9px] font-mono uppercase tracking-wider text-[#6b7280]">{chain.replace("privacy_", "")}</span>
                  <span className="text-[9px] font-mono text-[#6b7280]">policy {policy}</span>
                  {panic?.kill_switch_active && policy === "drop" && <span className="indicator-critical" />}
                </div>
                <div className="font-mono text-[9px] text-[#6b7280] space-y-px pl-2">
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
                      {currentKey === "Hard" && (
                        <div className="text-[#4ade80]">{'ip daddr upstream-ip udp/tcp dport { 53, 853 } accept'}</div>
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
