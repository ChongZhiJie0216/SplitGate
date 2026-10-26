# 第 0 阶段验证结果

每一项写明：结果、实际命令、关键输出、和 SPEC 不符的地方。
验证顺序（维护者同意，2026-10-26）：V0 环境 → V1 TUN 全直连 → V2 WireGuard → V3 openconnect → V4 Cisco + sing-box → V5 DNS → V6 Xbox 进程名 → V7 Pangolin。

顺序调整（维护者定，2026-10-26）：V3a（sing-box 自带 OpenConnect）通过后，**跳过 V3b**（openconnect.exe + 连接脚本）；V4 改为测“VPN 外层连接固定走指定网卡（bind_interface）”。

---

## V0 环境准备 — ✅ 完成（2026-10-26）

### 版本锁定

详见 `third_party/VERSIONS.md`。sing-box 1.14.4、openconnect v9.21、Wintun 0.14.1、Olm 1.10.1。
重新下载并校验：`powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1`。

- sing-box、Wintun、Olm 的 SHA256 和官方公布值一致。
- openconnect 官方**没有发布 Windows 的 Release 文件**，只有 CI 产物。这次用的是 tag `v9.21` 那次 CI 的 `MinGW64/GnuTLS` 任务（job 14877945030）。它是 NSIS 安装包，用 7-Zip 解压，**不运行安装程序**。安装包没有数字签名；GitLab CI 产物可能过期，所以下载的文件要自己留一份备份。
- openconnect 安装包自带的 `wintun.dll` 和 wintun.net 的 0.14.1 amd64 版本相同（SHA256 一致）。

实际输出：

```
sing-box version 1.14.4  (go1.26.10 windows/amd64)
Tags: with_gvisor,with_quic,with_dhcp,with_wireguard,...,with_clash_api,with_tailscale,...,with_openvpn,with_openconnect,...
OpenConnect version v9.21  (GnuTLS 3.8.11)
Default vpnc-script (override with --script): ...\third_party\bin\openconnect\vpnc-script-win.js
wintun.dll FileVersion 0.14.1, signed by WireGuard LLC
olm.exe signed by Fossorial Inc. (Valid)
```

### 许可证

详见 `third_party/VERSIONS.md` 的“许可证”一节。和 SPEC 不同或需要补充的地方：

- sing-box 除了 GPL-3.0-or-later，还有**附加条款**：衍生作品不得使用 sing-box 的名字，也不得暗示和它有关联。SplitGate 这个名字没问题，但界面和文档里不要写成“基于 sing-box 的 XX”这类宣传语（“关于”页按许可证要求注明组件来源可以）。
- Olm 确认是 **AGPL-3.0 + Fossorial Commercial** 双授权。
- Wintun 只能随“只通过 wintun.h API 使用它的软件”一起分发，不能修改。

### 网络快照与清理脚本

- `phase0/snapshot.ps1`：只读，把路由、网卡、DNS、NRPT 保存到 `phase0/logs/`（不进 git）。第一次运行时写 `baseline.json`。
- `phase0/cleanup-all.ps1`：默认是检查模式，只列出要做的事；加 `-Apply`（要管理员）才真的执行。执行内容：结束进程 → 删除 `SplitGate*` 网卡上的路由 → 删除残留的 `SplitGate*` 网卡 → DNS 和 baseline 不同就还原 → 新出现的默认路由和 NRPT 规则只提示、不删 → 删除运行时配置 → 清空 DNS 缓存。
- 约定：SplitGate 建的所有网卡名都以 `SplitGate` 开头（`SplitGate-TUN`、`SplitGate-OC`、`SplitGate-Olm`），清理脚本靠这个前缀找它们。

baseline（2026-10-26 13:03）：

```
0.0.0.0/0  172.20.10.1    Wi-Fi     metric 30
0.0.0.0/0  192.168.0.254  Ethernet  metric 25
::/0       fe80::...      Wi-Fi
所有网卡的 DNS 都是自动获取（没有手动设置），NRPT 规则 0 条
cleanup-all.ps1（检查模式）：nothing left over
```

注意：这台电脑同时连着有线和 Wi-Fi，`172.20.10.x` 看起来是手机热点。V5（学校 DNS 屏蔽）一定要在只连学校网络的时候测。

### V0 期间发现的重要事实（会影响 SPEC）

1. **sing-box 1.14 自带 OpenConnect 客户端**（`endpoints` 的 `type: "openconnect"`，从 1.14.0 开始）。字段有 `server`、`flavor`（默认 anyconnect）、`username`、`password`、`auth_group`、`cookie`、`token`（totp / hotp / stoken / oidc）、`form_entries`、`tls.peer_fingerprint`（支持 `pin-sha256:`），还有 `system`（用系统网卡，默认是用户态网卡）。另外有 `type: "openconnect"` 的 DNS server，可以直接用 VPN 推送的 DNS。
   → 如果能用，Cisco 出口可以和 WireGuard 一样做成 sing-box endpoint：**不需要 openconnect.exe、wintun、oc-script、tunnel.json、bind_interface**。这相当于 SPEC 的“做法 B”，而且不用自己写。V3 会两条路都测。
   来源：https://sing-box.sagernet.org/configuration/endpoint/openconnect/
