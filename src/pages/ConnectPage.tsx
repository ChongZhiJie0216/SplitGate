import { useEffect, useState } from "react";
import { useStatus } from "../store/status";
import { useProfiles } from "../store/profiles";
import { useApp } from "../store/app";
import ConnectButton from "../components/ConnectButton";
import Icon, { IconName } from "../components/Icon";
import { TrafficBadges } from "./ConnectionsPage";
import { T } from "../lib/text";
import { duration } from "../lib/format";
import { connectionProfiles } from "../lib/ipc";

const L = T.connectPage;

export default function ConnectPage() {
  const { status, actionError, traffic, disconnect } = useStatus();
  const view = useProfiles((s) => s.view);
  const setPage = useApp((s) => s.setPage);
  const chosen = connectionProfiles(view);
  const [now, setNow] = useState(Date.now());
  const retrying = status.state === "error" && status.retrying;

  useEffect(() => {
    if (status.state !== "connected") return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [status.state]);

  // 等待自动重连时显示成黄色（和“连接中”一样），不是红色的“错误”。
  const look = retrying ? "connecting" : status.state;
  // 连上时以后端说的为准（旧的后台服务没有 exits，只有第一个出口）；没连上时显示将要连的。
  const exits =
    status.state === "connected"
      ? (status.exits?.map((e) => ({ name: e.name, type: e.exit })) ?? [{ name: status.profile_name, type: status.exit }])
      : chosen.map((p) => ({ name: p.name, type: p.type }));
  const exitName = exits.length ? exits.map((e) => `${e.name}（${T.profile.types[e.type]}）`).join(" + ") : null;

  return (
    <div className="card connect-card">
      <div className="hero">
        <div className={`hero-ring ${look}`}>
          <Icon name="power" size={30} />
        </div>
        <div>
          <h2>{retrying ? T.status.retrying : T.status[status.state]}</h2>
          {exitName && (
            <p className="muted hero-sub">
              {L.exit}：{exitName}
            </p>
          )}
        </div>
      </div>

      {status.state === "connected" && (
        <div className="tiles">
          <Tile icon="clock" label={L.since} value={duration(now - status.since_ms)} />
          <Tile icon="gauge" label={L.probe} value={`${status.probe_ms} ms`} />
          <div className="tile wide">
            <span className="tile-label">
              <Icon name="arrows" size={15} />
              {L.speed}
            </span>
            <TrafficBadges traffic={traffic} />
          </div>
        </div>
      )}
      {status.state === "connected" &&
        status.warnings.map((w) => (
          <div key={w} className="banner warn">
            <b>{L.homeWarnTitle}：</b>
            {w}
          </div>
        ))}

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

      {status.state === "disconnected" && <p className="muted">{chosen.length ? L.idle : L.noProfile}</p>}
      <div className="row">
        <ConnectButton big />
      </div>
    </div>
  );
}

function Tile({ icon, label, value }: { icon: IconName; label: string; value: string }) {
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
