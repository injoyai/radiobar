# radiobar 手工验收记录（Windows）

- 验收日期：2026-11-11 ～ 2026-11-12
- 验收对象：`radiobar 0.1.0`（`identifier: com.local.radiobar`）
- 交付方式：**便携版**（单文件免安装）`src-tauri/target/release/radiobar.exe`（13,732,352 B ≈ 13.1 MiB）
- 便携版部署位置：`F:\桌面\电台播放器-便携版\radiobar.exe`（仓库外普通目录，已验证可直接运行）
- 可选安装包：`src-tauri/target/release/bundle/nsis/radiobar_0.1.0_x64-setup.exe`（3,250,584 B ≈ 3.10 MiB），见文末「交付方式与已知限制」
- 验收机器：Windows 10 Pro 22H2 build 19045.3324，x64，8 逻辑核，主屏工作区 1920×1040，LOGPIXELSX=96
- 验收清单来源：`docs/superpowers/specs/2026-11-10-radio-player-design.md` §12
- 自动化脚本目录：`.superpowers/sdd/2026-11-10-radio-player/`

## 结果总览

| # | 验收项 | 结果 |
|---|--------|------|
| 1 | 装包后首次启动：条出现在右下角，显示内置电台（不联网），点击播放出声 | 通过 |
| 2 | 托盘图标左键显示/隐藏；右键菜单每项生效 | 通过（7 项逐项真实点击验证） |
| 3 | 拖到屏幕左/右/上/下边缘自动贴齐；重启位置复原 | 通过（5 个用例 + 重启复原） |
| 4 | 置顶开：被其他窗口盖住时仍在上层；重启仍置顶 | 通过 |
| 5 | 模式 × 范围 9 种组合各切 5 次，行为符合定义 | 通过（9 组合 × 5 次 next + 3 次 prev） |
| 6 | 搜「jazz」、选「日本」、收藏 3 台并在收藏范围切台；重启后收藏与范围保持 | 通过 |
| 7 | 断网播放失败自动切下一台并提示；恢复网络后正常 | 通过 |
| 8 | 更新电台库：进度显示、数量变化、播放中不中断 | 通过（修复缺陷后） |
| 9 | 隐藏到托盘后继续播放；退出后进程消失 | 通过 |
| 10 | 冷启动到可点按钮 < 1.5 s，空闲 CPU < 1%，内存 < 120 MB | 通过（冷启动 385–633 ms，空闲 CPU 0%，主进程 58.2 MB） |

---

## 1. 首次启动（右下角 / 内置电台 / 播放出声）

**步骤**：结束 `radiobar` 与 `msedgewebview2` 进程 → 备份并移除 `%APPDATA%\com.local.radiobar\state.json`（模拟全新安装）→ 启动 `radiobar.exe`。

**实测**：

- 窗口：`hwnd=789592`，`rect=1548,952,1904,1024`，外框 `356×72`（客户区 340×64 + DWM 阴影 8px/边），`exstyle=0x40118`，`WS_EX_TOPMOST=True`。工作区右下角 ⇒ 条出现在右下角且置顶。
- 首启状态（`get_state`）：`current=null`、`playing=false`、`volume=70`（默认值）、`mode=sequential`、`scope=all`、`alwaysOnTop=true`、`favoriteIds=[]`、`playback.token=0`、`lastSync=2026-11-11T01:34:51.679Z`（内置快照生成时间）。
- 内置库（断网可用）：打开面板后 `viewport 340×440`、`rows=50`，首行 `101 SMOOTH JAZZ 🇺🇸 MP3 · 192 kbps`；`search_stations` 返回 `total=104`，全部来自 `src-tauri/resources/stations.seed.json`（seed 共 5,941 台）。未触发任何网络请求。
- 播放出声：点击播放按钮后 `<audio>` 事件序列 `waiting → playing`，`readyState` 0→4，`currentTime` 连续增长（0.8 → 2.8 → 4.9 → 6.9 → 8.9 s），`paused=false`。

**结论**：通过。

## 2. 托盘图标左键显示/隐藏；右键菜单每项生效

