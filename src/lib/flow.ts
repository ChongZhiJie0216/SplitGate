// 流量画布的数据：把连接列表整理成“程序 → 规则 → 出口”三列。
// sing-box 只告诉我们命中了合并后的哪条规则，看不出是哪个规则集；这里按规则集的内容在前端再比一次，
// 好让中间一列显示 Xbox / Steam / 家里内网 这样的名字。
import type { Connection, RuleSet } from "./ipc";
import { fileName } from "./format";

export type Column = "program" | "rule" | "exit";

export interface FlowNode {
  id: string;
  column: Column;
  label: string;
  /** 鼠标停留时显示的完整内容（例如程序路径）。 */
  title: string;
  vpn: boolean;
  count: number;
  bytes: number;
}

export interface FlowEdge {
  id: string;
  source: string;
  target: string;
  vpn: boolean;
  count: number;
  bytes: number;
}

export interface FlowLabels {
  system: string;
  otherPrograms: (n: number) => string;
  home: string;
  otherVpnRule: string;
  noRule: string;
  otherDirect: string;
  vpnExit: (profile: string) => string;
  directExit: string;
}

/** 程序太多时只画流量最大的这么多个，其余合并成一个“其他”。 */
export const MAX_PROGRAMS = 15;

function parseV4(ip: string): number | null {
  const p = ip.split(".");
  if (p.length !== 4) return null;
  let n = 0;
  for (const x of p) {
    const v = Number(x);
    if (!/^\d+$/.test(x) || v > 255) return null;
    n = n * 256 + v;
  }
  return n;
}

export function inCidr(ip: string, cidr: string): boolean {
  const [net, lenText] = cidr.split("/");
  const a = parseV4(ip);
  const b = parseV4(net);
  const len = lenText === undefined ? 32 : Number(lenText);
  if (a === null || b === null || !(len >= 0 && len <= 32)) return false;
  if (len === 0) return true;
  const size = 2 ** (32 - len);
  return Math.floor(a / size) === Math.floor(b / size);
}

const suffixMatch = (host: string, s: string) => host === s || host.endsWith(`.${s}`);

/** 这条走 VPN 的连接是哪个规则集让它走的；找不到时返回 null。 */
export function matchRuleSet(c: Connection, sets: RuleSet[]): RuleSet | null {
  const host = c.host.toLowerCase();
  const path = c.process_path.toLowerCase();
  const name = fileName(path);
  return (
    sets.find(
      (r) =>
        (path && r.process_path.some((p) => p.toLowerCase() === path)) ||
        (name && r.process_name.some((p) => p.toLowerCase() === name)) ||
        (host && r.domain_suffix.some((s) => suffixMatch(host, s.toLowerCase()))) ||
        (host && r.domain_keyword.some((k) => host.includes(k.toLowerCase()))) ||
        (c.ip && r.ip_cidr.some((x) => inCidr(c.ip, x))),
    ) ?? null
  );
}

export function buildFlow(
  connections: Connection[],
  sets: RuleSet[],
  homeCidrs: string[],
  profileName: string,
  L: FlowLabels,
): { nodes: FlowNode[]; edges: FlowEdge[]; members: Map<string, Connection[]> } {
  const nodes = new Map<string, FlowNode>();
  // 每个方块经过了哪些连接（点方块时的详情面板用）。
  const members = new Map<string, Connection[]>();
  const member = (id: string, c: Connection) => members.set(id, [...(members.get(id) ?? []), c]);
  const edges = new Map<string, FlowEdge>();
  const add = (id: string, column: Column, label: string, title: string, vpn: boolean, bytes: number) => {
    const n = nodes.get(id) ?? { id, column, label, title, vpn, count: 0, bytes: 0 };
    n.count++;
    n.bytes += bytes;
    nodes.set(id, n);
  };
  const link = (source: string, target: string, vpn: boolean, bytes: number) => {
    const id = `${source}->${target}`;
    const e = edges.get(id) ?? { id, source, target, vpn, count: 0, bytes: 0 };
    e.count++;
    e.bytes += bytes;
    edges.set(id, e);
  };

  // 先按程序算总流量，只留最大的几个。
  const perProgram = new Map<string, number>();
  for (const c of connections) {
    const k = c.process_path.toLowerCase();
    perProgram.set(k, (perProgram.get(k) ?? 0) + c.upload + c.download);
  }
  const keep = new Set([...perProgram.entries()].sort((a, b) => b[1] - a[1]).slice(0, MAX_PROGRAMS).map(([k]) => k));
  const dropped = perProgram.size - keep.size;

  for (const c of connections) {
    const bytes = c.upload + c.download;
    const vpn = c.outbound === "vpn";
    const key = c.process_path.toLowerCase();
    const prog = keep.has(key) ? `p:${key}` : "p:*other";
    const progLabel = keep.has(key) ? (c.process_path ? fileName(c.process_path) : L.system) : L.otherPrograms(dropped);

    let rule: string;
    let ruleLabel: string;
    if (!vpn) {
      [rule, ruleLabel] = c.rule === "final" ? ["r:final", L.noRule] : ["r:direct", L.otherDirect];
    } else if (c.ip && homeCidrs.some((h) => inCidr(c.ip, h))) {
      [rule, ruleLabel] = ["r:home", L.home];
    } else {
      const set = matchRuleSet(c, sets);
      [rule, ruleLabel] = set ? [`r:set:${set.id}`, set.name] : ["r:other", L.otherVpnRule];
    }
    const exit = vpn ? "x:vpn" : "x:direct";

    add(prog, "program", progLabel, keep.has(key) ? c.process_path || L.system : progLabel, vpn, bytes);
    add(rule, "rule", ruleLabel, ruleLabel, vpn, bytes);
    add(exit, "exit", vpn ? L.vpnExit(profileName) : L.directExit, "", vpn, bytes);
    link(prog, rule, vpn, bytes);
    link(rule, exit, vpn, bytes);
    for (const id of [prog, rule, exit]) member(id, c);
  }
  // 程序节点里 vpn 表示“至少有一条走 VPN”。
  for (const e of edges.values()) {
    const n = nodes.get(e.source);
    if (n && e.vpn) n.vpn = true;
  }
  const order = (n: FlowNode) => (n.vpn ? 0 : 1);
  const sorted = [...nodes.values()].sort((a, b) => order(a) - order(b) || b.bytes - a.bytes);
  return { nodes: sorted, edges: [...edges.values()], members };
}