2. **Windows 版 openconnect 调用 `--script` 的方式**（源码 `script.c`，v9.21）：固定执行 `cscript.exe "<脚本>"`，`.js` 文件会加 `/e:JScript`。所以脚本**必须是 Windows Script Host 脚本**（JScript / VBScript），不能直接指定 exe 或 cmd。要用 exe，就让 JScript 再去调用它。V3 实测确认。
3. **Olm 的 README 写明“Pangolin CLI 正在取代 Olm，Olm 不应作为独立的机器客户端使用”**。Pangolin CLI 0.19.0 已经有 Windows exe（`pangolin-cli_windows_amd64.exe`）。SPEC 的方案 B 原本假设 Pangolin CLI 只能在 WSL2 里跑，现在可能可以直接在 Windows 上跑。
4. Olm 支持用环境变量传参（源码 `config.go`）：`OLM_SECRET`、`OLM_ID`、`PANGOLIN_ENDPOINT`、`INTERFACE`（网卡名）、`DISABLE_ROUTES_AND_ALIASES`、`OVERRIDE_DNS`、`TUNNEL_DNS`、`ENABLE_API`、`HTTP_ADDR` 等。所以 **Secret 可以用环境变量传，不必放在命令行参数里**（官方例子 `olm debug --secret ...` 会让 Secret 出现在进程列表里）。Olm 在 Windows 上改的是**它自己网卡的 DNS**（写注册表），不碰物理网卡。
5. Olm 1.10.1 不加参数运行时会去连 Windows 服务管理器，非管理员会报 `Access is denied`。
6. sing-box 1.14 配置格式（官方 migration 文档）：DNS server 写成 `{"type":"udp","server":"..."}` 这种对象（1.12 起）；`sniff`、`hijack-dns` 是 route rule 的 action（1.11 起）；WireGuard 是 `endpoints`，不再是 outbound（1.11 起）；`route.default_domain_resolver`（1.12 起）。SPEC 第 3 节的描述大致吻合，第 1 阶段生成配置时照新格式写。
7. sing-box 的 WireGuard endpoint 里，peer 的 `allowed_ips` 决定 sing-box 会往隧道送哪些目标地址。要让“规则决定一切”，生成配置时这一项要写 `0.0.0.0/0, ::/0`，不能照抄 `.conf` 里的 AllowedIPs（SPEC 写的“忽略”应改成“固定为全部”）。V2 实测确认。

### 未完成

- Pangolin CLI 0.19.0 的下载和 `--help` 被本机权限设置拦下，留到 V7 再处理。

---

## V1 TUN 全直连 — ✅ 通过（2026-10-26，第 4 次）

文件：`phase0/v1-tun-direct/`（config.json、run.ps1、test.ps1、cleanup.ps1）。
`sing-box check -c phase0\v1-tun-direct\config.json`：通过（exit 0，没有警告）。

### 第 1 次尝试（2026-10-26）— 无效，要重测

- test.ps1 运行时 sing-box 没有在跑：没有 `SplitGate-TUN` 网卡，默认路由和 baseline 一样，`phase0/logs` 里也没有 `v1-before` 快照和 sing-box 日志，说明 run.ps1 在快照之前就退出了（推测是没用管理员身份）。“1.1 秒恢复”不算数。
- 发现本机有 IPv6 出口（ifconfig.me 返回 `2001:d08:…`）。原配置的 TUN 只有 IPv4 地址，IPv6 流量可能绕过分流。
- 修改（第 1 次后）：TUN 加 IPv6 地址 `fdfe:dcba:9876::1/126`（SPEC 第 3 节的 inbounds 也要补上 IPv6）；test.ps1 在 sing-box 没运行或没有网卡时直接报错退出；test.ps1 分别测 IPv4 / IPv6 出口，并列出 TUN 网卡上的路由。

### 第 2 次尝试（2026-10-26 13:15）— 脚本 bug，要重测

- run.ps1 以管理员身份运行，快照正常，sing-box 输出 `inbound/tun[tun-in]: started at SplitGate-TUN` 后脚本立刻中断（`NativeCommandError`）。
- 原因：PowerShell 5.1 里对原生 exe 用 `2>&1`，stderr 的每一行都会变成错误记录；脚本设了 `$ErrorActionPreference = 'Stop'`，第一行日志就让脚本中断，sing-box 也跟着结束。
- 收获：sing-box 1.14.4 能在本机建立 `SplitGate-TUN` 网卡；sing-box 被中断后，网卡和路由都自动撤掉了（cleanup-all 检查结果为干净）。
- 修改：改由 `cmd.exe /c "... 2>&1"` 合并输出，再接 `Tee-Object`；加上 `--disable-color`。已用一个故意失败的命令确认：输出能正确显示和写进日志，退出码也能传回来。
- 第 1 阶段要注意：Rust 端读 sidecar 输出时要同时读 stdout 和 stderr，sing-box 的日志在 stderr。

### 第 3 次尝试（2026-10-26 13:19）— IPv4 通过，IPv6 导致部分网站打不开

结果：

