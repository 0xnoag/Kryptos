function getToken(): string {
  const meta = document.querySelector<HTMLMetaElement>('meta[name="api-token"]');
  return meta?.content ?? "";
}

interface IpcRequest {
  type: string;
  payload?: Record<string, unknown>;
}

interface IpcResponse {
  type: string;
  payload?: unknown;
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getToken();
  const res = await fetch(path, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...init?.headers,
    },
  });
  if (res.status === 401) {
    // Token expired (daemon restarted) — reload page to get fresh token
    window.location.reload();
    // never returns
    throw new Error("Token expired, reloading");
  }
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error || res.statusText);
  }
  return res.json();
}

class IpcClient {
  async connect(): Promise<void> {
    // HTTP client is always connected
  }

  async send(request: IpcRequest): Promise<IpcResponse> {
    switch (request.type) {
      case "GetStatus": {
        const data = await apiFetch<{ services: unknown; panic: unknown }>("/api/status");
        return { type: "Status", payload: data };
      }
      default:
        throw new Error(`Unknown IPC request type: ${request.type}`);
    }
  }

  disconnect() {
    // no-op
  }
}

export const ipc = new IpcClient();
