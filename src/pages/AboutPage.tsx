import { useApp } from "../store/app";
import { T } from "../lib/text";

export default function AboutPage() {
  const info = useApp((s) => s.info);
  return (
    <div className="card">
      <h2>{T.nav.about}</h2>
      <table className="kv">
        <tbody>
          <tr>
            <th>{T.about.version}</th>
            <td className="mono">{info?.version ?? "…"}</td>
          </tr>
          <tr>
            <th>sing-box</th>
            <td className="mono">{info ? info.singboxVersion ?? "（未找到）" : "…"}</td>
          </tr>
          <tr>
            <th>设置文件夹</th>
            <td className="mono">{info?.dataDir ?? "…"}</td>
          </tr>
        </tbody>
      </table>
      <p className="muted">{T.about.licenses}</p>
    </div>
  );
}
