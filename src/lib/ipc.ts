// 所有 Tauri invoke / listen 的类型封装。前端其它地方不直接调用 invoke。
// Profile 相关字段名和 profiles.json 一致（snake_case，见 SPEC“数据模型”）。
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
