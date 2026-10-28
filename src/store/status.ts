// 连接状态和日志：开头读一次，之后靠后端事件更新（status_changed / log）。
import { create } from "zustand";
import * as ipc from "../lib/ipc";

const MAX_LINES = 2000;

interface StatusState {
  status: ipc.Status;
  logs: ipc.LogLine[];
  traffic: ipc.Traffic;
  actionError: string | null;
  /** 连接期间改过使用中的规则或 Profile：要重新连接才生效（顶部提示）。 */
  stale: boolean;
  /** 规则 / Profile 保存后调用；只有已连接时才会提示。 */
  markStale: () => void;
  start: () => Promise<void>;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /** 断开，等到“已断开”，再连接（规则改了要重新连接才生效）。 */
  reconnect: () => Promise<void>;
  clearLogs: () => void;
}

const NO_TRAFFIC: ipc.Traffic = { vpn_up: 0, vpn_down: 0, direct_up: 0, direct_down: 0 };

let started = false;

export const useStatus = create<StatusState>((set, get) => ({
  status: { state: "disconnected" },
  logs: [],
  traffic: NO_TRAFFIC,
  actionError: null,
  stale: false,
  markStale: () => set({ stale: get().status.state === "connected" }),
  start: async () => {
    if (started) return;
    started = true;
    // 不在“已连接”时（重新连接、断开）提示就没意义了：下次连接会用新的规则。
    await ipc.onStatus((status) => set(status.state === "connected" ? { status } : { status, traffic: NO_TRAFFIC, stale: false }));
    await ipc.onTraffic((traffic) => set({ traffic }));
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
  reconnect: async () => {
    await get().disconnect();
    for (let i = 0; i < 60 && get().status.state !== "disconnected"; i++) {
      await new Promise((r) => setTimeout(r, 250));
    }
    if (get().status.state === "disconnected") await get().connect();
  },
  clearLogs: () => set({ logs: [] }),
}));
