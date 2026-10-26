# SplitGate（暂定名）

Windows 上按域名 / 程序分流的 VPN 客户端：只有命中规则的流量（Xbox、Minecraft、Steam）走 VPN 出口，其他流量照常走本地网络。

目前在第 1 阶段（MVP 开发中），还不能连接。

- 规格：`docs/SPEC.md`
- 第 0 阶段验证结果：`docs/phase0-findings.md`
- 更新记录：`CHANGELOG.md`
- 第三方组件版本：`third_party/VERSIONS.md`（二进制不进 git，用 `third_party/fetch.ps1` 下载）
- 网络出问题时：用管理员 PowerShell 执行 `powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1 -Apply`

## 开发

需要：Node 22+、Rust（MSVC 工具链）、Visual Studio 2022 C++ 生成工具、WebView2。（7-Zip 只有重跑第 0 阶段脚本、用 `fetch.ps1 -All` 时才需要。）

```
powershell -ExecutionPolicy Bypass -File third_party\fetch.ps1        # 下载并校验 sing-box
powershell -ExecutionPolicy Bypass -File scripts\prepare-sidecar.ps1  # 复制 sing-box 到 src-tauri\binaries
npm install
npx tauri dev                                                         # 开发版（不要求管理员，不能连接）
cd src-tauri; cargo test                                              # 测试
npx tauri build                                                       # 安装包（要求管理员运行）
build.bat                                                             # 便携版（双击也可以；先跑测试，失败就停）
```

便携版输出在 `portable\SplitGate\`（`SplitGate.exe` + `sing-box.exe` + `licenses\`），另有同名 zip。两个 exe 要放在同一个文件夹；双击 `SplitGate.exe` 会要求管理员权限。设置和日志仍保存在 `%APPDATA%\SplitGate\`。

为了方便测试，`SplitGate.exe` 和 `sing-box.exe` 也会复制一份到仓库根目录（不进 git），直接双击根目录的 `SplitGate.exe` 就行。打包前要先从托盘“退出”SplitGate，否则 exe 被占用，脚本会一开始就停下并提示。

关闭窗口只是缩到托盘（连接不断开）；要完全退出，右键托盘图标 →“退出”。开机自启、自动连接等在“设置”页。
