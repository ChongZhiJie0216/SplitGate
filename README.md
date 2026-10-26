# SplitGate（暂定名）

Windows 上按域名 / 进程分流的 VPN 客户端：只有命中规则的流量（第一个场景是 Xbox）走 VPN 出口，其他流量照常走本地网络。

目前在第 0 阶段（技术验证），还没有可用的程序。

- 规格：`docs/SPEC.md`
- 验证结果：`docs/phase0-findings.md`
- 第三方组件版本：`third_party/VERSIONS.md`（二进制不进 git，用 `third_party/fetch.ps1` 下载）
- 网络出问题时：用管理员 PowerShell 执行 `powershell -ExecutionPolicy Bypass -File phase0\cleanup-all.ps1 -Apply`
