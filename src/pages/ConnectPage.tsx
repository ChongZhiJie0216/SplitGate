import { useEffect, useState } from "react";
import { useStatus } from "../store/status";
import { useProfiles } from "../store/profiles";
import { useApp } from "../store/app";
import ConnectButton from "../components/ConnectButton";
import { TrafficBadges } from "./ConnectionsPage";
import { T } from "../lib/text";
import { duration } from "../lib/format";

const L = T.connectPage;

export default function ConnectPage() {
  const { status, actionError, traffic, disconnect } = useStatus();
  const view = useProfiles((s) => s.view);
  const setPage = useApp((s) => s.setPage);
  const active = view?.profiles.find((p) => p.profile.id === view.active)?.profile;
  const [now, setNow] = useState(Date.now());
  const retrying = status.state === "error" && status.retrying;

  useEffect(() => {
    if (status.state !== "connected") return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [status.state]);

  return (
    <div className="card connect-card">
      {/* 等待自动重连时显示成黄色（和“连接中”一样），不是红色的“错误”。 */}
      <div className={`state-dot ${retrying ? "connecting" : status.state}`} />
      <h2>{retrying ? T.status.retrying : T.status[status.state]}</h2>

      {status.state === "connected" && (
        <table className="kv">
          <tbody>
            <tr>
              <th>{L.exit}</th>
              <td>
                {status.profile_name}（{T.profile.types[status.exit]}）
              </td>
            </tr>
            <tr>
              <th>{L.since}</th>
              <td className="mono">{duration(now - status.since_ms)}</td>
            </tr>
            <tr>
              <th>{L.probe}</th>
              <td className="mono">{status.probe_ms} ms</td>
            </tr>
            <tr>
              <th>{L.speed}</th>
              <td>
                <TrafficBadges traffic={traffic} />
              </td>
            </tr>
          </tbody>
        </table>
      )}

      {status.state === "connecting" && (
        <ol className="steps">
          {(Object.keys(T.steps) as (keyof typeof T.steps)[]).map((k) => {
            const order = Object.keys(T.steps);
            const cur = order.indexOf(status.step);
            const i = order.indexOf(k);
            return (
              <li key={k} className={i < cur ? "done" : i === cur ? "now" : ""}>
                {T.steps[k]}
              </li>
            );
          })}
        </ol>
      )}

      {status.state === "error" && (
        <div className={status.retrying ? "banner warn" : "banner bad"}>
          <b>{status.retrying ? L.retryingTitle : L.errorTitle}：</b>
          {status.message}{" "}
          {status.retrying && (
            <button className="link" onClick={disconnect}>
              {L.stopRetry}
            </button>
          )}{" "}
          <button className="link" onClick={() => setPage("logs")}>
            {L.seeLogs}
          </button>
        </div>
      )}
      {actionError && <div className="banner bad">{actionError}</div>}

      {status.state === "disconnected" && <p className="muted">{active ? L.idle : L.noProfile}</p>}
      {active && status.state !== "connected" && status.state !== "connecting" && (
        <p>
          {L.exit}：{active.name}（{T.profile.types[active.type]}）
        </p>
      )}
      <div className="row">
        <ConnectButton big />
      </div>
    </div>
  );
}
