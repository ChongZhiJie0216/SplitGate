# SplitGate 开发规格：按域名 / 程序分流的 VPN 客户端

Oct 26, 2026 · @ChongZhiJie · 第 0 阶段实测后修订（维护者同意全部 20 条修改，2026-10-26；依据见 `docs/phase0-findings.md`）

## 概述

SplitGate（暂定名）是一个 Windows 桌面客户端：连接一个 VPN 出口（Cisco AnyConnect / AnyLink、Pangolin 或 WireGuard），但只让命中规则的域名或程序走这个出口，其他流量照常走本地网络。第一个使用场景是让 Xbox、Minecraft、Steam 的流量经 VPN 出去，其他流量留在学校网络。

**目标**

- 一键连接或断开，不需要 Docker、命令行或手动改路由；支持 Cisco、Pangolin、WireGuard 三种出口，同一时间启用一个。
- 按域名和程序路径分流，内置可编辑的规则预设（首批：Xbox、Minecraft、Steam）。
- **拦截检测**：发现某个网址被当前网络拦截（证书被换成 Fortinet 等），提示用户一键加进规则。
- 实时显示哪些连接走了 VPN，方便补规则。
- Cisco 密码、WireGuard 私钥存在 Windows 凭据管理器，不写进明文配置；Pangolin 的凭据由官方客户端保管，SplitGate 不接触。

**非目标（MVP 不做）**

- macOS、Linux、移动端。
- 可视化节点画布（类似 React Flow 的拖拽路由编辑），留到后续版本。
- 订阅、同时连接多个出口、代理协议（Shadowsocks、VLESS 等）。
- 全隧道模式，以及与官方 Cisco Secure Client 共存。
- 分流期间的 IPv6（见“网络策略”）。

本文档用于交给 Claude Code 实现。遇到与本文不符的事实，以实测为准，并回写本文。

## 架构总览

系统流量只由 sing-box 的 TUN 接管，SplitGate 只有 **sing-box 一个 sidecar**：

- **Cisco 出口**：sing-box 自带的 OpenConnect endpoint（`endpoints: [{type: "openconnect"}]`），不打包 openconnect.exe，不另建 VPN 网卡，不改系统路由。
- **WireGuard 出口**：sing-box 自带的 WireGuard endpoint。
- **Pangolin 出口**：“外部客户端”类型。官方 Pangolin 客户端负责登录、凭据和隧道；SplitGate 删掉它的全隧道路由、加一条最低优先级默认路由，再让 sing-box 的 `direct` 出口用 `bind_interface` 绑定它的网卡。

&#91;embedded content: 架构总览 · 主程序、sing-box sidecar、三种出口\]

主程序本身不处理任何数据包，只负责生成配置、启停 sing-box、监视外部客户端的路由和显示状态。

## 技术栈与版本锁定

所有外部二进制都锁定到具体版本并记录校验和（`third_party/VERSIONS.md`）；sing-box 的配置格式在小版本之间也会变，不能用 latest。

| 层 | 选型 | 说明 |
| --- | --- | --- |
| 应用外壳 | Tauri 2（Rust） | 进程管理、配置读写、凭据存取、路由监视、IPC |
| 前端 | React 18 + TypeScript + Vite | 状态管理用 zustand，UI 组件库自选（建议 shadcn/ui） |
| 分流核心 | sing-box 1.14.4（Tauri sidecar） | TUN 入站、按域名 / 程序分流、DNS 分流、Clash API、API 服务；同时提供 OpenConnect 和 WireGuard 出口 |
| Cisco 出口 | sing-box OpenConnect endpoint | 第 0 阶段 V3a 实测可用（AnyLink 服务器，账号密码 + 组别） |
| Pangolin 出口 | 官方 Pangolin 客户端（用户自己安装） | 需 Pangolin 服务器 1.24 及以上（Exit Node）；SplitGate 不打包它 |
| WireGuard 出口 | sing-box WireGuard endpoint | 不需要额外 sidecar |
| 虚拟网卡驱动 | sing-box TUN | 第 1 阶段打包时核实 sing-box.exe 单独即可建立 TUN，是否还要另附 wintun.dll |
| 凭据存储 | `keyring` crate | 对应 Windows 凭据管理器 |
| 安装包 | Tauri bundler，NSIS | 安装时以管理员身份写入程序目录 |

**备用做法**：sing-box 的 OpenConnect endpoint 遇到不支持的服务器时，退回打包 openconnect.exe + 自定义连接脚本（原规格做法 A，第 0 阶段跳过 V3b，**未实测**）。

## 目录结构

