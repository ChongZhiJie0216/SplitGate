# 更新记录

版本号格式：`主.次.修订`。唯一来源：`package.json` 的 `version`（`src-tauri/tauri.conf.json` 读取它；`src-tauri/Cargo.toml` 必须同值，测试会检查）。

## 未发布

## 0.1.0 — 2026-10-26（第 1 阶段：项目骨架）

### 新增
- Tauri 2 + React 18 + TypeScript + Vite + zustand 项目骨架：左侧导航（连接、规则、日志、Profile、设置、关于），主题可选自动 / 亮 / 暗。
- “关于”页显示程序版本、sing-box 版本、设置文件夹位置。
- 正式版要求以管理员身份运行；开发版不要求，没有管理员权限时顶部提示“不能连接”。
- sing-box 1.14.4 作为附带程序打包（`scripts/prepare-sidecar.ps1` 从 `third_party/bin` 复制，不进 git）。
