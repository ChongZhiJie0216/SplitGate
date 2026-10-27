# 开发与发布

给要自己编译、修改或发布 SplitGate 的人看。使用说明见 [README](../README.md)。

## 需要的工具

Node 22+、Rust（MSVC 工具链）、Visual Studio 2022 C++ 生成工具、WebView2。7-Zip 只有重跑第 0 阶段脚本、用 `fetch.ps1 -All` 时才需要。

## 编译和测试

```
powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1        # 下载并校验 sing-box
powershell -ExecutionPolicy Bypass -File scripts\prepare-sidecar.ps1  # 复制 sing-box 到 src-tauri\binaries
npm install
npx tauri dev                                                         # 开发版（不要求管理员，不能连接）
cd src-tauri; cargo test; cargo clippy --all-targets -- -D warnings   # 测试
npm run build                                                         # 前端检查（含 tsc）
build.bat                                                             # 便携版（双击也可以）
```

Windows 专用的代码在 Linux 上也能检查（云端会话用）：`rustup target add x86_64-pc-windows-msvc` 后执行 `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings`（`src-tauri\binaries\` 里要有一个 `sing-box-x86_64-pc-windows-msvc.exe`，空文件也行）。

装了后台服务（`SplitGateService`）、而且它正从仓库根目录或 `portable\SplitGate\` 运行时，`build.bat` 会停下：先在设置页移除服务，或者用管理员执行 `sc.exe stop SplitGateService`。

开发版看界面时，把环境变量 `SPLITGATE_HOME` 指向一个临时文件夹，就不会动到真正的 `data\`。

`build.bat` 会先跑测试，失败就停；前端依赖有变化时会自动 `npm ci`。输出：

- `portable\SplitGate\`：`SplitGate.exe` + `sing-box.exe` + `licenses\`。重新打包时里面的 `data\` 会保留。
- `portable\SplitGate-<版本>-portable.zip`：发布用，不包含 `data\`。
- 仓库根目录也会复制一份 `SplitGate.exe` 和 `sing-box.exe`（不进 git），方便直接测试；它的设置在仓库根目录的 `data\`（也不进 git）。

打包前要先从托盘“退出”SplitGate，否则 exe 被占用，脚本会一开始就停下并提示。

## 发布新版本

1. 改好 `package.json` 的版本号（唯一来源；`src-tauri/Cargo.toml` 要同值，测试会检查）和 `CHANGELOG.md`（第一个版本段落要是这次的版本），合并进 `main`。
2. 在 `main` 上双击 `build.bat`。
3. 先检查：`powershell -ExecutionPolicy Bypass -File scripts\publish-release.ps1`（只显示要做什么）。
4. 再发布：同一个指令加 `-Yes`。需要 [GitHub CLI](https://cli.github.com/)（`gh auth login` 登录过）。Release 的说明取自 CHANGELOG 里这个版本的那一段。

仓库是公开的，Release 谁都看得到：zip 里只有程序和许可证，不会有 `data\`。不要把服务器地址、账号、IP 写进仓库里的任何文件。

## 网络清理脚本

开发和测试时如果网络没恢复：用管理员 PowerShell 在仓库根目录执行 `powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1`（只检查），加 `-Apply` 才真的清理。第 1 阶段的验收脚本在 `phase1\`，用法见 [docs/progress.md](progress.md)。

## 文档

| 文件 | 内容 |
|---|---|
| [docs/SPEC.md](SPEC.md) | 完整规格、开发阶段与验收标准 |
| [docs/progress.md](progress.md) | 开发进度、接着做时先读 |
| [docs/test-plan.md](test-plan.md) | 测试清单 |
| [docs/phase0-findings.md](phase0-findings.md) | 第 0 阶段技术验证的结果 |
| [third_party/VERSIONS.md](../third_party/VERSIONS.md) | 第三方组件版本、SHA256、许可证（二进制不进 git） |
| [CLAUDE.md](../CLAUDE.md) | 项目规范（也是给 Claude Code 的说明） |
| [CHANGELOG.md](../CHANGELOG.md) | 更新记录（软件“关于”页也看得到） |