- ✅ sing-box 建立 `SplitGate-TUN`，加了 `0.0.0.0/0` 和 `::/0` 两条默认路由（metric 0，next hop 172.19.0.2 / fdfe:dcba:9876::2），物理网卡的路由保持不变。
- ✅ IPv4：`curl -4 ifconfig.me` 返回本地出口 219.92.246.113，所有连接都走 `outbound/direct`。
- ✅ DNS：系统的 DNS 查询发到 TUN 的 IPv6 地址，被 `hijack-dns` 接管，经 `local` 解析正常。
- ❌ IPv6：所有 IPv6 连接都报 `dial tcp [...]: connectex: A socket operation was attempted to an unreachable network.`；`www.microsoft.com` 测试失败（`web: False`）。
- ⚠️ `-Kill` 那一段的输出不完整（停在 `[kill] Stop-Process -Force sing-box`），恢复时间要重测。测试后 cleanup-all 检查结果为干净，IPv4 / IPv6 都恢复正常。

原因：本机只有 Wi-Fi（手机热点）有 IPv6，有线网卡没有。sing-box 的 `auto_detect_interface` 选了有线（`updated default interface Ethernet, index 9`），所以 IPv6 没有出口。TUN 先接受了 TCP 连接、再断开，程序来不及改用 IPv4（happy eyeballs 不起作用），所以优先走 IPv6 的网站打不开。

这对 SPEC 很重要：**“任何情况下都不能断网”要把 IPv6 考虑进去**。不能假设出口网卡一定有 IPv6。

修改（第 4 次测试用）：

- `dns.strategy: "ipv4_only"`：DNS 只返回 A 记录，程序就不会去连 IPv6。
- route rule `{"ip_version": 6, "action": "reject"}`（放在 `hijack-dns` 后面）：程序直接连 IPv6 地址时立刻拒绝，让它马上改用 IPv4。
- TUN 仍保留 IPv6 地址，这样 IPv6 流量不会绕过 TUN（否则命中规则的域名可能从 IPv6 漏出去，不经过 VPN）。
- `sing-box check`：通过，没有警告。
### 第 4 次（2026-10-26 13:23）— ✅ 通过

- 配置：第 3 次的配置加上 `dns.strategy: ipv4_only` 和 `ip_version: 6 → reject`。
- `SplitGate-TUN` Up；TUN 上有 `0.0.0.0/0` 和 `::/0` 两条路由，物理网卡的路由不变。
- IPv4 出口 219.92.246.113（本地）；IPv6 出口为空（被拒绝，符合预期）。
- `www.microsoft.com`：True；`www.google.com`：True。sing-box 日志里没有 ERROR，所有连接都走 `outbound/direct`。
- 用 `Stop-Process -Force` 结束 sing-box：**0.7 秒后网络恢复**（目标是 5 秒内）。之后 cleanup-all 检查结果为干净：没有残留网卡和路由，DNS 和 baseline 相同。
- 结论：sing-box 进程被强制结束时，Windows 会自动撤掉 TUN 网卡和它的路由。SPEC 的“应用崩溃不能让电脑断网”在 sing-box 这一层成立；cleanup.rs 仍然要做，用来处理 openconnect 或 Olm 留下的东西。
---

## V2 WireGuard — ✅ 通过（经手机热点）；有线网络挡 WireGuard

文件：`phase0/v2-wireguard/`（make-config.ps1、run.ps1、test.ps1、cleanup.ps1）。

- `make-config.ps1` 读取标准 `.conf`（必须放在仓库外面，否则拒绝执行），生成 `phase0/runtime/v2-singbox.json`。文件权限只给当前用户、Administrators 和 SYSTEM。run.ps1 结束时会删除它，cleanup-all 也会删。屏幕上不显示任何密钥。
- 生成规则：WireGuard endpoint `tag: vpn`；peer 的 `allowed_ips` 固定为 `0.0.0.0/0, ::/0`；`.conf` 的 DNS 只取 IP（搜索域名会被忽略）作为 `vpn-dns`（`detour: vpn`）；`ifconfig.me` 走 `vpn` 并用 `vpn-dns` 解析；其余直连。
- 用随机生成的假 `.conf` 测试：`sing-box check` 通过，ACL 正确，放在仓库里的 `.conf` 会被拒绝。
### 第 1 次（2026-10-26 13:28）— ❌ 隧道不通

- `.conf`：Endpoint 是一个 DDNS 域名，端口 51820；Address 10.0.0.2/24；没有 DNS，所以 vpn-dns 改用 1.1.1.1（经隧道）。
- 分流规则 ✅：ifconfig.me 的 DNS 查询照规则走进了 `endpoint/wireguard[vpn]`（`outbound packet connection to 1.1.1.1:53`）。不在规则里的 api.ipify.org 走直连，返回本地出口 219.92.246.113。
- 隧道 ❌：经隧道发出的 DNS 查询 **8.99 秒没有回应**，ifconfig.me 的结果是空的。sing-box 能解析 peer 的域名（第一次用到 vpn 出口时才解析），但 info 级别的日志里没有握手信息。
- 推测原因（待确认）：当前网络挡了 UDP 51820；家里服务器没有在跑；密钥或 peer 公钥不对。
- 结束进程后 5.7 秒恢复：测试循环里每次检查最多等 5 秒，第一次撞上正在关闭的连接就白等了 5 秒。已改成最多等 2 秒，下次重测。
- 设计提示：vpn-dns 不通时，命中规则的域名会**整个打不开**，而不是悄悄改走直连（fail closed）。这符合“规则流量必须走 VPN”。但界面要能看出“隧道不通”，不能只显示“已连接”。第 1 阶段的状态检测要包含“隧道里能不能收到回应”。
- 修改：make-config / run.ps1 加 `-LogLevel` 选项（默认 info），用 `-LogLevel debug` 可以看到握手过程。- 维护者说明（2026-10-26）：学校网络可能挡官方 WireGuard。线索：同一时间这台电脑连家里 IP 的 TCP 8443 是通的（日志里有 `outbound connection to <家里IP>:8443`），所以家里服务器连得到，被挡的可能只是 WireGuard 的 UDP。
- 诊断：make-config / run.ps1 加 `-BindInterface <网卡名>`，让 WireGuard 的 UDP 封包从指定网卡出去（例如手机热点的 `Wi-Fi`），用来对比两条网络。`sing-box check` 通过。
- **对 SPEC 的影响（待确认）**：如果学校挡 WireGuard，那 WireGuard 出口和 **Pangolin 出口（Olm 也是 WireGuard）** 在学校都可能用不了。能在学校用的可能只剩走 TCP/TLS 443 的 Cisco，或者要把 WireGuard 包进 TCP/TLS（需要服务器配合，SPEC 目前没有这一项）。
### 第 2 次（2026-10-26 13:33）— ✅ 通过（WireGuard 封包走 Wi-Fi 手机热点）

