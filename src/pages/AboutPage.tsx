import { useEffect, useState } from "react";
import { useApp } from "../store/app";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";
import Markdown from "../components/Markdown";

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
            <button className="btn primary" onClick={() => ipc.openReleasePage(update.url!)}>
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
              <b>{L.newVersion(update.latest ?? "")}</b>（{L.version} {update.current}）。{L.updateHint}
            </div>
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
