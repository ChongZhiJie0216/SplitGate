// 应用级状态：当前页面、后端信息。连接状态以后由后端事件推送，前端不自行推断。
import { create } from "zustand";
import { AppInfo, getAppInfo, checkUpdate, UpdateInfo, ServiceState, getServiceState, serviceUsable } from "../lib/ipc";

export type Page = "connect" | "rules" | "connections" | "canvas" | "usage" | "blocked" | "logs" | "profiles" | "settings" | "about";

interface AppState {
  page: Page;
  info: AppInfo | null;
  infoError: string | null;
  update: UpdateInfo | null;
  updateError: string | null;
  checkingUpdate: boolean;
  /** 后台服务的状态；null = 还没读到。 */
  service: ServiceState | null;
  setPage: (p: Page) => void;
  loadInfo: () => Promise<void>;
  /** quiet：启动时自动检查，失败不显示错误（例如没联网）。 */
  checkUpdate: (quiet?: boolean) => Promise<void>;
  loadService: () => Promise<void>;
  setService: (s: ServiceState) => void;
}

/** 能不能按“连接”：管理员，或者有可用的后台服务（还没读到服务状态时先不挡，后端会说明原因）。 */
export const canConnect = (s: Pick<AppState, "info" | "service">) =>
  !!s.info && (s.info.isAdmin || s.service === null || serviceUsable(s.service));

export const useApp = create<AppState>((set) => ({
  page: "connect",
  info: null,
  infoError: null,
  update: null,
  updateError: null,
  checkingUpdate: false,
  service: null,
  setPage: (page) => set({ page }),
  checkUpdate: async (quiet = false) => {
    set({ checkingUpdate: true, updateError: null });
    try {
      set({ update: await checkUpdate(), checkingUpdate: false });
    } catch (e) {
      set({ checkingUpdate: false, updateError: quiet ? null : String(e) });
    }
  },
  loadService: async () => {
    try {
      set({ service: await getServiceState() });
    } catch {
      // 读不到就当没有：设置页会显示原因
    }
  },
  setService: (service) => set({ service }),
  loadInfo: async () => {
    try {
      set({ info: await getAppInfo(), infoError: null });
    } catch (e) {
      set({ infoError: String(e) });
    }
  },
}));