命令：`run.ps1 -BindInterface Wi-Fi -LogLevel debug`，`test.ps1 -Kill`

- 握手 ✅：`sending handshake initiation` 之后马上收到 `received handshake response`。第一个规则流量出现时才开始握手，peer 域名也是这时才解析。
- 分流 ✅：`api.ipify.org`（不在规则里）→ 219.92.246.113（本地，经有线）；`ifconfig.me`（在规则里）→ **202.186.213.216（家里出口）**。日志：`router: match[3] domain_suffix=ifconfig.me => route(vpn)`。
- DNS 分流 ✅：`dns: match[0] domain_suffix=ifconfig.me => route(vpn-dns)`，经隧道问 1.1.1.1，134 ms 返回。其余域名走 `local`；AAAA 查询照设置被拒（`strategy rejected`）。
- 其他流量照常直连，`www.microsoft.com`：True。
- 结束进程后 **0.7 秒**恢复网络；cleanup-all 检查结果为干净；run.ps1 退出时已删除运行时配置。

### 结论

- sing-box 1.14.4 的 WireGuard endpoint、`allowed_ips` 全开、按域名分流、vpn-dns 经隧道，都和 SPEC 第 7 节的设想一致。
- **有线网络（第 1 次）握手没有回应，Wi-Fi 热点（第 2 次）马上就有回应**：同一份 .conf、同一个 sing-box，只换了出口网卡。所以是有线网络挡了发往 UDP 51820 的 WireGuard（是挡端口还是按内容识别，还没区分）。同一时间连家里 IP 的 TCP 8443 是通的。
- 对 SPEC 的影响：
  1. 在这个网络上，WireGuard 出口（很可能 Pangolin/Olm 也一样，它也是 WireGuard）不能用；Cisco（TLS over TCP 443/8443）是最有希望的出口。
  2. 第 1 阶段要能清楚提示“出口握手失败”（握手超时 → 状态显示错误，并提示“可能被当前网络阻挡”），不能只看 sing-box 进程还在不在。
  3. 可以考虑在 profile 加“出口封包走指定网卡”（`bind_interface`），用来固定走手机热点。是否加入要等维护者决定。
---

## 维护者提供的环境信息（2026-10-26）

- 有线网络 192.168.0.x 是**学校内网**。所以 V2 的结论是：**学校网络挡 WireGuard（UDP 51820）**。
- Cisco 出口：`sslvpn.jiecloud.net:8443`。服务器是 **AnyLink**（兼容 ocserv 和 AnyConnect 协议，自建在家里），证书由 Let's Encrypt 签发，**只用账号密码登录**（没有 MFA、没有 authgroup）。所以 SPEC 待确认事项里的“MFA 要不要提前到 MVP”：不用。

## 配置走 STDIN，凭据不写进文件（2026-10-26）

- 实测：`sing-box check|run -c stdin` 可以用（报错信息写的是 `decode config at stdin`）。
- 坑：PowerShell 管道和 .NET Framework 的 `Process.StandardInput` 都会在开头加 UTF-8 BOM，sing-box 会报 `invalid character 'ï'`。`phase0/stdin-run.ps1` 的做法：启动前暂时把 `[Console]::InputEncoding` 设成不带 BOM 的 UTF-8，写完就关闭 stdin，再逐行读 stderr，同时显示和写进日志。
- **对 SPEC 的影响**：第 1 阶段的 Rust 端可以把 sing-box 配置直接写进子进程的 stdin。这样密码、WireGuard 私钥都不用写进 `runtime/singbox.json`，只有去掉凭据的版本才需要保存（用来诊断）。SPEC 第 3 节“生成 runtime/singbox.json”要相应修改。

## V3a sing-box 自带 OpenConnect — ✅ 通过（学校网络，有线）

文件：`phase0/v3a-singbox-openconnect/`（run.ps1、test.ps1、cleanup.ps1）；共用检查脚本 `phase0/split-test.ps1`（V2 的 test.ps1 也改成调用它）。

