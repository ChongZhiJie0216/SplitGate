import { useEffect, useMemo, useState } from "react";
import { Background, Controls, Edge, Node, Position, ReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import { useStatus } from "../store/status";
import { useRules } from "../store/rules";
import { useProfiles } from "../store/profiles";
import { T } from "../lib/text";
import { bytes } from "../lib/format";
import { buildFlow, Column, FlowNode } from "../lib/flow";
import * as ipc from "../lib/ipc";
import { TrafficBadges } from "./ConnectionsPage";
import Empty from "../components/Empty";
import Icon from "../components/Icon";
import { useApp } from "../store/app";
import { fileName } from "../lib/format";

const L = T.canvasPage;
const COLUMN_X: Record<Column, number> = { program: 0, rule: 340, exit: 660 };
const ROW = 62;
const REFRESH_MS = 2000;

/** 线的粗细跟着流量：1 KB 以下 1px，之后每多 10 倍加 1px，最多 7px。 */
const strokeWidth = (b: number) => Math.min(7, Math.max(1, Math.log10(b + 1) - 2));

function toFlow(list: FlowNode[], selected: string | null): Node[] {
  const perColumn: Record<Column, FlowNode[]> = { program: [], rule: [], exit: [] };
  for (const n of list) perColumn[n.column].push(n);
  const tallest = Math.max(...Object.values(perColumn).map((c) => c.length), 1);
  const out: Node[] = [];
  for (const column of Object.keys(perColumn) as Column[]) {
    const col = perColumn[column];
    // 每列垂直居中，三列对齐中线。
    const top = ((tallest - col.length) * ROW) / 2;
    col.forEach((n, i) =>
      out.push({
        id: n.id,
        position: { x: COLUMN_X[column], y: top + i * ROW },
        data: {
          label: (
            <div title={n.title}>
              <div className="flow-label">{n.label}</div>
              <div className="flow-sub">{L.summary(n.count, bytes(n.bytes))}</div>
            </div>
          ),
        },
        className: `flow-node ${column} ${n.vpn ? "vpn" : "direct"}${n.id === selected ? " picked" : ""}`,
        sourcePosition: Position.Right,
        targetPosition: Position.Left,
        type: column === "program" ? "input" : column === "exit" ? "output" : "default",
        draggable: false,
        connectable: false,
      }),
    );
  }
  return out;
}

export default function CanvasPage() {
  const { status, traffic } = useStatus();
  const connected = status.state === "connected";
  const { rulesets, load } = useRules();
  const view = useProfiles((s) => s.view);
  const [connections, setConnections] = useState<ipc.Connection[]>([]);
  const [paused, setPaused] = useState(false);
  const [onlyVpn, setOnlyVpn] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!connected || paused) return;
    let alive = true;
    const tick = () =>
      ipc
        .getConnections()
        .then((s) => alive && (setConnections(s.connections), setError(null)))
        .catch((e) => alive && setError(String(e)));
    tick();
    const t = setInterval(tick, REFRESH_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [connected, paused]);

  const profile = view?.profiles.find((p) => p.profile.id === view.active)?.profile;
  const { nodes, edges, flowNodes, members, sets } = useMemo(() => {
    const used = profile?.ruleset_ids ?? [];
    const sets = (rulesets ?? []).map((r) => r.ruleset).filter((r) => r.enabled && used.includes(r.id));
    const list = onlyVpn ? connections.filter((c) => c.outbound === "vpn") : connections;
    const g = buildFlow(list, sets, profile?.home_cidrs ?? [], status.state === "connected" ? status.profile_name : "", L.labels);
    const flowEdges: Edge[] = g.edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      animated: e.vpn,
      className: e.vpn ? "flow-edge vpn" : "flow-edge",
      // 方向看三列就知道，不画箭头；数字写在两头的方块里，线上不再标。
      style: { strokeWidth: strokeWidth(e.bytes) },
    }));
    return { nodes: toFlow(g.nodes, selected), edges: flowEdges, flowNodes: g.nodes, members: g.members, sets };
  }, [connections, rulesets, profile, onlyVpn, status, selected]);
  const picked = flowNodes.find((n) => n.id === selected) ?? null;

  if (!connected) {
    return (
      <div className="card">
        <h2>{T.nav.canvas}</h2>
        <Empty icon="canvas" text={L.notConnected} />
      </div>
    );
  }

  return (
    <div className="card canvas-card">
      <div className="row conn-head">
        <h2>{T.nav.canvas}</h2>
        <TrafficBadges traffic={traffic} />
      </div>
      <div className="row">
        <label className="check">
          <input type="checkbox" checked={onlyVpn} onChange={(e) => setOnlyVpn(e.target.checked)} />
          {L.onlyVpn}
        </label>
        <label className="check">
          <input type="checkbox" checked={paused} onChange={(e) => setPaused(e.target.checked)} />
          {L.pause}
        </label>
        <span className="muted small">{L.hint}</span>
      </div>
      {error && <div className="banner bad">{error}</div>}
      <div className="canvas-wrap">
        {nodes.length === 0 ? (
          <p className="muted canvas-empty">{L.empty}</p>
        ) : (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            fitView
            fitViewOptions={{ padding: 0.15 }}
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable={false}
            onNodeClick={(_, n) => setSelected((cur) => (cur === n.id ? null : n.id))}
            onPaneClick={() => setSelected(null)}
            minZoom={0.3}
            maxZoom={2}
          >
            <Background gap={24} size={1} />
            <Controls showInteractive={false} />
          </ReactFlow>
        )}
        {picked && (
          <NodeDetail
            node={picked}
            conns={members.get(picked.id) ?? []}
            set={picked.id.startsWith("r:set:") ? (sets.find((r) => `r:set:${r.id}` === picked.id) ?? null) : null}
            homeCidrs={profile?.home_cidrs ?? []}
            onClose={() => setSelected(null)}
          />
        )}
      </div>
    </div>
  );
}

