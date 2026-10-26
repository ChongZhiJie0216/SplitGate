import { useEffect, useMemo, useRef, useState } from "react";
import { useStatus } from "../store/status";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";

const L = T.logsPage;
type Filter = "all" | "app" | "sing-box" | "problems";

export default function LogsPage() {
  const { logs, clearLogs } = useStatus();
  const [filter, setFilter] = useState<Filter>("all");
  const [paused, setPaused] = useState(false);
  const [copied, setCopied] = useState(false);
  const [diag, setDiag] = useState<{ busy: boolean; path?: string; error?: string }>({ busy: false });
  const box = useRef<HTMLDivElement>(null);

  const shown = useMemo(
    () =>
      logs.filter((l) =>
        filter === "all" ? true : filter === "problems" ? l.level !== "info" : l.source === filter,
      ),
    [logs, filter],
  );

  useEffect(() => {
    if (!paused && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [shown, paused]);

  const copy = async () => {
    await navigator.clipboard.writeText(shown.map((l) => `${l.time} [${l.source}] ${l.text}`).join("\n"));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const exportDiag = async () => {
    setDiag({ busy: true });
    try {
      setDiag({ busy: false, path: await ipc.exportDiagnostics() });
    } catch (e) {
      setDiag({ busy: false, error: String(e) });
    }
  };

  const show = async (path: string) => {
    try {
      await ipc.showDiagnostics(path);
    } catch (e) {
      setDiag({ busy: false, error: String(e) });
    }
  };

  return (
    <div className="card logs-card">
      <div className="row top">
        <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
          <option value="all">{L.all}</option>
          <option value="app">{L.onlyApp}</option>
          <option value="sing-box">{L.onlySingbox}</option>
          <option value="problems">{L.onlyProblems}</option>
        </select>
        <label className="check">
          <input type="checkbox" checked={paused} onChange={(e) => setPaused(e.target.checked)} />
          {L.pause}
        </label>
        <div className="spacer" />
        <button className="btn" onClick={copy}>
          {copied ? L.copied : L.copy}
        </button>
        <button className="btn" onClick={clearLogs}>
          {L.clear}
        </button>
        <button className="btn" onClick={exportDiag} disabled={diag.busy} title={L.diagHint}>
          {diag.busy ? L.exporting : L.exportDiag}
        </button>
      </div>
      {diag.path && (
        <div className="banner good">
          {L.exported}
          <span className="mono small">{diag.path}</span>{" "}
          <button className="link" onClick={() => show(diag.path!)}>
            {L.showFile}
          </button>
        </div>
      )}
      {diag.error && <div className="banner bad">{diag.error}</div>}
      <div className="log-box mono" ref={box}>
        {shown.length === 0 && <div className="muted">{L.empty}</div>}
        {shown.map((l, i) => (
          <div key={i} className={`log-line ${l.level}`}>
            <span className="log-time">{l.time}</span>
            <span className={`log-src ${l.source === "app" ? "app" : "sb"}`}>{l.source}</span>
            <span className="log-text">{l.text}</span>
          </div>
        ))}
      </div>
      <p className="muted small">
        {L.fileHint}
        {L.diagHint}
      </p>
    </div>
  );
}
