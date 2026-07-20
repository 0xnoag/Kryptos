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
}

interface DaemonContextType extends DaemonState {
  startService: (name: string) => Promise<void>;
  stopService: (name: string) => Promise<void>;
  restartService: (name: string) => Promise<void>;
  setPanicLevel: (level: string, confirmation?: string) => Promise<void>;
  refresh: () => Promise<void>;
}

const DaemonContext = createContext<DaemonContextType | null>(null);

export function DaemonProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<DaemonState>({
    connected: false,
    services: [],
    panic: null,
    error: null,
  });

  const refresh = useCallback(async () => {
    try {
      const response = await ipc.send({ type: "GetStatus" });
      if (response.type === "Status") {
        const payload = response.payload as {
          services: ServiceInfo[];
          panic: PanicStatus;
        };
        setState({
          connected: true,
          services: payload.services,
          panic: payload.panic,
          error: null,
        });
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

  const startService = useCallback(async (name: string) => {
    await ipc.send({ type: "StartService", payload: { service: name } });
    await refresh();
  }, [refresh]);

  const stopService = useCallback(async (name: string) => {
    await ipc.send({ type: "StopService", payload: { service: name } });
    await refresh();
  }, [refresh]);

  const restartService = useCallback(async (name: string) => {
    await ipc.send({ type: "RestartService", payload: { service: name } });
    await refresh();
  }, [refresh]);

  const setPanicLevel = useCallback(async (level: string, confirmation?: string) => {
    const payload: Record<string, unknown> = { level };
    if (confirmation) {
      payload.confirmation = confirmation;
    }
    const response = await ipc.send({
      type: "SetPanicLevel",
      payload,
    });
    if (response.type === "PanicStatus") {
      setState((s) => ({
        ...s,
        panic: response.payload as PanicStatus,
      }));
    }
  }, []);

  return (
    <DaemonContext.Provider
      value={{
        ...state,
        startService,
        stopService,
        restartService,
        setPanicLevel,
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
