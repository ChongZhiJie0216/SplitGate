import { useEffect } from "react";
import { Page, useApp } from "./store/app";
import { useProfiles } from "./store/profiles";
import { useStatus } from "./store/status";
import { T } from "./lib/text";
import { setActiveProfile } from "./lib/ipc";
import ConnectButton from "./components/ConnectButton";
import AboutPage from "./pages/AboutPage";
import ConnectPage from "./pages/ConnectPage";
import LogsPage from "./pages/LogsPage";
import ProfilesPage from "./pages/ProfilesPage";
import RulesPage from "./pages/RulesPage";
import SettingsPage from "./pages/SettingsPage";

const PAGES: Page[] = ["connect", "rules", "logs", "profiles", "settings", "about"];

export default function App() {
  const { page, setPage, info, infoError, loadInfo } = useApp();
  const { view, load: loadProfiles, run } = useProfiles();
  const { status, start } = useStatus();
  const busy = status.state !== "disconnected" && status.state !== "error";

  useEffect(() => {
    loadInfo();
    loadProfiles();
    start();
  }, [loadInfo, loadProfiles, start]);

  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">{T.appName}</div>
        {PAGES.map((p) => (
          <button key={p} className={p === page ? "nav-item active" : "nav-item"} onClick={() => setPage(p)}>
            {T.nav[p]}
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
        {info && !info.isAdmin && <div className="banner warn">{T.notAdmin}</div>}
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
          ) : (
            <SettingsPage />
          )}
        </section>
      </main>
    </div>
  );
}
