import { T } from "../lib/text";

export default function PlaceholderPage({ title }: { title: string }) {
  return (
    <div className="card">
      <h2>{title}</h2>
      <p className="muted">{T.comingSoon}</p>
    </div>
  );
}
