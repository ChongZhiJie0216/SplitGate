# SplitGate 项目规范

完整规格：@docs/SPEC.md
第 0 阶段结果：@docs/phase0-findings.md
开发进度（接着做时先读）：@docs/progress.md

## 本项目例外（维护者定，2026-10-26）

SplitGate 是维护者**自己用的网络工具**，不是给老师用的程序。以下几条不照全局规范：

- 技术栈用 Tauri 2 + React 18 + TypeScript + Vite + zustand（全局规范是“不加前端框架和打包工具”）。
- 交付形式是 Tauri 桌面应用 + NSIS 安装包，需要管理员权限（全局规范是“单一 exe + 黑色命令窗口、免安装”）。
- 不用 `config.ini`；设置存在 `%APPDATA%\SplitGate\*.json`（见 SPEC“数据模型”）。
- 端口：只有 sing-box 的 Clash API，监听 127.0.0.1 的随机端口，不占端口表。

其余照全局规范：中文沟通和中文 commit、版本号只有一个来源、CHANGELOG、不把凭据写进任何文件。

## 开发阶段

- 严格按 SPEC 的“开发阶段”推进。第 0 阶段已完成（2026-10-26），SPEC 已按实测修订；维护者 2026-10-26 同意进入第 1 阶段。
- 版本号唯一来源：`package.json` 的 `version`；`src-tauri/Cargo.toml` 必须同值（`src-tauri/tests/structure.rs` 检查）。
- 测试：`cd src-tauri; cargo test`、`cargo clippy --all-targets -- -D warnings`；前端 `npm run build`（含 `tsc`）。
- 开发版不要求管理员（看界面用）；正式版清单要求管理员（`src-tauri/build.rs`）。
- SPEC 里标“待核实”的内容一律以实测为准，结果写进 `docs/phase0-findings.md`。

## 安全

- VPN 密码、Pangolin Secret、WireGuard 私钥：由维护者自己输入，不写进文件、命令行参数、日志、git。
- 会改路由、网卡或 DNS 的命令：先列出命令和回滚方法，等维护者同意再执行。
- 每个验证脚本都有对应的清理脚本；总清理脚本是 `phase0/cleanup-all.ps1`。
- 第三方二进制不进 git：版本和 SHA256 记在 `third_party/VERSIONS.md`，用 `third_party/fetch.ps1` 重新下载。

## 脚本格式

- `phase0/*.ps1`、`phase1/*.ps1` 要兼容 Windows PowerShell 5.1，内容用纯 ASCII（输出的提示用英文），CRLF 换行。
- `.bat`（根目录 `build.bat`、`scripts/`）同样纯 ASCII + CRLF，结尾 `pause`，支持 `--no-pause`。

## 全局规范摘要（云端会话读不到维护者电脑上的全局规范，以这里为准）

- **沟通**：一律用简体中文，用词浅白、简洁，不要长篇技术细节。每次改完回报四项：
  1. 改了什么（在哪个页面、哪个功能）；2. 影响（没有影响就写“输出不变”）；3. 你要做什么（重新打包 / 什么都不用做……）；4. 需要你决定的事（没有就不写）。
- 规则有歧义时用实际例子问清楚；影响大的先问，影响小的照最合理的解读做并说明。
- **用词**：大陆简体（文件夹、文件、默认、设置、信息、程序、网络、支持、导出、日志、保存）。
- **版本号**：`主.次.修订`；修 bug 修订 +1，新功能 / 界面改动次版本 +1。只改文档不改版本号。
- **CHANGELOG.md**：中文、写给使用者看，最新在上面，分类只用 新增 / 改变 / 修正 / 规则 / 性能 / 移除。
- **Git**：主分支 `main`；较大的改动开分支（`feature/xxx`、`fix/xxx`），维护者确认后再合并。commit 讯息用中文、一行说清楚。不使用 GitHub Actions。提交前核对版本号和 CHANGELOG 第一条是本次的。
- **完成的定义**：自动测试全部通过、clippy 0 警告、`npm run build` 通过；改到界面要实际看过；更新 CHANGELOG（使用方式变了改 README，结构变了改本文件）。
- **依赖**：新增第三方依赖前先问维护者，说明体积影响。
- 不要擅自删除维护者的文件；维护者的 `todo.md`、`Notes/` 只供参考。

## 云端会话（Claude Code on the web）

- 云端是 Linux：**不能**打包 Windows exe、不能运行程序、不能连接 VPN、不能用 CDP 看界面。能做的：写代码、`npm run build`、改文档；`cargo test` / `clippy` 需要 Tauri 的 Linux 系统库（`libwebkit2gtk-4.1-dev` 等），装不了就只跑前端检查，并在回报里说明没跑后端测试。
- 打包和实测一律由维护者在 Windows 上做：`git pull` → 双击 `build.bat` → 以管理员身份运行根目录的 `SplitGate.exe`（`build.bat` 复制过来的，和 `portable\SplitGate\` 里的相同）。
- 维护者用 gpg 签名 commit；云端提交照云端环境的设置。
- 仓库是**公开**的：不要把服务器地址、账号、IP 等新的个人信息写进文件。