```
splitgate/
├─ src/                      # 前端（React + TS）
│  ├─ pages/                 # 连接、规则、连接列表、日志、Profile、设置、关于
│  ├─ components/
│  ├─ store/                 # zustand：配置、连接状态、日志缓冲
│  └─ lib/ipc.ts             # 所有 Tauri invoke / listen 的类型封装
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ commands.rs         # 暴露给前端的 command
│  │  ├─ supervisor.rs       # 连接状态机
│  │  ├─ exits/              # ExitAdapter trait 与三种出口：cisco.rs、pangolin.rs、wireguard.rs
│  │  ├─ singbox.rs          # 配置生成（经 stdin 传入）、启动、Clash API / API 服务客户端
│  │  ├─ route_guard.rs      # 外部客户端网卡的路由处理与持续监视
│  │  ├─ block_detect.rs     # 拦截检测（证书签发者 / 证书链检查）
│  │  ├─ credentials.rs      # keyring 封装
│  │  ├─ config.rs           # profiles.json / rules.json 读写与校验
│  │  └─ cleanup.rs          # 启动时和断开后清理残留网卡、路由、进程
│  ├─ binaries/              # sidecar：sing-box
│  ├─ resources/
│  │  └─ presets/            # 内置规则预设：xbox.json、minecraft.json、steam.json
│  └─ tauri.conf.json
├─ third_party/              # 各组件版本、SHA256、许可证文本与来源说明
├─ docs/SPEC.md              # 本文档
└─ README.md
```

## 连接状态机

supervisor.rs 统一管理状态，前端只显示状态，不自行推断。同一时间只允许一次连接或断开操作，重复点击一律忽略。

&#91;embedded content: 连接状态机 · 7 个状态\]

- **“已连接”只在出口真正可用后才显示**：Cisco 要等 sing-box 报告隧道已建立（OpenConnect `tunnel established` / API 状态），WireGuard 要等第一次握手成功，Pangolin 要等官方客户端从 sing-box 启动造成的断线中恢复（见组件 6）。
- 连接中命中规则的流量要**快速失败**（直接拒绝），不能让程序卡在超时上。
- WireGuard 握手或 Cisco 连接在超时内没有成功时，提示“可能被当前网络阻挡（学校网络挡 WireGuard UDP 和 DTLS）”。
- 已连接时 sing-box 意外退出、或出口隧道断开，进入“错误”并先清理；开启自动重连时，清理完成后按指数退避重新进入“认证中”，最多 5 次。
  - v0.8.0 实现：等待重连时状态是“错误”加 `retrying: true`（界面显示黄色“等待重新连接”）；间隔 2、4、8、16、32 秒；期间按“连接”立即重连、按“断开”停止。设置 `auto_reconnect`（默认开）。
  - “出口隧道断开”的判断：连接期间每 60 秒经 `vpn` 出口测一次（和连接时同一个测试网址），连续 20 秒都不通就算断开。
  - 睡眠唤醒的判断：每 5 秒看一次时钟，一次跳过 25 秒以上就是刚睡醒（不用 Windows 电源通知）；之后马上测出口，60 秒内不通就算断开。**待核实**：实际睡眠 / 唤醒。

## 组件规格

### 0. 出口适配器（exits/）

三种出口实现同一个 Rust trait，supervisor 和状态机只依赖这个 trait：

```rust
#[async_trait]
pub trait ExitAdapter {
    async fn start(&mut self, profile: &Profile, prompts: PromptChannel) -> Result<ExitReady>;
    async fn stop(&mut self) -> Result<()>;
    fn is_alive(&self) -> bool;
}

pub enum ExitReady {
    // Cisco、WireGuard：直接写进 sing-box endpoints
    SingboxEndpoint { config: serde_json::Value, vpn_dns: Vec<IpAddr> },
    // Pangolin：外部客户端的网卡，sing-box 用 direct + bind_interface；
    // bypass = 外部客户端自己的例外地址，必须排除在 TUN 之外，vpn-dns 也要避开
    ExternalInterface { ifname: String, vpn_dns: Vec<IpAddr>, bypass: Vec<IpAddr> },
}
```

singbox.rs 按 `ExitReady` 的种类生成 `vpn` 出口；状态机中的“等待网卡”只对外部客户端有效。

### 1. Cisco 出口（exits/cisco.rs，sing-box OpenConnect endpoint）

生成的 endpoint（字段名以 sing-box 1.14.4 为准）：

```json
{ "type": "openconnect", "tag": "vpn", "server": "<server>", "username": "<user>", "password": "<从 keyring 读出>",
  "auth_group": "<组别>", "no_udp": true, "bind_interface": "<出口网卡，可省略>" }
```

