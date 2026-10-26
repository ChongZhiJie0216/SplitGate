import { useEffect, useState } from "react";
import { Page, useApp } from "./store/app";
import { useProfiles } from "./store/profiles";
import { useStatus } from "./store/status";
import { T } from "./lib/text";
import { Theme, applyTheme, loadTheme } from "./lib/theme";
import { setActiveProfile } from "./lib/ipc";
import ConnectButton from "./components/ConnectButton";
import AboutPage from "./pages/AboutPage";
import ConnectPage from "./pages/ConnectPage";
import LogsPage from "./pages/LogsPage";
import PlaceholderPage from "./pages/PlaceholderPage";
import ProfilesPage from "./pages/ProfilesPage";
import RulesPage from "./pages/RulesPage";

const PAGES: Page[] = ["connect", "rules", "logs", "profiles", "settings", "about"];

export default function App() {
  const { page, setPage, info, infoError, loadInfo } = useApp();
  const { view, load: loadProfiles, run } = useProfiles();
  const { status, start } = useStatus();
  const [theme, setTheme] = useState<Theme>(loadTheme());
  const busy = status.state !== "disconnected" && status.state !== "error";

  useEffect(() => {
    loadInfo();
    loadProfiles();
    start();
  }, [loadInfo, loadProfiles, start]);

  const changeTheme = (t: Theme) => {
    setTheme(t);
    applyTheme(t);
  };

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
          <label className="theme">
            {T.theme.label}
            <select value={theme} onChange={(e) => changeTheme(e.target.value as Theme)}>
              <option value="auto">{T.theme.auto}</option>
              <option value="light">{T.theme.light}</option>
              <option value="dark">{T.theme.dark}</option>
            </select>
          </label>
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
            <PlaceholderPage title={T.nav[page]} />
          )}
        </section>
      </main>
    </div>
  );
}