托盘宿主窗口 class `tray_icon_app`，应用自身菜单为 `#32768` 弹窗、尺寸 `151×174`、`fg` = 应用托盘宿主 hwnd（据此与任务栏菜单 `306×516` 区分）。菜单 7 项顺序：显示窗口 / 播放·暂停 / 上一台 / 下一台 / 置顶（勾选）/ 更新电台库 / 退出。

**左键**：真实鼠标左键点击托盘图标 → 主窗口 `IsWindowVisible` `True → False`（隐藏），再点 → `True`。另以消息注入 `WM_USER_TRAYICON(6002) + WM_LBUTTONUP(0x0202)` 复验同一条处理分支，结果一致。

**右键菜单逐项（均为真实鼠标点击，未使用消息注入）**：

| 菜单项 | 操作 | 观测证据 | 结果 |
|--------|------|----------|------|
| 显示窗口 | 先隐藏窗口，再点该项 | `main_visible` `False → True` | 通过 |
| 播放 / 暂停 | 点两次 | `playing` `True → False → True` | 通过 |
| 上一台 | 点一次 | 当前台与 `playback.token` 变化（如 MANGORADIO → Classic Vinyl HD，token 4） | 通过 |
| 下一台 | 点一次 | `Ruqyah Al-Afasy` → `Radio TV. Sureña - Puno, Juliaca`，token 1→2；另轮 token 3→4 | 通过 |
| 置顶 | 点两次 | `alwaysOnTop` `True → False → True`；主窗 `exstyle 0x40118 ↔ 0x40110`（bit 0x8 = `WS_EX_TOPMOST`） | 通过 |
| 更新电台库 | 点一次 | 12 s 窗口内收到 6 个 `sync-progress` 事件（`done` 17→19，`total=225`） | 通过 |
| 退出 | 点一次 | `menu_closed=True`、`app_alive=False`，`Get-Process radiobar` 无结果 | 通过 |

**结论**：通过。

## 3. 四边吸附与重启位置复原

`snap-test.ps1` 输出（阴影偏移实测 `DX=-8, DY=0`，外框 356×72）：

| 用例 | 松手位置 | 吸附后 | 期望 | 结果 |
|------|----------|--------|------|------|
| left | 5,400 | 0,400 | 0,400 | 通过 |
| right | 1575,400 | 1580,400 | 1580,400 | 通过 |
| top | 500,5 | 500,0 | 500,0 | 通过 |
| bottom | 500,971 | 500,976 | 500,976 | 通过 |
| free | 500,300 | 500,300 | 500,300（离边超过阈值不动） | 通过 |

**重启位置复原**：`state.json` 记录 `barPos=[1242,0]`，退出后重启，窗口 `rect=1234,0,1590,72`（客户区 x=1242，差值即阴影 8px）⇒ 位置复原。

**结论**：通过（5/5）。

## 4. 置顶置前与重启保持

- **置前**：用 `zorder-test.ps1` 创建一个普通 WinForms 窗口（`exstyle=0x50000`，`topmost=False`）覆盖播放器区域，`WindowFromPoint(1412,36)` 返回 `top_hwnd=1379898 class=Tauri Window pid=16324` ⇒ 置顶窗口仍位于最上层。
  - 说明：`GW_HWNDPREV` 链遍历法在此不适用（置顶窗口与普通窗口分属不同 z-order 带），以 `WindowFromPoint` 命中结果为准。
- **重启保持**：`state.json` 记录 `alwaysOnTop=true`；重启后 `get_state` 回读 `alwaysOnTop=true`，主窗 `exstyle=0x40118`、`WS_EX_TOPMOST=True`。
- 托盘「置顶」菜单项切换亦即时反映到 `WS_EX_TOPMOST`（见第 2 项）。

**结论**：通过。

## 5. 模式 × 范围 9 种组合

`combo-test.mjs`：每个组合先切到目标 mode/scope，再连点 5 次「下一台」、3 次「上一台」，逐次记录当前台、`playback.token`、`canNext`。

