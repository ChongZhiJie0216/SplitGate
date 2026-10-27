import { useEffect, useState } from "react";
import { useStatus } from "../store/status";
import { T } from "../lib/text";
import { bytes, fileName } from "../lib/format";
import * as ipc from "../lib/ipc";
import Icon from "../components/Icon";
import Empty from "../components/Empty";

const L = T.usagePage;
const REFRESH_MS = 2000;

/** 流量统计：只算走 VPN 的，总上传 / 下载，再分到每个程序。 */
export default function UsagePage() {
  const status = useStatus((s) => s.status);
  const connected = status.state === "connected";
  const [usage, setUsage] = useState<ipc.Usage | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      ipc
        .getVpnUsage()
        .then((u) => alive && (setUsage(u), setError(null)))
        .catch((e) => alive && setError(String(e)));
    tick();
    // 断开后数字不会再变，不用一直刷新。
    if (!connected) return () => void (alive = false);
    const t = setInterval(tick, REFRESH_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [connected]);

  const reset = async () => {
    try {
      setUsage(await ipc.resetVpnUsage());
    } catch (e) {
      setError(String(e));
    }
  };

  const total = (usage?.upload ?? 0) + (usage?.download ?? 0);
  const max = Math.max(1, ...(usage?.apps ?? []).map((a) => a.upload + a.download));
  const since = usage?.since_ms ? new Date(usage.since_ms).toLocaleString("zh-CN", { hour12: false }) : null;

  return (
    <div className="stack wide-stack">
      <div className="card">
        <div className="row conn-head">
          <h2 className="title-icon">
            <Icon name="chart" size={20} />
            {T.nav.usage}
          </h2>
          <button className="btn small" onClick={reset} disabled={!usage?.apps.length}>
            <Icon name="refresh" />
            {L.reset}
          </button>
        </div>
        <p className="muted small">{L.intro}</p>
        {since && <p className="muted small">{L.since(since)}</p>}
        {!connected && usage && usage.apps.length > 0 && <p className="muted small">{L.notConnected}</p>}
        {error && <div className="banner bad">{error}</div>}
        <div className="tiles three">
          <Stat icon="upload" label={L.up} value={bytes(usage?.upload ?? 0)} />
          <Stat icon="download" label={L.down} value={bytes(usage?.download ?? 0)} />
          <Stat icon="arrows" label={L.total} value={bytes(total)} />
        </div>
      </div>

      <div className="card">
        <h2>{L.apps}</h2>
        {!usage || usage.apps.length === 0 ? (
          <Empty icon="chart" text={L.empty} />
        ) : (
          <table className="conn-table usage-table">
            <thead>
              <tr>
                <th>{L.program}</th>
                <th className="num">{L.up}</th>
                <th className="num">{L.down}</th>
                <th className="num">{L.total}</th>
                <th className="share">{L.share}</th>
              </tr>
            </thead>
            <tbody>
              {usage.apps.map((a) => {
                const t = a.upload + a.download;
                return (
                  <tr key={a.process_path || "system"}>
                    <td title={a.process_path}>{a.process_path ? fileName(a.process_path) : T.canvasPage.labels.system}</td>
                    <td className="num mono">{bytes(a.upload)}</td>
                    <td className="num mono">{bytes(a.download)}</td>
                    <td className="num mono">{bytes(t)}</td>
                    <td className="share">
                      <div className="share-cell">
                        <div className="bar">
                          <div className="bar-fill" style={{ width: `${(t / max) * 100}%` }} />
                        </div>
                        <span className="muted small mono">{total ? `${((t / total) * 100).toFixed(1)}%` : ""}</span>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function Stat({ icon, label, value }: { icon: "upload" | "download" | "arrows"; label: string; value: string }) {
  return (
    <div className="tile">
      <span className="tile-label">
        <Icon name={icon} size={15} />
        {label}
      </span>
      <span className="tile-value mono">{value}</span>
    </div>
  );
}
