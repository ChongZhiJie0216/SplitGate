# 开发进度记录

> 每次暂停前更新这里；接着做时先读这份，再读 `CLAUDE.md`、`docs/SPEC.md`“开发阶段与验收标准”。
> 最后更新：2026-10-26，版本 v0.4.0（commit `7ae99ab`）。

## 现在在哪一步

**第 1 阶段（MVP），功能大部分写完，还没做第一次真正的连接测试。**

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

### 已验证（不改网络的部分）
- `cargo test` 29 项通过（含用真正的 sing-box 检查生成的 Cisco、WireGuard 配置）；`cargo clippy -D warnings` 0 警告；`npm run build` 通过。
- 非管理员开发版：连接按钮是灰的，直接调用会提示“需要以管理员身份运行”；启动检查显示“没有残留”；日志页正常。
- 便携版 `portable\SplitGate\`：`SplitGate.exe` 4.2 MB（清单要求管理员，属性版本 0.4.0）+ `sing-box.exe` 78.3 MB；zip 28.2 MB。

### 还没有验证
- **真正连接**（Cisco、WireGuard 都还没连过）。状态机的“隧道建立”和“测试隧道”两步只在第 0 阶段用脚本测过，没在程序里跑过。

## 下一步（照顺序）

1. **第一次真正连接测试**（要先得到维护者同意）
   - 开始前列出会改动的东西：建立 `SplitGate-TUN` 网卡、加 strict_route 路由、接管 DNS。
   - 回滚：按“断开” → 关掉 SplitGate 窗口 → 管理员 PowerShell 执行 `phase0\cleanup-all.ps1 -Apply`。
   - 维护者以管理员身份启动（开发版或 `portable\SplitGate\SplitGate.exe`），**自己**在 Profile 页输入 Cisco 密码或导入 WireGuard `.conf`，再点“连接”。
   - 待维护者决定：先测 Cisco 还是 WireGuard。
2. 按 SPEC 第 1 阶段验收逐条测：Xbox App / PrismLauncher / Steam 能登录；非规则流量出口仍是学校网络；任务管理器结束 SplitGate 后 5 秒内恢复；断开再连接 20 次没有残留；日志和 `%APPDATA%\SplitGate\` 搜不到密码；核实 Xbox App 进程名补进预设。
3. 第 1 阶段剩下的页面：设置页、关于页补充内容、系统托盘。
4. 第 1 阶段全部验收后，等维护者确认才进入第 2 阶段（Pangolin、诊断包）。

## 与 SPEC 不完全一样的地方（要告诉维护者）

- SPEC 写“ExitAdapter trait”：目前是 `singbox.rs` 里按出口类型分开生成（`match`），还没抽成 trait；第 2 阶段加 Pangolin 时再整理。
- 便携版是文件夹（SplitGate.exe + sing-box.exe），不是单一 exe：sing-box 本身 78 MB，单一 exe 要每次解压。设置仍在 `%APPDATA%\SplitGate\`。
- Cisco 服务器要求额外登录信息时，目前只提示“请填写组别”，还不能在界面里回答（留到之后）。

## 接着做时要记得

- 提交用 gpg 签名，密码缓存过期会卡住：用 20 秒超时的 job 提交，超时就请维护者自己 `git commit`，之后用 `git log` 核对。
- 开发时看界面：`SPLITGATE_HOME` 指向临时文件夹，`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` 再用 CDP 操作，不动真正的 `%APPDATA%\SplitGate\`。
- 远端推送（origin）由维护者自己决定，目前没推。
