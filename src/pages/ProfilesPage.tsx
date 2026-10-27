import { useEffect, useState } from "react";
import { useProfiles } from "../store/profiles";
import { useRules } from "../store/rules";
import { T } from "../lib/text";
import * as ipc from "../lib/ipc";

const L = T.profile;
const DEFAULT_RULESETS = ["xbox", "minecraft", "steam"];

function blank(type: ipc.ExitType): ipc.Profile {
  const base = { id: "", type, name: "", ruleset_ids: [...DEFAULT_RULESETS], home_cidrs: [] as string[] };
  if (type === "cisco")
    return { ...base, cisco: { server: "", user: "", auth_group: null, no_udp: true, bind_interface: null, mfa: "none" } };
  if (type === "pangolin")
    return { ...base, pangolin: { interface: "Pangolin", vpn_dns: ["9.9.9.9"], alias_suffixes: [], alias_dns: "100.96.128.1" } };
  return {
    ...base,
    wireguard: { address: [], peer: { endpoint: "", public_key: "", persistent_keepalive: null }, dns: [], mtu: 1420 },
  };
}

const splitList = (s: string) =>
  s
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean);

export default function ProfilesPage() {
  const { view, error, load, run } = useProfiles();
  const { rulesets, load: loadRules } = useRules();
  const [draft, setDraft] = useState<ipc.Profile | null>(null);
  const [hasSecret, setHasSecret] = useState(false);
  const [secrets, setSecrets] = useState<ipc.SecretsInput>({});
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [confirmDel, setConfirmDel] = useState(false);
  // Cisco：从服务器读到的组别（null = 还没读），以及最近一次测试登录。
  const [groups, setGroups] = useState<{ server: string; list: string[] | null; busy: boolean; error: string | null }>({
    server: "",
    list: null,
    busy: false,
    error: null,
  });
  const [test, setTest] = useState<{ busy: boolean; key: string; result: ipc.CiscoLoginResult | null }>({
    busy: false,
    key: "",
    result: null,
  });
  /** 测试没通过时，再按一次“仍然保存”才保存。 */
  const [saveAnyway, setSaveAnyway] = useState(false);

  useEffect(() => {
    load();
    loadRules();
  }, [load, loadRules]);

  const select = (p: ipc.Profile, saved: boolean) => {
    setDraft(structuredClone(p));
    setHasSecret(saved);
    setSecrets({});
    setMsg(null);
    setConfirmDel(false);
    setTest({ busy: false, key: "", result: null });
    setSaveAnyway(false);
    setGroups({ server: "", list: null, busy: false, error: null });
    if (p.type === "cisco" && p.cisco?.server.trim()) fetchGroups(p.cisco.server);
  };

  const fetchGroups = async (server: string) => {
    const s = server.trim();
    if (!s) return;
    setGroups({ server: s, list: null, busy: true, error: null });
    try {
      const list = await ipc.ciscoGroups(s);
      setGroups((g) => (g.server === s ? { ...g, list, busy: false } : g));
    } catch (e) {
      setGroups((g) => (g.server === s ? { ...g, busy: false, error: String(e) } : g));
    }
  };

  /** 账号、密码、组别、服务器任何一项改了，之前的测试结果就不算数。 */
  const testKey = (p: ipc.Profile) =>
    JSON.stringify([p.cisco?.server.trim(), p.cisco?.user.trim(), p.cisco?.auth_group ?? "", secrets.secret ?? ""]);

  const runTest = async (): Promise<ipc.CiscoLoginResult | null> => {
    if (!draft?.cisco) return null;
    const key = testKey(draft);
    setTest({ busy: true, key, result: null });
    let result: ipc.CiscoLoginResult;
    try {
      result = await ipc.testCiscoLogin(draft, secrets.secret);
    } catch (e) {
      result = { result: "failed", message: String(e) };
    }
    setTest({ busy: false, key, result });
    return result;
  };

  const save = async () => {
    if (!draft) return;
    // Cisco：先测试登录，通过了才保存；没通过时要再按一次“仍然保存”。
    // “仍然保存”只对按下那一刻的内容有效：之后又改了就重新测试。
    if (draft.type === "cisco" && !(saveAnyway && test.key === testKey(draft))) {
      const tested = test.key === testKey(draft) && test.result?.result === "ok";
      if (!tested) {
        const r = await runTest();
        if (r?.result !== "ok") {
          setSaveAnyway(true);
          return;
        }
      }
    }
    setSaveAnyway(false);
    let id = draft.id;
    const err = await run(async () => {
      const r = await ipc.saveProfile(draft, secrets);
      id = r.id; // 新建的 Profile 由后端给 id
      return r.view;
    });
    if (err) return setMsg({ ok: false, text: err });
    const saved = useProfiles.getState().view?.profiles.find((x) => x.profile.id === id);
    if (saved) select(saved.profile, saved.has_secret);
    setMsg({ ok: true, text: L.saved });
  };

  const del = async () => {
    if (!draft?.id) return;
    if (!confirmDel) return setConfirmDel(true);
    const err = await run(() => ipc.deleteProfile(draft.id));
    if (err) return setMsg({ ok: false, text: err });
    setDraft(null);
  };

  const patch = <K extends "cisco" | "pangolin" | "wireguard">(key: K, value: Partial<NonNullable<ipc.Profile[K]>>) =>
    setDraft((d) => (d ? { ...d, [key]: { ...d[key]!, ...value } } : d));

  return (
    <div className="split">
      <div className="card list-card">
        <h2>{L.list}</h2>
        {error && <div className="banner bad">{error}</div>}
        {view && view.profiles.length === 0 && <p className="muted">{L.empty}</p>}
        <ul className="plist">
          {view?.profiles.map(({ profile, has_secret }) => (
            <li key={profile.id}>
              <button
                className={draft?.id === profile.id ? "pitem active" : "pitem"}
                onClick={() => select(profile, has_secret)}
              >
                <span>{profile.name}</span>
                <span className="tag">{L.types[profile.type]}</span>
                {view.active === profile.id && <span className="tag good">{L.active}</span>}
              </button>
            </li>
          ))}
        </ul>
        <div className="row">
          <span className="muted">{L.new}：</span>
          {(["cisco", "wireguard", "pangolin"] as ipc.ExitType[]).map((t) => (
            <button key={t} className="btn" onClick={() => select(blank(t), false)}>
              {L.types[t]}
            </button>
          ))}
        </div>
      </div>

      {draft && (
        <div className="card form-card">
          <h2>
            {draft.id ? draft.name || draft.id : `${L.new} ${L.types[draft.type]}`}
            <span className="tag">{L.types[draft.type]}</span>
          </h2>
          <Field label={L.name}>
            <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          </Field>
          <div className="field">
            <span className="field-label">{L.rulesets}</span>
            <div className="checks">
              {rulesets?.map(({ ruleset: r }) => (
                <label className="check" key={r.id}>
                  <input
                    type="checkbox"
                    checked={draft.ruleset_ids.includes(r.id)}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        ruleset_ids: e.target.checked ? [...draft.ruleset_ids, r.id] : draft.ruleset_ids.filter((x) => x !== r.id),
                      })
                    }
                  />
                  {r.name}
                  {!r.enabled && <span className="tag warn">{T.rules.off}</span>}
                </label>
              ))}
            </div>
            <span className="field-hint">{L.rulesetsHint}</span>
          </div>
          <ListField
            label={L.homeCidrs}
            hint={L.homeCidrsHint}
            value={draft.home_cidrs}
            onChange={(v) => setDraft({ ...draft, home_cidrs: v })}
          />

          {draft.type === "cisco" && draft.cisco && (
            <>
              <Field label={L.cisco.server} hint={L.cisco.serverHint}>
                <input
                  value={draft.cisco.server}
                  onChange={(e) => patch("cisco", { server: e.target.value })}
                  onBlur={(e) => e.target.value.trim() !== groups.server && fetchGroups(e.target.value)}
                />
              </Field>
              <Field label={L.cisco.user}>
                <input value={draft.cisco.user} autoComplete="off" onChange={(e) => patch("cisco", { user: e.target.value })} />
              </Field>
              <Field label={L.cisco.password} hint={hasSecret ? L.secretSaved : L.secretMissing}>
                <input
                  type="password"
                  autoComplete="new-password"
                  value={secrets.secret ?? ""}
                  onChange={(e) => setSecrets({ ...secrets, secret: e.target.value })}
                />
              </Field>
              <GroupField
                value={draft.cisco.auth_group}
                state={groups}
                onChange={(v) => patch("cisco", { auth_group: v })}
                onFetch={() => fetchGroups(draft.cisco!.server)}
              />
              <label className="check">
                <input type="checkbox" checked={draft.cisco.no_udp} onChange={(e) => patch("cisco", { no_udp: e.target.checked })} />
                {L.cisco.noUdp}
              </label>
              <Field label={L.cisco.bind} hint={L.cisco.bindHint}>
                <input
                  value={draft.cisco.bind_interface ?? ""}
                  onChange={(e) => patch("cisco", { bind_interface: e.target.value || null })}
                />
              </Field>
              <div className="field">
                <div className="row">
                  <button className="btn" disabled={test.busy} onClick={runTest}>
                    {test.busy ? L.cisco.testing : L.cisco.test}
                  </button>
                </div>
                <span className="field-hint">{L.cisco.testHint}</span>
              </div>
              {test.result && test.key === testKey(draft) && <TestBanner r={test.result} />}
            </>
          )}

          {draft.type === "pangolin" && draft.pangolin && (
            <>
              <p className="banner warn">{L.pangolin.note}</p>
              <Field label={L.pangolin.iface}>
                <input value={draft.pangolin.interface} onChange={(e) => patch("pangolin", { interface: e.target.value })} />
              </Field>
              <ListField label={L.pangolin.vpnDns} hint={L.pangolin.vpnDnsHint} value={draft.pangolin.vpn_dns} onChange={(v) => patch("pangolin", { vpn_dns: v })} />
              <ListField label={L.pangolin.aliasSuffixes} value={draft.pangolin.alias_suffixes} onChange={(v) => patch("pangolin", { alias_suffixes: v })} />
              <Field label={L.pangolin.aliasDns}>
                <input value={draft.pangolin.alias_dns} onChange={(e) => patch("pangolin", { alias_dns: e.target.value })} />
              </Field>
            </>
          )}

          {draft.type === "wireguard" && draft.wireguard && (
            <WireguardForm
              wg={draft.wireguard}
              hasSecret={hasSecret}
              onImport={(c) => {
                patch("wireguard", c.wireguard);
                setSecrets({ secret: c.private_key, preshared_key: c.preshared_key ?? undefined });
                setMsg({ ok: true, text: L.wireguard.imported });
              }}
              onError={(t) => setMsg({ ok: false, text: t })}
            />
          )}

          {msg && <div className={msg.ok ? "banner good" : "banner bad"}>{msg.text}</div>}
          <div className="row">
            <button className="btn primary" onClick={save} disabled={test.busy}>
              {draft.type === "cisco" && saveAnyway && test.key === testKey(draft) ? L.cisco.saveAnyway : L.save}
            </button>
            {draft.id && view?.active !== draft.id && (
              <button className="btn" onClick={() => run(() => ipc.setActiveProfile(draft.id)).then((e) => e && setMsg({ ok: false, text: e }))}>
                {L.setActive}
              </button>
            )}
            <div className="spacer" />
            {draft.id && (
              <button className="btn danger" onClick={del}>
                {confirmDel ? L.confirmDel : L.del}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      {children}
      {hint && <span className="field-hint">{hint}</span>}
    </label>
  );
}

/** Cisco 组别：读到服务器的组别后改成下拉选择，免得打错字连不上；读不到时可以手动填。 */
function GroupField(props: {
  value: string | null;
  state: { list: string[] | null; busy: boolean; error: string | null };
  onChange: (v: string | null) => void;
  onFetch: () => void;
}) {
  const C = L.cisco;
  const { list, busy, error } = props.state;
  const value = props.value ?? "";
  if (list && list.length > 0) {
    const missing = value && !list.includes(value);
    return (
      <Field label={C.group} hint={C.groupPickHint}>
        <select value={value} onChange={(e) => props.onChange(e.target.value || null)}>
          <option value="">{C.groupChoose}</option>
          {list.map((g) => (
            <option key={g} value={g}>
              {g}
            </option>
          ))}
          {missing && (
            <option value={value}>
              {value} {C.groupNotOnServer}
            </option>
          )}
        </select>
      </Field>
    );
  }
  const hint = busy ? C.groupLoading : error ? C.groupFailed(error) : list ? C.groupNone : C.groupHint;
  return (
    <Field label={C.group} hint={hint}>
      <div className="row">
        <input className="grow" value={value} onChange={(e) => props.onChange(e.target.value || null)} />
        <button className="btn" disabled={busy} onClick={props.onFetch} type="button">
          {C.groupFetch}
        </button>
      </div>
    </Field>
  );
}

function TestBanner({ r }: { r: ipc.CiscoLoginResult }) {
  const C = L.cisco;
  if (r.result === "ok") return <div className="banner good">{C.testOk}</div>;
  if (r.result === "rejected") return <div className="banner bad">{C.testRejected(r.message)}</div>;
  if (r.result === "needs_more") return <div className="banner warn">{C.testNeedsMore(r.message)}</div>;
  return <div className="banner bad">{C.testFailed(r.message)}</div>;
}

/** 逗号分隔的列表：输入时保留原文，离开输入框时才整理。 */
function ListField({ label, hint, value, onChange }: { label: string; hint?: string; value: string[]; onChange: (v: string[]) => void }) {
  const [text, setText] = useState(value.join(", "));
  useEffect(() => setText(value.join(", ")), [value]);
  return (
    <Field label={label} hint={hint}>
      <input value={text} onChange={(e) => setText(e.target.value)} onBlur={() => onChange(splitList(text))} />
    </Field>
  );
}

function WireguardForm(props: {
  wg: ipc.WireguardProfile;
  hasSecret: boolean;
  onImport: (c: ipc.ImportedConf) => void;
  onError: (t: string) => void;
}) {
  const { wg } = props;
  const [paste, setPaste] = useState("");
  const parse = async (text: string) => {
    try {
      props.onImport(await ipc.importWireguardConf(text));
      setPaste("");
    } catch (e) {
      props.onError(String(e));
    }
  };
  return (
    <>
      <Field label={L.wireguard.importFile} hint={props.hasSecret ? L.secretSaved : L.secretMissing}>
        <input
          type="file"
          accept=".conf,text/plain"
          onChange={async (e) => {
            const f = e.target.files?.[0];
            if (f) await parse(await f.text());
            e.target.value = "";
          }}
        />
      </Field>
      <Field label={L.wireguard.pasteLabel}>
        <textarea rows={4} value={paste} onChange={(e) => setPaste(e.target.value)} spellCheck={false} />
      </Field>
      <div className="row">
        <button className="btn" disabled={!paste.trim()} onClick={() => parse(paste)}>
          {L.wireguard.parse}
        </button>
      </div>
      <table className="kv">
        <tbody>
          <tr>
            <th>{L.wireguard.address}</th>
            <td className="mono">{wg.address.join(", ") || "—"}</td>
          </tr>
          <tr>
            <th>{L.wireguard.endpoint}</th>
            <td className="mono">{wg.peer.endpoint || "—"}</td>
          </tr>
          <tr>
            <th>{L.wireguard.publicKey}</th>
            <td className="mono">{wg.peer.public_key || "—"}</td>
          </tr>
          <tr>
            <th>{L.wireguard.dns}</th>
            <td className="mono">{wg.dns.join(", ") || "—"}</td>
          </tr>
          <tr>
            <th>{L.wireguard.mtu}</th>
            <td className="mono">{wg.mtu}</td>
          </tr>
        </tbody>
      </table>
      <p className="muted">{L.wireguard.allowedIps}</p>
    </>
  );
}
