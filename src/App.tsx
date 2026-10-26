import { useEffect, useState } from "react";
import { Page, useApp } from "./store/app";
import { T } from "./lib/text";
import { Theme, applyTheme, loadTheme } from "./lib/theme";
import AboutPage from "./pages/AboutPage";
import PlaceholderPage from "./pages/PlaceholderPage";

const PAGES: Page[] = ["connect", "rules", "logs", "profiles", "settings", "about"];

export default function App() {
  const { page, setPage, info, infoError, loadInfo } = useApp();
  const [theme, setTheme] = useState<Theme>(loadTheme());

  useEffect(() => {
    loadInfo();
  }, [loadInfo]);

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
          <button className="connect-btn" disabled>
            {T.status.disconnected}
          </button>
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
          {page === "about" ? <AboutPage /> : <PlaceholderPage title={T.nav[page]} />}
        </section>
      </main>
    </div>
  );
}
