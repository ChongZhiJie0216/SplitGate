# 第三方组件版本（锁定）

二进制文件不进 git。用 `third_party/fetch.ps1` 下载到 `third_party/downloads/`、校验 SHA256、解压到 `third_party/bin/`。
核实日期：2026-10-26。

| 组件 | 版本 | 下载文件 | 来源 | SHA256 | 校验依据 | 签名 |
|---|---|---|---|---|---|---|
| sing-box | 1.14.4（2026-10-24 发布） | `sing-box-1.14.4-windows-amd64.zip` | https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-windows-amd64.zip | `99185d75d314a07dbba4a130c8ad986cbeb629a07927975636b341ca61f991df` | 与 GitHub Release 公布的 digest 一致 | exe 无签名 |
| openconnect | v9.21（2026-06-16 tag） | `openconnect-installer-MinGW64-GnuTLS.exe`（NSIS 安装包，只用 7-Zip 解压，不安装） | https://gitlab.com/openconnect/openconnect/-/jobs/14877945030/artifacts/raw/openconnect-installer-MinGW64-GnuTLS.exe（tag v9.21 的 CI 任务 `MinGW64/GnuTLS`） | `6ee9e8eb9bc59ef70bb0717df7f99703f8a2ccd11d8e45d58a61f9a2e6ef7d00` | 官方不公布校验和；这是本次下载算出的值 | **无签名** |
| Wintun | 0.14.1 | `wintun-0.14.1.zip` | https://www.wintun.net/builds/wintun-0.14.1.zip | `07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51` | 与 wintun.net 公布值一致 | DLL 由 WireGuard LLC 签名 |
| Olm | 1.10.1（2026-10-02） | `olm_windows_amd64.exe` | https://github.com/fosrl/olm/releases/download/1.10.1/olm_windows_amd64.exe | `7ca1ca7f3e64d4590e9c93e57f99039727117f9ac403025f2f386af897e23563` | 与 GitHub Release digest 一致 | Fossorial Inc. 签名（Valid） |

openconnect 安装包里自带的 `wintun.dll` 和 wintun.net 的 0.14.1 amd64 版本完全相同（SHA256 一致），两者只需要保留一份。

## 候选（还没下载）

| 组件 | 版本 | 来源 | SHA256（Release digest） | 备注 |
|---|---|---|---|---|
| Pangolin CLI | 0.19.0（2026-10-20） | https://github.com/fosrl/cli/releases/download/0.19.0/pangolin-cli_windows_amd64.exe | `f44b92e8287a7e498c5c985cba486023fb6abce1b8bffb9d73df35809ac5b2a8` | Olm README 说 Pangolin CLI 正在取代 Olm，V7 时评估 |

## 许可证（2026-10-26 到各项目核实）

| 组件 | 许可证 | 依据 | 对本项目的要求 |
|---|---|---|---|
| sing-box | GPL-3.0-or-later，**另加一条附加条款**：“no derivative work may use the name or imply association with this application without prior consent” | Release 压缩包里的 `LICENSE` | 作为独立进程分发，附上许可证和源码链接；产品名称、界面文字不能用 sing-box 的名字，也不能暗示和它有关联 |
| openconnect | LGPL-2.1 | 仓库 `COPYING.LGPL` | 做法 A 分发独立 exe：附上许可证和源码链接。它依赖的 GnuTLS、nettle、gmp 等 DLL 也要附上各自的许可证 |
| Wintun | WireGuard LLC 的 Prebuilt Binaries License | zip 里的 `LICENSE.txt` | 不能修改 DLL；只有在随“只通过 wintun.h API 使用它的软件”一起分发时，才可以再分发 |
| Olm | AGPL-3.0 与 Fossorial Commercial License 双授权 | GitHub 仓库许可证、README | 作为独立进程分发；如果 fork 修改，必须公开修改后的源码 |
| Pangolin CLI | 混合授权：每个文件的文件头写明是 AGPL 还是 Fossorial Commercial | 仓库 `LICENSE` | 采用前要逐个确认用到的部分 |
