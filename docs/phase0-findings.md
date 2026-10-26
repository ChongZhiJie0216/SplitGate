# 第 0 阶段验证结果

每一项写明：结果、实际命令、关键输出、和 SPEC 不符的地方。
验证顺序（维护者同意，2026-10-26）：V0 环境 → V1 TUN 全直连 → V2 WireGuard → V3 openconnect → V4 Cisco + sing-box → V5 DNS → V6 Xbox 进程名 → V7 Pangolin。

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