- endpoint：`{"type":"openconnect","tag":"vpn","server":"https://sslvpn.jiecloud.net:8443","username":…,"password":…}`。
- vpn-dns 默认用 `{"type":"openconnect","endpoint":"vpn","accept_default_resolvers":true}`，也就是 VPN 推送的 DNS；也可以用 `-VpnDns <IP>` 改成经隧道的 UDP DNS。`-NoUdp` 可以关闭 DTLS。
- 密码由 `Read-Host -AsSecureString` 输入，只放在内存里，经 stdin 交给 sing-box。
- `run.ps1 -CheckOnly`（用假账号）：两种 DNS 设置都通过 `sing-box check`，`phase0/runtime` 里没有产生任何文件。
### 第 1 次（2026-10-26 13:44）— ❌ 停在“等待认证”

- TUN 正常，非规则流量照常直连（api.ipify.org → 219.92.246.113，www.microsoft.com：True）。
- endpoint 日志：`endpoint/openconnect[vpn]: waiting for authentication`，之后没有任何进展；ifconfig.me 的结果是 `endpoint is not ready yet`。
- 也就是说，**配置里填的 `username` / `password` 没有被自动提交**，sing-box 停下来等人回答服务器发来的登录表单。推测：AnyLink 的登录表单还有其他字段（例如组别选择），或者字段名和 sing-box 自动填写时认的名字不同。
- 源码（v1.14.4 `protocol/openconnect/status.go` 的 `watchAuthForms`）：无法自动完成的表单会变成“auth challenge”，要通过 **sing-box API 服务**（`services: [{"type":"api"}]`，gRPC）由客户端回答。CLI 是 `sing-box api openconnect status|auth|cancel`，用 `BOX_API_URL` / `BOX_API_SECRET` 指定连接哪个实例。
- 结束进程后 12.3 秒才恢复，比 V1/V2 的 0.7 秒慢很多，原因未明，下次重测。
- 修改：run.ps1 加上 API 服务（只听 127.0.0.1，端口和 secret 每次随机生成），启动时显示第二个窗口要用的命令。`-CheckOnly` 通过。
- 对 SPEC 的影响（预计）：如果走自带 OpenConnect，第 1 阶段的 Rust 端要做 API 服务的 gRPC 客户端，用来收表单、填表单（等于 SPEC 的 `prompt` / `answer_prompt` 事件）；或者用 `form_entries` 预先填好。
### 第 2 次（2026-10-26 13:47）— ❌ 同样停在“等待认证”（没有回答表单）

- API 服务正常启动（`service/api[api]: tcp server started at 127.0.0.1:<随机端口>`），但没有在第二个窗口执行 `status` / `auth`，所以表单没人回答。
- 结束进程后 **1.4 秒**恢复上网（第 1 次的 12.3 秒没有再出现，当作偶发）；没有残留。
- 修改：run.ps1 运行期间把 API 地址和本次随机密钥写进 `phase0/runtime/v3a-api.env`（被 .gitignore 忽略，退出时删除，cleanup-all 也会删 runtime 文件）；新增只读的 `status.ps1`，读这个文件后执行 `sing-box api openconnect status`。这个密钥是程序每次随机生成的，不是 VPN 凭据。
### 第 3 次（2026-10-26 13:51）— ✅ 分流成功

- 表单（`sing-box api openconnect auth` 显示）：提示语「请输入你的用户名和密码」，只问了 **GROUP** 一个下拉选项（Master Network [默认] / Test Network / All Tunnel / China Domain / Public Network）。用户名和密码已由配置自动带入，没有再问。选 1 后显示 `vpn: connected`。
  - 结论：前两次卡住是因为 **GROUP 下拉没人选**，不是账号密码的问题。配置加 `auth_group` 就能全自动（run.ps1 新增 `-AuthGroup`，`-CheckOnly` 通过，待实测）。
- 日志：`WARN DTLS unavailable; retrying while CSTP remains active: ... handshake error: context deadline exceeded`，随后 `anyconnect tunnel established using cstp`。**学校网络挡 DTLS（UDP）**，自动退回 TLS（TCP）。从回答表单到 established 大约等了 DTLS 超时的时间；学校网络下建议 `no_udp`。
- `dns/openconnect[vpn-dns]: updated 0 DNS routes and 2 resolvers`：VPN 推送了 2 个 DNS；ifconfig.me 的查询经 `endpoint/openconnect[vpn]` 发到 1.1.1.1:53。
- 测试：api.ipify.org → 219.92.246.113（本地），ifconfig.me → **202.186.213.216（家里）**，www.microsoft.com：True。没有另建 VPN 网卡，默认路由没有变化。
- 结束 sing-box 后恢复时间：**6.0 秒**（目标 5 秒以内）。V3a 四次的数字是 12.3 / 1.4 / 6.0 秒，波动很大，V1/V2 都是 0.7 秒。待查：可能和 OpenConnect 连接中的 TCP 连接、或 Windows 检测网络状态有关。第 1 阶段要有“结束后主动清理 + 检查”的步骤，不能只靠进程退出。
- 残留检查：干净。

### 对 SPEC 的影响

- Cisco 出口可以**不用 openconnect.exe**：sing-box 自带的 OpenConnect endpoint 在 AnyLink（ocserv 兼容）上实测可用，不需要 wintun 网卡、不需要 vpnc-script，也不会改系统路由。
- Profile 要有“组别（auth_group）”和“只用 TCP（no_udp）”两个选项。
- 服务器额外要求的表单（MFA、组别等）通过 sing-box API 服务（gRPC）回答，对应 SPEC 的 prompt 事件。
### 第 4 次（2026-10-26 13:55）— ✅ 全自动连接（`-AuthGroup "Master Network" -NoUdp`）