- **组别**（`auth_group`）：AnyLink 等服务器的登录表单有组别下拉框（例：Master Network）；不填时登录会卡在“等待认证”。
- **只用 TCP**（`no_udp`）：学校网络挡 DTLS，DTLS 超时会让连接慢 16–50 秒；学校默认开。家里路由器转发 UDP 8443 后可以在其他网络关掉。
- **出口网卡**（`bind_interface`）：VPN 外层连接固定走某张网卡（例：学校有线不通时走手机热点）；留空 = 系统自动。
- **登录表单**：服务器额外要求的输入（组别、MFA）通过 sing-box 的 **API 服务**（`services: [{type: "api"}]`，gRPC）读取和回答，对应 IPC 的 `prompt` / `answer_prompt`。API 只听 127.0.0.1，端口和 secret 每次随机生成，不写进文件。
- 密码只存在 keyring，只经 stdin 传给 sing-box（见组件 3）。
- 首次连接遇到未知服务器证书时的处理方式（指纹确认、固定证书）**待核实**：第 0 阶段的服务器证书受信任，没有测到。
- MVP 不做 MFA（当前服务器只要账号密码 + 组别）；Duo / SAML 留到第 3 阶段。

### 2. 外部客户端出口的接法（route_guard.rs）

适用于 Pangolin（以及以后任何“自己建网卡的 VPN 客户端”）。Windows 实测规则：

1. **删除全隧道路由**：Pangolin 开 Exit Node 时会在它的网卡上加 `0.0.0.0/1` 和 `128.0.0.0/1`，要删掉。
2. **加一条最低优先级的默认路由**：`0.0.0.0/0`，接口跃点 9999，只放 ActiveStore（重启即失效）。原因：Windows 上 `bind_interface`（IP_UNICAST_IF）**要求该网卡上有覆盖目标的路由**，否则直接报“网络不可达”；跃点最低，一般程序不会选它。
3. **持续监视**（每 5 秒）：外部客户端重连、换 Exit Node 会把全隧道路由加回来，发现就再删；网卡消失就进入“错误”。
4. **例外地址**：读取外部客户端加在物理网卡上的 /32 例外路由（Pangolin 实测：它的服务器、site 端点、家里 IP、物理网卡的 DNS 1.1.1.1），全部加进 TUN 的 `route_exclude_address`，避免外部客户端自己的封包被 sing-box 吸进去形成回环。
5. **断开时**：只撤销 SplitGate 自己加的低优先级路由；全隧道路由不补回，由用户在官方客户端里断开 / 重连恢复。cleanup.rs 只处理自己加的路由，**不碰外部客户端的网卡和 DNS**。

### 3. sing-box 配置生成（singbox.rs）

每次连接都由 profile、规则和出口信息在内存中生成配置，**经 stdin 传给 sing-box**（`sing-box check -c stdin`、`sing-box run -c stdin`），含密码的配置不落盘。Windows 写 stdin 时会在前面加 UTF-8 BOM，要去掉。sing-box 的 stderr 逐行读出当日志（密码、secret 脱敏）。

- `inbounds`：一个 `tun`，网卡名 `SplitGate-TUN`，`auto_route`、`strict_route`，地址 `172.19.0.1/30` + `fdfe:dcba:9876::1/126`（保留 IPv6 地址，防止 IPv6 流量绕过 TUN）；外部客户端出口时加 `route_exclude_address`。
- `outbounds`：`direct` 走本地；Pangolin 时另有 `vpn` = `direct` + `bind_interface: <外部网卡>`。Cisco、WireGuard 的 `vpn` 是 endpoint。
- `route.rules`：依次 `sniff` → `hijack-dns` → `ip_version: 6` 拒绝 → rules.json 生成的 `process_path`、`process_name`、`domain_suffix`、`domain_keyword`、`ip_cidr` 规则指向 `vpn`；`final` 是 `direct`；开启 `auto_detect_interface`，有程序规则时开启 `find_process`。
- `dns`：`strategy: ipv4_only`。
  - `local`：**不用 sing-box 的 `local` 类型**（它读系统 DNS 列表，外部客户端的 DNS 会排在最前面而导致超时），改用物理网卡自己的 DNS 服务器（`udp`，连接时从默认网卡读取）。
  - `vpn-dns`：Cisco 用服务器下发的 DNS（sing-box `openconnect` DNS 类型 + `accept_default_resolvers`）；WireGuard 用 `.conf` 的 `DNS`；Pangolin 用设置的公共 DNS（默认 9.9.9.9），并**避开外部客户端的例外地址**。detour 到 `vpn`。
  - 命中规则的域名用 `vpn-dns`，其余用 `local`。Pangolin 资源别名的域名后缀交给 Pangolin 自己的 DNS（100.96.128.1，经 `vpn`）解析，否则 `hijack-dns` 会让这些别名解析不到。
