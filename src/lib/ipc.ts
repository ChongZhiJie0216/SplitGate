// 所有 Tauri invoke / listen 的类型封装。前端其它地方不直接调用 invoke。
// Profile 相关字段名和 profiles.json 一致（snake_case，见 SPEC“数据模型”）。
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type Step = "prepare" | "cleanup" | "check" | "start" | "tunnel" | "probe";

/** 连接状态：后端是唯一来源（supervisor.rs），前端只显示。 */
export type Status =
  | { state: "disconnected" }
  | { state: "connecting"; profile_name: string; step: Step }
  | { state: "connected"; profile_name: string; exit: ExitType; since_ms: number; probe_ms: number }
  | { state: "disconnecting" }
  | { state: "error"; message: string };

export interface LogLine {
  time: string;
  source: "app" | "sing-box";
  level: "info" | "warn" | "error";
  text: string;
}

export const connect = (profileId?: string) => invoke<void>("connect", { profileId: profileId ?? null });
export const disconnect = () => invoke<void>("disconnect");
export const getStatus = () => invoke<Status>("get_status");
export const getLogs = () => invoke<LogLine[]>("get_logs");
export const onStatus = (f: (s: Status) => void): Promise<UnlistenFn> => listen<Status>("status_changed", (e) => f(e.payload));
export const onLog = (f: (l: LogLine) => void): Promise<UnlistenFn> => listen<LogLine>("log", (e) => f(e.payload));

export interface AppInfo {
  version: string;
  singboxVersion: string | null;
  isAdmin: boolean;
  dataDir: string;
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export type ExitType = "cisco" | "pangolin" | "wireguard";

export interface CiscoProfile {
  server: string;
  user: string;
  auth_group: string | null;
  no_udp: boolean;
  bind_interface: string | null;
  mfa: string;
}

export interface PangolinProfile {
  interface: string;
  vpn_dns: string[];
  alias_suffixes: string[];
  alias_dns: string;
}

export interface WireguardProfile {
  address: string[];
  peer: { endpoint: string; public_key: string; persistent_keepalive: number | null };
  dns: string[];
  mtu: number;
}

export interface Profile {
  id: string;
  type: ExitType;
  name: string;
  ruleset_ids: string[];
  cisco?: CiscoProfile;
  pangolin?: PangolinProfile;
  wireguard?: WireguardProfile;
}

export interface ProfilesView {
  active: string | null;
  profiles: { profile: Profile; has_secret: boolean }[];
}

/** 留空 = 保留凭据管理器里已保存的值。 */
export interface SecretsInput {
  secret?: string;
  preshared_key?: string;
}

export interface ImportedConf {
  wireguard: WireguardProfile;
  private_key: string;
  preshared_key: string | null;
}

export const listProfiles = () => invoke<ProfilesView>("list_profiles");
export const saveProfile = (profile: Profile, secrets?: SecretsInput) =>
  invoke<{ id: string; view: ProfilesView }>("save_profile", { profile, secrets: secrets ?? null });
export const deleteProfile = (id: string) => invoke<ProfilesView>("delete_profile", { id });
export const setActiveProfile = (id: string) => invoke<ProfilesView>("set_active_profile", { id });
export const importWireguardConf = (text: string) => invoke<ImportedConf>("import_wireguard_conf", { text });

export interface RuleSet {
  id: string;
  name: string;
  enabled: boolean;
  domain_suffix: string[];
  domain_keyword: string[];
  process_path: string[];
  process_name: string[];
  ip_cidr: string[];
}

export interface RuleSetView {
  ruleset: RuleSet;
  /** 内置预设：不能删除，可以恢复预设。 */
  preset: boolean;
}

export interface RunningProcess {
  name: string;
  path: string;
}

export const listRulesets = () => invoke<RuleSetView[]>("list_rulesets");
export const saveRuleset = (ruleset: RuleSet) => invoke<{ id: string; rulesets: RuleSetView[] }>("save_ruleset", { ruleset });
export const deleteRuleset = (id: string) => invoke<RuleSetView[]>("delete_ruleset", { id });
export const resetRulesetToPreset = (id: string) => invoke<RuleSetView[]>("reset_ruleset_to_preset", { id });
export const listRunningProcesses = () => invoke<RunningProcess[]>("list_running_processes");

/** settings.json（字段名和文件一致）。主题不在这里：存在浏览器的 localStorage（lib/theme.ts）。 */
export interface Settings {
  version: number;
  auto_connect: boolean;
  singbox_log_level: "info" | "debug" | "trace";
  tun_ipv4: string;
}

export interface AutostartState {
  enabled: boolean;
  command: string | null;
  matches_this_exe: boolean;
}

export interface SettingsView {
  settings: Settings;
  autostart: AutostartState;
  error: string | null;
}

export const getSettings = () => invoke<SettingsView>("get_settings");
export const saveSettings = (settings: Settings) => invoke<Settings>("save_settings", { settings });
export const setAutostart = (enabled: boolean) => invoke<AutostartState>("set_autostart", { enabled });
