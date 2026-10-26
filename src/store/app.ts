// 应用级状态：当前页面、后端信息。连接状态以后由后端事件推送，前端不自行推断。
import { create } from "zustand";
import { AppInfo, getAppInfo, checkUpdate, UpdateInfo } from "../lib/ipc";

export type Page = "connect" | "rules" | "connections" | "canvas" | "blocked" | "logs" | "profiles" | "settings" | "about";

interface AppState {
  page: Page;
  info: AppInfo | null;
  infoError: string | null;
  update: UpdateInfo | null;
  updateError: string | null;
  checkingUpdate: boolean;
  setPage: (p: Page) => void;
  loadInfo: () => Promise<void>;
  /** quiet：启动时自动检查，失败不显示错误（例如没联网）。 */
  checkUpdate: (quiet?: boolean) => Promise<void>;
}

export const useApp = create<AppState>((set) => ({
  page: "connect",
  info: null,
  infoError: null,
  update: null,
  updateError: null,
  checkingUpdate: false,
  setPage: (page) => set({ page }),
  checkUpdate: async (quiet = false) => {
    set({ checkingUpdate: true, updateError: null });
    try {
      set({ update: await checkUpdate(), checkingUpdate: false });
    } catch (e) {
      set({ checkingUpdate: false, updateError: quiet ? null : String(e) });
    }
  },
  loadInfo: async () => {
    try {
      set({ info: await getAppInfo(), infoError: null });
    } catch (e) {
      set({ infoError: String(e) });
    }
  },
}));