- `experimental.clash_api`：只听 127.0.0.1 的随机端口，随机 secret，供连接列表页使用。
- `services`：Cisco 出口时加 `api` 服务（见组件 1）。
- 防回环：VPN 服务器地址固定走 `direct`。
- 先 `check` 通过才 `run`。

### 4. 凭据（credentials.rs）

- 服务名 `SplitGate`，账户键为 `<profile_id>:<user>`。存 Cisco 密码、WireGuard 私钥。
- 删除 profile 时同步删除凭据。
- 日志、错误信息、诊断包中一律不输出密码、cookie、secret、私钥。

### 5. 权限

- TUN 和改路由都需要管理员权限。MVP 在应用清单里要求 `requireAdministrator`，启动时弹一次 UAC。
- 后续版本改为安装一个 Windows 服务负责启停核心，界面以普通权限运行，两者通过命名管道通信。

### 6. Pangolin 出口（exits/pangolin.rs，官方客户端）

**不运行 Olm 或 Pangolin CLI**：实测 Olm 1.10.1 和 Pangolin CLI 0.19.0 在 Windows 上都必须装成 Windows 服务，会把 Secret 写进 `ProgramData\olm\service_args.json`、事件日志或注册表，违反“凭据不落盘”的规则。原规格的“方案 A：sidecar 运行 Olm / fork Olm”和“方案 B：WSL2 + Pangolin CLI”取消。

前提：

- Pangolin 服务器 1.24 或以上，已建 Exit Node 资源并关联 site。
- **Exit Node 要在 Pangolin 后台开启**才生效（只在 App 里选不算）。
- 后台的“Exit Node Takes Precedence Over Resources”开着时，客户端会拿掉连回家里内网的路由；需要同时访问内网资源时要关掉它。
- 用户已安装官方 Pangolin 客户端并登录；SplitGate 不读、不存 Secret。

流程：

1. 用户在官方客户端里连接并选好 Exit Node。
2. SplitGate 找到网卡（默认名 `Pangolin`，设置里可改），按组件 2 处理路由，读取例外地址。
3. 启动 sing-box。**sing-box 启动会让官方客户端断线约 10–15 秒**（它跟着系统默认路由改例外路由，短暂指向 TUN），之后自行恢复。SplitGate 要等它恢复（经 `vpn` 出口探测成功）才显示“已连接”。
4. Exit Node 背后是多个 site 组成的 gateway 组，客户端会自动换出口 site，**出口 IP 会变**；连接页显示“当前出口 IP”时要定时刷新，不要当成错误。
5. IPv6 不经过 Exit Node（与本项目只用 IPv4 的策略一致）。

### 7. WireGuard 出口（exits/wireguard.rs）

- 用户导入标准的 `.conf` 文件（或粘贴内容），程序解析成 sing-box 的 `wireguard` endpoint；私钥存进 keyring，不留在 profiles.json。
- `allowed_ips` 固定写 `0.0.0.0/0`、`::/0`，忽略 `.conf` 里的 `AllowedIPs`，分流完全由规则决定。
- 以 `.conf` 里的 `DNS` 作为 `vpn-dns`。
- 学校有线网络挡 WireGuard UDP（实测只有手机热点能通）；握手超时要按状态机的提示处理。

### 8. 拦截检测（block_detect.rs）

学校的 Fortinet 是 **SSL 深度检测 + 按网址（SNI）拦截**：被拦截的网站拿到 Fortinet 自签、不受信任的证书；DNS 不拦截（DHCP 给 1.1.1.1 / 8.8.8.8，被拦截的域名都能正常解析）。拦截名单会变，固定清单不够。

