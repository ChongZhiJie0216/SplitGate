// 连接状态和日志：开头读一次，之后靠后端事件更新（status_changed / log）。
import { create } from "zustand";
import * as ipc from "../lib/ipc";

const MAX_LINES = 2000;

interface StatusState {
  status: ipc.Status;
  logs: ipc.LogLine[];
  actionError: string | null;
  start: () => Promise<void>;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  clearLogs: () => void;
}

let started = false;

export const useStatus = create<StatusState>((set, get) => ({
  status: { state: "disconnected" },
  logs: [],
  actionError: null,
  start: async () => {
    if (started) return;
    started = true;
    await ipc.onStatus((status) => set({ status }));
    await ipc.onLog((l) => set({ logs: [...get().logs.slice(-(MAX_LINES - 1)), l] }));
    const [status, logs] = await Promise.all([ipc.getStatus(), ipc.getLogs()]);
    set({ status, logs });
  },
  connect: async () => {
    set({ actionError: null });
    try {
      await ipc.connect();
    } catch (e) {
      set({ actionError: String(e) });
    }
  },
  disconnect: async () => {
    set({ actionError: null });
    try {
      await ipc.disconnect();
    } catch (e) {
      set({ actionError: String(e) });
    }
  },
  clearLogs: () => set({ logs: [] }),
}));
