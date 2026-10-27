# 开发进度记录

> 每次暂停前更新这里；接着做时先读这份，再读 `CLAUDE.md`、`docs/SPEC.md`“开发阶段与验收标准”。
> 最后更新：2026-10-27，版本 v0.22.0。

## 现在在哪一步

**项目功能全部完成。** 最后一项 Pangolin 出口（v0.21.0）维护者 2026-10-27 实测通过（`docs/test-plan.md` I），等合并和发布。

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
| 0.22.0 | — | 界面打磨（维护者 2026-10-27 定：自己画 SVG、不加依赖、布局不变）：`components/Icon.tsx`（28 个线条图标）、`Logo.tsx`、`Empty.tsx`；导航图标、连接页状态卡（圆环 + 小格子）、按钮图标、空白页、输入框焦点样式；关于页重排，更新记录改成右上角图标弹窗（维护者要求）。新程序图标（维护者选方案 B），原稿 `src-tauri/icons/source.svg`，`npx tauri icon` 生成各尺寸。云端用假数据看过亮 / 暗主题。**还没在 Windows 上看过** |
| 0.21.0 | — | Pangolin 出口（第 0 阶段 V8 2026-10-27 通过）：`exits/pangolin.rs` = sing-box `socks` 出口 + `bind_interface`，vpn-dns TCP 经它，local DNS 用物理网卡的 DNS；连接前只读检查网卡和到代理的路由；代理密码存凭据管理器 `<id>:pangolin-proxy`；Profile 页新表单；诊断包脱敏代理 IP。云端用 sing-box 1.14.4（Linux）`check` 过生成的配置。维护者 2026-10-27 实测 I 部分全部通过 |
| 0.20.0 | — | 后台服务改从 `%ProgramFiles%\SplitGate\` 运行自己的一份（`--app` 指回设置所在的那一份；版本不同 / 旧装法时设置页提示）；服务设失败恢复（5 秒后重启，启动清理恢复网络）；旧服务的说明文字会更新；清理认得两个位置的 sing-box；出口代码整理成 `exits/` 的 `ExitAdapter`；`progress.md` 整理。维护者 2026-10-27 实测 H 部分通过 |
| 0.19.0 | — | 在线升级（便携版，维护者 2026-10-27 定）：`updater.rs` 下载 Release zip + `.sha256` 核对 → 改名 `*.old` 换文件（失败换回）→ 有服务时停 / 启（非管理员经 `elevate.rs` `apply-update`）→ `--wait-pid` 重启，新版本删旧文件；正在连接时按钮写“断开连接并更新”；`build.bat` 生成 `.sha256`、`publish-release.ps1` 上传；测试用 `SPLITGATE_PRETEND_VERSION`。维护者 2026-10-27 实测 G 部分全部通过 |
| 0.18.0 | — | 后台服务第 3 步：清单改 `asInvoker`；`elevate.rs`：要管理员的单个步骤（安装 / 移除服务、开机自启任务）以 `--admin-task` 临时提权（UAC），“以管理员身份重新启动”（`--wait-pid`）；开机自启有服务时普通权限（`LeastPrivilege`）、没有时最高权限，和现状不符时设置页提示；不是管理员又没有服务时顶部提示两条路；托盘“连接”不再因为不是管理员变灰。维护者 2026-10-27 实测 F 部分全部通过 |
| 0.17.2 | — | （维护者 2026-10-27 实测 E 部分全部通过）服务被强行结束时界面等 5 秒自己清理残留；sing-box 进 Job Object 要核对（`IsProcessInJob`），启动时记 pid；服务日志记连接 / 断开请求、界面关掉后断开的结果、启动清理、panic 原因。测试文档改用 `sc.exe`（PowerShell 的 `sc` 是 Set-Content） |
| 0.17.1 | — | 修正 v0.17.0“后台服务没有回应”（同步管道上读写线程互等）：拆成命令连接 + 事件连接，每条连接一个线程；经服务连接失败也写日志；托盘连接 / 断开移出主线程 |
| 0.17.0 | — | 后台服务第 2 步：状态机在服务里跑（`service.rs` hello / connect / disconnect + 事件、`remote.rs` 界面端、`backend.rs` 选择）；防冒充核对管道对方进程编号；界面断开管道 → 服务断开清理；Clash API 交给界面读。2026-10-27 实测通过（v0.17.2），见 `docs/test-plan.md` E |
| 0.16.0 | — | （维护者 2026-10-26 实测可用）Cisco：从服务器读组别改成下拉选择、测试登录、保存前先测试（`exits/cisco.rs`，AnyConnect XML init / auth-reply，PowerShell + stdin 传密码）。维护者 2026-10-26 要求，插在 Windows 服务第 2 步之前（服务第 2、3 步顺延 v0.17.0、v0.18.0）。**还没对真正的 AnyLink 服务器试过**，测试见 `docs/test-plan.md` D |
| 0.15.0 | — | （维护者 2026-10-26 实测 C 部分通过）后台服务第一步（`service.rs`：`--service` 服务模式、命名管道 ping、设置页安装 / 启动 / 移除 / 改成这一个）；build.bat 检查服务是否在跑；云端开始做 Windows 版 clippy（`--target x86_64-pc-windows-msvc`）。**还没在 Windows 上跑过**，测试见 `docs/test-plan.md` C |
| 0.14.0 | — | 关于页“更新记录”（`CHANGELOG.md` 打包时 `include_str!` 编进程序，`get_changelog`，当前版本默认展开）；README 改成使用说明，开发与发布搬到 `docs/development.md`；Profile 页 Pangolin 说明改正 |
| 0.13.0 | — | 检查更新（`update.rs`：GitHub 最新 Release + 那个 tag 的 CHANGELOG；关于页按钮 + 启动时检查，设置可关；导航提示点）；`scripts\publish-release.ps1`。维护者定：会在别的电脑上用，在线升级排在 Windows 服务后。云端用假数据看过界面 |
| 0.12.0 | — | 设置 / 日志 / 诊断包改放 `SplitGate.exe` 旁的 `data\`（维护者定），启动时从 `%APPDATA%\SplitGate` 复制缺少的三个设置文件；打包保留 `data\`、zip 不含；phase1 脚本跟着改。**还没在 Windows 上跑过** |
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
- Pangolin 出口在学校网络下能不能用（还没确认测过）。

## 下一步（照顺序）

1. 合并 v0.21.0 的 PR、发布 v0.21.0。（v0.20.0 发布后，从 v0.19.0 真正在线更新一次：H 最后一项。）
2. 学校网络 B 部分维护者 2026-10-27 实测全部通过。有机会再看看 Pangolin 在学校网络下能不能用。
3. SPEC“待决定的事”只剩项目正式名称。（已答：Cisco VPN 自己管理；家里路由器转发 UDP 8443 给 AnyLink；Pangolin 和 WireGuard 都留、Profile 页自己选。维护者 2026-10-27。）

回滚方法一律是：按“断开” → 托盘“退出” → 管理员 PowerShell 执行 `phase0\cleanup-all.ps1 -Apply`。验收辅助脚本在 `phase1\`（用法见 `docs/test-plan.md`）；结果日志在 `phase1\logs\`（不进 git）。

## 与 SPEC 不完全一样的地方（要告诉维护者）

- 关窗口缩到托盘后，只有托盘菜单“退出”才会断开并结束。删除便携版前，先在设置页关掉开机自启、移除后台服务（否则任务计划和服务里会留下指向旧位置的项目）。
- 便携版是文件夹（SplitGate.exe + sing-box.exe），不是单一 exe：sing-box 本身 78 MB，单一 exe 要每次解压。设置在程序旁边的 `data\`（v0.12.0 起）。后台服务用的那一份在 `C:\Program Files\SplitGate\`（v0.20.0 起）。
- Cisco 服务器要求额外登录信息时，只提示“请填写组别”，还不能在界面里回答（Duo / SAML 维护者定跳过）。
- 在线更新的 `.sha256` 只防下载损坏，不是签名（维护者定不做安装包和签名）。

## 分支

- `main`：已合并到 v0.19.0（PR #1–#7），v0.19.0 已发布。
- `claude/modest-bell-xe9osa`（云端会话的分支）：每个 PR 合并后从最新的 `main` 重新开始。维护者测试时：`git fetch origin claude/modest-bell-xe9osa` → `git checkout -B test/<版本> origin/claude/modest-bell-xe9osa`，之后 `git merge --ff-only origin/claude/modest-bell-xe9osa` 拉新的修改；合并后回 `main` `git pull`。

## 接着做时要记得

- 云端（Linux）也能检查 Windows 代码：`cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings`（见 `docs/development.md`）；`cleanup::tests::check_mode_runs` 要 Windows PowerShell，云端必定失败。
- 界面在云端用 Playwright + 假的 `window.__TAURI_INTERNALS__` 截图看。
- 在 PowerShell 里停 / 启服务要写 `sc.exe`（`sc` 是 Set-Content）；读日志加 `-Encoding UTF8`。
- 维护者本机编辑含反斜杠的内容（Rust / TS 字符串转义、Windows 路径）用编辑工具，不要用 Bash heredoc 里的 python / sed：本机的 Bash 会吞掉 `\\`。
- 维护者用 gpg 签名 commit，密码缓存过期会卡住：用 20 秒超时的 job 提交，超时就请维护者自己 `git commit`。
