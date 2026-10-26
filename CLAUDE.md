# SplitGate 项目规范

完整规格：@docs/SPEC.md
第 0 阶段结果：@docs/phase0-findings.md

## 本项目例外（维护者定，2026-10-26）

SplitGate 是维护者**自己用的网络工具**，不是给老师用的程序。以下几条不照全局规范：

- 技术栈用 Tauri 2 + React 18 + TypeScript + Vite + zustand（全局规范是“不加前端框架和打包工具”）。
- 交付形式是 Tauri 桌面应用 + NSIS 安装包，需要管理员权限（全局规范是“单一 exe + 黑色命令窗口、免安装”）。
- 不用 `config.ini`；设置存在 `%APPDATA%\SplitGate\*.json`（见 SPEC“数据模型”）。
- 端口：只有 sing-box 的 Clash API，监听 127.0.0.1 的随机端口，不占端口表。

其余照全局规范：中文沟通和中文 commit、版本号只有一个来源、CHANGELOG、不把凭据写进任何文件。

## 开发阶段

- 严格按 SPEC 的“开发阶段”推进。第 0 阶段只写验证脚本（`phase0/`），不搭 Tauri 项目。
- 第 0 阶段完成后，要等维护者确认才能进入第 1 阶段。
- SPEC 里标“待核实”的内容一律以实测为准，结果写进 `docs/phase0-findings.md`。

## 安全

- VPN 密码、Pangolin Secret、WireGuard 私钥：由维护者自己输入，不写进文件、命令行参数、日志、git。
- 会改路由、网卡或 DNS 的命令：先列出命令和回滚方法，等维护者同意再执行。
- 每个验证脚本都有对应的清理脚本；总清理脚本是 `phase0/cleanup-all.ps1`。
- 第三方二进制不进 git：版本和 SHA256 记在 `third_party/VERSIONS.md`，用 `third_party/fetch.ps1` 重新下载。

## 脚本格式

- `phase0/*.ps1` 要兼容 Windows PowerShell 5.1，内容用纯 ASCII（输出的提示用英文），CRLF 换行。
