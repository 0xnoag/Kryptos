import { createContext, useContext, useState, useCallback, useEffect, type ReactNode } from "react";
import { ipc } from "./ipc";

interface ServiceInfo {
  name: string;
  status: string;
  uptime_secs: number;
  restart_count: number;
  pid: number | null;
}

interface PanicStatus {
  level: string;
  kill_switch_active: boolean;
  interfaces_down: boolean;
  dns_flushed: boolean;
  kernel_caches_purged: boolean;
}

interface DaemonState {
  connected: boolean;
  services: ServiceInfo[];
  panic: PanicStatus | null;
  error: string | null;
  firstRun: boolean;
}

interface DaemonContextType extends DaemonState {
  refresh: () => Promise<void>;
}

const DaemonContext = createContext<DaemonContextType | null>(null);

export function DaemonProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<DaemonState>({
    connected: false,
    services: [],
    panic: null,
    error: null,
    firstRun: document.querySelector<HTMLMetaElement>('meta[name="first-run"]')?.content === "true",
  });

  const refresh = useCallback(async () => {
    try {
      const response = await ipc.send({ type: "GetStatus" });
      if (response.type === "Status") {
        const payload = response.payload as {
          services: ServiceInfo[];
          panic: PanicStatus;
        };
        setState((s) => ({
          connected: true,
          services: payload.services,
          panic: payload.panic,
          error: null,
          firstRun: s.firstRun,
        }));
      }
    } catch (e) {
      setState((s) => ({
        ...s,
        connected: false,
        error: e instanceof Error ? e.message : "Connection failed",
      }));
    }
  }, []);

  useEffect(() => {
    ipc.connect().then(() => {
      refresh();
      const interval = setInterval(refresh, 2000);
      return () => clearInterval(interval);
    });
  }, [refresh]);

  return (
    <DaemonContext.Provider
      value={{
        ...state,
        refresh,
      }}
    >
      {children}
    </DaemonContext.Provider>
  );
}

export function useDaemon() {
  const ctx = useContext(DaemonContext);
  if (!ctx) throw new Error("useDaemon must be used within DaemonProvider");
  return ctx;
}
