// 所有 Tauri invoke / listen 的类型封装。前端其它地方不直接调用 invoke。
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  singboxVersion: string | null;
  isAdmin: boolean;
  dataDir: string;
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
