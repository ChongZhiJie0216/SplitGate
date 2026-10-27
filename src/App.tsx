import { lazy, Suspense, useEffect, useState } from "react";
import { Page, useApp } from "./store/app";
import { useProfiles } from "./store/profiles";
import { useStatus } from "./store/status";
import { useBlocked } from "./store/blocked";
import { T } from "./lib/text";
import { getSettings, restartAsAdmin, serviceUsable, setActiveProfile } from "./lib/ipc";
import ConnectButton from "./components/ConnectButton";
import Icon, { IconName } from "./components/Icon";
import Logo from "./components/Logo";
import AboutPage from "./pages/AboutPage";
import BlockedPage from "./pages/BlockedPage";
import ConnectPage from "./pages/ConnectPage";
import ConnectionsPage from "./pages/ConnectionsPage";
import LogsPage from "./pages/LogsPage";
import ProfilesPage from "./pages/ProfilesPage";
import RulesPage from "./pages/RulesPage";
import SettingsPage from "./pages/SettingsPage";

// 流量画布带着 React Flow（约 190 KB），打开那一页才加载。
const CanvasPage = lazy(() => import("./pages/CanvasPage"));

const PAGES: Page[] = ["connect", "rules", "connections", "canvas", "blocked", "logs", "profiles", "settings", "about"];
const NAV_ICONS: Record<Page, IconName> = {
  connect: "power",
  rules: "rules",
  connections: "connections",
  canvas: "canvas",
  blocked: "blocked",
  logs: "logs",
  profiles: "profiles",
  settings: "settings",
  about: "about",
};

export default function App() {
  const { page, setPage, info, infoError, loadInfo, update, checkUpdate, service, loadService } = useApp();
  const { view, load: loadProfiles, run } = useProfiles();
  const { status, start } = useStatus();
  const { unseen, start: startBlocked } = useBlocked();
  const busy = status.state !== "disconnected" && status.state !== "error";

  useEffect(() => {
    loadInfo();
    loadService();
    loadProfiles();
    start();
    startBlocked();
    // 启动时检查一次更新（设置里可以关）；只是提示，失败也不打扰。
    getSettings()
      .then((v) => {
        if (v.settings.check_updates) checkUpdate(true);
      })
      .catch(() => {});
  }, [loadInfo, loadService, loadProfiles, start, startBlocked, checkUpdate]);

  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">
          <Logo />
          {T.appName}
        </div>
        {PAGES.map((p) => (
          <button key={p} className={p === page ? "nav-item active" : "nav-item"} onClick={() => setPage(p)}>
            <Icon name={NAV_ICONS[p]} />
            <span className="nav-label">{T.nav[p]}</span>
            {p === "blocked" && unseen > 0 && page !== "blocked" && <span className="nav-badge">{unseen}</span>}
            {p === "about" && update?.newer && <span className="nav-badge" title={T.about.newVersion(update.latest ?? "")}>●</span>}
          </button>
        ))}
        <div className="nav-foot">{info ? `v${info.version}` : ""}</div>
      </nav>
      <main className="main">
        <header className="topbar">
          <ConnectButton />
          <label className="theme">
            {T.profile.pick}
            <select
              value={view?.active ?? ""}
              disabled={!view?.profiles.length || busy}
              onChange={(e) => run(() => setActiveProfile(e.target.value))}
            >
              {!view?.profiles.length && <option value="">{T.profile.none}</option>}
              {view?.profiles.map(({ profile }) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}（{T.profile.types[profile.type]}）
                </option>
              ))}
            </select>
          </label>
          <div className="spacer" />
        </header>
        {info && !info.isAdmin && service && !serviceUsable(service) && <NotAdminBanner />}
        {infoError && <div className="banner bad">{infoError}</div>}
        <section className="content">
          {page === "connect" ? (
            <ConnectPage />
          ) : page === "logs" ? (
            <LogsPage />
          ) : page === "about" ? (
            <AboutPage />
          ) : page === "profiles" ? (
            <ProfilesPage />
          ) : page === "rules" ? (
            <RulesPage />
          ) : page === "connections" ? (
            <ConnectionsPage />
          ) : page === "canvas" ? (
            <Suspense fallback={<div className="card muted">…</div>}>
              <CanvasPage />
            </Suspense>
          ) : page === "blocked" ? (
            <BlockedPage />
          ) : (
            <SettingsPage />
          )}
        </section>
      </main>
    </div>
  );
}

/** 不是管理员、也没有可用的后台服务：说明为什么连不了，给两条路。 */
function NotAdminBanner() {
  const setPage = useApp((s) => s.setPage);
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="banner warn">
      {T.notAdmin}{" "}
      <button className="btn" onClick={() => setPage("settings")}>
        {T.notAdminInstall}
      </button>{" "}
      <button className="btn" onClick={() => restartAsAdmin().catch((e) => setError(String(e)))}>
        {T.notAdminRestart}
      </button>
      {error && <div>{error}</div>}
    </div>
  );
}
