# 开发进度记录

> 每次暂停前更新这里；接着做时先读这份，再读 `CLAUDE.md`、`docs/SPEC.md`“开发阶段与验收标准”。
> 最后更新：2026-10-26，版本 v0.11.1。

## 现在在哪一步

**第 1 阶段验收：不靠学校网络的项目都在手机热点下通过了；剩下要在学校有线网络测的两项（App 登录、非规则流量出口）。** 维护者 2026-10-26 同意学校那两项延后，先做第 2 阶段（连接列表页 v0.6.0、拦截检测 v0.7.0、睡眠唤醒与自动重连 v0.8.0、诊断包 v0.9.0、访问家里内网 v0.10.0 已做，第 2 阶段功能全部完成；Pangolin 移到项目最后）。

## 已完成

| 版本 | commit | 内容 |
|---|---|---|
| — | `6d2021f` | 第 0 阶段全部验证完成（V3b 维护者定跳过），结果在 `docs/phase0-findings.md` |
| — | `f31a342` | SPEC 按实测修订（维护者同意全部 20 条） |
| 0.1.0 | `bc21377` | Tauri 2 项目骨架、sing-box 1.14.4 附带程序、正式版要求管理员、结构测试 |
| 0.2.0 | `df7833b` | Profile 页（Cisco / WireGuard / Pangolin）、`profiles.json`、密码和私钥存 Windows 凭据管理器、导入 WireGuard `.conf` |
| 0.3.0 | `d10daf7` | 规则页、`rules.json`、内置 Xbox / Minecraft / Steam 预设、从运行中的程序挑选 |
| 0.4.0 | `5e830a5` | 生成 sing-box 配置（经 stdin，不落盘）、连接状态机、断开 / 失败 / 崩溃后的清理、Job Object、启动时清残留、连接页、日志页（日志文件保留 30 天，凭据替换成 `***`） |
| 0.4.0 | `7ae99ab` | 便携版打包脚本 `scripts\build-portable.ps1` |
| 0.11.1 | — | 网络变化时（sing-box 日志 “default interface”）马上测出口，30 秒内不通就断线重连；`build.bat` 依赖变化时自动 `npm ci`。测试清单 A8 诊断包维护者实测通过 |
| 0.11.0 | — | “流量画布”页（`@xyflow/react` 12.12.0，程序 → 规则集 → 出口，2 秒刷新，按需加载）。云端用假数据看过亮 / 暗两种主题；**还没在真正连接下看过** |
| 0.10.0 | — | Profile 加“家里内网网段”（`home_cidrs`，一律走出口，只收内网 IPv4 网段）；连接时和本机网段比较，撞号就在连接页显示黄色提示（`Status::Connected.warnings`）。维护者已实测手动 IP 段规则可访问家里设备 |
| 0.9.0 | — | 诊断包（`diagnostics.rs`，日志页“导出诊断包”，PowerShell 打包 zip，密码 / 私钥 / 服务器 / 账号 / Windows 用户名都换掉）；`phase1\secret-scan.ps1` 会解开 zip 再搜。云端用假数据看过界面；**还没在 Windows 上导出过** |
| 0.8.0 | — | 睡眠唤醒检测（时钟跳跃）+ 每 60 秒测出口 + 连接中断后自动重连（2–32 秒退避，最多 5 次，`auto_reconnect` 默认开）；等待重连显示黄色“等待重新连接”。云端用假数据看过界面；**还没实测睡眠唤醒** |
| 0.7.0 | — | 拦截检测（`block_detect.rs`：直连 HTTPS 域名主动握手一次，看证书）+“拦截提示”页（加规则 / 忽略 / 清空、导航显示新发现个数）；修正“加进规则”对话框选项被挤成多行。云端用假数据看过界面；**还没在真正连接下看过** |
| 0.6.1 | — | Xbox 预设补上程序名 `XboxPcApp.exe`、`XboxPcAppFT.exe`（实测核实） |
| 0.6.0 | — | 连接列表页（Clash API，每秒刷新、搜索 / 筛选、关闭单条、一键加规则 + 重新连接）、VPN / 直连速率；`find_process` 一直开。第 2 阶段项目，维护者同意提前做；**还没在真正连接下看过** |
| 0.5.0 | `cb05976` | 设置页（开机自启用任务计划程序、自动连接、主题、sing-box 日志级别、TUN 网段，`settings.json`）、系统托盘（状态圆点 + 右键菜单）、关窗口缩到托盘、只能开一个、关于页补充 |