/** 点方块后的详情：规则集的规则、家里内网网段，以及经过这个方块的连接。 */
function NodeDetail(props: { node: FlowNode; conns: ipc.Connection[]; set: ipc.RuleSet | null; homeCidrs: string[]; onClose: () => void }) {
  const { node, conns, set, homeCidrs } = props;
  const D = L.detail;
  const setPage = useApp((s) => s.setPage);
  const groups: [string, string[]][] = set
    ? [
        [D.domainSuffix, set.domain_suffix],
        [D.domainKeyword, set.domain_keyword],
        [D.processName, set.process_name],
        [D.processPath, set.process_path],
        [D.ipCidr, set.ip_cidr],
        [D.subscribed, [...set.sub_domain_suffix, ...set.sub_domain_keyword]],
      ]
    : node.id === "r:home"
      ? [[D.ipCidr, homeCidrs]]
      : [];
  const note = D.notes[node.id as keyof typeof D.notes];
  // 同一个目标的多条连接合成一行，流量大的在前。
  const rows = [...conns.reduce((m, c) => {
    const target = c.host || c.ip;
    const key = `${c.process_path}|${target}|${c.outbound}`;
    const r = m.get(key) ?? { prog: c.process_path ? fileName(c.process_path) : L.labels.system, target, vpn: c.outbound === "vpn", rule: c.rule, n: 0, bytes: 0 };
    r.n++;
    r.bytes += c.upload + c.download;
    return m.set(key, r);
  }, new Map<string, { prog: string; target: string; vpn: boolean; rule: string; n: number; bytes: number }>()).values()].sort((a, b) => b.bytes - a.bytes);

  return (
    <aside className="flow-detail">
      <div className="modal-head">
        <h3>{node.label}</h3>
        <button className="icon-btn small" title={T.about.close} aria-label={T.about.close} onClick={props.onClose}>
          <Icon name="x" size={16} />
        </button>
      </div>
      {node.title && node.title !== node.label && <p className="muted small mono break">{node.title}</p>}
      <p className="muted small">{L.summary(node.count, bytes(node.bytes))}</p>
      {note && <p className="small">{note}</p>}
      {set && !set.enabled && <p className="banner warn small">{D.disabled}</p>}
      {groups.map(
        ([label, list]) =>
          list.length > 0 && (
            <div key={label} className="detail-group">
              <div className="detail-label">
                {label} <span className="muted">({list.length})</span>
              </div>
              <div className="chips">
                {list.map((x) => (
                  <span key={x} className="chip mono">
                    {x}
                  </span>
                ))}
              </div>
            </div>
          ),
      )}
      {(set || node.id === "r:home") && (
        <button className="btn small" onClick={() => setPage(set ? "rules" : "profiles")}>
          <Icon name="rules" />
          {set ? D.editRules : D.editHome}
        </button>
      )}
      <div className="detail-label">{D.connections}</div>
      <ul className="detail-conns">
        {rows.slice(0, 60).map((r) => (
          <li key={`${r.prog}|${r.target}|${r.vpn}`}>
            <span className={r.vpn ? "tag good" : "tag"}>{r.vpn ? T.traffic.vpn : T.traffic.direct}</span>
            <span className="mono break">{r.target}</span>
            <span className="muted small">
              {node.column === "program" ? "" : `${r.prog} · `}
              {r.n > 1 ? `${D.count(r.n)} · ` : ""}
              {bytes(r.bytes)}
            </span>
          </li>
        ))}
      </ul>
      {rows.length > 60 && <p className="muted small">{D.more(rows.length - 60)}</p>}
    </aside>
  );
}
