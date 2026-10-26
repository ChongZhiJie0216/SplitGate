# SplitGate 开发规格：Cisco 按域名分流客户端

Oct 26, 2026 · @ChongZhiJie

## 概述

SplitGate（暂定名）是一个 Windows 桌面客户端：连接一个 VPN 出口（Cisco AnyConnect、Pangolin 或 WireGuard），但只让命中规则的域名或进程走这个出口，其他流量照常走本地网络。第一个使用场景是让 Xbox 相关流量经 Cisco 出去，其他流量留在学校网络。

**目标**

- 一键连接或断开，不需要 Docker、命令行或手动改路由；支持 Cisco、Pangolin、WireGuard 三种出口，同一时间启用一个。
- 按域名和进程名分流，内置可编辑的规则预设（首个预设：Xbox）。
- 命中规则的域名改用 VPN 端 DNS 解析，以绕过本地 DNS 封锁。
- 实时显示哪些连接走了 VPN，方便补规则。
- 密码存在 Windows 凭据管理器，不写进明文配置。

**非目标（MVP 不做）**

- macOS、Linux、移动端。
- 可视化节点画布（类似 React Flow 的拖拽路由编辑），留到后续版本。
- 订阅、同时连接多个出口、代理协议（Shadowsocks、VLESS 等）。
- 全隧道模式，以及与官方 Cisco Secure Client 共存。

本文档用于交给 Claude Code 实现。Claude Code 先完成第 0 阶段的技术验证，再按阶段推进；遇到与本文不符的事实，以验证结果为准，并回写本文。

## 架构总览

系统流量只由 sing-box 的 TUN 接管；出口程序（openconnect 或 Olm）只负责建立隧道，不改系统路由；WireGuard 出口由 sing-box 自己处理。sing-box 命中规则时，把流量从绑定到 openconnect 网卡的出口送出去。

&#91;embedded content: 架构总览 · 主程序、两个 sidecar、两个出口\]

主程序本身不处理任何数据包，只负责生成配置、启停两个 sidecar 和显示状态。

## 技术栈与版本锁定

所有外部二进制都锁定到具体版本并记录校验和；sing-box 的配置格式在小版本之间也会变，不能用 latest。

| 层 | 选型 | 说明 |
| --- | --- | --- |
| 应用外壳 | Tauri 2（Rust） | 负责进程管理、配置读写、凭据存取、IPC |
| 前端 | React 18 + TypeScript + Vite | 状态管理用 zustand，UI 组件库自选（建议 shadcn/ui） |
| 分流核心 | sing-box（Tauri sidecar） | TUN 入站、按域名/进程分流、DNS 分流、Clash API；同时直接提供 WireGuard 出口 |
| Cisco 出口 | openconnect（Windows 版，sidecar） | 走 AnyConnect 协议，依赖 wintun.dll |
| Pangolin 出口 | Olm（Windows 版，sidecar 或系统服务） | Pangolin 客户端的网络核心，需 Pangolin 1.24 及以上（Exit Node） |
| WireGuard 出口 | sing-box 原生 WireGuard | 不需要额外 sidecar，适合自建的普通 WireGuard 服务器 |
| 虚拟网卡驱动 | Wintun | 随 openconnect / Olm 一起打包 |
| 凭据存储 | `keyring` crate | 对应 Windows 凭据管理器 |
| 安装包 | Tauri bundler，NSIS | 安装时以管理员身份写入程序目录 |

**做法 A 与做法 B**：MVP 采用做法 A，即打包 openconnect.exe，由它建立 wintun 网卡，sing-box 的出口绑定这张网卡。做法 B 是像 Bluewhale 那样静态链接 libopenconnect，再接用户态协议栈（smoltcp），对 sing-box 提供本地 SOCKS5。B 更干净，但工作量大得多，列为后续版本。

## 目录结构