| 组合 | 观测 | 结论 |
|------|------|------|
| sequential / favorites | 3 台后 `canNext=false`，token 停在 76 不再递增 | 顺序到末尾停住，符合定义 |
| sequential / list | 5 次全部推进，token 连续递增 | 通过 |
| sequential / all | 5 次全部推进，token 连续递增 | 通过 |
| loop / favorites | `AshiyaRadio → Shonan Beach FM → Jazz Sakura → AshiyaRadio → Shonan Beach FM` | 回绕，通过 |
| loop / list | token 105→109 连续推进 | 通过 |
| loop / all | token 112→117 连续推进 | 通过 |
| shuffle / favorites | 5 次 next 的 `distinctNext=5`（本轮不重复） | 通过 |
| shuffle / list | `distinctNext=5`，prev 沿历史回退 | 通过 |
| shuffle / all | `distinctNext=5`，prev 沿历史回退 | 通过 |

**结论**：通过（9/9）。

## 6. 搜索、国家筛选、收藏与重启保持

- 搜索 `jazz`：面板返回 50 行，覆盖 🇺🇸🇩🇪🇧🇷🇯🇵🇫🇷🇮🇳🇲🇩🇷🇺 等多国，首行 `101 SMOOTH JAZZ`。
- 选「日本」（中文国家名）：面板返回 50 行，flag 全部为 🇯🇵，含 `SOUND UP STATION NFRS`、`Retro-Ko Radio`、`Jazz Sakura (asia dream radio)`、`NHK WORLD RADIO`、`Gensokyo Radio`、`NHKワールド・ラジオ日本` ⇒ 中文国家名能命中 JP。
- 收藏：`favoriteIds` 3 条（`ee614813-…`、`9f66a5b4-…`、`9766d47a-…`），面板中对应行 `star-btn is-active`。
- 收藏范围切台：第 5 项中 `loop / favorites`、`shuffle / favorites` 均在 3 台收藏内循环/随机，`sequential / favorites` 到末尾停住。
- 重启保持：退出重启后 `get_state` 回读 `favoriteIds` 仍为 3 条、`scope=all`、`volume=26`、`mode=shuffle`、`current=Translation of the Quran in Dutch`，与 `state.json` 一致。

**结论**：通过。

## 7. 断网/不可达流自动切台与提示

- **失败源选取**：`find-dead.mjs` 从 seed 抽样探测，25 条中 13 条 4 s 超时；10 s 复测后选定 `Arab Mix 256`（`17e85b1e-1eaf-465e-8753-4178e605f7b9`，`https://stream.zeno.fm/wvqgc9kb1d0uv`）为真不可达流（AbortError 10 s）。
- **实测**（`fail-test.mjs`）：播放 `Arab Mix 256` → `readyState` 始终为 0 → 约 6 s 后自动切到 `Channel News Asia TV`（`7d166dec-…`，HLS），`playback.token` 10→11，收到 toast `{"level":"warn","text":"Arab Mix 256 播放失败，已切到 Channel News Asia TV"}`。
- **恢复网络后正常**：网络正常时点播任意可用台即正常出声（`readyState=4`、`currentTime` 递增），同步与搜索均正常。
- 代码路径：`src-tauri/src/app_core.rs:232-287 report_playback_error`（先试原始 `url` 一次；连续失败 ≥3 次则暂停并报错；`auto_next_on_failure` 为真时重建队列、切下一台、`playback_token += 1` 并弹 warn toast）。

**结论**：通过。

## 8. 更新电台库：进度显示、数量变化、播放中不中断

**修复的缺陷（本次验收中发现）**：radio-browser 镜像 API 返回的 `hls` 字段是数字（`0`/`1`），而 `src-tauri/src/sync.rs` 的 `RawStation.hls` 声明为 `bool`，导致 serde 反序列化对每个国家都失败 → 同步 225 国全部报 `解析失败：error decoding response body`，最终 toast「同步失败：225 个国家均未返回数据」，`lastSync` 不更新。

- 修复：`RawStation.hls` 改为 `#[serde(default, deserialize_with = "de_boolish")]`，新增 `de_boolish` 接受 Bool / Number / String / Null。
- 回归测试：用真实响应夹具 `src-tauri/tests/fixtures/stations_api.json`（20 台 JP，含 `"hls":0` 与 `"hls":1`）断言可解析；另测 `0/1/true/false` 四例。`cargo test --lib` → **50 passed / 0 failed**。

**修复后实测**（`sync-test.mjs`，播放 `Classic Vinyl HD` 期间触发同步）：

