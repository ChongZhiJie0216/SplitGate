//! Cisco / AnyLink checks for the Profile page (maintainer's request 2026-10-26): read the group
//! list from the server (so the group is picked, not typed) and test a login before saving. Uses
//! the AnyConnect XML login itself (the same first steps sing-box's OpenConnect endpoint takes):
//! POST "init" → the server's login form, which holds the group list; POST "auth-reply" with
//! user / password / group → "complete" means the login works. No tunnel is built, no route or
//! DNS changes. HTTPS goes through Windows PowerShell (Windows decides about the certificate);
//! the password reaches it only through stdin, never an argument, environment variable or file.

use std::io::Write;
use std::process::Stdio;

use serde::Serialize;
use serde_json::Value;

use crate::{platform, singbox};

/// Pure ASCII PowerShell 5.1. Inputs: SG_URL, SG_MODE (groups / login), SG_USER, SG_GROUP; the
/// password is the first stdin line (login only). Prints {init, reply} (raw server XML) or {error}.
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
function Esc([string]$s) { return [Security.SecurityElement]::Escape($s) }
$headers = @{ 'X-Transcend-Version' = '1'; 'X-Aggregate-Auth' = '1'; 'X-AnyConnect-Platform' = 'win' }
$ua = 'Open AnyConnect VPN Agent v9.21'
$head = '<?xml version="1.0" encoding="UTF-8"?>'
$ver = '<version who="vpn">9.21</version><device-id>win</device-id>'
function Post([string]$body) {
    $bytes = [Text.Encoding]::UTF8.GetBytes($body)
    $r = Invoke-WebRequest -Uri $env:SG_URL -Method Post -Body $bytes -Headers $headers -UserAgent $ua -ContentType 'application/x-www-form-urlencoded' -UseBasicParsing -TimeoutSec 15 -WebSession $script:web -MaximumRedirection 5
    return [Text.Encoding]::UTF8.GetString($r.RawContentStream.ToArray())
}
try {
    $script:web = New-Object Microsoft.PowerShell.Commands.WebRequestSession
    $init = Post ($head + '<config-auth client="vpn" type="init" aggregate-auth-version="2">' + $ver + '<group-access>' + (Esc $env:SG_URL) + '</group-access></config-auth>')
    $reply = ''
    if ($env:SG_MODE -eq 'login') {
        $pw = [Console]::In.ReadLine()
        $group = ''
        if ($env:SG_GROUP) { $group = '<group-select>' + (Esc $env:SG_GROUP) + '</group-select>' }
        $reply = Post ($head + '<config-auth client="vpn" type="auth-reply" aggregate-auth-version="2">' + $ver + '<auth><username>' + (Esc $env:SG_USER) + '</username><password>' + (Esc $pw) + '</password></auth>' + $group + '</config-auth>')
        $pw = $null
    }
    @{ init = $init; reply = $reply } | ConvertTo-Json -Compress
} catch {
    $m = $_.Exception
    while ($m.InnerException) { $m = $m.InnerException }
    @{ error = $m.Message } | ConvertTo-Json -Compress
}
"#;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum LoginResult {
    /// The server accepted user, password and group.
    Ok,
    /// Wrong user / password / group; `message` is the server's own text when it gave one.
    Rejected { message: String },
    /// The password was accepted but the server asks for more (MFA / OTP): not supported yet.
    NeedsMore { message: String },
    /// Could not reach the server, or its answer was not an AnyConnect login.
    Failed { message: String },
}