```
splitgate/
├─ src/                      # 前端（React + TS）
│  ├─ pages/                 # 连接、规则、连接列表、日志、设置、关于
│  ├─ components/
│  ├─ store/                 # zustand：配置、连接状态、日志缓冲
│  └─ lib/ipc.ts             # 所有 Tauri invoke / listen 的类型封装
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ commands.rs         # 暴露给前端的 command
│  │  ├─ supervisor.rs       # 连接状态机，统一管理两个 sidecar
│  │  ├─ exits/              # ExitAdapter trait 与三种出口：cisco.rs（openconnect）、pangolin.rs（Olm）、wireguard.rs
│  │  ├─ singbox.rs          # 配置生成、启动、Clash API 客户端
│  │  ├─ tunnel_info.rs      # 接收 script 回传的网卡与 DNS 信息
│  │  ├─ credentials.rs      # keyring 封装
│  │  ├─ config.rs           # profiles.json / rules.json 读写与校验
│  │  └─ cleanup.rs          # 启动时清理残留网卡、路由、进程
│  ├─ binaries/              # sidecar：sing-box、openconnect、olm 及其 DLL
│  ├─ resources/
│  │  ├─ oc-script/          # 自定义连接脚本（见组件规格）
│  │  └─ presets/xbox.json   # 内置规则预设
│  └─ tauri.conf.json
├─ third_party/              # 各组件许可证文本与来源说明
├─ docs/SPEC.md              # 本文档的导出
└─ README.md
```

## 连接状态机

supervisor.rs 统一管理状态，前端只显示状态，不自行推断。同一时间只允许一次连接或断开操作，重复点击一律忽略。

&#91;embedded content: 连接状态机 · 7 个状态\]

已连接时任一 sidecar 意外退出，也进入“错误”并先清理；开启自动重连时，清理完成后按指数退避重新进入“认证中”，最多 5 次。

## 组件规格

### 0. 出口适配器（exits/）

三种出口实现同一个 Rust trait，supervisor 和状态机只依赖这个 trait，不关心具体类型：

```rust
#[async_trait]
pub trait ExitAdapter {
    async fn start(&mut self, profile: &Profile, prompts: PromptChannel) -> Result<ExitReady>;
    async fn stop(&mut self) -> Result<()>;
    fn is_alive(&self) -> bool;
}

pub enum ExitReady {
    Interface { ifname: String, dns: Vec<IpAddr> },      // Cisco、Pangolin：sing-box 用 bind_interface
    SingboxEndpoint { config: serde_json::Value },       // WireGuard：直接写进 sing-box endpoints
    Socks { addr: SocketAddr },                          // 后备：WSL2 中的 Pangolin CLI
}
```

singbox.rs 按 `ExitReady` 的种类生成 `vpn` 出口；状态机中的“认证中”“等待网卡”对 WireGuard 直接跳过。目录 `src-tauri/src/exits/` 下分别是 `cisco.rs`、`pangolin.rs`、`wireguard.rs`。

### 1. openconnect 管理（openconnect.rs）

启动参数（示意，以第 0 阶段验证结果为准）：

```
openconnect.exe --protocol=anyconnect
  --user=<user> [--authgroup=<group>]
  --passwd-on-stdin --non-inter
  --interface=SplitGate-OC
  --script=<资源目录>\oc-script\<脚本>
  [--servercert=<pin>]
  <server>
```

- 网卡名固定为 `SplitGate-OC`，sing-box 靠这个名字绑定出口。
- 密码从 keyring 读出，经 stdin 传入，不出现在命令行参数和日志里。
- MFA：Duo push 时 stdin 第二行写 `push`；SAML 列为第 3 阶段，优先评估 `--external-browser`，否则用 Tauri WebView 登录拿 cookie，再走 `--cookie-on-stdin`。
- 首次连接遇到未知服务器证书时，把指纹弹给用户确认，确认后保存为 `servercert` 固定值。
- 逐行解析 stdout/stderr，转成结构化日志事件发给前端。

### 2. 自定义连接脚本（oc-script）

openconnect 默认的 vpnc-script 会改系统默认路由，这正是要避免的。自定义脚本只做两件事：

