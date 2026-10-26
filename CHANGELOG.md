# 更新记录

版本号格式：`主.次.修订`。唯一来源：`package.json` 的 `version`（`src-tauri/tauri.conf.json` 读取它；`src-tauri/Cargo.toml` 必须同值，测试会检查）。

## 未发布

## 0.5.0 — 2026-10-26（设置页、系统托盘）

### 新增
- “设置”页：
  - 开机自启：登录 Windows 后在托盘里启动，不弹窗口。用 Windows 任务计划程序以管理员身份启动，不会每次弹出确认框；便携版搬了位置时会提示并可以一键改过来。
  - 启动后自动连接（使用中的 Profile）。
  - 主题（跟随系统 / 亮 / 暗），从顶部栏搬到这里。
  - sing-box 日志级别（info / debug / trace）和 TUN 网段，下一次连接生效；填错会指出哪里不对，不会保存。设置保存在 `%APPDATA%\SplitGate\settings.json`。
- 系统托盘：图标右下角的圆点表示状态（灰 = 已断开、黄 = 连接中、绿 = 已连接、红 = 错误）；右键菜单有连接、断开、打开主窗口、退出。
- “关于”页：加上管理员身份、sing-box 来源与许可证、使用提醒。
- 便携版：`scripts\build-portable.ps1` 先跑测试，再生成 `portable\SplitGate\`（`SplitGate.exe` + `sing-box.exe`）和 zip，不用安装。根目录的 `build.bat` 可以直接双击执行它。

### 改变
- 关闭窗口不再结束程序：窗口缩到托盘，连接不断开。要完全退出，右键托盘图标 →“退出”（会先断开）。
- 同一时间只能开一个 SplitGate：再双击一次会把已经开着的窗口叫出来。

## 0.4.0 — 2026-10-26（连接与分流：Cisco、WireGuard）

### 新增
- “连接”页和顶部的连接按钮：点“连接”后用当前 Profile 的出口分流，只有命中规则集的流量走 VPN，其他照常走本地网络。支持 Cisco 和 WireGuard 出口；Pangolin 第 2 阶段才能连接。
  - 连接过程分步显示：清理上次的残留 → 检查配置 → 启动分流 → 连接 VPN 服务器 → 测试隧道。连接中可以按“取消”。
  - 只有经 VPN 实际测试成功才显示“已连接”，同时显示已连接时间和测试延迟。
  - 常见错误直接说明原因：没有管理员权限、没勾选规则集、没保存密码 / 私钥、Cisco 服务器要求填写组别、WireGuard 握手不成功（学校网络挡 UDP）。
- “日志”页：实时显示 SplitGate 和 sing-box 的日志，可以筛选、暂停滚动、复制。日志文件保存在设置文件夹的 `logs\`，保留 30 天；密码和私钥在写入前会被替换成 `***`。
- 断开和崩溃后的清理：
  - 断开、连接失败、sing-box 意外停止时，自动删除 SplitGate 留下的网卡和路由，并清空 DNS 缓存。
  - SplitGate 被强制结束时，sing-box 跟着结束（Windows Job Object）；下次启动先检查并清理残留。
  - 只清理 SplitGate 自己的 sing-box 和 `SplitGate*` 网卡，不碰 Pangolin 等其他 VPN。
- sing-box 配置在内存里生成，经标准输入交给 sing-box，不写成文件。VPN 服务器本身固定直连，避免绕回 VPN。

## 0.3.0 — 2026-10-26（规则集）

### 新增
- “规则”页：内置 Xbox、Minecraft、Steam 三组预设，可以编辑、关掉、恢复预设（不能删除）；可以新建、删除自己的规则集。
  - 每组可填：域名（含子域名）、域名关键字、程序完整路径、程序名、IP 段，一行一个。
  - 输入会自动整理：`*.Example.COM` → `example.com`，重复的去掉；格式不对的会指出是哪一行。
  - “从正在运行的程序里挑”：列出正在运行的程序和完整路径（Windows 自带的程序不列），一键加入。
- “Profile”页可以勾选这个出口用哪些规则集。
- 规则保存在 `%APPDATA%\SplitGate\rules.json`。删除自己的规则集时，所有 Profile 里对它的勾选也一起去掉。

### 规则
- 内置预设的域名来自第 0 阶段 V5 实测（2026-10-26）：`xboxlive.com`、`xbox.com`、`minecraft.net`、`steampowered.com`、`steamcommunity.com`、`steamserver.net`、`steamstatic.com`、`steamcdn-a.akamaihd.net`（不放整个 `akamaihd.net`）。Xbox 另含原规格的 `xboxservices.com`、`gamepass.com`、`xboxab.com`、`playfabapi.com`（待第 1 阶段验收时核实）。

## 0.2.0 — 2026-10-26（Profile 与凭据管理）

### 新增
- “Profile”页：新建、编辑、删除三种出口（Cisco、WireGuard、Pangolin），并可设为使用中；顶部可以切换当前出口。
  - Cisco：服务器、账号、密码、组别、只用 TCP（默认勾选）、出口网卡。
  - WireGuard：导入 `.conf` 文件或粘贴内容；AllowedIPs 一律忽略。
  - Pangolin：官方客户端的网卡名、规则域名用的 DNS（默认 9.9.9.9）、资源别名后缀；第 2 阶段才能连接。
- 设置保存在 `%APPDATA%\SplitGate\profiles.json`（先写临时文件再改名，保留一份 `.bak`），不含任何密码。
- Cisco 密码、WireGuard 私钥存在 Windows 凭据管理器（`SplitGate` 名下）；改账号时自动搬过去，删 Profile 时一起删除。

### 改变
- 安装包识别名改为 `SplitGate`（维护者定，2026-10-26）。

## 0.1.0 — 2026-10-26（第 1 阶段：项目骨架）

### 新增
- Tauri 2 + React 18 + TypeScript + Vite + zustand 项目骨架：左侧导航（连接、规则、日志、Profile、设置、关于），主题可选自动 / 亮 / 暗。
- “关于”页显示程序版本、sing-box 版本、设置文件夹位置。
- 正式版要求以管理员身份运行；开发版不要求，没有管理员权限时顶部提示“不能连接”。
- sing-box 1.14.4 作为附带程序打包（`scripts/prepare-sidecar.ps1` 从 `third_party/bin` 复制，不进 git）。