fn run(url: &str, mode: &str, user: &str, group: &str, password: Option<&str>) -> Result<(String, String), String> {
    let mut child = platform::powershell(SCRIPT)
        .env("SG_URL", url)
        .env("SG_MODE", mode)
        .env("SG_USER", user)
        .env("SG_GROUP", group)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法执行检查：{e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(password.unwrap_or("").as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let out = child.wait_with_output().map_err(|e| format!("检查没有完成：{e}"))?;
    let v: Value = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).map_err(|_| "检查失败：没有得到服务器的回应".to_string())?;
    if let Some(e) = v["error"].as_str() {
        return Err(format!("连不到服务器：{e}"));
    }
    Ok((v["init"].as_str().unwrap_or("").to_string(), v["reply"].as_str().unwrap_or("").to_string()))
}

/// The server's group list (empty when it does not ask for a group).
pub fn groups(server: &str) -> Result<Vec<String>, String> {
    if server.trim().is_empty() {
        return Err("请先填写服务器".into());
    }
    let (init, _) = run(&singbox::cisco_server_url(server), "groups", "", "", None)?;
    if !is_anyconnect(&init) {
        return Err("服务器的回应不是 AnyConnect / AnyLink 登录页面，请检查服务器地址".into());
    }
    Ok(group_options(&init))
}

pub fn test_login(server: &str, user: &str, group: Option<&str>, password: &str) -> LoginResult {
    if server.trim().is_empty() || user.trim().is_empty() || password.is_empty() {
        return LoginResult::Failed { message: "请先填写服务器、账号和密码".into() };
    }
    match run(&singbox::cisco_server_url(server), "login", user.trim(), group.unwrap_or("").trim(), Some(password)) {
        Ok((init, reply)) if is_anyconnect(&init) => interpret_reply(&reply),
        Ok(_) => LoginResult::Failed { message: "服务器的回应不是 AnyConnect / AnyLink 登录页面，请检查服务器地址".into() },
        Err(e) => LoginResult::Failed { message: e },
    }
}

fn is_anyconnect(xml: &str) -> bool {
    xml.contains("<config-auth")
}

fn interpret_reply(xml: &str) -> LoginResult {
    if !is_anyconnect(xml) {
        return LoginResult::Failed { message: "服务器的回应看不懂".into() };
    }
    if xml.contains(r#"type="complete""#) || xml.contains("<session-token>") {
        return LoginResult::Ok;
    }
    let message = error_text(xml).unwrap_or_default();
    // Asked again, but for something other than user/password (e.g. an OTP code).
    let asks_password_again = xml.contains(r#"name="password""#) || xml.contains(r#"name="username""#);
    if message.is_empty() && !asks_password_again && xml.contains("<form") {
        let label = message_text(xml).unwrap_or_else(|| "服务器还要求额外的验证（例如动态验证码）".into());
        return LoginResult::NeedsMore { message: label };
    }
    LoginResult::Rejected { message: if message.is_empty() { "登录被拒绝（账号、密码或组别不对）".into() } else { message } }
}

/// `<select name="group_list"> <option value="A">A</option> <option>B</option> </select>`
fn group_options(xml: &str) -> Vec<String> {
    let Some(start) = xml.find(r#"name="group_list""#) else { return Vec::new() };
    let rest = &xml[start..];
    let block = &rest[..rest.find("</select>").unwrap_or(rest.len())];
    let mut out: Vec<String> = Vec::new();
    for part in block.split("<option").skip(1) {
        let (attrs, after) = part.split_once('>').unwrap_or((part, ""));
        let text = after.split("</option>").next().unwrap_or("").trim();
        let value = attr(attrs, "value").unwrap_or_else(|| text.to_string());
        let value = unescape(value.trim());
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = attrs.find(&key)? + key.len();
    attrs[i..].split('"').next().map(str::to_string)
}

/// `<error id="88" param1="用户名或密码错误" param2="">登陆失败:  %s</error>` → the readable part.
fn error_text(xml: &str) -> Option<String> {
    let i = xml.find("<error")?;
    let tag = &xml[i..];
    let (attrs, after) = tag.split_once('>')?;
    let body = after.split("</error>").next().unwrap_or("");
    let param = attr(attrs, "param1").filter(|p| !p.trim().is_empty());
    let text = match param {
        Some(p) if body.contains("%s") => body.replace("%s", &p),
        Some(p) => p,
        None => body.to_string(),
    };
    let text = unescape(text.split_whitespace().collect::<Vec<_>>().join(" ").trim());
    (!text.is_empty()).then_some(text)
}

fn message_text(xml: &str) -> Option<String> {
    let i = xml.find("<message")?;
    let body = xml[i..].split_once('>')?.1.split("</message>").next()?.trim();
    (!body.is_empty()).then(|| unescape(body))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of AnyLink's init answer (its auth_request template), names replaced.
    const INIT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<config-auth client="vpn" type="auth-request" aggregate-auth-version="2">
    <opaque is-for="sg"><tunnel-group>Master Network</tunnel-group></opaque>
    <auth id="main">
        <title>Login</title>
        <message>请输入你的用户名和密码</message>
        <banner></banner>
        <form method="post" action="/">
            <input type="text" name="username" label="Username:"></input>
            <input type="password" name="password" label="Password:"></input>
            <select name="group_list" label="GROUP:">
                <option>Master Network</option>
                <option value="ops">运维 &amp; 测试</option>
                <option>Master Network</option>
            </select>
        </form>
    </auth>
</config-auth>"#;

    #[test]
    fn script_is_ascii() {
        assert!(SCRIPT.is_ascii());
    }

    #[test]
    fn reads_group_list() {
        assert_eq!(group_options(INIT), vec!["Master Network", "ops"], "value attribute wins, duplicates dropped");
        assert!(group_options("<config-auth><form></form></config-auth>").is_empty(), "server without groups");
    }

    #[test]
    fn interprets_login_answers() {
        let ok = r#"<config-auth client="vpn" type="complete"><session-id>1</session-id><session-token>x</session-token></config-auth>"#;
        assert_eq!(interpret_reply(ok), LoginResult::Ok);

        let wrong = INIT.replace("<banner></banner>", r#"<banner></banner><error id="88" param1="用户名或密码错误" param2="">登陆失败:  %s</error>"#);
        assert_eq!(interpret_reply(&wrong), LoginResult::Rejected { message: "登陆失败: 用户名或密码错误".into() });

        let otp = r#"<config-auth client="vpn" type="auth-request"><auth id="otp"><message>请输入动态码</message><form><input type="password" name="secondary_password"></input></form></auth></config-auth>"#;
        assert_eq!(interpret_reply(otp), LoginResult::NeedsMore { message: "请输入动态码".into() });

        assert!(matches!(interpret_reply(INIT), LoginResult::Rejected { .. }), "asked for the password again = rejected");
        assert!(matches!(interpret_reply("<html>not vpn</html>"), LoginResult::Failed { .. }));
    }

    #[test]
    fn login_needs_password() {
        assert!(matches!(test_login("vpn.example.com", "u", None, ""), LoginResult::Failed { .. }));
    }
}
