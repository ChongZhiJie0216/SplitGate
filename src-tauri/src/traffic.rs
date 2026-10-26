//! Connection list and traffic rates (SPEC "连接列表" / `traffic` event; phase 2 item, done early
//! by the maintainer's decision 2026-10-26). While connected, a thread reads sing-box's Clash API
//! once a second, keeps the latest snapshot for the Connections page, emits `traffic` and hands
//! the connections to block detection.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::block_detect;
use crate::singbox::{ClashApi, VPN_TAG};

const POLL: Duration = Duration::from_secs(1);

/// Bytes per second.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Traffic {
    pub vpn_up: u64,
    pub vpn_down: u64,
    pub direct_up: u64,
    pub direct_down: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Connection {
    pub id: String,
    /// tcp / udp
    pub network: String,
    /// Domain from DNS or sniffing; empty when only the IP is known.
    pub host: String,
    pub ip: String,
    pub port: String,
    /// Empty when Windows does not tell (system services).
    pub process_path: String,
    /// Outbound tag: "vpn" or "direct".
    pub outbound: String,
    pub rule: String,
    pub upload: u64,
    pub download: u64,
    /// RFC 3339 start time from sing-box.
    pub start: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Snapshot {
    pub connections: Vec<Connection>,
    pub traffic: Traffic,
    pub upload_total: u64,
    pub download_total: u64,
}

struct Live {
    api: ClashApi,
    snapshot: Snapshot,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

/// Starts polling; it stops by itself once `still_current()` turns false (disconnect, error).
pub fn start(app: AppHandle, api: ClashApi, still_current: impl Fn() -> bool + Send + 'static) {
    if let Ok(mut l) = LIVE.lock() {
        *l = Some(Live { api: api.clone(), snapshot: Snapshot::default() });
    }
    block_detect::start_session();
    thread::spawn(move || {
        let mut rates = Rates::default();
        while still_current() {
            let t = Instant::now();
            if let Ok((200, body)) = api.get("/connections", Duration::from_secs(3)) {
                if let Ok(v) = serde_json::from_str::<Value>(body.trim()) {
                    let mut snap = parse(&v);
                    snap.traffic = rates.update(&snap, Instant::now());
                    let _ = app.emit("traffic", snap.traffic);
                    block_detect::observe(&app, &snap.connections);
                    if let Ok(mut l) = LIVE.lock() {
                        if let Some(live) = l.as_mut() {
                            live.snapshot = snap;
                        }
                    }
                }
            }
            thread::sleep(POLL.saturating_sub(t.elapsed()));
        }
        stop();
        block_detect::end_session();
        let _ = app.emit("traffic", Traffic::default());
    });
}

fn stop() {
    if let Ok(mut l) = LIVE.lock() {
        *l = None;
    }
}

/// Latest snapshot; empty when not connected.
pub fn snapshot() -> Snapshot {
    LIVE.lock().ok().and_then(|l| l.as_ref().map(|x| x.snapshot.clone())).unwrap_or_default()
}

pub fn close(id: &str) -> Result<(), String> {
    // Ids are UUIDs; anything else must not reach the URL.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err("无效的连接编号".into());
    }
    let api = LIVE.lock().ok().and_then(|l| l.as_ref().map(|x| x.api.clone())).ok_or("现在没有连接")?;
    match api.request("DELETE", &format!("/connections/{id}"), Duration::from_secs(3))? {
        (s, _) if (200..300).contains(&s) => Ok(()),
        (s, _) => Err(format!("关闭连接失败（HTTP {s}）")),
    }
}

fn parse(v: &Value) -> Snapshot {
    let s = |x: &Value| x.as_str().unwrap_or("").to_string();
    let mut connections: Vec<Connection> = v["connections"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| {
                    let m = &c["metadata"];
                    // The chain lists the outbound last-to-first; the first entry is the one used.
                    let outbound = c["chains"].as_array().and_then(|ch| ch.first()).map(s).unwrap_or_default();
                    Connection {
                        id: s(&c["id"]),
                        network: s(&m["network"]),
                        host: s(&m["host"]),
                        ip: s(&m["destinationIP"]),
                        port: match &m["destinationPort"] {
                            Value::Number(n) => n.to_string(),
                            p => s(p),
                        },
                        process_path: s(&m["processPath"]),
                        outbound,
                        rule: s(&c["rule"]),
                        upload: c["upload"].as_u64().unwrap_or(0),
                        download: c["download"].as_u64().unwrap_or(0),
                        start: s(&c["start"]),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    connections.sort_by(|a, b| b.start.cmp(&a.start));
    Snapshot {
        connections,
        traffic: Traffic::default(),
        upload_total: v["uploadTotal"].as_u64().unwrap_or(0),
        download_total: v["downloadTotal"].as_u64().unwrap_or(0),
    }
}

/// Rates from two snapshots: the totals give the whole rate; per-connection deltas give the VPN part;
/// direct is the rest (bytes of connections that closed in between count as direct-or-vpn by totals only).
#[derive(Default)]
struct Rates {
    last: Option<(Instant, u64, u64)>,
    seen: HashMap<String, (u64, u64)>,
}

impl Rates {
    fn update(&mut self, snap: &Snapshot, now: Instant) -> Traffic {
        let (mut vpn_up, mut vpn_down) = (0u64, 0u64);
        let mut seen = HashMap::with_capacity(snap.connections.len());
        for c in &snap.connections {
            let (pu, pd) = self.seen.get(&c.id).copied().unwrap_or((0, 0));
            if c.outbound == VPN_TAG {
                vpn_up += c.upload.saturating_sub(pu);
                vpn_down += c.download.saturating_sub(pd);
            }
            seen.insert(c.id.clone(), (c.upload, c.download));
        }
        self.seen = seen;
        let out = match self.last {
            Some((t, up, down)) => {
                let secs = now.duration_since(t).as_secs_f64().max(0.001);
                let up_all = snap.upload_total.saturating_sub(up);
                let down_all = snap.download_total.saturating_sub(down);
                let per = |b: u64| (b as f64 / secs) as u64;
                Traffic {
                    vpn_up: per(vpn_up),
                    vpn_down: per(vpn_down),
                    direct_up: per(up_all.saturating_sub(vpn_up)),
                    direct_down: per(down_all.saturating_sub(vpn_down)),
                }
            }
            None => Traffic::default(),
        };
        self.last = Some((now, snap.upload_total, snap.download_total));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape captured from sing-box 1.14.4 (2026-10-26), names replaced.
    const SAMPLE: &str = r#"{"connections":[
        {"chains":["vpn"],"download":586546,"id":"c4e8b16d-033a-45fc-86e0-268e6a745b59","metadata":{"destinationIP":"","destinationPort":"443","dnsMode":"normal","host":"speed.cloudflare.com","network":"tcp","processPath":"C:\\Tools\\curl.exe","sourceIP":"127.0.0.1","sourcePort":"59555","type":"mixed/in"},"rule":"domain_suffix=cloudflare.com => route(vpn)","rulePayload":"","start":"2026-10-26T20:23:26.3884537+08:00","upload":673},
        {"chains":["direct"],"download":151200,"id":"bbe09ecf-7114-4c6e-bbee-2ac56e0b5964","metadata":{"destinationIP":"1.2.3.4","destinationPort":80,"host":"","network":"udp","processPath":"","type":"tun/tun-in"},"rule":"final","start":"2026-10-26T20:23:27+08:00","upload":103}
    ],"downloadTotal":737746,"memory":5283840,"uploadTotal":776}"#;

    #[test]
    fn parses_singbox_connections() {
        let snap = parse(&serde_json::from_str(SAMPLE).unwrap());
        assert_eq!(snap.connections.len(), 2);
        let first = &snap.connections[0];
        assert_eq!(first.id, "bbe09ecf-7114-4c6e-bbee-2ac56e0b5964", "newest first");
        assert_eq!(first.port, "80", "numeric port accepted too");
        assert_eq!(first.outbound, "direct");
        let vpn = &snap.connections[1];
        assert_eq!(vpn.host, "speed.cloudflare.com");
        assert_eq!(vpn.process_path, r"C:\Tools\curl.exe");
        assert_eq!(vpn.outbound, "vpn");
        assert_eq!((snap.upload_total, snap.download_total), (776, 737746));
    }

    #[test]
    fn rates_split_vpn_and_direct() {
        let conn = |id: &str, out: &str, up, down| Connection {
            id: id.into(),
            network: "tcp".into(),
            host: String::new(),
            ip: String::new(),
            port: "443".into(),
            process_path: String::new(),
            outbound: out.into(),
            rule: String::new(),
            upload: up,
            download: down,
            start: String::new(),
        };
        let t0 = Instant::now();
        let mut r = Rates::default();
        let s1 = Snapshot { connections: vec![conn("a", "vpn", 100, 1000)], upload_total: 150, download_total: 1500, ..Default::default() };
        assert_eq!(r.update(&s1, t0), Traffic::default(), "first sample has no rate");
        let s2 = Snapshot {
            connections: vec![conn("a", "vpn", 300, 5000), conn("b", "direct", 50, 500)],
            upload_total: 150 + 200 + 50,
            download_total: 1500 + 4000 + 500,
            ..Default::default()
        };
        let t = r.update(&s2, t0 + Duration::from_secs(2));
        assert_eq!(t, Traffic { vpn_up: 100, vpn_down: 2000, direct_up: 25, direct_down: 250 });
    }

    #[test]
    fn close_rejects_odd_ids() {
        assert!(close("../configs").is_err());
        assert!(close("").is_err());
    }
}
