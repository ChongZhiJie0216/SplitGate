// 所有 Tauri invoke / listen 的类型封装。前端其它地方不直接调用 invoke。
// Profile 相关字段名和 profiles.json 一致（snake_case，见 SPEC“数据模型”）。
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type Step = "prepare" | "cleanup" | "check" | "start" | "tunnel" | "probe";

/** 连接状态：后端是唯一来源（supervisor.rs），前端只显示。 */
export type Status =
  | { state: "disconnected" }
  | { state: "connecting"; profile_name: string; step: Step }
  /** warnings：连上了，但有地方不会照预期工作（例如家里内网和本地网络撞号）。 */
  | { state: "connected"; profile_name: string; exit: ExitType; since_ms: number; probe_ms: number; warnings: string[] }
  | { state: "disconnecting" }
  /** retrying：连接中断后正在等待自动重新连接。 */
  | { state: "error"; message: string; retrying: boolean };

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

/** Pangolin 出口（v0.21.0）：经官方客户端的网卡，连到家里 VPN-Server 虚拟机上的 SOCKS5 代理。 */
export interface PangolinProfile {
  interface: string;
  proxy_host: string;
  proxy_port: number;
  proxy_user: string;
  vpn_dns: string[];
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
  /** 家里内网网段（IPv4 CIDR），一律走这个出口。 */
  home_cidrs: string[];
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
/** Cisco / AnyLink：测试登录的结果（不建立隧道）。 */
export type CiscoLoginResult =
  | { result: "ok" }
  | { result: "rejected"; message: string }
  | { result: "needs_more"; message: string }
  | { result: "failed"; message: string };

/** 服务器登录页上的组别列表；不需要选组别的服务器返回空列表。 */
export const ciscoGroups = (server: string) => invoke<string[]>("cisco_groups", { server });
/** password 留空 = 用凭据管理器里已保存的密码。 */
export const testCiscoLogin = (profile: Profile, password?: string) =>
  invoke<CiscoLoginResult>("test_cisco_login", { profile, password: password || null });
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
  auto_reconnect: boolean;
  check_updates: boolean;
  singbox_log_level: "info" | "debug" | "trace";
  tun_ipv4: string;
}

export interface AutostartState {
  enabled: boolean;
  command: string | null;
  matches_this_exe: boolean;
  /** 任务以管理员身份启动 SplitGate（没有后台服务时需要）。 */
  elevated: boolean;
}

export interface SettingsView {
  settings: Settings;
  autostart: AutostartState;
  error: string | null;
}

export const getSettings = () => invoke<SettingsView>("get_settings");
export const saveSettings = (settings: Settings) => invoke<Settings>("save_settings", { settings });
export const setAutostart = (enabled: boolean) => invoke<AutostartState>("set_autostart", { enabled });

/** 每秒的流量（字节/秒），连接期间由后端推送 `traffic` 事件。 */
export interface Traffic {
  vpn_up: number;
  vpn_down: number;
  direct_up: number;
  direct_down: number;
}

export interface Connection {
  id: string;
  network: string;
  /** 域名；只知道 IP 时为空。 */
  host: string;
  ip: string;
  port: string;
  /** Windows 不告诉的（系统服务）为空。 */
  process_path: string;
  /** "vpn" 或 "direct" */
  outbound: string;
  rule: string;
  upload: number;
  download: number;
  start: string;
}

export interface ConnectionsSnapshot {
  connections: Connection[];
  traffic: Traffic;
  upload_total: number;
  download_total: number;
}

export const getConnections = () => invoke<ConnectionsSnapshot>("get_connections");
export const closeConnection = (id: string) => invoke<void>("close_connection", { id });
export const onTraffic = (f: (t: Traffic) => void): Promise<UnlistenFn> => listen<Traffic>("traffic", (e) => f(e.payload));

/** 拦截检测（block_detect.rs）：直连的 HTTPS 域名证书被换掉、不受信任或握手失败。 */
export type BlockReason = "fortinet" | "untrusted" | "handshake";