- 只输入账号密码，没有弹出任何表单；`sing-box started` 和 `anyconnect tunnel established using cstp` 在**同一秒**（13:55:48）。第 3 次因为等 DTLS 超时，慢了大约 50 秒以上。
- 分流结果和第 3 次相同：api.ipify.org → 219.92.246.113，ifconfig.me → 202.186.213.216，网页 True，无残留。
- 结束后恢复时间 **5.1 秒**（V3a 五次：12.3 / 1.4 / 6.0 / 5.1 秒）。仍然待查，第 1 阶段处理。
- 结论：学校网络下的 Cisco profile 默认用 `auth_group` + `no_udp`，就能一键连接。
---

## V3b openconnect.exe + 连接脚本 — ⏭ 跳过（维护者定，2026-10-26）

V3a 已证明 sing-box 自带的 OpenConnect 可用，不需要另建 VPN 网卡和连接脚本。openconnect.exe 的下载记录保留在 VERSIONS.md，作为备用方案。

---

## V4 VPN 外层连接固定走指定网卡 — ✅ 通过

目的：OpenConnect endpoint 加 `bind_interface`（Dial Fields）之后，VPN 自己的封包从指定网卡（Wi-Fi 手机热点）出去；不命中规则的流量照旧走默认网卡（有线）。顺便看热点上 DTLS（UDP）能不能用。

- 脚本：`phase0/v4-bind-interface/run.ps1`（转调 V3a 的 run.ps1，默认 `-BindInterface Wi-Fi -AuthGroup "Master Network"`，**不**加 `-NoUdp`）。V3a 的 run.ps1 新增 `-BindInterface` 参数。
- `test.ps1`：用 `Get-NetTCPConnection` 查 sing-box 到 202.186.213.216:8443 的连接，本地地址必须是 Wi-Fi 的 IP；再从日志看用的是 DTLS 还是 CSTP；最后跑共用的 split-test。
- `-CheckOnly` 通过。
### 第 1 次（2026-10-26 13:59，Wi-Fi，DTLS 开）— 部分通过（测试跑早了）

- **外层连接走 Wi-Fi：通过。** sing-box 到 202.186.213.216:8443 的两条 TCP 连接，本地地址都是 172.20.10.4（Wi-Fi）；默认网卡仍是 Ethernet，api.ipify.org 照旧走本地。
- **热点上 DTLS 也失败**（`DTLS unavailable ... context deadline exceeded`，启动后 16 秒才 `established using cstp`）。学校有线和手机热点都失败，所以多半是**服务器那边没开 UDP**（例如家里路由器只转发了 TCP 8443），不是学校的问题。→ 这个 profile 默认用 `no_udp`。
- ifconfig.me 为空：测试在 13:59:22–34 跑完，隧道 13:59:35 才建立。**在等 DTLS 的这段时间里，命中规则的流量没有任何报错，就是没反应**（日志里连 ifconfig.me 的 DNS 查询都没有记录）。→ 第 1 阶段：状态要等 `established` 才显示“已连接”；连接中命中规则的流量应快速失败，不要让用户以为卡住了。
- 结束后恢复：3.1 秒；无残留。
- 修改：test.ps1 在日志里没有 `tunnel established` 时拒绝运行。
### 第 2 次（2026-10-26 14:01，Wi-Fi，`-NoUdp`）— ✅ 通过

- 外层连接：两条 TCP 连接都是 172.20.10.4（Wi-Fi）→ 202.186.213.216:8443。
- 分流：api.ipify.org → 219.92.246.113（本地，走有线），ifconfig.me → 202.186.213.216（VPN），网页 True。
- 结束后恢复 1.5 秒；无残留。
- 结论：OpenConnect endpoint 的 `bind_interface` 可用，能把 VPN 外层连接固定在指定网卡，不影响其他流量的默认网卡。→ Profile 可以加“出口网卡”选项（留空 = 自动）。

---

## V5 DNS 屏蔽 — 预查（2026-10-26 14:03，只读）

学校有线的 DNS（DHCP 给的）是 **1.1.1.1 / 8.8.8.8**，不是学校自己的 DNS 服务器。直接问 1.1.1.1，以下 Xbox 相关域名**都能正常解析**，结果是微软 / CDN 的 IP，和手机热点的结果同一类：
www.xbox.com、xboxlive.com、user.auth.xboxlive.com、xsts.auth.xboxlive.com、title.mgt.xboxlive.com、login.live.com、assets1.xboxlive.com、xbl.io、gamepass.com、store-images.s-microsoft.com。

→ 至少这些域名，学校**不是靠 DNS 屏蔽**。如果 Xbox 在学校用不了，比较可能是按 IP / SNI 阻挡，或者挡了某些端口（例如 Xbox Live 的 UDP 3074 / Teredo）。SPEC 的 V5 前提（“学校 DNS 屏蔽 Xbox 域名”）要先向维护者确认实际症状。
### 维护者说明的症状（2026-10-26）

Microsoft 账号本身能登录；PrismLauncher（Minecraft 登录）、Steam 这类会被学校的 **Fortinet 防火墙**挡住。

### 阻挡方式探测（2026-10-26 14:06，只读，`phase0/v5-block-probe/probe.ps1`）

