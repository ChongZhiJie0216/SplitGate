// 疑似被拦截的域名。后端是唯一来源：有新发现（blocked_detected）就整个重读。
import { create } from "zustand";
import * as ipc from "../lib/ipc";

interface BlockedState {
  list: ipc.Blocked[];
  /** 打开“拦截提示”页之后才发现的个数，显示在左侧导航。 */
  unseen: number;
  error: string | null;
  start: () => Promise<void>;
  seen: () => void;
  run: (op: () => Promise<ipc.Blocked[]>) => Promise<void>;
}

let started = false;

export const useBlocked = create<BlockedState>((set, get) => ({
  list: [],
  unseen: 0,
  error: null,
  start: async () => {
    if (started) return;
    started = true;
    await ipc.onBlocked(async (b) => {
      const isNew = !get().list.some((x) => x.host === b.host);
      await get().run(ipc.listBlocked);
      if (isNew) set({ unseen: get().unseen + 1 });
    });
    await get().run(ipc.listBlocked);
  },
  seen: () => set({ unseen: 0 }),
  run: async (op) => {
    try {
      set({ list: await op(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },
}));
