import { useEffect, useMemo, useState } from "react";
import { useBlocked } from "../store/blocked";
import { useRules } from "../store/rules";
import { useProfiles } from "../store/profiles";
import { useStatus } from "../store/status";
import { T } from "../lib/text";
import { fileName } from "../lib/format";
import * as ipc from "../lib/ipc";
import { AddRuleModal } from "./ConnectionsPage";

const L = T.blockedPage;

/** 加规则对话框用的是连接列表的那一个，这里给它一条只有域名和程序的“连接”。 */
const asConnection = (b: ipc.Blocked): ipc.Connection => ({
  id: "",
  network: "tcp",
  host: b.host,
  ip: "",
  port: "443",
  process_path: b.process_path,
  outbound: "direct",
  rule: "",
  upload: 0,
  download: 0,
  start: "",
});

export default function BlockedPage() {
  const { list, error, run, seen } = useBlocked();
  const { rulesets, load } = useRules();
  const view = useProfiles((s) => s.view);
  const connected = useStatus((s) => s.status.state === "connected");
  const [adding, setAdding] = useState<ipc.Blocked | null>(null);

  useEffect(() => {
    load();
    run(ipc.listBlocked);
  }, [load, run]);
  // 在这一页看到的都算看过了。
  useEffect(() => seen(), [list, seen]);

  // 已经被使用中 Profile 的规则集覆盖的域名：加过规则，重新连接后就走 VPN。
  const covered = useMemo(() => {
    const used = view?.profiles.find((p) => p.profile.id === view.active)?.profile.ruleset_ids ?? [];
    const sets = (rulesets ?? []).map((r) => r.ruleset).filter((r) => r.enabled && used.includes(r.id));
    return (host: string) =>
      sets.some(
        (r) => r.domain_suffix.some((s) => host === s || host.endsWith(`.${s}`)) || r.domain_keyword.some((k) => host.includes(k)),
      );
  }, [rulesets, view]);

  return (
    <div className="card conn-card">
      <div className="row conn-head">
        <h2>{T.nav.blocked}</h2>
        {list.length > 0 && (
          <button className="btn small" onClick={() => run(ipc.clearBlocked)}>
            {L.clear}
          </button>
        )}
      </div>
      <p className="muted small">{connected ? L.intro : L.notConnected}</p>
      {error && <div className="banner bad">{error}</div>}
      {list.length === 0 ? (
        <p className="muted">{L.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="conn-table blocked-table">
            <thead>
              <tr>
                <th>{L.cols.host}</th>
                <th>{L.cols.reason}</th>
                <th>{L.cols.program}</th>
                <th className="num">{L.cols.time}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {list.map((b) => (
                <tr key={b.host}>
                  <td className="wrap">
                    <div>{b.host}</div>
                    {covered(b.host) && <span className="tag good">{L.covered}</span>}
                  </td>
                  <td className="wrap">
                    <span className={b.reason === "handshake" ? "tag" : "tag warn"}>{L.reasons[b.reason]}</span>
                    <div className="muted small">{b.detail}</div>
                  </td>
                  <td title={b.process_path}>{b.process_path ? fileName(b.process_path) : <span className="muted">{T.connectionsPage.system}</span>}</td>
                  <td className="num mono small">{b.time}</td>
                  <td className="actions">
                    <button className="btn small" onClick={() => setAdding(b)}>
                      {T.connectionsPage.addRule}
                    </button>
                    <button className="btn small" onClick={() => run(() => ipc.dismissBlocked(b.host))}>
                      {L.dismiss}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {adding && <AddRuleModal conn={asConnection(adding)} onClose={() => setAdding(null)} />}
    </div>
  );
}
