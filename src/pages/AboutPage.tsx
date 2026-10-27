import { useEffect, useState } from "react";
import { useApp } from "../store/app";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";
import Markdown from "../components/Markdown";
import { useStatus } from "../store/status";
import Icon, { IconName } from "../components/Icon";
import Logo from "../components/Logo";

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
  const [dialog, setDialog] = useState<"log" | "licenses" | null>(null);
  const [licenses, setLicenses] = useState<ipc.License[]>([]);
  useEffect(() => {
    ipc
      .getLicenses()
      .then(setLicenses)
      .catch(() => setLicenses([]));
  }, []);
  useEffect(() => {
    if (!dialog) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setDialog(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialog]);
  const close = () => setDialog(null);
  return (
    <div className="stack">
      <div className="card about-hero">
        <Logo size={64} />
        <div className="about-title">
          <h2>
            {T.appName} <span className="pill mono">v{info?.version ?? "…"}</span>
          </h2>
          <p className="muted">{L.tagline}</p>
        </div>
        <div className="row tight">
          {licenses.length > 0 && (
            <button className="icon-btn" title={L.licenses} aria-label={L.licenses} onClick={() => setDialog("licenses")}>
              <Icon name="scale" size={20} />
            </button>
          )}
          {changelog.length > 0 && (
            <button className="icon-btn" title={L.changelog} aria-label={L.changelog} onClick={() => setDialog("log")}>
              <Icon name="history" size={20} />
            </button>
          )}
        </div>
      </div>

      <div className="card">
        <div className="tiles">
          <InfoTile icon="shield" label="sing-box" value={info ? (info.singboxVersion ?? L.notFound) : "…"} mono />
          <InfoTile icon="key" label={L.admin} value={info ? (info.isAdmin ? L.yes : L.no) : "…"} />
          <div className="tile wide">
            <span className="tile-label">
              <Icon name="folder" size={15} />
              {L.dataDir}
            </span>
            <span className="mono small break">{info?.dataDir ?? "…"}</span>
          </div>
        </div>
        <p className="muted small">{L.licensesShort}</p>
      </div>

      <div className="card">
        <h2 className="title-icon">
          <Icon name="download" size={20} />
          {L.update}
        </h2>
        <div className="row">
          <button className="btn" onClick={() => checkUpdate()} disabled={checkingUpdate}>
            <Icon name="refresh" />
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

      {dialog === "licenses" && (
        <div className="modal-back" onClick={close}>
          <div className="card modal" role="dialog" aria-label={L.licenses} onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h2>
                <Icon name="scale" size={20} /> {L.licenses}
              </h2>
              <button className="icon-btn" title={L.close} aria-label={L.close} onClick={close}>
                <Icon name="x" size={20} />
              </button>
            </div>
            <p className="muted small">{L.licensesHint}</p>
            <div className="modal-body">
              {licenses.map((l) => (
                <details key={l.name} className="update-note" open={l.name === "SplitGate"}>
                  <summary className="update-ver">
                    {l.name} <span className="tag">{l.license}</span>
                  </summary>
                  <p className="muted small mono break">{l.url}</p>
                  {l.text && <pre className="license-text">{l.text}</pre>}
                </details>
              ))}
            </div>
          </div>
        </div>
      )}
      {dialog === "log" && (
        <div className="modal-back" onClick={close}>
          <div className="card modal" role="dialog" aria-label={L.changelog} onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h2>
                <Icon name="history" size={20} /> {L.changelog}
              </h2>
              <button className="icon-btn" title={L.close} aria-label={L.close} onClick={close}>
                <Icon name="x" size={20} />
              </button>
            </div>
            <p className="muted small">{L.changelogHint}</p>
            <div className="modal-body">
              {changelog.map((n) => (
                <details key={n.version} className="update-note" open={n.version === info?.version}>
                  <summary className="update-ver">
                    v{n.version} <span className="muted small">{n.title}</span>
                  </summary>
                  <Markdown text={n.body} />
                </details>
              ))}
            </div>
          </div>
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
            <Icon name="download" />
            {job.state === "failed" ? L.retry : L.download}
          </button>
        )}
        {job.state === "ready" && (
          <button className="btn primary" onClick={install}>
            <Icon name="check" />
            {connected ? L.installConnected : L.install}
          </button>
        )}
      </div>
    </div>
  );
}

function InfoTile({ icon, label, value, mono }: { icon: IconName; label: string; value: string; mono?: boolean }) {
  return (
    <div className="tile">
      <span className="tile-label">
        <Icon name={icon} size={15} />
        {label}
      </span>
      <span className={mono ? "tile-value mono" : "tile-value"}>{value}</span>
    </div>
  );
}
