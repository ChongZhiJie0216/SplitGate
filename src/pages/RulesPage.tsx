import { useEffect, useMemo, useState } from "react";
import { useRules } from "../store/rules";
import { useProfiles } from "../store/profiles";
import { useStatus } from "../store/status";
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
  const [subBusy, setSubBusy] = useState(false);
  const { view, run: runProfiles } = useProfiles();
  const markStale = useStatus((s) => s.markStale);
  const active = view?.profiles.find((p) => p.profile.id === view.active)?.profile;
  const used = (id: string) => !active || active.ruleset_ids.includes(id);

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
    if (used(id)) markStale();
    reselect(id, L.saved);
    // 新的订阅（或地址改了）：马上下载。
    const stored = useRules.getState().rulesets?.find((x) => x.ruleset.id === id)?.ruleset;
    if (stored?.subscription && !stored.subscription.updated_ms) await updateSub(id);
  };

  const updateSub = async (id: string) => {
    setSubBusy(true);
    const err = await run(() => ipc.updateSubscription(id));
    setSubBusy(false);
    // 更新失败时后端已记下原因（显示在订阅那一栏），列表照旧。
    if (err) await useRules.getState().load();
    if (!err && used(id)) markStale();
    reselect(id, err ? err : L.saved);
    if (err) setMsg({ ok: false, text: err });
  };

  /** 把这个规则集加进使用中的 Profile（其他设置不变）。 */
  const addToProfile = async () => {
    if (!draft?.id || !active) return;
    const err = await runProfiles(async () => (await ipc.saveProfile({ ...active, ruleset_ids: [...active.ruleset_ids, draft.id] })).view);
    if (err) return setMsg({ ok: false, text: err });
    markStale();
    setMsg({ ok: true, text: L.addedToProfile(active.name) });
  };

  const reset = async () => {
    if (!draft) return;
    if (confirm !== "reset") return setConfirm("reset");
    const err = await run(() => ipc.resetRulesetToPreset(draft.id));
    if (err) return setMsg({ ok: false, text: err });
    if (used(draft.id)) markStale();
    reselect(draft.id, L.saved);
  };

  const del = async () => {
    if (!draft) return;
    if (confirm !== "del") return setConfirm("del");
    const err = await run(() => ipc.deleteRuleset(draft.id));
    if (err) return setMsg({ ok: false, text: err });
    if (used(draft.id)) markStale();
    setDraft(null);
    setTexts(null);
  };

  const newSet = () => {
    const r: ipc.RuleSet = {
      id: "",
      name: L.newName,
      enabled: true,
      domain_suffix: [],
      domain_keyword: [],
      process_path: [],
      process_name: [],
      ip_cidr: [],
      subscription: null,
      sub_domain_suffix: [],
      sub_domain_keyword: [],
    };
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
                {v.ruleset.enabled && active && !used(v.ruleset.id) && (
                  <span className="tag" title={L.unusedTitle(active.name)}>
                    {L.unused}
                  </span>
                )}
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
          {draft.id && active && !used(draft.id) && (
            <div className="banner warn">
              {L.notInProfile(active.name)}{" "}
              <button className="btn small" onClick={addToProfile}>
                <Icon name="check" />
                {L.addToProfile(active.name)}
              </button>
            </div>
          )}
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
          <SubscriptionField
            draft={draft}
            stored={rulesets?.find((x) => x.ruleset.id === draft.id)?.ruleset ?? null}
            busy={subBusy}
            onChange={(sub) => setDraft({ ...draft, subscription: sub })}
            onUpdate={() => draft.id && updateSub(draft.id)}
          />
          {msg && <div className={msg.ok ? "banner good" : "banner bad"}>{msg.text}</div>}
          <div className="row">
            <button className="btn primary" onClick={save} disabled={subBusy}>
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

/** 订阅：地址、广告开关、上次更新的结果，和下载到的域名（只看，不在这里改）。 */
function SubscriptionField(props: {
  draft: ipc.RuleSet;
  stored: ipc.RuleSet | null;
  busy: boolean;
  onChange: (s: ipc.Subscription | null) => void;
  onUpdate: () => void;
}) {
  const S = L.sub;
  const sub = props.draft.subscription;
  const saved = props.stored?.subscription ?? null;
  const changed = !!sub && !!saved && (sub.url.trim() !== saved.url || sub.skip_ads !== saved.skip_ads);
  const list = [...(props.stored?.sub_domain_suffix ?? []), ...(props.stored?.sub_domain_keyword ?? [])];
  const SHOW = 300;
  return (
    <div className="field sub-field">
      <span className="field-label">{S.title}</span>
      <label className="check">
        <input
          type="checkbox"
          checked={!!sub}
          onChange={(e) =>
            props.onChange(e.target.checked ? { url: "", skip_ads: true, updated_ms: 0, count: 0, skipped: 0, error: null } : null)
          }
        />
        {S.enable}
      </label>
      {sub && (
        <>
          <label className="field">
            <span className="field-label">{S.url}</span>
            <input className="mono" value={sub.url} placeholder="https://…  /  spotify" onChange={(e) => props.onChange({ ...sub, url: e.target.value })} />
            <span className="field-hint">{S.urlHint}</span>
          </label>
          <label className="check">
            <input type="checkbox" checked={sub.skip_ads} onChange={(e) => props.onChange({ ...sub, skip_ads: e.target.checked })} />
            {S.skipAds}
          </label>
          {changed && <p className="muted small">{S.changedUrl}</p>}
          {!changed && saved && (
            <p className="small">
              {saved.updated_ms ? S.status(new Date(saved.updated_ms).toLocaleString("zh-CN", { hour12: false }), saved.count, saved.skipped) : S.never}
            </p>
          )}
          {!changed && saved?.error && <div className="banner warn small">{S.failed(saved.error)}</div>}
          {props.draft.id && saved && !changed && (
            <span>
              <button type="button" className="btn small" disabled={props.busy} onClick={props.onUpdate}>
                <Icon name="refresh" />
                {props.busy ? S.updating : S.update}
              </button>
            </span>
          )}
          {!changed && list.length > 0 && (
            <details className="sub-list">
              <summary>{S.list(list.length)}</summary>
              <div className="chips">
                {list.slice(0, SHOW).map((d) => (
                  <span key={d} className="chip mono">
                    {d}
                  </span>
                ))}
              </div>
              {list.length > SHOW && <p className="muted small">{S.more(list.length - SHOW)}</p>}
            </details>
          )}
        </>
      )}
    </div>
  );
}