同一批网址，分别从有线（学校，192.168.0.x）和 Wi-Fi（手机热点）的地址各做一次 TLS 握手和 HTTPS 请求：

| 结果 | 网址 |
|---|---|
| **有线被拦截**：证书换成 Fortinet 设备自签的（Windows 不信任，curl exit 60）；热点上是正常证书 | user.auth.xboxlive.com、xsts.auth.xboxlive.com、title.mgt.xboxlive.com、www.xbox.com、session.minecraft.net、textures.minecraft.net、libraries.minecraft.net、resources.download.minecraft.net、store / login / api.steampowered.com、steamcommunity.com |
| 两边都正常 | login.live.com、login.microsoftonline.com、api.minecraftservices.com、sessionserver.mojang.com、piston-meta.mojang.com、steamcdn-a.akamaihd.net、www.microsoft.com、www.google.com |

结论：
- 学校用的是 **FortiGate SSL 深度检测 + 按网址分类拦截**：DNS 正常、TCP 正常，TLS 握手时按 SNI 把证书换掉。这正好解释“Microsoft 能登录，PrismLauncher 不能”：Microsoft 登录（login.live.com）没被挡，但下一步的 Xbox Live 认证（user.auth / xsts.auth.xboxlive.com）被挡了。
- 对 SPEC 的影响：V5 的重点**不是 DNS**，而是**按域名（SNI）分流**。sing-box 的 `sniff` + `domain_suffix` 正好可以处理；vpn-dns 不是必须的（但保留，避免以后学校改用 DNS 屏蔽）。
- 内置规则集的第一版至少要有：`xboxlive.com`、`xbox.com`、`minecraft.net`、`steampowered.com`、`steamcommunity.com`（加上 Steam 客户端用的 `steamserver.net`、`steamstatic.com`，待实测）。
- 判断“被拦截”的方法可以做进程序：证书签发者是 Fortinet / 证书链不受信任 → 提示“这个网址被当前网络拦截，建议加进规则”。

### V5 验证准备

- `phase0/v5-block-probe/run.ps1`：调用 V3a 的 run.ps1，只把上表中被拦截的域名（加上 ifconfig.me）送进 VPN，`-AuthGroup "Master Network" -NoUdp`。`-CheckOnly` 通过。
- `probe.ps1 -ViaTun`：经过 sing-box 再探测一次。预期：被拦截的网址变成正常证书，其他网址结果不变。
### 第 1 次（2026-10-26 14:09，有线，`-NoUdp`）— ✅ 通过

- `probe.ps1 -ViaTun`：上表中**所有被拦截的网址都变成正常证书**（Microsoft / Let's Encrypt），HTTP 有正常回应。没在规则里的网址结果不变。
- sing-box 日志：规则里的域名（steampowered.com、steamcommunity.com、xbox.com、xboxlive.com……）都是 `endpoint/openconnect[vpn]: outbound connection`；其他都是 `outbound/direct`。DNS 用本地解析（学校没有在 DNS 上做手脚，见上）。
- 两个**直连**网址出现异常，和 VPN 无关：
  - `steamcdn-a.akamaihd.net`：.NET 握手拿到 DigiCert 证书，但 curl 连试 3 次都是 `SEC_E_UNTRUSTED_ROOT`（证书链不受信任）。14:06 直连时 curl 是 200。推测 Fortinet 对这个 CDN 的拦截不是每次都一样（可能和 TLS 指纹 / ALPN 有关，或是 Fortinet 看到 Steam 流量后才开始拦）。→ 规则要加 `steamcdn-a.akamaihd.net`（只加这个主机名，**不要**加整个 akamaihd.net，太多别的网站也用它）。
  - `www.microsoft.com`：一次超时（curl exit 28），之后连试 3 次都是 200。当作偶发。
- 对 SPEC 的影响：只靠一份固定的规则清单不够，因为学校的拦截会变。程序要有“拦截检测”：看到证书签发者是 Fortinet / 证书链不受信任，就提示“这个网址被当前网络拦截，要不要加进规则”。
### 实际登录（2026-10-26，维护者测）— ✅ 成功

开着 V5 的 run.ps1（规则已含 steamcdn-a.akamaihd.net），**PrismLauncher 登录 Microsoft 账号成功，Steam 登录成功**。

**V5 结论：** 学校 Fortinet 的拦截可以靠“按域名（SNI）送进 Cisco 出口”绕过；DNS 不用特别处理。

---

## V6 按程序分流 — ✅ 通过

本机看到的程序（2026-10-26）：`prismlauncher.exe`（D:\PrismLauncher\），Minecraft 本体是 `javaw.exe`（PrismLauncher 自带的 Java，D:\PrismLauncher\java\...）。`javaw.exe` 这个名字很常见（别的 Java 程序也叫这个），所以**规则要支持完整路径**（sing-box `process_path`），不能只靠程序名。