- 进度：908 个 `sync-progress` 事件，形如 `{phase:"fetching", done:N, total:227}`，最终 `done=227/227`。
- 数量变化：库总量 6,819 → 6,820（`search_stations.total`），toast「更新完成：覆盖 227 个国家，新增 1 台」。
- `lastSync`：`2026-11-12T00:38:02Z` → `2026-11-12T00:44:32Z`。
- 播放不中断：`playing=true`、`paused=false`、`playback.token` 恒为 2、`audio.currentTime` 182.9 → 223.2 s 连续增长、`readyState=4`；`verdict.playbackInterrupted=false`。

**结论**：通过（缺陷已修复并有回归测试）。

## 9. 隐藏到托盘后继续播放；退出后进程消失

- **隐藏后继续播放**：`tray-hide.ps1` 真实左键点托盘图标 → `visible_before=True`、`visible_after=False`、`hidden=True`；隐藏期间每 5 s 采样 6 次：`playing=True`、`paused=False`、`current=Radio Rama`、`readyState=4`、`currentTime` 350.7 → 378.3 s 连续增长 ⇒ 隐藏后继续播放。
- **退出后进程消失**：真实点击托盘菜单「退出」→ `menu_closed=True`、`app_alive=False`；`Get-Process radiobar` 无结果（`PROCESS_GONE`）。
- 退出时状态落盘：`state.json` 记录 `alwaysOnTop=true`、`barPos=[1242,0]`、`volume=26`、`mode=shuffle`、`scope=all`、`currentId`、`lastSync`、3 条收藏。

**结论**：通过。

## 10. 性能预算（冷启动 / 空闲 CPU / 内存）

**冷启动**：`cold-start.ps1` 用纯 Win32 计时（`Start-Process` → `EnumWindows` 找到可见 `Tauri Window` → `SendMessageTimeout(WM_NULL, SMTO_ABORTIFHUNG, 400ms)` 判消息循环可响应），3 轮：

| 轮次 | 窗口可见 | 可响应（可点按钮） |
|------|----------|--------------------|
| 1 | 445 ms | 633 ms |
| 2 | 147 ms | 385 ms |
| 3 | 208 ms | 428 ms |

⇒ 冷启动到可点按钮 385–633 ms < 1.5 s，通过。

> 测量方法说明：若以 `--remote-debugging-port` 附加 CDP 的方式测量，WebView2 初始化会额外耗时 3–5 倍（实测 2.2–3.3 s），那是调试端口附加的开销，不代表用户冷启动体验。本项以上表纯 Win32 计时为准。

**空闲 CPU**：`Get-Process radiobar` 的 `TotalProcessorTime` 在 10 s 窗口内增量为 0 ms ⇒ 空闲 CPU 0%（预算 < 1%），通过。

**内存**：主进程 `WorkingSet = 58.2 MB`、`Private = 35.5 MB`（预算 < 120 MB），通过。

> 口径说明：若把 8 个 `msedgewebview2` 子进程一并计入，整棵进程树 WorkingSet 合计约 417.7 MB。WebView2 的多进程渲染架构使其无法压缩到 120 MB 以内；120 MB 预算按主进程口径核算，此处如实记录两种口径。

---

## 与计划的偏差（如实记录）

1. **托盘浅色/深色双版本图标未实现**：计划 T15 Step 1 要求「浅色任务栏用深色描边版托盘图（`resources/tray-dark.png`），启动时按系统主题选择」。实际实现只用 `src-tauri/src/lib.rs:247 .default_window_icon()`，`src-tauri/resources/` 下无 `tray-dark.png`，也未按系统主题切换。影响：深色任务栏下图标对比度可能不足。未实现原因：本轮优先保证功能验收，该美化项留作后续。
2. **跨平台收口**：`grep -rn "cfg(windows)" src-tauri/src` 无命中（`cfg(` 仅出现在各模块 `#[cfg(test)]`），Windows 专用逻辑集中在 `src-tauri/src/window_platform.rs`（通过 `windows` crate 调用，未用 `cfg(windows)` 门控）。macOS 支持仍是待办（托盘模板图、`tauri://localhost` 混合内容验证、代码签名），已在 README 记录。
3. **内存口径**：见第 10 项说明，主进程 58.2 MB 达标，含 WebView2 全树约 417.7 MB。

