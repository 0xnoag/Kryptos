interface IpcRequest {
  type: string;
  payload?: unknown;
}

interface IpcResponse {
  type: string;
  payload?: unknown;
}

class IpcClient {
  private socket: WebSocket | null = null;
  private pending = new Map<string, {
    resolve: (v: IpcResponse) => void;
    reject: (e: Error) => void;
    timeout: ReturnType<typeof setTimeout>;
  }>();

  async connect(path?: string): Promise<void> {
    if (this.socket?.readyState === WebSocket.OPEN) return;

    if (window.__TAURI__) {
      const { invoke } = await import("@tauri-apps/api/core");
      const response = await invoke<IpcResponse>("ipc_request", {
        request: { type: "GetStatus" }
      });
    } else {
      return;
    }
  }

  async send(request: IpcRequest): Promise<IpcResponse> {
    if (window.__TAURI__) {
      const { invoke } = await import("@tauri-apps/api/core");
      const response = await invoke<IpcResponse>("ipc_request", {
        request,
      });
      return response;
    }

    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error("IPC not connected");
    }

    return new Promise((resolve, reject) => {
      const id = crypto.randomUUID();
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("IPC request timed out"));
      }, 5000);

      this.pending.set(id, { resolve, reject, timeout });
      this.socket!.send(JSON.stringify({ id, ...request }));
    });
  }

  disconnect() {
    this.socket?.close();
    this.socket = null;
  }
}

export const ipc = new IpcClient();