- 对“直连”流量中 TLS 握手失败、或证书签发者是 Fortinet / 证书链不受信任的域名，记成“疑似被拦截”。
- 界面提示：“这个网址被当前网络拦截，要不要加进规则？”，一键加进某个规则集。
- 检测只读，不改任何连接。
- **检测方式（v0.7.0 定）：主动做一次 TLS 握手。** sing-box 的 info 日志只有 IP，没有域名，被动看日志判断不了。做法：连接列表每秒读一次 Clash API，挑出“直连 + TCP 443 + 有域名”的新连接，由 SplitGate 用 Windows PowerShell 的 `SslStream` 对这个域名握手一次（和 `phase0\v5-block-probe\probe.ps1` 一样，只用 IPv4），证书由 Windows 自己的信任列表判断。不加第三方依赖。
  - 签发者或根证书含 Fortinet → “证书被 Fortinet 换掉”；证书链不受信任 → “证书不受信任”；TCP 通但握手失败 → “握手失败（不一定是拦截）”；只有名称不符或 TCP 不通 → 不下结论。之后检查正常的会从列表拿掉。
  - 每次连接每个域名只查一次，最多 300 个；每批 6 个，每个最多 3 秒连接 + 4 秒握手。结果只放在内存，关掉 SplitGate 就清掉。
  - 很短的连接（被拦截的程序拿到坏证书马上断开）可能在两次读取之间就结束了，但程序通常会重试，一般还是能抓到。**待核实**：在学校网络下实测。

## 网络策略

- **分流期间只用 IPv4**：DNS `ipv4_only` + 拒绝 IPv6 流量。本机只有手机热点有 IPv6，选有线当出口时 IPv6 本来就不通；Pangolin Exit Node 也不接管 IPv6。
- 学校网络：挡 WireGuard UDP、挡 Cisco DTLS；Cisco 的 DTLS 在手机热点上也失败（多半是服务器那边没转发 UDP 8443）。
- 结束后恢复上网的时间实测 0.7–12.3 秒，不稳定：断开和崩溃后都要**主动清理并检查**（cleanup.rs），不能只靠进程退出。

## 数据模型