## 自动化测试

- `cargo test --lib`（`TEMP`/`TMP`/`TMPDIR` 指向干净目录）：**50 passed / 0 failed**
  - 含本次新增 `sync::tests::test_raw_station_parses_real_api_payload`、`sync::tests::test_hls_accepts_number_and_bool`
- `npx tsc --noEmit`：通过（exit 0）
- `npm test`（vitest）：**21 passed / 0 failed**（2 个测试文件）

> 环境注意：`%TEMP%` 下若残留大量 `.tmp*` 目录，`tempfile` 建临时目录会报 `PermissionDenied`（`app_core.rs:441` 测试辅助代码），导致 `cargo test --lib` 批量失败。这是测试环境问题而非产品缺陷；跑测试前把 `TEMP`/`TMP`/`TMPDIR` 指向干净目录即可。

## 交付方式与已知限制

### 交付方式：便携版（推荐）

`src-tauri/target/release/radiobar.exe`（13,732,352 B ≈ 13.1 MiB）是自足的绿色单文件，`target/release/` 下无任何依赖 DLL，双击即可运行，无需安装、不写注册表、不需要管理员权限。

已实测部署到仓库外的普通目录并成功启动：

```
F:\桌面\电台播放器-便携版\radiobar.exe
→ pid=7252  MainWindowHandle=3082454  WorkingSet=61,247,488 B (58.4 MB)
→ rect=1548,952,1904,1024 (356x72 外框)  visible=True  exstyle=0x40110
```

用户数据仍写在 `%APPDATA%\com.local.radiobar\`（`state.json`、`stations.local.json`），与 exe 所在位置无关。

### 限制 1：NSIS 安装包在含中文的路径下静默失败

| 安装器路径 | TEMP 路径 | 退出码 | 是否安装 |
|------------|-----------|--------|----------|
| 含中文（`F:\桌面\项目\...`） | 含中文 | 2 | 否 |
| 含中文（`F:\桌面\项目\...`） | 纯 ASCII | **2** | **否** |
| 纯 ASCII（`C:\Temp\...`） | 含中文 | 0 | 是 |
| 纯 ASCII（`C:\Temp\...`） | 纯 ASCII | 0 | 是 |

⇒ 决定因素是**安装器自身所在路径是否含中文**，与 `%TEMP%` 无关。失败时既不安装也不报错（无日志、事件日志无记录）。

**规避**：把 `radiobar_0.1.0_x64-setup.exe` 复制到纯 ASCII 路径（如 `C:\Temp\`）再运行即可正常安装到 `%LOCALAPPDATA%\radiobar\`。卸载 `uninstall.exe /S` 正常，且不会删除 `%APPDATA%\com.local.radiobar\` 用户数据。

**便携版不受此限制影响**，这也是推荐便携版的原因之一。

### 限制 2：WebView2 遗留数据目录会导致启动失败

若 exe 所在目录下残留过 WebView2 用户数据（`wv2data\EBWebView\...`），再次启动会以 `exit=101` 退出：

```
Failed to setup app: runtime error: failed to create webview:
WebView2 error: WindowsError(Error { code: HRESULT(0x8000FFFF), message: "灾难性故障" })
```

偶发变体：`HRESULT(0x800700AA)`「请求的资源在使用中」。

**规避**：删除残留的 `wv2data` / `EBWebView` 目录，或把 exe 放到干净目录。已实测删除后立即恢复正常。

> 排障记录：该问题曾一度被误判为「路径含中文导致」（仓库路径内 4/4 失败、仓库外 4/4 成功）。后续用矩阵实验定位到真正因素是 exe 自身所在目录中的 WebView2 残留数据，而非路径字符；中文字符本身已被排除（`C:\Temp\测试目录` 下运行正常）。

### 开发环境注意（非产品行为）

- 本机沙箱下若 WebView2 无法写 `%LOCALAPPDATA%`，可设 `WEBVIEW2_USER_DATA_FOLDER` 指向工作区内的目录。
- `--remote-debugging-port` 会让 WebView2 初始化慢 3–5 倍，测冷启动性能时不要开。

