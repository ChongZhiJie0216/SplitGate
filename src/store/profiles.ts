// Profile 列表。后端是唯一来源：每次修改后用后端返回的结果整个替换。
import { create } from "zustand";
import * as ipc from "../lib/ipc";

interface ProfilesState {
  view: ipc.ProfilesView | null;
  error: string | null;
  load: () => Promise<void>;
  /** 执行一次修改；成功返回 null，失败返回中文错误（同时显示在页面上）。 */
  run: (op: () => Promise<ipc.ProfilesView>) => Promise<string | null>;
}

export const useProfiles = create<ProfilesState>((set) => ({
  view: null,
  error: null,
  load: async () => {
    try {
      set({ view: await ipc.listProfiles(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  run: async (op) => {
    try {
      set({ view: await op(), error: null });
      return null;
    } catch (e) {
      return String(e);
    }
  },
}));