配置存放在 `%APPDATA%\SplitGate\`，均为 JSON，并带 `version` 字段，方便以后迁移。

**profiles.json**：VPN 连接配置（不含密码）

```json
{
  "version": 1,
  "active": "school-vpn",
  "profiles": [
    {
      "id": "school-vpn",
      "type": "cisco",
      "name": "Cisco 出口",
      "ruleset_ids": ["xbox", "minecraft", "steam"],
      "cisco": { "server": "vpn.example.com", "user": "user01", "auth_group": "Master Network", "no_udp": true, "bind_interface": null, "mfa": "none" }
    },
    {
      "id": "home-pangolin",
      "type": "pangolin",
      "name": "Pangolin 家里出口",
      "ruleset_ids": ["xbox", "minecraft", "steam"],
      "pangolin": { "interface": "Pangolin", "vpn_dns": ["9.9.9.9"], "alias_suffixes": [], "alias_dns": "100.96.128.1" }
    },
    {
      "id": "home-wg",
      "type": "wireguard",
      "name": "WireGuard 家里出口",
      "ruleset_ids": ["xbox"],
      "wireguard": { "address": ["10.8.0.2/32"], "peer": { "endpoint": "home.example.com:51820", "public_key": "..." }, "dns": ["10.8.0.1"], "mtu": 1420 }
    }
  ]
}
```

`type` 决定使用哪个 ExitAdapter，并只读取同名的子对象。Cisco 的 `mfa` 取值为 `none` / `duo-push` / `saml`（MVP 只有 `none`）；`bind_interface` 为 `null` 表示系统自动。Cisco 密码、WireGuard 私钥存在 keyring；Pangolin 没有要存的凭据。同一时间只有 `active` 指向的出口生效。

**rules.json**：规则集（内置预设首次运行时复制进来，用户可改）

```json
{
  "version": 1,
  "rulesets": [
    {
      "id": "xbox",
      "name": "Xbox",
      "enabled": true,
      "domain_suffix": ["xboxlive.com", "xbox.com", "xboxservices.com", "gamepass.com", "xboxab.com", "playfabapi.com"],
      "domain_keyword": [],
      "process_path": [],
      "process_name": [],
      "ip_cidr": []
    },
    {
      "id": "minecraft",
      "name": "Minecraft",
      "enabled": true,
      "domain_suffix": ["minecraft.net", "xboxlive.com"],
      "domain_keyword": [],
      "process_path": [],
      "process_name": [],
      "ip_cidr": []
    },
    {
      "id": "steam",
      "name": "Steam",
      "enabled": true,
      "domain_suffix": ["steampowered.com", "steamcommunity.com", "steamserver.net", "steamstatic.com", "steamcdn-a.akamaihd.net"],
      "domain_keyword": [],
      "process_path": [],
      "process_name": [],
      "ip_cidr": []
    }
  ]
}
```

- 第 0 阶段 V5 实测：`xboxlive.com`、`xbox.com`、`minecraft.net`、Steam 这几项都被学校拦截，加进规则后 PrismLauncher 和 Steam 都能登录。`steamcdn-a.akamaihd.net` 只放这一个主机名，**不要放整个 `akamaihd.net`**。
- 程序规则优先用 **`process_path`（完整路径）**：`javaw.exe` 这类名字很多程序共用，按名字会误伤（V6 实测只用路径就能让 PrismLauncher 登录）。预设里不放路径（每台电脑不同），由用户在界面上从正在运行的程序里挑。
- Xbox App 的进程名（2026-10-26 实测）：`XboxPcApp.exe`（主程序）、`XboxPcAppFT.exe`，已补进预设。都在 `C:\Program Files\WindowsApps\Microsoft.GamingApp_<版本>\` 下，路径带版本号，所以用程序名不用路径。`gamingservices.exe`（下载和安装游戏）、`GameBar.exe` 没有放进预设。

**settings.json**：设置页（v0.5.0 起）

```json
{ "version": 1, "auto_connect": false, "auto_reconnect": true, "singbox_log_level": "info", "tun_ipv4": "172.19.0.1/30" }
```

- `singbox_log_level` 只能是 `info` / `debug` / `trace`：状态机靠 info 级的 “tunnel established” 判断隧道建立，不能更安静。
- `tun_ipv4` 是私有 IPv4 主机地址 + /8–/30 前缀。文件坏了或值不对时，连接照用默认值，不阻止连接。
- 开机自启不存在这里：以任务计划程序里名为 `SplitGate` 的任务（登录时、最高权限、参数 `--autostart`）为准。
- 主题是界面偏好，存在 WebView 的 localStorage，避免启动时闪一下。

## Tauri IPC 接口

前端只通过 `src/lib/ipc.ts` 调用后端，类型和 Rust 端保持一致（建议用 `specta` 或 `ts-rs` 自动生成）。

**Commands（前端调用）**

| Command | 参数 | 返回 |
| --- | --- | --- |
| `list_profiles` | 无 | Profile\[\] |
| `save_profile` | Profile, secret? | 无（secret = Cisco 密码或 WireGuard 私钥） |
| `delete_profile` | id | 无 |
| `list_rulesets` | 无 | RuleSet\[\] |
| `save_ruleset` | RuleSet | 无 |
| `reset_ruleset_to_preset` | id | RuleSet |
| `list_running_processes` | 无 | {name, path}\[\]（给“加程序”用） |
| `list_interfaces` | 无 | 网卡列表（给“出口网卡”“外部客户端网卡”用） |
| `connect` | profile\_id | 无（进度通过事件推送） |
| `disconnect` | 无 | 无 |
| `get_status` | 无 | Status |
| `answer_prompt` | prompt\_id, value | 无（登录表单、证书确认） |
| `get_connections` | 无 | Connection\[\]（经 Clash API） |
| `close_connection` | conn\_id | 无 |
| `list_blocked` | 无 | 疑似被拦截的域名列表 |
| `dismiss_blocked` | host | 列表（这次运行期间不再提示这个域名） |
| `clear_blocked` | 无 | 列表（清空） |
| `export_diagnostics` | 无 | 文件路径（日志打包，已脱敏） |

**Events（后端推送）**

| Event | 内容 |
| --- | --- |
| `status_changed` | 新状态、原因、时间戳 |
| `prompt` | 需要用户输入的内容：登录表单字段、证书确认 |
| `log` | 来源（sing-box / route-guard / app）、级别、文本 |
| `traffic` | 每秒上下行速率，分 vpn 和 direct |
| `blocked_detected` | 新发现的疑似被拦截域名 |

## UI 页面

布局沿用 Bluewhale 的思路：左侧导航，顶部固定一个连接按钮和当前 profile 选择器；默认深色主题，支持跟随系统。界面文字使用简体中文，文案集中放在一个文件里，方便以后加英文。

| 页面 | 内容 | 阶段 |
| --- | --- | --- |
| 连接 | 大号连接按钮、状态、当前出口 IP（定时刷新）、VPN 内网 IP、上下行速率、已连接时长 | MVP |
| 规则 | 规则集列表与开关；编辑域名后缀、关键字、程序路径（从运行中的程序里挑）、程序名、IP 段；“恢复预设” | MVP |
| 连接列表 | 实时连接：程序、域名、出口（vpn / direct）、流量；可筛选、可关闭单条连接、一键加规则 | 第 2 阶段 |
| 拦截提示 | 疑似被拦截的域名，一键加进规则 | 第 2 阶段 |
| 日志 | 按来源和级别筛选，支持暂停滚动、复制、导出诊断包 | MVP |
| Profile | 先选出口类型，再显示对应表单。Cisco：服务器、账号、密码、组别、只用 TCP、出口网卡；Pangolin：外部客户端网卡名、vpn-dns、资源别名后缀（附“先在官方客户端连接并在后台开 Exit Node”的说明）；WireGuard：导入 .conf | MVP（Pangolin 表单第 2 阶段） |
| 设置 | 开机自启、启动后自动连接、主题、sing-box 日志级别、TUN 网段 | MVP |
| 关于 | 版本、组件版本、许可证与第三方声明 | MVP |
| 流量画布 | 用 React Flow 显示“应用 → 规则 → 出口”的可视化图 | 后续 |

**交互要点**

- 连接过程中，按钮显示当前步骤（认证中、等待隧道、启动分流、等待外部客户端恢复），失败时显示原因和“查看日志”入口。
- 需要回答登录表单或确认证书时，弹出模态框，用户回答后调用 `answer_prompt`。
- 系统托盘：图标颜色表示状态；右键菜单提供连接、断开、打开主窗口、退出。关闭窗口时缩到托盘，不断开连接。

## 错误处理与清理

最重要的一条：任何情况下都不能让用户的电脑断网。sing-box 的 TUN 接管了系统路由，如果它残留而出口已断，流量会被导进一个不存在的出口。

| 情况 | 处理 |
| --- | --- |
| Cisco 认证失败 | 停 sing-box，停在“已断开”，显示服务器返回的原始错误，不重试 |
| Cisco 隧道断开（sing-box 仍在跑） | 规则流量快速失败；按设置决定是否重连（指数退避，最多 5 次） |
| WireGuard 握手超时 | 提示“可能被当前网络阻挡”，停 sing-box |
| sing-box `check` 不通过或启动失败 | 显示错误；配置不落盘，诊断包里附脱敏后的配置 |
| sing-box 运行中退出 | 状态改为“错误”，执行清理 |
| 外部客户端网卡消失 / 全隧道路由被加回 | 网卡消失 → “错误”并停 sing-box；路由被加回 → 自动再删并记日志 |
| 外部客户端 60 秒内没从断线中恢复 | 视为失败，停 sing-box，撤销自己加的路由 |
| 应用被强制结束或崩溃 | 下次启动时 cleanup.rs 结束残留的 sing-box，移除 `SplitGate*` 网卡和对应路由，撤销自己加在外部网卡上的低优先级路由 |
| 系统睡眠后唤醒 | 检测出口是否仍然可用，不可用则走重连流程 |

- sing-box 放进 Windows Job Object，并设置 `KILL_ON_JOB_CLOSE`，主进程一退出它就结束。
- 断开顺序固定为：先停 sing-box（恢复系统路由），再撤销外部网卡上的路由，最后跑一次清理检查。
- 所有用户可见的错误都附带“复制详情”按钮。

## 授权合规

Bluewhale 只用来参考产品形态，不复制它的代码、图标、名称或文案。许可证已于 2026-10-26 到各项目核实，详见 `third_party/VERSIONS.md`。

| 组件 | 许可证 | 要求 |
| --- | --- | --- |
| SplitGate 自身 | 自定（建议 MIT） | 无 |
| sing-box | GPL-3.0-or-later，**附加条款：不得使用 sing-box 的名称或暗示与它有关联** | 作为独立进程分发；附带许可证文本，注明来源和版本，提供源码链接；产品名、安装包、界面文字不用 sing-box 的名字 |
| Wintun | WireGuard LLC 的预编译二进制许可 | 只有在需要另附 wintun.dll 时适用：不修改 DLL，随使用它的软件一起分发 |
| openconnect | LGPL-2.1 | 只在启用备用做法时适用 |
| 官方 Pangolin 客户端 | — | 不打包，由用户自己安装 |
| 前端依赖 | 多为 MIT | 构建时生成第三方许可证清单 |

不再打包 Olm（AGPL-3.0），许可证负担变小。

**使用提醒**：通过他人的 VPN 绕过学校网络限制，可能违反学校或 VPN 提供方的使用政策，使用前请自行确认。

## 开发阶段与验收标准

共分四个阶段，每个阶段结束都要能实际使用，并通过列出的验收项。

### 第 0 阶段：技术验证（已完成，2026-10-26；详见 `docs/phase0-findings.md`）

- [x] V0 环境与版本锁定（sing-box 1.14.4 等，SHA256 记在 `third_party/VERSIONS.md`）
- [x] V1 sing-box TUN 全直连，结束后网络恢复
- [x] V2 sing-box WireGuard endpoint（只在手机热点上通；学校有线挡 WireGuard UDP）
- [x] V3a sing-box OpenConnect endpoint 连上 AnyLink 服务器（`auth_group` + `no_udp` 全自动）
- [ ] ~~V3b openconnect.exe + 自定义连接脚本~~（维护者定跳过，2026-10-26）
- [x] V4 VPN 外层连接固定走指定网卡（`bind_interface`）
- [x] V5 按域名分流绕过学校 Fortinet 拦截；PrismLauncher、Steam 登录成功
- [x] V6 按程序路径分流；PrismLauncher 登录成功
- [x] V7 Pangolin：官方客户端 + 删全隧道路由 + 低优先级默认路由 + `bind_interface`，规则流量从 Exit Node 出去
- [x] 核实各组件许可证
- 原规格“`bind_interface` 在没有该网卡路由时仍能送进隧道”：**实测不成立**（见组件 2）。

### 第 1 阶段：MVP

- [x] Tauri 项目骨架、sing-box sidecar 打包、管理员权限清单（v0.1.0）
- [x] profile 与凭据管理、规则集管理、内置 Xbox / Minecraft / Steam 预设；实现 Cisco 和 WireGuard 两种出口（v0.2.0–v0.4.0；ExitAdapter trait 还没抽出，见 `docs/progress.md`）
- [ ] 配置经 stdin 传入、连接状态机（含“出口真正可用才显示已连接”）、错误处理与清理（代码已完成 v0.4.0，待真正连接测试）
- [x] 连接页、规则页（含从运行中的程序挑选）、日志页、Profile 页、设置页、关于页、系统托盘（v0.5.0；托盘右键菜单待维护者实际点一次）

进度记录：`docs/progress.md`。

**验收**

- [ ] 点一次按钮完成连接，Xbox App 能登录、浏览商店、打开 Game Pass；PrismLauncher、Steam 能登录
- [ ] 连接期间，非规则流量的出口 IP 仍是学校网络
- [x] 在任务管理器中结束 SplitGate 进程后，网络能在 5 秒内恢复正常（v0.6.0，手机热点 + WireGuard，1.6 秒，2026-10-26）
- [x] 断开再连接 20 次，没有残留进程、网卡或路由（v0.6.0，手机热点 + WireGuard，每次断开后 0.7–1.1 秒清干净，2026-10-26）
- [ ] 日志、诊断包和 `%APPDATA%\SplitGate\` 中搜索不到密码（v0.6.0 日志、设置、WebView2 数据、便携版文件夹、命令行、任务计划都搜不到 Cisco 密码和 WireGuard 私钥，2026-10-26；诊断包第 2 阶段才有，到时再测）
- [x] 用任务管理器核实 Xbox App 的进程，补进预设（`XboxPcApp.exe`、`XboxPcAppFT.exe`，v0.6.1）

### 第 2 阶段：可观测性与 Pangolin

- [x] 连接列表页（Clash API）、流量统计、从连接列表一键加规则（v0.6.0；维护者 2026-10-26 同意在第 1 阶段验收前先做；`find_process` 改成一直开，才能显示每条连接的程序）
- [x] 拦截检测与拦截提示页（v0.7.0；主动 TLS 握手，见组件 8；**还没在学校网络下实测**）
- [x] 睡眠唤醒检测与自动重连（v0.8.0；**还没实测睡眠唤醒**）
- [ ] Pangolin 出口（外部客户端方式：route_guard、例外地址排除、等待恢复、资源别名 DNS）

### 第 3 阶段：增强

- [ ] Duo push 和 SAML 登录
- [ ] 用 Windows 服务取代整个应用以管理员身份运行
- [ ] React Flow 流量画布
- [ ] 评估备用做法（openconnect.exe）是否还需要

## 待确认事项

- [ ] 项目正式名称（SplitGate 只是暂定；不能含 sing-box 的名字）
- [x] Cisco 服务器的认证方式：账号密码 + 组别，不需要 MFA（第 0 阶段实测，2026-10-26）
- [ ] 这个 Cisco VPN 是你自己管理的，还是别人的？如果是自己的，服务器端的 dynamic split tunneling 可以作为备用方案
- [ ] 家里路由器有没有转发 UDP 8443（决定 Cisco 能不能用 DTLS）
- [ ] 是否需要支持多台电脑或便携版（免安装）
- [x] 代码仓库：GitHub `ChongZhiJie0216/SplitGate`，由维护者自己推送（维护者定，2026-10-26）
- [x] Pangolin 是自建，版本 1.24 以上，Exit Node 已在后台开启（维护者确认，2026-10-26）
- [ ] 同时有 Pangolin 和 WireGuard 时，家里出口优先用哪一个
