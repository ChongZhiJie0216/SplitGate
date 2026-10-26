# 更新记录

版本号格式：`主.次.修订`。唯一来源：`package.json` 的 `version`（`src-tauri/tauri.conf.json` 读取它；`src-tauri/Cargo.toml` 必须同值，测试会检查）。

## 未发布

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