export interface Blocked {
  host: string;
  reason: BlockReason;
  /** 证书签发者或错误信息。 */
  detail: string;
  process_path: string;
  /** 最后一次检测到的时间 HH:MM:SS。 */
  time: string;
}

/** 后台服务（service.rs）：装了、在运行、指向这一份 SplitGate 时，连接由它负责，SplitGate 不用管理员身份。 */
export interface ServiceState {
  installed: boolean;
  running: boolean;
  command: string | null;
  /** 服务指向的是不是这一份 SplitGate.exe（便携版搬了位置时为 false）。 */
  matches_this_exe: boolean;
  /** 服务负责的那一份 SplitGate.exe。 */
  app: string | null;
  /** 服务从只有管理员能改的文件夹运行（v0.20.0 起）；false = 旧版装的，直接用程序文件夹里的文件。 */
  protected: boolean;
  /** 服务经命名管道回报的版本；没回应时为 null。 */
  version: string | null;
  /** 服务的版本和这个 SplitGate 不同。 */
  outdated: boolean;
}

export const getServiceState = () => invoke<ServiceState>("get_service_state");
export const installService = () => invoke<ServiceState>("install_service");
export const removeService = () => invoke<ServiceState>("remove_service");
/** 弹出 Windows 的管理员确认，新的 SplitGate 启动后这一个就退出。 */
export const restartAsAdmin = () => invoke<void>("restart_as_admin");
export const serviceUsable = (s: ServiceState | null) => !!s && s.installed && s.running && s.matches_this_exe && !s.outdated;

/** 检查更新（update.rs）：只提示，不下载、不安装。 */
export interface UpdateSection {
  version: string;
  title: string;
  /** 这个版本的 CHANGELOG（Markdown）。 */
  body: string;
}

export interface UpdateInfo {
  current: string;
  /** 还没有发布任何版本时为 null。 */
  latest: string | null;
  newer: boolean;
  url: string | null;
  /** 比现在新的版本，最新的在前面。 */
  notes: UpdateSection[];
  /** 能在程序里更新时的下载信息；旧版本的发布页没有校验文件时为 null（只能手动下载）。 */
  download: { version: string; zip_url: string; size: number; sha_url: string } | null;
}

/** 在线升级（updater.rs，便携版）。 */
export type UpdateJob =
  | { state: "idle" }
  | { state: "downloading"; version: string; done: number; total: number }
  | { state: "ready"; version: string }
  | { state: "installing"; version: string }
  | { state: "failed"; message: string };

export const downloadUpdate = () => invoke<UpdateJob>("download_update");
export const getUpdateJob = () => invoke<UpdateJob>("get_update_job");
/** 先断开（页面已经问过）、换文件、启动新版本，这一个退出。 */
export const installUpdate = () => invoke<void>("install_update");

export const checkUpdate = () => invoke<UpdateInfo>("check_update");
/** 这个版本自带的 CHANGELOG（打包时编进程序），最新的在前面。 */
export const getChangelog = () => invoke<UpdateSection[]>("get_changelog");

/** 关于页“许可证”：SplitGate 和 sing-box 附全文，其他开源组件只列名称和许可证。 */
export interface License {
  name: string;
  license: string;
  url: string;
  text: string | null;
}
export const getLicenses = () => invoke<License[]>("get_licenses");
export const openReleasePage = (url: string) => invoke<void>("open_release_page", { url });

/** 诊断包：返回 zip 的完整路径（设置文件夹的 diagnostics 里）。 */
export const exportDiagnostics = () => invoke<string>("export_diagnostics");
export const showDiagnostics = (path: string) => invoke<void>("show_diagnostics", { path });

export const listBlocked = () => invoke<Blocked[]>("list_blocked");
export const dismissBlocked = (host: string) => invoke<Blocked[]>("dismiss_blocked", { host });
export const clearBlocked = () => invoke<Blocked[]>("clear_blocked");
export const onBlocked = (f: (b: Blocked) => void): Promise<UnlistenFn> => listen<Blocked>("blocked_detected", (e) => f(e.payload));
