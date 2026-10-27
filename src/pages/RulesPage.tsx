import { useEffect, useMemo, useState } from "react";
import { useRules } from "../store/rules";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";
import Icon from "../components/Icon";

const L = T.rules;
const FIELDS = ["domain_suffix", "domain_keyword", "process_path", "process_name", "ip_cidr"] as const;
type ListField = (typeof FIELDS)[number];
type Texts = Record<ListField, string>;

const toTexts = (r: ipc.RuleSet): Texts =>
  Object.fromEntries(FIELDS.map((f) => [f, r[f].join("\n")])) as Texts;
const lines = (s: string) =>
  s
    .split(/\r?\n/)
    .map((x) => x.trim())
    .filter(Boolean);

export default function RulesPage() {
  const { rulesets, error, load, run } = useRules();
  const [draft, setDraft] = useState<ipc.RuleSet | null>(null);
  const [isPreset, setIsPreset] = useState(false);
  const [texts, setTexts] = useState<Texts | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [confirm, setConfirm] = useState<"reset" | "del" | null>(null);
  const [picking, setPicking] = useState(false);

  useEffect(() => {
    load();
  }, [load]);

  // 第一次进来自动选第一个。
  useEffect(() => {
    if (!draft && rulesets?.length) select(rulesets[0]);
  }, [rulesets]);

  const select = (v: ipc.RuleSetView) => {
    setDraft(structuredClone(v.ruleset));
    setIsPreset(v.preset);
    setTexts(toTexts(v.ruleset));
    setMsg(null);
    setConfirm(null);
  };

  const reselect = (id: string, ok: string) => {
    const v = useRules.getState().rulesets?.find((x) => x.ruleset.id === id);
    if (v) select(v);
    setMsg({ ok: true, text: ok });
  };

  const save = async () => {
    if (!draft || !texts) return;
    const r: ipc.RuleSet = { ...draft, ...(Object.fromEntries(FIELDS.map((f) => [f, lines(texts[f])])) as Record<ListField, string[]>) };
    let id = draft.id;
    const err = await run(async () => {
      const res = await ipc.saveRuleset(r);
      id = res.id;
      return res.rulesets;
    });
    if (err) return setMsg({ ok: false, text: err });
    reselect(id, L.saved);
  };

  const reset = async () => {
    if (!draft) return;
    if (confirm !== "reset") return setConfirm("reset");
    const err = await run(() => ipc.resetRulesetToPreset(draft.id));
    if (err) return setMsg({ ok: false, text: err });
    reselect(draft.id, L.saved);
  };

  const del = async () => {
    if (!draft) return;
    if (confirm !== "del") return setConfirm("del");
    const err = await run(() => ipc.deleteRuleset(draft.id));
    if (err) return setMsg({ ok: false, text: err });
    setDraft(null);
    setTexts(null);
  };

  const newSet = () => {
    const r: ipc.RuleSet = { id: "", name: L.newName, enabled: true, domain_suffix: [], domain_keyword: [], process_path: [], process_name: [], ip_cidr: [] };
    select({ ruleset: r, preset: false });
  };

  const addPath = (p: string) =>
    setTexts((t) => (t && !lines(t.process_path).some((x) => x.toLowerCase() === p.toLowerCase()) ? { ...t, process_path: [...lines(t.process_path), p].join("\n") } : t));

  return (
    <div className="split">
      <div className="card list-card">
        <h2>{L.list}</h2>
        <p className="muted small">{L.hint}</p>
        {error && <div className="banner bad">{error}</div>}
        <ul className="plist">
          {rulesets?.map((v) => (
            <li key={v.ruleset.id}>
              <button className={draft?.id === v.ruleset.id ? "pitem active" : "pitem"} onClick={() => select(v)}>
                <span>{v.ruleset.name}</span>
                {v.preset && <span className="tag">{L.preset}</span>}
                {!v.ruleset.enabled && <span className="tag warn">{L.off}</span>}
              </button>
            </li>
          ))}
        </ul>
        <div className="row">
          <button className="btn" onClick={newSet}>
            <Icon name="plus" />
            {L.new}
          </button>
        </div>
      </div>

      {draft && texts && (
        <div className="card form-card">
          <h2>
            {draft.id ? draft.name : L.new}
            {isPreset && <span className="tag">{L.preset}</span>}
          </h2>
          <label className="field">
            <span className="field-label">{L.name}</span>
            <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          </label>
          <label className="check">
            <input type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} />
            {L.enabled}
          </label>
          {FIELDS.map((f) => (
            <label className="field" key={f}>
              <span className="field-label">
                {L.fields[f][0]} <span className="muted small">（{L.onePerLine}）</span>
              </span>
              <textarea rows={f === "domain_suffix" || f === "process_path" ? 5 : 2} value={texts[f]} spellCheck={false} onChange={(e) => setTexts({ ...texts, [f]: e.target.value })} />
              <span className="field-hint">{L.fields[f][1]}</span>
              {f === "process_path" && (
                <span>
                  <button type="button" className="btn" onClick={() => setPicking(true)}>
                    {L.pick}
                  </button>
                </span>
              )}
            </label>
          ))}
          {msg && <div className={msg.ok ? "banner good" : "banner bad"}>{msg.text}</div>}
          <div className="row">
            <button className="btn primary" onClick={save}>
              <Icon name="check" />
              {L.save}
            </button>
            <div className="spacer" />
            {isPreset && (
              <button className="btn" onClick={reset}>
                <Icon name="refresh" />
                {confirm === "reset" ? L.confirmReset : L.reset}
              </button>
            )}
            {draft.id && !isPreset && (
              <button className="btn danger" onClick={del}>
                <Icon name="trash" />
                {confirm === "del" ? L.confirmDel : L.del}
              </button>
            )}
          </div>
        </div>
      )}

      {picking && texts && <ProcessPicker chosen={lines(texts.process_path)} onAdd={addPath} onClose={() => setPicking(false)} />}
    </div>
  );
}

function ProcessPicker({ chosen, onAdd, onClose }: { chosen: string[]; onAdd: (p: string) => void; onClose: () => void }) {
  const [list, setList] = useState<ipc.RunningProcess[] | null>(null);
  const [filter, setFilter] = useState("");
  const refresh = () => {
    setList(null);
    ipc.listRunningProcesses().then(setList, () => setList([]));
  };
  useEffect(refresh, []);
  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (list ?? []).filter((p) => !q || p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q));
  }, [list, filter]);
  const has = (p: string) => chosen.some((c) => c.toLowerCase() === p.toLowerCase());

  return (
    <div className="modal-back" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()}>
        <h2>{T.rules.pickTitle}</h2>
        <div className="row">
          <input className="grow" placeholder={T.rules.pickFilter} value={filter} onChange={(e) => setFilter(e.target.value)} autoFocus />
          <button className="btn" onClick={refresh}>
            <Icon name="refresh" />
            {T.rules.refresh}
          </button>
          <button className="btn" onClick={onClose}>
            {T.rules.close}
          </button>
        </div>
        <div className="proc-list">
          {list === null && <p className="muted">…</p>}
          {list !== null && shown.length === 0 && <p className="muted">{T.rules.pickNone}</p>}
          {shown.map((p) => (
            <div className="proc" key={p.path}>
              <div className="proc-text">
                <div>{p.name}</div>
                <div className="mono muted small">{p.path}</div>
              </div>
              <button className="btn" disabled={has(p.path)} onClick={() => onAdd(p.path)}>
                {has(p.path) ? T.rules.added : T.rules.add}
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