- V3a 的 run.ps1 新增 `-VpnProcesses`（程序名，sing-box `process_name`）和 `-VpnProcessPaths`（完整路径，`process_path`），并开 `route.find_process`。规则顺序：sniff → hijack-dns → IPv6 reject → **程序规则** → 域名规则。
- 修正：`powershell -File` 会把多个值合成一个逗号分隔的字符串，脚本里拆回列表。
- `phase0/v6-process-rules/run.ps1`：默认只把 `curl.exe` 的流量全部送进 VPN。
- `test.ps1`：同一个网址 api.ipify.org（不在域名规则里），curl.exe 取得的 IP 应该是 VPN 的，PowerShell 自己请求取得的应该是本地的；再从日志看两者各走了哪个出口。
- `-CheckOnly` 通过（含 PrismLauncher 两个路径）。
### 第 1 次（2026-10-26 14:17，有线，`-NoUdp`，规则：curl.exe + PrismLauncher 两个路径）— ✅ 通过

- 同一个网址 api.ipify.org：curl.exe → **202.186.213.216（VPN）**，powershell.exe → **219.92.246.113（本地）**。
- 日志：14:17:16 `endpoint/openconnect[vpn]: outbound connection to 172.67.74.152:443`（curl），14:17:17 `outbound/direct[direct]` 同一个 IP（PowerShell）。同一个目标 IP，按程序分到不同出口。
- 结论：Windows 上 sing-box 的 `process_name` / `process_path` 可用（TUN 模式下能认出连接是哪个程序发的）。
- 修正：test.ps1 原本在 sing-box 还开着的时候做残留检查，列出的都是正在使用的网卡和路由，不是残留。已拿掉，改成提示关闭后再检查。
### 实际登录（2026-10-26，维护者测）— ✅ 成功

规则里**只有程序路径**（prismlauncher.exe + 自带 javaw.exe），没有任何 Xbox / Minecraft 域名，PrismLauncher 退出后重新登录 Microsoft 账号成功。

**V6 结论：** 按程序分流可以取代一长串域名，学校新增拦截的网址也不受影响。对 SPEC：
- 规则要支持 `process_path`（完整路径），不能只有 `process_name`；`javaw.exe` 这种通用名字只靠名字会误伤别的程序。
- 界面上“加程序”最好让用户从正在运行的程序里挑，自动填完整路径（Minecraft 本体是 PrismLauncher 启动后才出现的 javaw.exe）。
- Xbox App 的程序名（XboxPcApp.exe、gamingservices.exe 等）本机没有安装，没测；先用域名规则。
---

## V7 Pangolin — ⚠ 方案 A（独立运行 Olm）在 Windows 上不符合安全规则，暂停，等维护者决定

维护者信息（2026-10-26）：服务器 `pangolin.jiecloud.net`，Pangolin **1.24.0**；上星期在学校用官方客户端能连通。本机已安装官方 Pangolin Windows 客户端（服务 `PangolinManager` 运行中，隧道服务 `PangolinTunnel$olm` 平时停止；配置在 `C:\ProgramData\pangolin\`、`%LOCALAPPDATA%\pangolin\`，**没有读取内容**）。

读 Olm 1.10.1 源码（`main.go`、`config.go`、`service_windows.go`）发现：

1. **Windows 上没有真正的“控制台模式”**：除了 install / remove / start 等子命令，其余任何参数都走 `default` 分支 → 服务 `OlmWireguardService` 没装就**自动安装**（LocalSystem，手动启动），然后通过服务管理器把服务启动起来。也就是说 olm.exe 只是“遥控器”，真正跑隧道的是**另一个服务进程**。
2. 所以通过环境变量传 `OLM_SECRET` **传不到**服务进程；只能用 `--secret` 参数。而参数会被：
   - 写进 `C:\ProgramData\olm\service_args.json`（权限 0644，所有人可读）；
   - 写进 Windows 事件日志（`Service Execute called with args: ...`、`Final args to use: ...`）。
3. 另外，每次启动都把完整配置（含 secret）写进 `%PROGRAMDATA%\olm\olm-client\config.json`（可以用 `CONFIG_FILE` 指向不存在的文件夹来避免，这一条有解）。
4. `OverrideDNS` 默认是**开**（会改系统 DNS），环境变量 `OVERRIDE_DNS=false` 无效，只能用 `--override-dns=false`。
5. Exit Node 在 1.10.1 里只给服务器 IP 加 /32 路由；“全隧道”来自 **gateway site**（源码注释：gateway routes 永远会装，`--disable-routes-and-aliases` 也关不掉）。SPEC 对 Exit Node 的描述要按这个改。

→ 违反“凭据不写进文件、命令行参数、日志”，**没有让维护者运行**。我自己只跑过一次不带管理员的 `--show-config` 检查：Olm 在查询服务状态时就因权限不足退出，**没有装服务、没有建 `ProgramData\olm`**（已确认）。写好的 run-olm.ps1 已删除；`observe.ps1`（只读，查看网卡 / 路由 / DNS / 出口 IP）保留，改成通用。

可选方向（等维护者决定）：
- **A. 用已安装的官方 Pangolin 客户端**：由它负责登录和凭据，SplitGate 不碰 Secret；连上后用 observe.ps1 看它的网卡和路由，再让 sing-box `bind_interface` 到那张网卡。风险：如果它装了全隧道默认路由，会和 sing-box 的 TUN 抢流量。
- **B. 自己改 Olm（fork）**：加一个真正的控制台模式（不装服务、secret 只从环境变量读）。需要装 Go 编译，属于新增依赖。
- **C. 评估 Pangolin CLI 0.19.0**：之前下载被权限拦下，可由维护者手动下载；要先查它在 Windows 上是不是也走服务。
- **D. SPEC 方案 B（WSL2 + Linux 版客户端 + SOCKS5）**。