import { useEffect, useMemo, useState } from "react";
import { useStatus } from "../store/status";
import { useRules } from "../store/rules";
import { useProfiles } from "../store/profiles";
import { T } from "../lib/text";
import { bytes, duration, fileName, rate } from "../lib/format";
import * as ipc from "../lib/ipc";
import Icon from "../components/Icon";
import Empty from "../components/Empty";

const L = T.connectionsPage;
const MAX_ROWS = 300;

type Scope = "all" | "vpn" | "direct";

/** sing-box 的时间有 7 位小数，先截成毫秒再解析。 */
const startMs = (s: string) => Date.parse(s.replace(/(\.\d{3})\d+/, "$1"));

export default function ConnectionsPage() {
  const { status, traffic } = useStatus();
  const connected = status.state === "connected";
  const [snap, setSnap] = useState<ipc.ConnectionsSnapshot | null>(null);
  const [paused, setPaused] = useState(false);
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<Scope>("all");
  const [adding, setAdding] = useState<ipc.Connection | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!connected || paused) return;
    let alive = true;
    const tick = () =>
      ipc
        .getConnections()
        .then((s) => alive && setSnap(s))
        .catch((e) => alive && setError(String(e)));
    tick();
    const t = setInterval(tick, 1000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [connected, paused]);

  const all = connected ? snap?.connections ?? [] : [];
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return all.filter(
      (c) =>
        (scope === "all" || c.outbound === scope) &&
        (!q || c.host.toLowerCase().includes(q) || c.ip.includes(q) || c.process_path.toLowerCase().includes(q)),
    );
  }, [all, query, scope]);

  if (!connected) {
    return (
      <div className="card">
        <h2>{T.nav.connections}</h2>
        <Empty icon="connections" text={L.notConnected} />
      </div>
    );
  }

  const close = async (id: string) => {
    try {
      await ipc.closeConnection(id);
      setSnap((s) => (s ? { ...s, connections: s.connections.filter((c) => c.id !== id) } : s));
    } catch (e) {
      setError(String(e));
    }
  };

  const now = Date.now();
  return (
    <div className="card conn-card">
      <div className="row conn-head">
        <h2>{T.nav.connections}</h2>
        <TrafficBadges traffic={traffic} />
      </div>
      <div className="row">
        <input className="grow" placeholder={L.filter} value={query} onChange={(e) => setQuery(e.target.value)} />
        <select value={scope} onChange={(e) => setScope(e.target.value as Scope)}>
          <option value="all">{L.all}</option>
          <option value="vpn">{L.onlyVpn}</option>
          <option value="direct">{L.onlyDirect}</option>
        </select>
        <label className="check">
          <input type="checkbox" checked={paused} onChange={(e) => setPaused(e.target.checked)} />
          {L.pause}
        </label>
        <span className="muted small">{L.count(shown.length, all.length)}</span>
      </div>
      {error && <div className="banner bad">{error}</div>}
      {shown.length > MAX_ROWS && <p className="muted small">{L.tooMany(MAX_ROWS)}</p>}
      <div className="table-wrap">
        <table className="conn-table">
          <thead>
            <tr>
              <th>{L.cols.program}</th>
              <th>{L.cols.target}</th>
              <th>{L.cols.exit}</th>
              <th>{L.cols.rule}</th>
              <th className="num">{L.cols.traffic}</th>
              <th className="num">{L.cols.age}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {shown.slice(0, MAX_ROWS).map((c) => (
              <tr key={c.id}>
                <td title={c.process_path}>{c.process_path ? fileName(c.process_path) : <span className="muted">{L.system}</span>}</td>
                <td className="target" title={`${c.host || c.ip}:${c.port} (${c.network})`}>
                  {c.host || c.ip}
                  <span className="muted">:{c.port}</span>
                  {c.network === "udp" && <span className="tag">UDP</span>}
                </td>
                <td>
                  <span className={c.outbound === "vpn" ? "tag good" : "tag"}>{c.outbound === "vpn" ? T.traffic.vpn : T.traffic.direct}</span>
                </td>
                <td className="muted small rule-cell" title={c.rule}>
                  {c.rule === "final" ? L.finalRule : c.rule.replace(/\s*=>.*$/, "")}
                </td>
                <td className="num mono small">
                  {bytes(c.upload)} / {bytes(c.download)}
                </td>
                <td className="num mono small">{duration(now - startMs(c.start))}</td>
                <td className="actions">
                  <button className="btn small" onClick={() => setAdding(c)}>
                    <Icon name="plus" />
                    {L.addRule}
                  </button>
                  <button className="btn small" onClick={() => close(c.id)}>
                    <Icon name="x" />
                    {L.close}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {shown.length === 0 && <p className="muted">{L.empty}</p>}
      </div>
      {adding && <AddRuleModal conn={adding} onClose={() => setAdding(null)} />}
    </div>
  );
}

export function TrafficBadges({ traffic }: { traffic: ipc.Traffic }) {
  return (
    <div className="traffic">
      <span className="tag good">
        {T.traffic.vpn} ↑ {rate(traffic.vpn_up)} ↓ {rate(traffic.vpn_down)}
      </span>
      <span className="tag">
        {T.traffic.direct} ↑ {rate(traffic.direct_up)} ↓ {rate(traffic.direct_down)}
      </span>
    </div>
  );
}

type Kind = "domain_suffix" | "process_path" | "ip_cidr";

/** login.live.com → ["live.com"]：上一级域名，到只剩两段为止。 */
function parentDomains(host: string): string[] {
  const parts = host.split(".");
  const out: string[] = [];
  for (let i = 1; i < parts.length - 1; i++) out.push(parts.slice(i).join("."));
  return out;
}

export function AddRuleModal({ conn, onClose }: { conn: ipc.Connection; onClose: () => void }) {
  const A = T.addRule;
  const { rulesets, load, run } = useRules();
  const view = useProfiles((s) => s.view);
  const { status, reconnect, markStale } = useStatus();
  const kinds: Kind[] = [
    ...(conn.host ? (["domain_suffix"] as const) : []),
    ...(conn.process_path ? (["process_path"] as const) : []),
    ...(conn.ip ? (["ip_cidr"] as const) : []),
  ];
  const initial = (k: Kind) => (k === "domain_suffix" ? conn.host : k === "process_path" ? conn.process_path : conn.ip);
  const [kind, setKind] = useState<Kind>(kinds[0] ?? "domain_suffix");
  const [value, setValue] = useState(initial(kinds[0] ?? "domain_suffix"));
  const used = view?.profiles.find((p) => p.profile.id === view.active)?.profile.ruleset_ids ?? [];
  const [target, setTarget] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    load();
  }, [load]);
  useEffect(() => {
    if (!target && rulesets?.length) setTarget(rulesets.find((r) => used.includes(r.ruleset.id))?.ruleset.id ?? rulesets[0].ruleset.id);
  }, [rulesets, target, used]);

  const pick = (k: Kind) => {
    setKind(k);
    setValue(initial(k));
    setMsg(null);
  };

  const save = async () => {
    const rs = rulesets?.find((r) => r.ruleset.id === target)?.ruleset;
    if (!rs || !value.trim()) return;
    const list = rs[kind];
    if (list.some((x) => x.toLowerCase() === value.trim().toLowerCase())) return setMsg({ ok: false, text: A.already });
    const err = await run(async () => (await ipc.saveRuleset({ ...rs, [kind]: [...list, value.trim()] })).rulesets);
    if (!err && used.includes(target)) markStale(); // 关掉这个窗口后顶部也会提示重新连接
    setMsg(err ? { ok: false, text: err } : { ok: true, text: A.saved });
  };

  const labels: Record<Kind, string> = { domain_suffix: A.byDomain, process_path: A.byPath, ip_cidr: A.byIp };
  return (
    <div className="modal-back" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()}>
        <h2>{A.title}</h2>
        <div className="field">
          <span className="field-label">{A.kind}</span>
          <div className="checks">
            {kinds.map((k) => (
              <label className="check" key={k}>
                <input type="radio" checked={kind === k} onChange={() => pick(k)} />
                {labels[k]}
              </label>
            ))}
          </div>
        </div>
        <label className="field">
          <span className="field-label">{A.value}</span>
          <input className="mono" value={value} onChange={(e) => setValue(e.target.value)} />
          {kind === "domain_suffix" && parentDomains(conn.host).length > 0 && (
            <span className="field-hint">
              {A.parents}
              {parentDomains(conn.host).map((d) => (
                <button key={d} className="link" onClick={() => setValue(d)}>
                  {d}
                </button>
              ))}
            </span>
          )}
          {kind === "domain_suffix" && <span className="field-hint">{T.rules.fields.domain_suffix[1]}</span>}
          {kind === "process_path" && <span className="field-hint">{T.rules.fields.process_path[1]}</span>}
        </label>
        <label className="field">
          <span className="field-label">{A.ruleset}</span>
          <select value={target} onChange={(e) => setTarget(e.target.value)}>
            {rulesets?.map(({ ruleset: r }) => (
              <option key={r.id} value={r.id}>
                {r.name}
                {r.enabled ? "" : A.off}
              </option>
            ))}
          </select>
        </label>
        {target && !used.includes(target) && <div className="banner warn">{A.notInProfile}</div>}
        {msg && <div className={msg.ok ? "banner good" : "banner bad"}>{msg.text}</div>}
        <div className="row">
          {msg?.ok ? (
            <button
              className="btn primary"
              disabled={status.state !== "connected"}
              onClick={() => {
                onClose();
                reconnect();
              }}
            >
              {A.reconnect}
            </button>
          ) : (
            <button className="btn primary" disabled={!value.trim() || !target} onClick={save}>
              {A.save}
            </button>
          )}
          <button className="btn" onClick={onClose}>
            {A.cancel}
          </button>
        </div>
      </div>
    </div>
  );
}