1. `reason=connect`：给网卡配上 `INTERNAL_IP4_ADDRESS` / `INTERNAL_IP4_NETMASK`，**不加任何路由，不改系统 DNS**；然后把网卡名、IP、`INTERNAL_IP4_DNS`、`CISCO_DEF_DOMAIN`、`CISCO_SPLIT_INC_*` 写成 JSON，交给主程序（写到 `%LOCALAPPDATA%\SplitGate\tunnel.json`，或回连本地命名管道）。
2. `reason=disconnect`：删除 tunnel.json，通知主程序。

脚本语言待第 0 阶段确定：Windows 版 openconnect 如何调用脚本（cscript JS、cmd 还是任意 exe）需要实测。必要时脚本本身就是一个用 Rust 写的小 exe。

### 3. sing-box 配置生成（singbox.rs）

每次连接都由 profile、规则和 tunnel.json 生成 `runtime/singbox.json`，不让用户直接编辑。生成规则：

- `inbounds`：一个 `tun`，开启 `auto_route`、`strict_route`，地址使用私有网段（例如 172.19.0.1/30）。
- `outbounds`：`vpn` 是 `direct` 类型，加 `bind_interface: "SplitGate-OC"`；`direct` 走本地。
- `route.rules`：先 `sniff`，再 `hijack-dns`；接着按 rules.json 生成 `domain_suffix`、`domain_keyword`、`process_name`、`ip_cidr` 等规则，指向 `vpn`；`final` 是 `direct`；开启 `auto_detect_interface`。
- `dns`：两个服务器，`vpn-dns` 使用 tunnel.json 里的 DNS 并 detour 到 `vpn`，`local` 是系统 DNS；命中规则的域名用 `vpn-dns` 解析，其余用 `local`。
- `experimental.clash_api`：只监听 127.0.0.1 的随机端口，并设置随机 secret，供连接页使用。
- 防回环：Cisco 服务器地址必须固定走 `direct`。
- 生成后先运行 `sing-box check -c`，通过才启动。

### 4. 凭据（credentials.rs）

- 服务名 `SplitGate`，账户键为 `<profile_id>:<user>`。
- 删除 profile 时同步删除凭据。
- 日志和错误信息中一律不输出密码、cookie、secret。

### 5. 权限

- TUN 和 wintun 都需要管理员权限。MVP 在应用清单里要求 `requireAdministrator`，启动时弹一次 UAC。
- 后续版本改为安装一个 Windows 服务负责启停核心，界面以普通权限运行，两者通过命名管道通信。

### 6. Pangolin 出口（exits/pangolin.rs）

前提：Pangolin 服务器为 1.24 或以上版本；在 homelab 部署一个 Newt site，创建 Exit Node 类型的私有资源并关联这个 site；在 Pangolin 后台为 SplitGate 创建一个机器客户端，取得 ID、Secret 和 Endpoint。

难点：Olm 选中 Exit Node 后会安装默认路由，变成全隧道，这会和 sing-box 的 TUN 冲突。按以下优先顺序实现：

1. **方案 A（首选）**：以 sidecar 方式运行 Olm（或调用已安装的 Olm 服务），通过它的 HTTP / 命名管道 API 连接 Exit Node。连上后由 SplitGate 删除 Olm 加的默认路由（或在 Olm 选项里关闭），只保留网卡；sing-box 用 `bind_interface` 绑定 Olm 的网卡名。若 Olm 不支持固定网卡名或会把删掉的路由加回来，就 fork Olm（Go），增加“只建隧道不装默认路由”的选项。
2. **方案 B（后备）**：在 WSL2 中运行 Linux 版 Pangolin CLI 并选中 Exit Node，全隧道只发生在 WSL 内；同一环境中运行 gost 提供 SOCKS5（含 UDP），sing-box 用 socks 出口连接。

- Secret 存进 keyring，与 Cisco 密码同样处理。
- 断开时必须确认 Olm 加过的路由全部撤销，纳入 cleanup.rs 的检查范围。
- Pangolin 私有资源的 alias 域名可选加入 DNS 规则，交给 Olm 的隧道 DNS 解析。

