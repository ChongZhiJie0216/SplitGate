//! WireGuard exit. For now: parse a standard .conf into the profile fields + secrets.
//! AllowedIPs is ignored on purpose: SplitGate's rules decide what goes through (SPEC 组件 7).

use serde::Serialize;

use std::net::IpAddr;

use serde_json::json;

use super::{ExitAdapter, ExitParts};
use crate::config::{WireguardPeer, WireguardProfile};
use crate::singbox::{self, Secrets, VPN_TAG};

/// Used when a WireGuard .conf has no DNS line (phase 0 V2 did the same).
const FALLBACK_DNS: &str = "1.1.1.1";

/// sing-box's WireGuard endpoint (phase 0 V2); DNS goes to the .conf's DNS through the tunnel.
impl ExitAdapter for WireguardProfile {
    fn parts(&self, secrets: &Secrets) -> Result<ExitParts, String> {
        let (host, port) = split_host_port(&self.peer.endpoint)?;
        let mut peer = json!({
            "address": host,
            "port": port,
            "public_key": self.peer.public_key,
            // Rules decide what enters the tunnel, so it must accept every destination (SPEC 组件 7).
            "allowed_ips": ["0.0.0.0/0", "::/0"],
        });
        if let Some(psk) = &secrets.preshared_key {
            peer["pre_shared_key"] = json!(psk);
        }
        if let Some(k) = self.peer.persistent_keepalive {
            peer["persistent_keepalive_interval"] = json!(k);
        }
        let ep = json!({
            "type": "wireguard",
            "tag": VPN_TAG,
            "address": self.address,
            "private_key": secrets.main,
            "mtu": self.mtu,
            "peers": [peer],
        });
        let server = self
            .dns
            .iter()
            .find(|d| d.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv4()))
            .cloned()
            .unwrap_or_else(|| FALLBACK_DNS.to_string());
        let dns = json!({ "type": "udp", "tag": "vpn-dns", "server": server, "detour": VPN_TAG });
        Ok(ExitParts { endpoint: ep, dns, server_host: host })
    }
}

fn split_host_port(endpoint: &str) -> Result<(String, u16), String> {
    let port = endpoint.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok());
    match port {
        Some(p) => Ok((singbox::host_of(endpoint), p)),
        None => Err(format!("WireGuard Endpoint“{endpoint}”要写成 主机:端口")),
    }
}

#[derive(Serialize, Debug)]
pub struct ImportedConf {
    pub wireguard: WireguardProfile,
    pub private_key: String,
    pub preshared_key: Option<String>,
}

pub fn parse_conf(text: &str) -> Result<ImportedConf, String> {
    let mut section = String::new();
    let mut peers = 0;
    let (mut private_key, mut address, mut dns, mut mtu) = (None, Vec::new(), Vec::new(), None);
    let (mut public_key, mut endpoint, mut psk, mut keepalive) = (None, None, None, None);

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').trim().to_ascii_lowercase();
            if section == "peer" {
                peers += 1;
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
        match (section.as_str(), k.as_str()) {
            ("interface", "privatekey") => private_key = Some(v),
            ("interface", "address") => address.extend(list(&v)),
            // DNS may also list search domains; keep only IP addresses.
            ("interface", "dns") => dns.extend(list(&v).into_iter().filter(|d| d.parse::<std::net::IpAddr>().is_ok())),
            ("interface", "mtu") => mtu = Some(v.parse::<u16>().map_err(|_| "MTU 不是数字".to_string())?),
            ("peer", "publickey") => public_key = Some(v),
            ("peer", "endpoint") => endpoint = Some(v),
            ("peer", "presharedkey") => psk = Some(v),
            ("peer", "persistentkeepalive") => keepalive = v.parse::<u16>().ok(),
            _ => {}
        }
    }

    if peers != 1 {
        return Err(format!("这个 .conf 有 {peers} 个 [Peer]，SplitGate 只支持 1 个"));
    }
    let private_key = private_key.ok_or("缺少 [Interface] PrivateKey")?;
    if address.is_empty() {
        return Err("缺少 [Interface] Address".into());
    }
    Ok(ImportedConf {
        wireguard: WireguardProfile {
            address,
            peer: WireguardPeer {
                endpoint: endpoint.ok_or("缺少 [Peer] Endpoint")?,
                public_key: public_key.ok_or("缺少 [Peer] PublicKey")?,
                persistent_keepalive: keepalive,
            },
            dns,
            mtu: mtu.unwrap_or(1420),
        },
        private_key,
        preshared_key: psk,
    })
}

fn list(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_needs_a_port() {
        assert_eq!(split_host_port("home.example.com:51820").unwrap(), ("home.example.com".into(), 51820));
        assert!(split_host_port("home.example.com").is_err());
    }

    // Synthetic keys (not real): 43 base64 chars + '='.
    const PRIV: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const PUB: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=";

    #[test]
    fn parses_standard_conf() {
        let text = format!(
            "[Interface]\nPrivateKey = {PRIV}\nAddress = 10.8.0.2/32, fd00::2/128\nDNS = 10.8.0.1, home.lan\nMTU = 1380\n\n\
             [Peer]\nPublicKey = {PUB}\nPresharedKey = {PUB}\nEndpoint = home.example.com:51820\nAllowedIPs = 10.8.0.0/24\nPersistentKeepalive = 25\n"
        );
        let c = parse_conf(&text).unwrap();
        assert_eq!(c.private_key, PRIV);
        assert_eq!(c.preshared_key.as_deref(), Some(PUB));
        assert_eq!(c.wireguard.address, vec!["10.8.0.2/32", "fd00::2/128"]);
        assert_eq!(c.wireguard.dns, vec!["10.8.0.1"]);
        assert_eq!(c.wireguard.mtu, 1380);
        assert_eq!(c.wireguard.peer.endpoint, "home.example.com:51820");
        assert_eq!(c.wireguard.peer.persistent_keepalive, Some(25));
    }

    #[test]
    fn rejects_two_peers_and_missing_key() {
        let two = format!("[Interface]\nPrivateKey={PRIV}\nAddress=10.0.0.2/32\n[Peer]\nPublicKey={PUB}\nEndpoint=a:1\n[Peer]\nPublicKey={PUB}\nEndpoint=b:1\n");
        assert!(parse_conf(&two).is_err());
        let nokey = format!("[Interface]\nAddress=10.0.0.2/32\n[Peer]\nPublicKey={PUB}\nEndpoint=a:1\n");
        assert!(parse_conf(&nokey).is_err());
    }
}
