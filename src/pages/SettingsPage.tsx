import { useEffect, useState } from "react";
import { useApp } from "../store/app";
import { T } from "../lib/text";
import { Theme, applyTheme, loadTheme } from "../lib/theme";
import * as ipc from "../lib/ipc";

const L = T.settings;
const LEVELS = ["info", "debug", "trace"] as const;
const DEFAULT_TUN = "172.19.0.1/30";

export default function SettingsPage() {
  const service = useApp((s) => s.service);
  const [view, setView] = useState<ipc.SettingsView | null>(null);
  const [draft, setDraft] = useState<ipc.Settings | null>(null);
  const [theme, setTheme] = useState<Theme>(loadTheme());
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [autoMsg, setAutoMsg] = useState<string | null>(null);
  const [autoBusy, setAutoBusy] = useState(false);

  useEffect(() => {
    ipc
      .getSettings()
      .then((v) => {
        setView(v);
        setDraft(v.settings);
      })
      .catch((e) => setMsg({ ok: false, text: String(e) }));
  }, []);

  const changeTheme = (t: Theme) => {
    setTheme(t);
    applyTheme(t);
  };

  const patch = (p: Partial<ipc.Settings>) => {
    setDraft((d) => (d ? { ...d, ...p } : d));
    setMsg(null);
  };

  const save = async () => {
    if (!draft) return;
    try {
      const s = await ipc.saveSettings(draft);
      setDraft(s);
      setView((v) => (v ? { ...v, settings: s, error: null } : v));
      setMsg({ ok: true, text: L.saved });
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  };

  const autostart = async (enabled: boolean) => {
    setAutoBusy(true);
    setAutoMsg(null);
    try {
      const a = await ipc.setAutostart(enabled);
      setView((v) => (v ? { ...v, autostart: a } : v));
    } catch (e) {
      setAutoMsg(String(e));
    } finally {
      setAutoBusy(false);
    }
  };

  if (!view || !draft) {
    return <div className="card">{msg ? <div className="banner bad">{msg.text}</div> : <p className="muted">…</p>}</div>;
  }

  const a = view.autostart;
  const dirty = !!view.error || JSON.stringify(draft) !== JSON.stringify(view.settings);

  return (
    <div className="stack">
      <div className="card">
        <h2>{L.startup}</h2>
        <label className="check">
          <input type="checkbox" checked={a.enabled} disabled={autoBusy} onChange={(e) => autostart(e.target.checked)} />
          {L.autostart}
        </label>
        <p className="field-hint">{L.autostartHint}</p>
        {a.enabled && !a.matches_this_exe && (
          <div className="banner warn">
            {L.autostartOther}
            <span className="mono">{a.command}</span>{" "}
            <button className="btn" disabled={autoBusy} onClick={() => autostart(true)}>
              {L.autostartFix}
            </button>
          </div>
        )}
        {a.enabled && a.matches_this_exe && service && a.elevated === ipc.serviceUsable(service) && (
          <div className="banner warn">
            {a.elevated ? L.autostartElevated : L.autostartLimited}{" "}
            <button className="btn" disabled={autoBusy} onClick={() => autostart(true)}>
              {L.autostartRedo}
            </button>
          </div>
        )}
        {autoMsg && <div className="banner bad">{autoMsg}</div>}
        <label className="check">
          <input type="checkbox" checked={draft.auto_connect} onChange={(e) => patch({ auto_connect: e.target.checked })} />
          {L.autoConnect}
        </label>
        <label className="check">
          <input type="checkbox" checked={draft.check_updates} onChange={(e) => patch({ check_updates: e.target.checked })} />
          {L.checkUpdates}
        </label>
        <p className="field-hint">{L.tray}</p>
      </div>

      <div className="card">
        <h2>{L.look}</h2>
        <label className="field">
          <span className="field-label">{T.theme.label}</span>
          <select value={theme} onChange={(e) => changeTheme(e.target.value as Theme)}>
            <option value="auto">{T.theme.auto}</option>
            <option value="light">{T.theme.light}</option>
            <option value="dark">{T.theme.dark}</option>
          </select>
        </label>
      </div>

      <div className="card">
        <h2>{L.connection}</h2>
        <label className="check">
          <input type="checkbox" checked={draft.auto_reconnect} onChange={(e) => patch({ auto_reconnect: e.target.checked })} />
          {L.autoReconnect}
        </label>
        <p className="field-hint">{L.autoReconnectHint}</p>
        <p className="muted">{L.connectionHint}</p>
        <label className="field">
          <span className="field-label">{L.logLevel}</span>
          <select
            value={draft.singbox_log_level}
            onChange={(e) => patch({ singbox_log_level: e.target.value as ipc.Settings["singbox_log_level"] })}
          >
            {LEVELS.map((l) => (
              <option key={l} value={l}>
                {L.logLevels[l]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">{L.tun}</span>
          <input className="mono" value={draft.tun_ipv4} onChange={(e) => patch({ tun_ipv4: e.target.value.trim() })} />
          <span className="field-hint">{L.tunHint}</span>
        </label>
      </div>

      <div className="card">
        {view.error && <div className="banner bad">{view.error}</div>}
        {msg && <div className={msg.ok ? "banner good" : "banner bad"}>{msg.text}</div>}
        <div className="row">
          <button className="btn primary" disabled={!dirty} onClick={save}>
            {L.save}
          </button>
          <button
            className="btn"
            onClick={() => patch({ auto_connect: false, auto_reconnect: true, check_updates: true, singbox_log_level: "info", tun_ipv4: DEFAULT_TUN })}
          >
            {L.reset}
          </button>
        </div>
      </div>
      <ServiceCard />
    </div>
  );
}

/** 后台服务：安装 / 移除马上生效（和开机自启一样，不经“保存”）。 */
function ServiceCard() {
  const state = useApp((s) => s.service);
  const setState = useApp((s) => s.setService);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);

  useEffect(() => {
    ipc
      .getServiceState()
      .then(setState)
      .catch((e) => setError(String(e)));
  }, [setState]);

  const run = async (op: () => Promise<ipc.ServiceState>) => {
    setBusy(true);
    setError(null);
    setConfirm(false);
    try {
      setState(await op());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const s = state;
  const status = !s
    ? "…"
    : !s.installed
      ? L.serviceNone
      : !s.running
        ? L.serviceStopped
        : s.version
          ? L.serviceRunning(s.version)
          : L.serviceNoAnswer;

  return (
    <div className="card">
      <h2>{L.service}</h2>
      <p className="field-hint">{L.serviceHint}</p>
      <p>{status}</p>
      {s?.installed && !s.matches_this_exe && (
        <div className="banner warn">
          {L.serviceOther}
          <span className="mono small">{s.app ?? s.command}</span>
        </div>
      )}
      {s?.installed && s.matches_this_exe && s.outdated && <div className="banner warn">{L.serviceOutdated(s.version ?? "?")}</div>}
      {s?.installed && s.matches_this_exe && !s.outdated && !s.protected && <div className="banner warn">{L.serviceUnprotected}</div>}
      {error && <div className="banner bad">{error}</div>}
      <div className="row">
        {(!s?.installed || !s.matches_this_exe || !s.running || s.outdated || !s.protected) && (
          <button className="btn" disabled={busy || !s} onClick={() => run(ipc.installService)}>
            {busy
              ? L.serviceBusy
              : !s?.installed
                ? L.serviceInstall
                : !s.matches_this_exe
                  ? L.serviceFix
                  : s.outdated
                    ? L.serviceUpdate
                    : !s.protected
                      ? L.serviceReinstall
                      : L.serviceStart}
          </button>
        )}
        {s?.installed && (
          <button
            className="btn"
            disabled={busy}
            onClick={() => (confirm ? run(ipc.removeService) : setConfirm(true))}
          >
            {confirm ? L.serviceConfirmRemove : L.serviceRemove}
          </button>
        )}
      </div>
    </div>
  );
}