### 7. WireGuard 出口（exits/wireguard.rs）

最简单的出口，适合直接在 homelab 的 VPN-Server 上加一个普通 WireGuard 服务。

- 用户导入标准的 `.conf` 文件（或粘贴内容），程序解析成 sing-box 的 `wireguard` endpoint 配置；私钥存进 keyring，不留在 profiles.json。
- 不需要 sidecar、wintun 或管理员额外操作；“连接”等于启动 sing-box。
- 以 `.conf` 里的 `DNS` 作为 `vpn-dns`；`AllowedIPs` 一律忽略，分流完全由 SplitGate 的规则决定。

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
      "ruleset_ids": ["xbox"],
      "cisco": { "server": "vpn.example.com", "user": "zhijie", "authgroup": null, "mfa": "none", "servercert": "pin-sha256:..." }
    },
    {
      "id": "home-pangolin",
      "type": "pangolin",
      "name": "Pangolin 家里出口",
      "ruleset_ids": ["xbox"],
      "pangolin": { "endpoint": "https://pangolin.example.com", "client_id": "31frd0uzbjvp721", "exit_node": "home-exit", "mode": "olm" }
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

`type` 决定使用哪个 ExitAdapter，并只读取同名的子对象。Cisco 的 `mfa` 取值为 `none` / `duo-push` / `saml`；Pangolin 的 `mode` 取值为 `olm` / `wsl-cli`。Cisco 密码、Pangolin secret、WireGuard 私钥都存在 keyring，不写进这个文件。同一时间只有 `active` 指向的出口生效。

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
      "process_name": ["XboxPcApp.exe", "GamingServices.exe", "GamingServicesNet.exe"],
      "ip_cidr": []
    }
  ]
}
```

预设中的进程名需在第 0 阶段用任务管理器核实。

**tunnel.json**：运行时文件，由连接脚本写入，断开即删除

```json
{ "ifname": "SplitGate-OC", "ip4": "10.1.2.3", "netmask": "255.255.255.0", "dns": ["10.0.0.53"], "default_domain": "corp.example.com" }
```

## Tauri IPC 接口

前端只通过 `src/lib/ipc.ts` 调用后端，类型和 Rust 端保持一致（建议用 `specta` 或 `ts-rs` 自动生成）。

**Commands（前端调用）**

| Command | 参数 | 返回 |
| --- | --- | --- |
| `list_profiles` | 无 | Profile\[\] |
| `save_profile` | Profile, password? | 无 |
| `delete_profile` | id | 无 |
| `list_rulesets` | 无 | RuleSet\[\] |
| `save_ruleset` | RuleSet | 无 |
| `reset_ruleset_to_preset` | id | RuleSet |
| `connect` | profile\_id | 无（进度通过事件推送） |
| `disconnect` | 无 | 无 |
| `get_status` | 无 | Status |
| `answer_prompt` | prompt\_id, value | 无（MFA 或证书确认） |
| `get_connections` | 无 | Connection\[\]（经 Clash API） |
| `close_connection` | conn\_id | 无 |
| `export_diagnostics` | 无 | 文件路径（日志打包，已脱敏） |

**Events（后端推送）**

| Event | 内容 |
| --- | --- |
| `status_changed` | 新状态、原因、时间戳 |
| `prompt` | 需要用户输入的内容：证书指纹确认、MFA 选项 |
| `log` | 来源（openconnect / sing-box / app）、级别、文本 |
| `traffic` | 每秒上下行速率，分 vpn 和 direct |

## UI 页面

布局沿用 Bluewhale 的思路：左侧导航，顶部固定一个连接按钮和当前 profile 选择器；默认深色主题，支持跟随系统。界面文字使用简体中文，文案集中放在一个文件里，方便以后加英文。

| 页面 | 内容 | 阶段 |
| --- | --- | --- |
| 连接 | 大号连接按钮、状态、出口 IP、VPN 内网 IP、上下行速率、已连接时长 | MVP |
| 规则 | 规则集列表与开关；编辑域名后缀、关键字、进程名、IP 段；“恢复预设”；从“连接”页一键把某个域名加入规则 | MVP |
| 连接列表 | 实时连接：进程、域名、出口（vpn / direct）、流量；可筛选、可关闭单条连接 | MVP |
| 日志 | 按来源和级别筛选，支持暂停滚动、复制、导出诊断包 | MVP |
| Profile | 先选出口类型，再显示对应表单。Cisco：服务器、账号、密码、authgroup、MFA、证书指纹；Pangolin：Endpoint、ID、Secret、Exit Node；WireGuard：导入 .conf | MVP |
| 设置 | 开机自启、启动后自动连接、主题、sing-box 日志级别、TUN 网段 | MVP |
| 关于 | 版本、组件版本、许可证与第三方声明 | MVP |
| 流量画布 | 用 React Flow 显示“应用 → 规则 → 出口”的可视化图 | 后续 |

**交互要点**

- 连接过程中，按钮显示当前步骤（认证中、等待网卡、启动分流），失败时显示原因和“查看日志”入口。
- 需要 MFA 或确认证书时，弹出模态框，用户回答后调用 `answer_prompt`。
- 系统托盘：图标颜色表示状态；右键菜单提供连接、断开、打开主窗口、退出。关闭窗口时缩到托盘，不断开连接。

## 错误处理与清理

最重要的一条：任何情况下都不能让用户的电脑断网。sing-box 的 TUN 接管了系统路由，如果它残留而 VPN 已断，流量会被导进一个不存在的出口。

| 情况 | 处理 |
| --- | --- |
| openconnect 认证失败 | 停在“已断开”，显示 openconnect 的原始错误，不重试 |
| openconnect 运行中退出 | 先停 sing-box，再按设置决定是否重连（指数退避，最多 5 次） |
| sing-box 启动失败或 `check` 不通过 | 断开 openconnect，显示错误和生成的配置路径 |
| sing-box 运行中退出 | 立刻断开 openconnect，状态改为“错误” |
| 等待网卡超过 30 秒 | 视为失败，按上一行处理 |
| 应用被强制结束或崩溃 | 下次启动时 cleanup.rs 结束残留的 sing-box、openconnect 进程，删除残留的 tunnel.json；检查并移除名为 SplitGate 的 TUN 网卡和对应路由 |
| 系统睡眠后唤醒 | 检测 VPN 是否仍然存活，不存活则走重连流程 |

- 两个 sidecar 都放进同一个 Windows Job Object，并设置 `KILL_ON_JOB_CLOSE`，主进程一退出它们就一起结束。
- 断开顺序固定为：先停 sing-box（恢复系统路由），再断 openconnect。
- 所有用户可见的错误都附带“复制详情”按钮。

## 授权合规

Bluewhale 只用来参考产品形态，不复制它的代码、图标、名称或文案。

| 组件 | 许可证 | 要求 |
| --- | --- | --- |
| SplitGate 自身 | 自定（建议 MIT） | 无 |
| sing-box | GPL-3.0 或更新版本（含附加条款） | 作为独立进程分发；附带许可证文本，注明来源和版本，并提供对应源码链接 |
| openconnect | LGPL-2.1 | 做法 A 分发独立可执行文件即可：附带许可证和源码链接。改成做法 B（静态链接）后，必须允许用户重新链接 |
| Olm（Pangolin） | 待核实（fosrl 的项目可能是 AGPL-3.0） | 作为独立进程分发；如果 fork 修改，必须公开修改后的源码 |
| Wintun | 预编译驱动有自己的分发条款 | 按 wintun.net 的条款随应用分发 |
| 前端依赖 | 多为 MIT | 构建时生成第三方许可证清单 |

以上许可证信息凭经验列出，第 0 阶段需逐一到各项目仓库核实，并把结果写进 `third_party/`。

**使用提醒**：通过他人的 VPN 绕过学校网络限制，可能违反学校或 VPN 提供方的使用政策，使用前请自行确认。

## 开发阶段与验收标准

共分四个阶段，每个阶段结束都要能实际使用，并通过列出的验收项。第 0 阶段的任何一项不通过，都要先回来修改本文的方案，再继续。

### 第 0 阶段：技术验证（只写脚本，不写 UI）

- [ ] 取得 Windows 版 openconnect 和 wintun，确认能用 `--interface` 指定网卡名并连上目标服务器
- [ ] 确认 Windows 版 openconnect 调用 `--script` 的方式，写出“只配 IP、不加路由”的脚本，连接后 `route print` 中默认路由没有变化
- [ ] 选定 sing-box 版本，按本文规则手写一份配置；确认 `bind_interface` 在没有该网卡路由的情况下仍能把流量送进隧道
- [ ] 验证：`curl https://ifconfig.me` 返回本地出口；把 ifconfig.me 加进规则后返回 Cisco 出口
- [ ] 验证：学校 DNS 屏蔽的 Xbox 域名能经 `vpn-dns` 正确解析
- [ ] 用任务管理器核实 Xbox 相关的进程名
- [ ] 核实各组件许可证