### 已验证（不改网络的部分）
- v0.5.0：`cargo test` 36 项通过；开发版实测：设置页保存 / 拒绝错误 TUN 网段、关于页、关窗口后进程仍在且窗口隐藏、再开一次只叫出原窗口（仍是 1 个进程），console 无错误。
- v0.4.0：`cargo test` 29 项通过（含用真正的 sing-box 检查生成的 Cisco、WireGuard 配置）；`cargo clippy -D warnings` 0 警告；`npm run build` 通过。
- 非管理员开发版：连接按钮是灰的，直接调用会提示“需要以管理员身份运行”；启动检查显示“没有残留”；日志页正常。
- 便携版 `portable\SplitGate\`：`SplitGate.exe` 4.2 MB（清单要求管理员，属性版本 0.4.0）+ `sing-box.exe` 78.3 MB；zip 28.2 MB。

### 第一次真正连接（2026-10-26，便携版，管理员）
- Cisco（v0.4.0，16:07）：6 秒连上，经 VPN 测试 138 ms；断开 2.7 秒。维护者用 PowerShell 测规则网站都能访问。
- WireGuard（v0.5.0，20:06，手机热点模拟外地）：4 秒连上，经 VPN 测试 351 ms，日志 0 警告 0 错误。
  - 非规则流量出口仍是手机网络；xbox.com、Steam 商店、minecraft.net 经日志确认走 WireGuard，都能访问（minecraft.net 不回应 curl，PowerShell 可以）。IPv6 被拒绝（照设计）。
  - `SplitGate-TUN` 是虚拟网卡，任务管理器“性能”页不显示，属正常；`ncpa.cpl` / `Get-NetAdapter` 可以看到。
  - 断开 1.4 秒；之后没有 sing-box 进程、没有 `SplitGate*` 网卡、没有 172.19 路由，默认路由和 DNS 回到 Wi-Fi，IPv4 / IPv6 都恢复。日志里搜不到私钥。

### 第 1 阶段验收（2026-10-26，v0.6.0 便携版，手机热点 + WireGuard，用 `phase1\` 脚本）
- 断开再连接 20 次：PASS，每次断开后 0.7–1.1 秒没有残留，默认路由和 DNS 回到热点。
- 任务管理器结束 SplitGate：PASS，1.6 秒网络恢复（limit 5 秒）。
- 搜密码：PASS，365 个文件（设置文件夹、`%LOCALAPPDATA%\SplitGate`、便携版文件夹）+ SplitGate / sing-box 命令行 + 任务计划，搜不到 Cisco 密码和 WireGuard 私钥。SplitGate 开着时 WebView2 的 `Cookies` 和几个空 `LOCK` 文件被锁住没搜到。
- Xbox App 进程名：`XboxPcApp.exe`（49 条连接）、`XboxPcAppFT.exe`，v0.6.1 补进预设。
- 之前用清理脚本在**已连接**时结束了 sing-box：SplitGate 断线，之后能重新连接，不是 bug。

### 还没有验证
- 在学校有线网络下：Xbox App（登录、商店、Game Pass）、PrismLauncher、Steam 登录；非规则流量出口仍是学校网络（`phase1\exit-ip.ps1`）。到学校先重新做基准 `phase0\snapshot.ps1 -Baseline`。
- 托盘右键菜单实际点一次；开机自启建立 / 删除、注销再登录在托盘启动。
- v0.6.1 的 Xbox 预设：要在“规则”页对 Xbox 规则集按“恢复预设”才会加上程序名，之后在连接列表里看 Xbox App 的连接是不是走 VPN。

## 下一步（照顺序）

- **测试清单：`docs/test-plan.md`**（PR #2，v0.6.0–v0.11.0；A 部分热点就能测，B 部分要在学校）。全部通过后合并 PR #2，再做 Windows 服务（维护者定：测完再做）。
- **第 3 阶段（维护者 2026-10-26 定进入）**：顺序 openconnect 评估（✅ 不需要，见 SPEC 第 3 阶段）→ React Flow 流量画布 v0.11（✅ 完成，`@xyflow/react` 12.12.0）→ Windows 服务（做法见 SPEC 组件 5，维护者已同意 6 点设计）。Duo / SAML 定跳过。第 2 阶段 v0.7.0–v0.10.0 的 Windows 实测和 PR [#2](https://github.com/ChongZhiJie0216/SplitGate/pull/2) 合并同时进行。
0. **流程（维护者 2026-10-26 定）**：Pangolin 只是备用出口，**移到整个项目的最后**（SPEC“最后：Pangolin”）。做法已定：不用 Exit Node，经 Pangolin 连到家里 VPN-Server 虚拟机上的 SOCKS5 代理；V8 脚本已备好（`phase0/v8-pangolin-socks/`），到时再测。第 2 阶段的“访问家里内网”v0.10.0 完成（Profile 的“家里内网网段” + 撞号检查）。**第 2 阶段功能全部完成，等维护者实测 v0.7.0–v0.10.0 后再进第 3 阶段。**
1. ~~第一次真正连接测试~~（2026-10-26 完成，Cisco 和 WireGuard 都通过，见上）。回滚方法仍是：按“断开” → 托盘“退出” → 管理员 PowerShell 执行 `phase0\cleanup-all.ps1 -Apply`。
2. 按 SPEC 第 1 阶段验收逐条测（2026-10-26 热点下能测的都通过，见上；剩学校网络两项）：Xbox App / PrismLauncher / Steam 能登录；非规则流量出口仍是学校网络；任务管理器结束 SplitGate 后 5 秒内恢复；断开再连接 20 次没有残留；日志和 `%APPDATA%\SplitGate\` 搜不到密码；核实 Xbox App 进程名补进预设。
   辅助脚本在 `phase1\`（2026-10-26 写好，**还没在 Windows 上跑过**，第一次用时留意报错）。除 `kill-recovery.ps1 -KillNow` 会结束 SplitGate 外都只读；回滚一律是 `phase0\cleanup-all.ps1 -Apply`。先在断开状态跑一次 `phase0\snapshot.ps1 -Baseline` 当基准。

   | 验收项 | 脚本 | 怎么用 |
   |---|---|---|
   | 非规则流量出口仍是学校网络 | `exit-ip.ps1` | 断开时跑一次（记下公网 IP），连接后再跑一次，相同就 PASS |
   | 结束 SplitGate 后 5 秒内恢复 | `kill-recovery.ps1` | 连接后启动，再到任务管理器“详细信息”结束 `SplitGate.exe`（或加 `-KillNow`，要管理员）；从进程消失算起，到没有残留且直连 DNS + TCP 成功 |
   | 断开再连接 20 次没有残留 | `reconnect-watch.ps1` | 断开时启动，然后自己按 20 次连接 / 断开；每次断开后 10 秒内要清干净 |
   | 搜不到密码 | `secret-scan.ps1` | 隐藏输入密码 / 私钥（不显示、不落盘），按 UTF-8 和 UTF-16 搜设置文件夹、`%LOCALAPPDATA%\SplitGate`（WebView2 数据）、`-Extra` 指定的文件夹，以及 SplitGate / sing-box 命令行和任务计划 |
   | Xbox App 进程名 | `find-process.ps1` | 打开 Xbox App 进商店后跑；列出名字含 xbox / gaming 的进程和它们的联网数，建议填 `process_name`（商店应用路径带版本号，不能用 `process_path`） |
   | 随时检查残留 | `residue-check.ps1` | 断开后跑，0 = 干净 |

   结果日志在 `phase1\logs\`（不进 git）。
3. ~~第 1 阶段剩下的页面~~（v0.5.0 完成）。还没验证：托盘右键菜单实际点一次、以管理员身份开 / 关开机自启（会建立 / 删除任务计划 `SplitGate`）、注销再登录后是否在托盘启动。
4. ~~第 1 阶段全部验收后才进入第 2 阶段~~：维护者 2026-10-26 同意学校网络两项延后，先做第 2 阶段（连接列表 v0.6.0、拦截检测 v0.7.0、睡眠唤醒与自动重连 v0.8.0、诊断包 v0.9.0、访问家里内网 v0.10.0 已做；接下来等实测，然后第 3 阶段；Pangolin 移到项目最后）。
   v0.10.0 要实测：Profile 填“家里内网网段”，删掉（或取消勾选）之前手动建的规则集，连接后打开 PVE 管理网页；用手机热点时把家里网段故意填成热点的网段（例如 172.20.10.0/28），连接页应出现黄色“注意”提示。
   v0.9.0 要实测：日志页“导出诊断包” → “在文件夹中显示” → 解开 zip 看内容；再跑 `phase1\secret-scan.ps1`（会解开 zip 搜），应该 PASS。
   v0.8.0 要实测（热点 + WireGuard 就可以）：① 连接后让电脑睡眠 1 分钟以上再唤醒：日志应出现“电脑从睡眠中恢复”，接着“睡眠唤醒后 VPN 出口正常”，或者“连接中断”后自动重连成功。② 已连接时用任务管理器只结束 `sing-box.exe`：应显示“等待重新连接”，2 秒后自动连回去。③ 等待时按“停止自动重连”，应变成“已断开”，不再重连。
   v0.7.0 要实测：热点下打开几个网站，“拦截提示”页应该是空的（热点不拦截）；到学校有线 + Cisco 时，暂时把某条规则关掉（例如 Steam），打开 Steam，`store.steampowered.com` 等应该出现在“拦截提示”，原因是“证书被 Fortinet 换掉”。

## 与 SPEC 不完全一样的地方（要告诉维护者）

- 关窗口缩到托盘后，只有托盘菜单“退出”才会断开并结束。删除便携版前，先在设置页关掉开机自启（否则任务计划里会留一个指向旧位置的任务）。

- SPEC 写“ExitAdapter trait”：目前是 `singbox.rs` 里按出口类型分开生成（`match`），还没抽成 trait；第 2 阶段加 Pangolin 时再整理。
- 便携版是文件夹（SplitGate.exe + sing-box.exe），不是单一 exe：sing-box 本身 78 MB，单一 exe 要每次解压。设置仍在 `%APPDATA%\SplitGate\`。
- Cisco 服务器要求额外登录信息时，目前只提示“请填写组别”，还不能在界面里回答（留到之后）。

## 分支

- `main`：v0.5.0（`cb05976`）。
- `claude/modest-bell-xe9osa`（云端会话的分支）：`feature/connections-page` + `phase1\` 验收脚本 + v0.6.1 Xbox 预设 + v0.7.0 拦截检测。维护者用 `git merge --ff-only origin/claude/modest-bell-xe9osa` 拉进 `feature/connections-page`。
- `feature/connections-page`：v0.6.0 连接列表页。**维护者在 Windows 上实测通过后再合并进 `main`**。要测：连接页速率、连接列表（VPN / 直连标示、搜索、关闭单条、加规则 + 现在重新连接）。

## 接着做时要记得

- 编辑含反斜杠的内容（Rust / TS 字符串转义、Windows 路径）用编辑工具，不要用 Bash heredoc 里的 python / sed：本机的 Bash 会吞掉 `\\`。

- 提交用 gpg 签名，密码缓存过期会卡住：用 20 秒超时的 job 提交，超时就请维护者自己 `git commit`，之后用 `git log` 核对。
- 开发时看界面：`SPLITGATE_HOME` 指向临时文件夹，`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` 再用 CDP 操作，不动真正的 `%APPDATA%\SplitGate\`。
- 远端推送（origin）由维护者自己决定，目前没推。
