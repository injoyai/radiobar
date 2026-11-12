# radiobar

Tauri 2 + TypeScript（无前端框架）的极简桌面电台播放器：340×64 无边框常驻小窗，托盘常驻、边缘吸附、置顶可选。

电台数据来自开源项目 [radio-browser](https://github.com/radio-browser/radio-browser)（数据 CC0）：随包内置精选快照（每国 top50 + 全球 top500 去重，实测 5,938 台），之后可在面板里手动增量更新到本地库。

## 环境要求

- Windows 10/11（x64）
- Rust stable（MSVC 工具链）
- Node.js 22.12+ / 24+
- Visual Studio 2022 Build Tools + Windows SDK
- WebView2 Runtime（Win11 自带；Win10 一般已随 Edge 安装）

## 开发

```powershell
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
npm run build          # 前端（Vite）
npm run tauri build    # 免安装 exe（同时会尝试打 NSIS 安装包，见下）
```

产物：
- `src-tauri/target/release/radiobar.exe` —— **便携版（推荐交付方式）**，单文件、无外部依赖
- `src-tauri/target/release/bundle/nsis/radiobar_0.1.0_x64-setup.exe` —— 可选安装包

### 便携版（推荐）

`radiobar.exe` 是自足的绿色单文件，直接双击即可运行，无需安装。建议先复制到仓库外的普通目录再运行，例如：

```powershell
New-Item -ItemType Directory -Force 'D:\Apps\radiobar' | Out-Null
Copy-Item 'src-tauri\target\release\radiobar.exe' 'D:\Apps\radiobar\' -Force
& 'D:\Apps\radiobar\radiobar.exe'
```

数据仍写在 `%APPDATA%\com.local.radiobar\`，与 exe 放在哪里无关。

> 已知限制（本机实测）：
> 1. **NSIS 安装包在含中文的路径下运行会静默失败**（`exit=2`，不安装也不报错）。把 setup.exe 复制到纯 ASCII 路径（如 `C:\Temp\`）再运行即可正常安装。便携版不受此影响。
> 2. 若 exe 所在目录下残留过 WebView2 用户数据（`wv2data\EBWebView`），再次启动可能报 `WebView2 error: HRESULT(0x8000FFFF)` 或 `0x800700AA` 而退出。删除该残留目录（或把 exe 移到干净目录）即可恢复。

## 快速检查

```powershell
npm run test           # Vitest（前端格式化/音频判定）
npx tsc --noEmit       # 前端类型检查
cargo test             # 在 src-tauri/ 下（Rust 纯逻辑）
```

## 使用

- 左键托盘图标：显示 / 隐藏窗口；右键：显示窗口、播放暂停、上一台、下一台、置顶、更新电台库、退出。
- 折叠条拖动后松手 150 ms 自动贴边（四边 12 px 阈值）；位置与展开态会记忆，重启复原。
- 折叠条按钮：上一台 / 播放暂停 / 下一台 / 静音 + 音量滑块 / 切换范围（收藏·列表·全部）/ 切换模式（顺序·循环·随机）/ 展开面板 / 置顶。
- 面板：搜索（台名 / 中文国家名）、按国家筛选、收藏、点击播放、「更新电台库」增量同步。
- 关闭窗口 = 隐藏到托盘，播放继续；托盘菜单「退出」才结束进程。

## 数据与配置位置

`%APPDATA%\com.local.radiobar\`
- `state.json` — 音量、模式、范围、置顶、位置、收藏、上次同步时间等
- `stations.local.json` — 本地电台库（首次启动由内置快照生成，之后只增量合并）

## 目录

- `src/` — TypeScript 前端（`ui/` 折叠条与面板、`audio.ts` 播放、`ipc.ts` 命令封装）
- `src-tauri/src/` — Rust 后端（`app_core` 状态机、`queue` 播放队列、`library` 电台库、`sync` 增量同步、`window_platform` 窗口几何、`commands` IPC、`lib.rs` 托盘与生命周期）
- `scripts/` — 生成内置快照与国家名映射的 Node 脚本
- `docs/superpowers/` — 设计文档、实现计划与验收记录

## 跨平台说明（macOS 待办）

代码中没有任何 `cfg(windows)` 分支：窗口几何用 Tauri 的 monitor API，吸附是纯几何计算，托盘用 `tauri::tray` 跨平台 API。移植 macOS 时仍需：

1. 托盘图标换模板图（template image，深浅色任务栏自适应）。
2. 验证 `tauri://localhost` 下音频与 HLS 的混合内容策略。
3. 打包与签名（Apple Developer ID + notarization）。
4. 无边框窗口在 macOS 上的拖动区域（`data-tauri-drag-region` 已就位，需实测）。