**Pangolin 验证项**

- [ ] 确认 Pangolin 服务器版本不低于 1.24，并建好 Exit Node 资源和机器客户端
- [ ] 在 Windows 上运行 Olm 连上 Exit Node，记录它添加的路由、网卡名和 DNS 设置
- [ ] 验证删除 Olm 的默认路由后，隧道仍然可用，且 Olm 不会把路由加回来；不行就评估 fork Olm 或改用方案 B
- [ ] 验证 sing-box `bind_interface` 绑定 Olm 网卡后，规则流量从 Exit Node 出去

**WireGuard 验证项**

- [ ] 用一份标准 `.conf` 手写 sing-box 的 wireguard endpoint，确认能通，且只有规则流量经过它

### 第 1 阶段：MVP

- [ ] Tauri 项目骨架、sidecar 打包、管理员权限清单
- [ ] profile 与凭据管理、规则集管理、内置 Xbox 预设；ExitAdapter trait，实现 Cisco 和 WireGuard 两种出口
- [ ] 连接状态机、配置生成、错误处理与清理
- [ ] 连接页、规则页、日志页、Profile 页、设置页、关于页、系统托盘

**验收**

- [ ] 点一次按钮完成连接，Xbox App 能登录、浏览商店、打开 Game Pass
- [ ] 连接期间，非规则流量的出口 IP 仍是学校网络
- [ ] 在任务管理器中结束 SplitGate 进程后，网络能在 5 秒内恢复正常
- [ ] 断开再连接 20 次，没有残留进程或网卡
- [ ] 日志和诊断包中搜索不到密码

### 第 2 阶段：可观测性

- [ ] 连接列表页（Clash API）、流量统计、从连接列表一键加规则
- [ ] 睡眠唤醒检测与自动重连；Pangolin 出口（按第 0 阶段选定的方案 A 或 B）

### 第 3 阶段：增强

- [ ] Duo push 和 SAML 登录
- [ ] 用 Windows 服务取代整个应用以管理员身份运行
- [ ] React Flow 流量画布
- [ ] 评估做法 B（libopenconnect + smoltcp）

## 待确认事项

- [ ] 项目正式名称（SplitGate 只是暂定）
- [ ] Cisco 服务器的认证方式：只要密码、Duo，还是 SAML？这决定 MFA 是否要提前到 MVP
- [ ] 这个 Cisco VPN 是你自己管理的，还是别人的？如果是自己的 ASA，服务器端的 dynamic split tunneling 可以作为备用方案
- [ ] 是否需要支持多台电脑或便携版（免安装）
- [ ] 代码仓库放在哪里，是否沿用你现有的 git 备份流程

* [ ] Pangolin 是自建还是 Pangolin Cloud？版本是否已到 1.24 以上？
* [ ] 同时有 Pangolin 和 WireGuard 时，家里出口优先用哪一个
