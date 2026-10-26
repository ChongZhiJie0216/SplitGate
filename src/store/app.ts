// 应用级状态：当前页面、后端信息。连接状态以后由后端事件推送，前端不自行推断。
import { create } from "zustand";
import { AppInfo, getAppInfo } from "../lib/ipc";

export type Page = "connect" | "rules" | "connections" | "canvas" | "blocked" | "logs" | "profiles" | "settings" | "about";

interface AppState {
  page: Page;
  info: AppInfo | null;
  infoError: string | null;
  setPage: (p: Page) => void;
  loadInfo: () => Promise<void>;
}

export const useApp = create<AppState>((set) => ({
  page: "connect",
  info: null,
  infoError: null,
  setPage: (page) => set({ page }),
  loadInfo: async () => {
    try {
      set({ info: await getAppInfo(), infoError: null });
    } catch (e) {
      set({ infoError: String(e) });
    }
  },
}));
