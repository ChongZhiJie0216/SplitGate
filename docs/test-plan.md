# 测试清单：PR #2（v0.6.0–v0.11.0）

> 2026-10-26 写。全部通过后合并 PR #2，再开始 Windows 服务。现在是 v0.11.1（A7 断网检测改进）。
> 打 `[x]` 记录结果；不通过的写一句发生了什么，连同日志 / 截图发给 Claude。
> 指令都在**管理员 PowerShell**、仓库根目录下执行。

## 0. 准备（每次换版本都做）

- [x] 拉下最新版：`git fetch origin claude/modest-bell-xe9osa` → `git merge --ff-only origin/claude/modest-bell-xe9osa`
- [x] 从托盘“退出”SplitGate，双击 `build.bat`。第一次会自动执行 `npm ci`（装 `@xyflow/react`），最后显示 `[ok] SplitGate v0.11.1`
- [x] 以管理员身份运行根目录的 `SplitGate.exe`，关于页显示 v0.11.1
- [ ] 连上手机热点，SplitGate **已断开**时记基准：`powershell -ExecutionPolicy Bypass -File phase0\snapshot.ps1 -Baseline`

出问题时的回滚：按“断开” → 托盘“退出” → `powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1 -Apply`。

## A. 热点就能测（WireGuard 出口）

### A1. 基本回归

- [ ] 连接 WireGuard：几秒内显示“已连接”，有“经 VPN 测试 xx ms”
- [ ] 断开后 `powershell -ExecutionPolicy Bypass -File phase1\residue-check.ps1` 显示 `[clean]`

### A2. 连接列表（v0.6.0）

- [ ] 连接页显示 VPN / 直连速率，打开网页时数字会动
- [ ] 连接列表有程序名、目标、出口（VPN 是绿色标签）、命中规则
- [ ] 搜索框、“只看 VPN / 只看直连”、“暂停刷新”都有效
- [ ] 对一条连接按“关闭”，它从列表消失
- [ ] 对一条直连按“加规则” → 选上一级域名 → 加入 → “现在重新连接”，重连后这个域名变成 VPN

### A3. Xbox 预设（v0.6.1）

- [ ] 规则页选 Xbox 规则集 → “恢复预设” → 程序名里有 `XboxPcApp.exe`、`XboxPcAppFT.exe`
- [ ] 连接后打开 Xbox App，连接列表里 `XboxPcApp.exe` 的连接是 VPN

### A4. 家里内网（v0.10.0）

- [ ] Profile 页“家里内网网段”填家里的网段（例如 `192.168.1.0/24`），保存
- [ ] 取消勾选之前手动建的“家里内网”规则集，连接后打开 PVE 管理网页（`https://<PVE IP>:8006`），能打开
- [ ] 填错会被拒绝并说明原因：`8.8.8.0/24`（不是内网）、`192.168.1.5/24`（会提示改成 `192.168.1.0/24`）
- [ ] 撞号提示：把家里网段暂时改成热点的 `172.20.10.0/28`，连接后连接页出现黄色“注意”。**测完改回来**

### A5. 流量画布（v0.11.0）

- [ ] 连接后同时开 Xbox App、Steam、PVE 网页，“流量画布”分三列：程序 → 规则集（Xbox / Steam / 家里内网）→ VPN；其他网页 → 没命中规则 → 直连
- [ ] 走 VPN 的线是绿色、会动；“只看走 VPN 的”有效；滚轮缩放、拖动正常
- [ ] 设置页切换亮 / 暗主题，画布颜色跟着变
- [ ] 断开后画布显示“连接之后……”的说明

### A6. 拦截检测（v0.7.0，热点部分）

- [ ] 连接后随便开 5–10 个网站，“拦截提示”页应该**一直是空的**（热点不拦截；有东西 = 误报，截图）
- [ ] 日志里**没有**“拦截检测无法执行”

### A7. 自动重连（v0.8.0；第 ② 项要 v0.11.1）— 前四项 2026-10-26 维护者实测通过（v0.11.1）

关掉 Wi-Fi 后，Windows 右下角的网络图标会从“无线”变成“有线”：那是 SplitGate-TUN 虚拟网卡，正常现象。

- [x] 已连接时，任务管理器 →“详细信息”只结束 `sing-box.exe`（不要结束 SplitGate）：连接页变成黄色“等待重新连接”，约 2 秒后自动连回去；托盘圆点等待时是黄色
- [x] 断网检测（v0.11.1）：已连接时**只关掉 Wi-Fi**（不结束任何程序）。日志马上出现“网络有变化……检查 VPN 出口”，**30 秒内**变成黄色“等待重新连接”（v0.11.0 以前要等大约 80 秒）
- [x] 停止自动重连：接着上一项，Wi-Fi 保持关着，重连会一直失败，等待时间变成 4、8、16 秒。在黄色“等待重新连接”时按“停止自动重连”（或者重连进行中按顶部按钮的“取消”），应变成“已断开”，之后不再重连。最后打开 Wi-Fi，按“连接”
- [x] 不自动重连：Wi-Fi 开着、已连接时，设置页关掉“连接中断后自动重新连接”并保存，再到任务管理器只结束 `sing-box.exe`：停在红色“错误”，不重连。**测完打开这个设置**
- [ ] 睡眠唤醒：已连接时让电脑睡眠 1 分钟以上再唤醒，日志出现“电脑从睡眠中恢复”，接着“睡眠唤醒后 VPN 出口正常”，或者“连接中断”后自动连回去

### A8. 诊断包（v0.9.0）

- [x] 日志页“导出诊断包” → “在文件夹中显示”，资源管理器选中 zip（2026-10-26 通过）
- [x] 解开 zip 看一眼：没有 Cisco 服务器地址、账号、WireGuard 对端地址、你的 Windows 用户名（应该是 `<cisco-server-1>`、`%USERPROFILE%` 这类代号）
- [x] `powershell -ExecutionPolicy Bypass -File phase1\secret-scan.ps1` 输入 Cisco 密码和 WireGuard 私钥，结果 `[PASS]`，而且输出里有 `...zip > ...`（表示解开 zip 搜过）

### A9. 第 1 阶段剩下的（热点就能测）

- [ ] 托盘右键菜单：连接、断开、打开主窗口、退出各点一次
- [ ] 设置页打开开机自启 → 注销再登录 → SplitGate 出现在托盘（不弹窗口）→ 回到设置页关掉开机自启

## B. 要在学校有线网络测（Cisco 出口）

到学校先重新记基准：`powershell -ExecutionPolicy Bypass -File phase0\snapshot.ps1 -Baseline`（SplitGate 断开时）。

- [ ] 非规则流量出口：断开时跑 `powershell -ExecutionPolicy Bypass -File phase1\exit-ip.ps1`，连接 Cisco 后再跑一次，结果 `[PASS]`
- [ ] Xbox App 能登录、浏览商店、打开 Game Pass；PrismLauncher、Steam 能登录
- [ ] 拦截检测：暂时关掉 Steam 规则集，重新连接，打开 Steam → “拦截提示”出现 `store.steampowered.com` 等，原因“证书被 Fortinet 换掉”；导航显示个数；试“加规则”“忽略”“清空”。**测完把 Steam 规则集开回来**
- [ ] 家里内网：连 Cisco 后打开 PVE 管理网页

## 测完发给 Claude

- 每一项的结果（通过 / 不通过 + 一句话）
- 不通过的那项：截图，加上日志页对应的几行；或者直接附诊断包（已脱敏）
- A 部分全部通过就可以合并 PR #2；B 部分可以之后补测
