# SplitGate（暂定名）

Windows 上按域名 / 程序分流的 VPN 客户端：只有命中规则的流量走 VPN，其他流量照常走本地网络。

用途举例：在学校网络里，只让 Xbox、Minecraft、Steam 经家里的 VPN 出去，绕过学校防火墙的拦截；上课用的网站、视频等照样走学校网络，不会变慢。

下载：[最新版本](https://github.com/ChongZhiJie0216/SplitGate/releases/latest) · 更新记录：[CHANGELOG.md](CHANGELOG.md)（软件里在“关于”页也看得到）

## 功能

- **出口**：Cisco AnyLink / OpenConnect（账号密码 + 组别）、WireGuard（导入 `.conf`）。Pangolin 是备用出口，排在整个项目的最后。
- **分流规则**：按域名、关键字、程序（完整路径或程序名）、IP 段。内置 Xbox / Minecraft / Steam 预设，可以从正在运行的程序里挑。
- **访问家里内网**：在 Profile 填家里的网段（例如 `192.168.1.0/24`），连上后可以直接打开家里的设备（PVE 等）；和当前网络撞号时会提示。
- **连接列表、流量画布**：实时看每条连接是哪个程序、去哪里、走 VPN 还是直连；画布用图显示“程序 → 规则集 → 出口”。
- **拦截提示**：直连的网站如果被学校防火墙换了证书（Fortinet），会列出来，一键加进规则。
- **稳定**：睡眠唤醒、断网、VPN 断线后会先清理让网络恢复，再自动重新连接（最多 5 次）。SplitGate 被强制结束时，网络也会在几秒内恢复。
- **其他**：系统托盘、开机自启、诊断包（已脱敏）、检查更新、关于页看更新记录。

## 安装和使用

需要：Windows 10 / 11（64 位）、WebView2（Windows 11 自带；Windows 10 没有的话装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)）、管理员权限（建立虚拟网卡要用）。

1. 从 [Releases](https://github.com/ChongZhiJie0216/SplitGate/releases/latest) 下载 `SplitGate-<版本>-portable.zip`，解压到任意文件夹。`SplitGate.exe` 和 `sing-box.exe` 要放在一起。
2. 双击 `SplitGate.exe`，同意管理员权限。
3. **Profile** 页：新建 Cisco 或 WireGuard 出口，勾选要用的规则集；需要访问家里设备的话填“家里内网网段”。
4. 按顶部的 **连接**。显示“已连接”和“经 VPN 测试 xx ms”就好了。

关闭窗口只是缩到托盘，连接不断开；要完全退出，右键托盘图标 → “退出”。开机自启、自动连接、自动重连、检查更新都在“设置”页。

### 设置存在哪里

| 内容 | 位置 |
|---|---|
| Profile、规则、设置（`*.json`）、日志、诊断包 | `SplitGate.exe` 旁边的 `data\` 文件夹 |
| VPN 密码、WireGuard 私钥 | Windows 凭据管理器（不写进任何文件） |
| 界面主题、WebView 缓存 | `%LOCALAPPDATA%\SplitGate\` |
| 开机自启 | Windows 任务计划程序里名为 `SplitGate` 的任务 |

每一份 `SplitGate.exe` 各有自己的 `data\`；搬动整个文件夹，设置就跟着走。v0.11 以前的设置在 `%APPDATA%\SplitGate\`，v0.12 起第一次启动时会自动复制到 `data\`（旧文件夹不动，确认没问题后可以自己删掉）。

### 更新

关于页“检查更新”（启动时也会自动查一次）会显示有没有新版本和更新内容，只提示，不会自动下载或安装。更新方法：

1. 从托盘“退出”SplitGate。
2. 下载新的 zip，用里面的 `SplitGate.exe`、`sing-box.exe`、`licenses\` 覆盖旧的。
3. `data\` 留着，设置就都在。

### 网络出问题时

正常情况下，断开、退出、甚至 SplitGate 被强制结束，网络都会自己恢复。万一没有恢复：

1. 右键托盘图标 → “退出”，再重新打开 SplitGate：它每次启动都会先清理残留的虚拟网卡和路由。
2. 有这个仓库的电脑，也可以用管理员 PowerShell 在仓库根目录执行：`powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1 -Apply`
3. 还是不行就重启电脑。

要问问题时，日志页可以“导出诊断包”：密码、私钥、服务器地址、账号、Windows 用户名都已经换成代号，可以直接发给别人。

## 开发

需要：Node 22+、Rust（MSVC 工具链）、Visual Studio 2022 C++ 生成工具、WebView2。7-Zip 只有重跑第 0 阶段脚本、用 `fetch.ps1 -All` 时才需要。

```
powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1        # 下载并校验 sing-box
powershell -ExecutionPolicy Bypass -File scripts\prepare-sidecar.ps1  # 复制 sing-box 到 src-tauri\binaries
npm install
npx tauri dev                                                         # 开发版（不要求管理员，不能连接）
cd src-tauri; cargo test; cargo clippy --all-targets -- -D warnings   # 测试
npm run build                                                         # 前端检查（含 tsc）
build.bat                                                             # 便携版（双击也可以）
```

`build.bat` 会先跑测试，失败就停；前端依赖有变化时会自动 `npm ci`。输出：

- `portable\SplitGate\`：`SplitGate.exe` + `sing-box.exe` + `licenses\`。重新打包时里面的 `data\` 会保留。
- `portable\SplitGate-<版本>-portable.zip`：发布用，不包含 `data\`。
- 仓库根目录也会复制一份 `SplitGate.exe` 和 `sing-box.exe`（不进 git），方便直接测试。

打包前要先从托盘“退出”SplitGate，否则 exe 被占用，脚本会一开始就停下并提示。

### 发布新版本

1. 改好 `package.json` 的版本号（唯一来源；`src-tauri/Cargo.toml` 要同值）和 `CHANGELOG.md`，合并进 `main`。
2. 在 `main` 上双击 `build.bat`。
3. 先检查：`powershell -ExecutionPolicy Bypass -File scripts\publish-release.ps1`（只显示要做什么）。
4. 再发布：同一个指令加 `-Yes`。需要 [GitHub CLI](https://cli.github.com/)（`gh auth login` 登录过）。Release 的说明取自 CHANGELOG 里这个版本的那一段。

仓库是公开的，Release 谁都看得到：zip 里只有程序和许可证，不会有 `data\`。

### 文档

| 文件 | 内容 |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | 完整规格、开发阶段与验收标准 |
| [docs/progress.md](docs/progress.md) | 开发进度、接着做时先读 |
| [docs/test-plan.md](docs/test-plan.md) | 测试清单 |
| [docs/phase0-findings.md](docs/phase0-findings.md) | 第 0 阶段技术验证的结果 |
| [third_party/VERSIONS.md](third_party/VERSIONS.md) | 第三方组件版本、SHA256、许可证（二进制不进 git） |
| [CLAUDE.md](CLAUDE.md) | 项目规范（也是给 Claude Code 的说明） |

## 许可证与声明

- SplitGate 本身的许可证还没有选定。
- 附带的 [sing-box](https://github.com/SagerNet/sing-box) 是独立程序，GPL-3.0-or-later，许可证在 `licenses\` 文件夹。SplitGate 与 sing-box 项目没有关联。
- 使用提醒：通过 VPN 绕过学校网络限制，可能违反学校或 VPN 提供方的使用政策，使用前请自行确认。
