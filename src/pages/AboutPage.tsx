import { useApp } from "../store/app";
import { T } from "../lib/text";

const L = T.about;

export default function AboutPage() {
  const info = useApp((s) => s.info);
  return (
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
            <td className="mono">{info ? info.singboxVersion ?? L.notFound : "…"}</td>
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
  );
}
