# radiobar 开发文档

面向维护者。用户向的功能与使用说明见 [README.md](../README.md)。

## 环境要求

- Windows 10/11（x64）
- Rust stable（MSVC 工具链）
- Node.js 22.12+ / 24+
- Visual Studio 2022 Build Tools + Windows SDK
- WebView2 Runtime（Win11 自带；Win10 一般已随 Edge 安装）

## 开发

仓库：<https://github.com/injoyai/radiobar>

```powershell
git clone https://github.com/injoyai/radiobar.git
cd radiobar
npm install
npm run tauri dev
```

Vite 开发服务器固定在 `http://localhost:1420`。

> 本机沙箱环境下若 WebView2 无法写 `%LOCALAPPDATA%`，先设
> `$env:WEBVIEW2_USER_DATA_FOLDER="$PWD\src-tauri\target\wv2data"`；
> 若 `cl.exe` 报 `%TEMP%` 拒绝访问，把 `$env:TMP`/`$env:TEMP` 指到 `src-tauri\target\ctmp`。
> 这两条只是本机限制，不属于代码行为。

## 构建

```powershell
npm run build        # 前端（Vite）
npm run tauri build  # 便携版 exe（唯一交付形态）
```

产物：`src-tauri/target/release/radiobar.exe` —— 单文件、无外部依赖。

`tauri.conf.json` 里 `bundle.active` 为 `false`，`npm run tauri build` 不会产出 NSIS 安装包。

> 已知限制（本机实测）：若把 `bundle.active` 改回 `true` 打 NSIS 安装包，setup.exe 在含中文的路径下运行会静默失败（`exit=2`，不安装也不报错），需复制到纯 ASCII 路径再运行。

## 快速检查

```powershell
npm run test           # Vitest（前端格式化/音频判定）
npx tsc --noEmit       # 前端类型检查
cargo test             # 在 src-tauri/ 下（Rust 纯逻辑）
```

## 目录

- `src/` — TypeScript 前端（`ui/` 折叠条与面板、`audio.ts` 播放、`ipc.ts` 命令封装）
- `src-tauri/src/` — Rust 后端（`app_core` 状态机、`queue` 播放队列、`library` 电台库、`sync` 增量同步、`window_platform` 窗口几何、`commands` IPC、`lib.rs` 托盘与生命周期）
- `scripts/` — 生成内置快照与国家名映射的 Node 脚本
- `docs/superpowers/` — 设计文档、实现计划与验收记录

## 数据流与关键约定

- 电台数据来自 [radio-browser](https://github.com/radio-browser/radio-browser)（CC0）。内置快照由 `scripts/` 生成，写入 `src-tauri/resources/stations.seed.json`（每国 top50 + 全球 top500 去重）。
- 未同步时电台库直接读编译期内嵌的 `stations.seed.json`；第一次「更新」才写出 `stations.local.json`（存在且非空时优先于内嵌快照），之后只做增量合并（同 id 覆盖、新 id 追加）。
- 同步走 `https://de1.api.radio-browser.info`，按国家 `bycountrycodeexact` + `hidebroken=true` + `order=clickcount` 拉取。
- 切换范围不会立刻重建播放队列，避免把正在播的台踢掉；队列在下次切台/播放时按新范围重建。
- 内置快照与本地库里的 `lastcheckok=false` 电台：进队列时跳过，列表页仍显示并标注「暂不可用」。

## 跨平台说明（macOS 待办）

代码中只有 `window_platform::apply_frame` 一个 `cfg(windows)` 分支（Windows 用一条 `SetWindowPos` 同时写尺寸与位置，避免中间帧撕裂），非 Windows 退回 Tauri 的 `set_size` + `set_position`。窗口几何用 Tauri 的 monitor API，吸附是纯几何计算，托盘用 `tauri::tray` 跨平台 API。移植 macOS 时仍需：

1. 托盘图标换模板图（template image，深浅色任务栏自适应）。
2. 验证 `tauri://localhost` 下音频与 HLS 的混合内容策略。
3. 打包与签名（Apple Developer ID + notarization）。
4. 无边框窗口在 macOS 上的拖动区域（`data-tauri-drag-region` 已就位，需实测）。
