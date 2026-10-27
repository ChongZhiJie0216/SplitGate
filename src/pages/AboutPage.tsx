import { useEffect, useState } from "react";
import { useApp } from "../store/app";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";
import Markdown from "../components/Markdown";
import { useStatus } from "../store/status";

const L = T.about;

export default function AboutPage() {
  const { info, update, updateError, checkingUpdate, checkUpdate } = useApp();
  const [changelog, setChangelog] = useState<ipc.UpdateSection[]>([]);
  useEffect(() => {
    ipc
      .getChangelog()
      .then(setChangelog)
      .catch(() => setChangelog([]));
  }, []);
  return (
    <div className="stack">
      <div className="card">
        <h2>{T.nav.about}</h2>
        <table className="kv">
          <tbody>
            <tr>
              <th>{L.version}</th>
              <td className="mono">{info?.version ?? "…"}</td>
            </tr>
            <tr>
              <th>sing-box</th>
              <td className="mono">{info ? (info.singboxVersion ?? L.notFound) : "…"}</td>
            </tr>
            <tr>
              <th>{L.admin}</th>
              <td>{info ? (info.isAdmin ? L.yes : L.no) : "…"}</td>
            </tr>
            <tr>
              <th>{L.dataDir}</th>
              <td className="mono">{info?.dataDir ?? "…"}</td>
            </tr>
          </tbody>
        </table>
        <p className="muted">{L.licenses}</p>
        <p className="muted">{L.singboxNote}</p>
        <p className="banner warn">{L.notice}</p>
      </div>

      <div className="card">
        <h2>{L.update}</h2>
        <div className="row">
          <button className="btn" onClick={() => checkUpdate()} disabled={checkingUpdate}>
            {checkingUpdate ? L.checking : L.check}
          </button>
          {update?.newer && update.url && (
            <button className={`btn${update.download ? "" : " primary"}`} onClick={() => ipc.openReleasePage(update.url!)}>
              {L.openPage}
            </button>
          )}
        </div>
        {updateError && <div className="banner bad">{updateError}</div>}
        {update && !update.newer && (
          <p className="muted">
            {!update.latest ? L.noRelease : update.latest === update.current ? L.upToDate(update.latest) : L.ahead(update.latest)}
          </p>
        )}
        {update?.newer && (
          <>
            <div className="banner good">
              <b>{L.newVersion(update.latest ?? "")}</b>（{L.version} {update.current}）。{update.download ? L.updateHint : L.updateManual}
            </div>
            {update.download && <UpdateSteps />}
            <h3>{L.changes}</h3>
            {update.notes.map((n) => (
              <div key={n.version} className="update-note">
                <div className="update-ver">
                  v{n.version} <span className="muted small">{n.title}</span>
                </div>
                <Markdown text={n.body} />
              </div>
            ))}
          </>
        )}
      </div>

      {changelog.length > 0 && (
        <div className="card">
          <h2>{L.changelog}</h2>
          <p className="muted small">{L.changelogHint}</p>
          {changelog.map((n) => (
            <details key={n.version} className="update-note" open={n.version === info?.version}>
              <summary className="update-ver">
                v{n.version} <span className="muted small">{n.title}</span>
              </summary>
              <Markdown text={n.body} />
            </details>
          ))}
        </div>
      )}
    </div>
  );
}

const mb = (n: number) => `${(n / 1_000_000).toFixed(1)} MB`;

/** 下载 → 核对 → 安装并重新启动。正在连接时，按钮写明会先断开（这就是“先问”）。 */
function UpdateSteps() {
  const [job, setJob] = useState<ipc.UpdateJob>({ state: "idle" });
  const [error, setError] = useState<string | null>(null);
  const status = useStatus((s) => s.status);
  const service = useApp((s) => s.service);
  const connected = status.state !== "disconnected" && status.state !== "error";
  const busy = job.state === "downloading" || job.state === "installing";

  useEffect(() => {
    ipc.getUpdateJob().then(setJob).catch(() => {});
  }, []);
  useEffect(() => {
    if (!busy) return;
    const t = setInterval(() => ipc.getUpdateJob().then(setJob).catch(() => {}), 500);
    return () => clearInterval(t);
  }, [busy]);

  const start = async () => {
    setError(null);
    try {
      setJob(await ipc.downloadUpdate());
    } catch (e) {
      setError(String(e));
    }
  };
  const install = async () => {
    setError(null);
    setJob((j) => (j.state === "ready" ? { state: "installing", version: j.version } : j));
    try {
      await ipc.installUpdate();
    } catch (e) {
      setError(String(e));
      ipc.getUpdateJob().then(setJob).catch(() => {});
    }
  };

  return (
    <div className="stack-sm">
      {job.state === "downloading" && <p>{L.downloading(mb(job.done), job.total ? mb(job.total) : "")}</p>}
      {job.state === "installing" && <p>{L.installing}</p>}
      {job.state === "failed" && <div className="banner bad">{job.message}</div>}
      {error && <div className="banner bad">{error}</div>}
      {job.state === "ready" && (
        <>
          <p>{L.ready(job.version)}</p>
          {connected && <div className="banner warn">{L.connectedWarn}</div>}
          {service?.installed && service.matches_this_exe && <p className="muted small">{L.serviceWarn}</p>}
        </>
      )}
      <div className="row">
        {(job.state === "idle" || job.state === "failed") && (
          <button className="btn primary" onClick={start}>
            {job.state === "failed" ? L.retry : L.download}
          </button>
        )}
        {job.state === "ready" && (
          <button className="btn primary" onClick={install}>
            {connected ? L.installConnected : L.install}
          </button>
        )}
      </div>
    </div>
  );
}
