# MEMORY.md

radiobar —— Tauri 2 + TypeScript（无前端框架）的 Windows 桌面电台播放器，340×64 无边框常驻小窗。

## 架构与契约

- 逻辑条尺寸 `BAR_W=340`、`BAR_H=64`、`PANEL_H=440`（`src-tauri/src/window_platform.rs`、`src-tauri/src/snap.rs`）。所有几何计算在**逻辑坐标**里跑；`window_platform` 负责 ±`SHADOW_PAD`(16) 的外框换算（`outer_size`/`outer_position`/`inner_rect`）。CSS `#app { inset: 16px }` 抵消留边，故可见内容左上 == 逻辑 `(bar.x, bar.y)`。
- 窗口关掉了 DWM 阴影（`"shadow": false`），圆角投影由 CSS 自画，四周留 16px 透明边。代价：留边仍接收鼠标点击。
- 吸附阈值 `SNAP_THRESHOLD = 20`，**必须大于 `SHADOW_PAD`**（见下）。

## 全局快捷键

- 默认键位在 `src-tauri/src/shortcuts.rs` 的 `DEFAULT_SHORTCUTS`：prev `control+ArrowLeft`、next `control+ArrowRight`、volumeUp `control+ArrowUp`、volumeDown `control+ArrowDown`、playPause `control+KeyP`；音量步长 `VOLUME_STEP = 5`。加速键字符串格式是 `修饰键+event.code`（如 `control+shift+F9`），不是显示文本。
- `install()` **必须在后台线程调用**：`tauri-plugin-global-shortcut` 内部 `run_on_main_thread` + `rx.recv()`，在主线程调会自锁。`spawn_install` / `commands::set_shortcut` 都为此起线程。
- 录制改键期间必须 `suspend()`（`unregister_all`）再 `restore()`，否则用户按下的组合会被全局钩子先吃掉，页面收不到 keydown。链路：前端 `startCapture` → `ipc.setShortcutCapture(true)` → `commands::set_shortcut_capture` → `shortcuts::suspend`；结束/失焦/Esc → `restore`。
- `rebind` 的校验顺序是「规范化 → 查重 → 重装 → 失败回滚旧表」，查重失败只 emit `toast`（`{"level":"warn"}`），不抛异常、不改 `state.json`。前端不要假设失败会 reject。
- 全局快捷键不需要额外 capability（`src-tauri/capabilities/default.json` 保持原样即可）。
- 真机验证用 CDP（`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9231`）驱动 UI + `keybd_event` 发系统级键位，先 `MinimizeAll()` 抢走焦点才能证明「不依赖窗口焦点」。脚本在 `.superpowers/`（`send-hotkey.ps1`、`capture-suspend.mjs`、`duplicate-rebind.mjs`、`read-state.mjs`、`press-key.mjs`）。

## 已踩过的坑

### 拖动吸顶钳制（已修）

- 鼠标拖动走 HTCAPTION/`startDragging()`，Windows 把窗口**外框**顶边钳在屏幕顶（外框 `y >= 0`），逻辑 `bar.y` 最低只到 `SHADOW_PAD`(16)。阈值若 ≤16 上边永远吸不上，表现为「上方贴边有 16px 空隙」。程序化 `set_position` 不受此钳制，负坐标可达（实测外框 `top=-16` 生效）。左右/下边系统允许拖出屏外，不受影响。
- 记忆位置若带着 `y=16` 落盘，重启会原样恢复、缝隙永久复现，所以 `initial_bar_rect` 恢复前先 `snap_bar` 一次。
- **吸附必须在每个会改变窗口矩形的入口都做，不只是折叠态**：展开矩形由 `bar` 算，`bar` 带 16 的缝就会把缝原样带进展开态。现在的统一入口是 `window_platform::snap_bar_to_work`，`apply_panel`（打开/收起面板）、`schedule_snap`（折叠与展开两个分支）都先过它。改几何时新增路径也要走这一条，否则「折叠态贴顶、展开态留缝」的 bug 会重演。
- `bar_rect_while_open` 的展开方向**只能**用 `AppCore::panel_grows_up`，不能从「上次条位置」猜：面板开着拖动时窗口矩形每帧都在变，猜出来的方向会错，反推出的 `bar` 位置也跟着错。
- **吸附必须在拖动结束后做，不能在拖动中做**：拖动走 HTCAPTION，系统拖动循环每帧按光标摆窗口；此时 `set_position` 抢同一个位置，观感是「先离开边缘、松手再闪一下贴边」。`schedule_snap` 用 `GetAsyncKeyState(VK_LBUTTON)` 轮询左键，松手才吸（去抖 40ms）。不要用 `Moved`/`Resized` 事件判断拖动结束——判断不出来。
- **改窗口尺寸+位置必须一次 `SetWindowPos`，不能连着调 `set_size` + `set_position`**：tao 0.37.2 这两条路径各发一条独立消息（`set_outer_position` 带 `SWP_NOSIZE`，`set_inner_size_physical` 带 `SWP_NOMOVE`），中间必然出现「新高度 + 旧位置」或「新位置 + 旧尺寸」的一帧，展开/收起面板会撕裂一下。Tauri 的 `Window` 没有合成 API（`Webview::set_bounds` 设的是 webview 在窗口内的范围，不是窗口本身）。统一入口是 `window_platform::apply_frame`：自己发 `SetWindowPos(hwnd, None, x, y, cx, cy, SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOACTIVATE)`，坐标用外框物理像素，`HWND` 需从 tauri 的 windows 0.62 转成本项目的 0.58（`HWND(hwnd.0)`）。跨线程调用**必须**带 `SWP_ASYNCWINDOWPOS`，否则会阻塞在窗口线程的消息循环上。
- **吸附缓动用 `glide_to`，6 帧 × 16ms**：位移上限是 `SNAP_THRESHOLD`(20)，96ms 的 `snap::eased_offset` 够读出「滑过去」。缓动期间 `snap_animating` 必须置真——逐帧 `apply_position` 会触发 `Moved`，回响若开启新一代去抖会把缓动自己的代数顶掉、把它打断在半路。每帧前查 `primary_button_down()`，用户重新按住就立刻停在原地让位。面板态滑完再 `apply_panel` 对齐尺寸与方向，折叠态最后一帧已到终点不用重写。**缓动被打断时窗口停在半路，`glide_to` 必须返回实际停下的矩形供落盘**——若仍写吸附终点，下次启动会跳到从没到过的位置（实测错误值 `barPos=616,416` vs 实际 `616,4`）。

### 单实例互斥体导致「双击没反应」（排查时最容易误判）

- `tauri-plugin-single-instance` 用命名互斥体 `com.local.radiobar-sim` 判定首实例；第二实例把 argv/cwd 经 `WM_COPYDATA` 转给首实例后 `app.cleanup_before_exit(); std::process::exit(0)`。**exit code 是 0，看起来完全正常**，实际什么都没起来。
- 残留实例未必叫 `radiobar.exe`：测试时改了名的副本（`rb_t.exe`）照样占着互斥体，而 `Get-Process radiobar` 查不到。排查启动问题**先**枚举全部可疑进程再下结论，否则整轮 A/B 对照全是假数据（本次就被污染过一轮）。
- 探测方法：`OpenMutex(SYNCHRONIZE, false, "com.local.radiobar-sim")`，返回句柄即有人持锁。

### WebView2 初始化失败 0x800700AA / 0x8000FFFF

- 症状：`Failed to setup app: runtime error: failed to create webview: WebView2 error: WindowsError(Error { code: HRESULT(0x800700AA), message: "请求的资源在使用中。" })`，进程 8 秒内退出；换新数据目录则变 `0x8000FFFF 灾难性故障`。
- **两个独立成因，别混在一起判断**：
  1. **进程写权限被按 exe 位置限制**（本次决定性结论）。同一探针 exe 放在项目目录内时，连 `F:\` 和 `%LOCALAPPDATA%` 都写不进去（`UnauthorizedAccessException`）；复制到项目目录外就一切正常。WebView2 必须写 `%LOCALAPPDATA%\com.local.radiobar\EBWebView`，写不了就报上面的错。这是**会话环境对可执行文件所在位置的限制，不是应用缺陷**。
  2. **共享数据目录被残留 WebView2 进程锁住**。`%LOCALAPPDATA%\com.local.radiobar\EBWebView` 是固定路径，所有副本共用；上一轮测试的 `msedgewebview2` 没退干净，下一次启动就报 `0x800700AA`。**反复在同一会话里连续启动测试会互相污染**，结论会自相矛盾。
- 若要隔离变量，用 `$env:WEBVIEW2_USER_DATA_FOLDER` 指向每次全新的目录，并且启动前确认 `Get-Process msedgewebview2,radiobar` 为空。
- 删 `%LOCALAPPDATA%\com.local.radiobar\EBWebView` 或把 exe 移到干净目录可临时恢复，但**根因是上面两条**，别把它当修复。

### 播放中途静默无声、暂停再继续才恢复（已修，2026-11-17）

- 根因：icecast 直播流被服务端断开（EOF 或断连）时 `<audio>` 会**静默**进入 `ended`/`stalled`，旧代码没有 `ended` 监听、8 秒看门狗只在 `loadstart` 装一次（中途断流不再触发 loadstart）→ 永久无声且后端完全不知情。「暂停再继续能恢复」是因为 resume 路径重新调 `play()` 拉流——这就是自愈的原理。
- **关键契约**：`autoNextOnFailure=true` 时 `report_playback_error` **一次上报就切台**（`consecutive_failures >= 3` 只管「连续多台失败后暂停」）。所以可自愈的瞬时断流绝不能上报，否则听得好好的突然被切台。
- 修复在 `src/audio.ts`：`ended` → `shouldResumeOnEnded(wantPlaying, hasSource, 直连流, endedResumes)` 闸门自愈（上限 `MAX_ENDED_RESUMES=3`，超过才上报 "stream ended" 交后端切台）；`waiting`/`stalled` → `armStallWatchdog()` 重装看门狗，直连流超时静默重连（`load()`+`play()`），HLS 流只上报不重连（hls.js 自管恢复）；`wantPlaying` 在 `sync()` 里维护，避免自愈复活用户已暂停的会话。
- 无法在沙箱里复现服务端断流，验证靠代码路径证据链 + 单测（闸门函数）；长期效果以用户实际收听为准。沙箱里 vitest/vite 也跑不了（`spawn EPERM`），要 `danger-full-access`。

## 验证吸附效果的正确做法

1. 把 `radiobar.exe` 复制到项目目录**之外**（如 `F:\桌面\电台播放器-便携版\`）。
2. `Start-Process explorer.exe -ArgumentList '"<exe 路径>"'` —— 用资源管理器当父进程，等价于用户双击，且脱离当前会话进程树。
3. `Get-Process radiobar` 确认 `MainWindowHandle` 非 0，再用 `GetWindowRect` 读外框：
   - 期望**外框 `y = -16`**（`SHADOW_PAD` 跑到屏幕外），可见条顶正好在屏幕 `y=0`。
   - 模拟拖动极限：`SetWindowPos(h, 0, x, 0, 372, 96, SWP_NOZORDER|SWP_NOACTIVATE)` 把外框放到 `y=0`，等约 200ms（去抖 40ms + 松手轮询 16ms），读回应变成 `y=-16`。对照：外框 `y=24`（逻辑 `y=40` > 阈值 20）不应移动。
   - 注意 `SetWindowPos` 的 `SWP_NOSIZE`：不传会改尺寸；本次传 `0x0014`（NOZORDER|NOACTIVATE）保持大小。
4. 截屏取首行像素：`CopyFromScreen(x, 0, …)`，首行应是条背景色（`#F7F9FA`），出现桌面色即说明还有缝。
5. 吸附结果会落盘到 `%APPDATA%\com.local.radiobar\state.json` 的 `barPos`，贴顶时应为 `[x, 0]`。
6. **验「拖动中不吸附」**：合成鼠标事件驱动不了 `startDragging()`（窗口纹丝不动），但可以用「按住左键」绕开：`mouse_event(0x0002)` 按下（`GetAsyncKeyState(1)` 应为 `-32768`）→ `SetWindowPos` 把外框摆到目标位置 → 每 40ms `GetWindowRect` 采样。期望全程不动，松手后首次采样才贴边。**不能靠 `SetCursorPos`+`mouse_event` 真实拖窗口来验证**。
7. **验展开态要趁 40ms 去抖窗口**：`SetWindowPos(y=0)` 之后立刻发双击，否则去抖线程先把窗口吸上去，测的就不是「带缝 → 展开」这条路径。双击点必须落在 `.bar-top`（逻辑条顶往下约 22px，即 `outerTop + 16 + 22`）；点在 `outerTop+48` 会落到 `.bar-controls` 上，那里没有 dblclick 处理。自动化双击常首次不生效（`data-tauri-drag-region` 的 `startDragging()` 吞掉第二次 click），要重试到 `panelOpen=true` 为止。展开成功判据：外框 `H=472`、`T=-16`、`state.json` 的 `panelOpen=true`。

## 构建与交付

### 交付形态：只有便携版

- 只交付**便携版** `src-tauri/target/release/radiobar.exe`，命令 `npm run tauri build`（**不需要** `--no-bundle`）。
- `tauri.conf.json` 的 `bundle.active` 已设为 `false`，`npm run tauri build` 只产出 exe，不再生成 `bundle\nsis\` 安装包。用户明确要求后续打包一律不出安装包，别再改回 `true`。
- 便携版单文件、无外部依赖；配置/收藏/历史都写在 `%APPDATA%\com.local.radiobar\`，与 exe 放哪无关。

### 打包踩坑

- **`npm run build` 报 `spawn EPERM`**：vite 在 `optimizeSafeRealPathSync` 里 `exec("net use")` 探测盘符，受限沙箱禁止带管道 stdio 起子进程。这是**沙箱限制，不是代码问题**；放宽权限（或直接在普通终端里跑）即可。注意 `tauri.conf.json` 的 `build.beforeBuildCommand` 就是 `npm run build`，所以 `tauri build` 会一起挂在这里，报错点在 vite 而不是 Rust。
- **`dist/` 与 `target/` 都被 gitignore**：新克隆的仓库既没有前端产物也没有 exe。`tauri build` 靠 `beforeBuildCommand` 先跑 `npm run build` 重建 `dist/`，所以正常流程没问题；**但单独跑 `cargo build` 不会重建前端**，它把当时 `dist/` 里的旧资源编进 exe——前端改了却只 `cargo build`，会得到「代码改了没生效」的假象。改过 `src/` 就必须走 `tauri build`（或先 `npm run build`）。
- **`dist/assets/` 会累积历史哈希文件**：每次构建留下一份新的 `index-<hash>.js/css`，`index.html` 只引用其中一份。目录里有多个 `index-*` 是正常的，别手动删——`index.html` 引用的那个才是当前生效的。想确认「exe 里嵌的是哪版前端」，看 `dist/index.html` 的引用，别看目录里文件的时间戳。
- **构建完 `release\radiobar.exe` 可能不在**：曾出现过构建成功后 `release\` 下只剩 `radiobar.d`/`radiobar.pdb` 的情况（疑似被杀毒扫描挪走，未能证实）。`target\release\deps\radiobar.exe` 是同一份二进制（实测 SHA256 与 `release\radiobar.exe` 一致），从那里复制回来即可，不必重新编译。
- **`cl.exe` 报 `%TEMP%` 拒绝访问**：编译需临时目录，受限环境下把 `$env:TMP`/`$env:TEMP` 指到项目内（如 `src-tauri\target\ctmp`）。
- **`cargo test --lib` 报 PermissionDenied**：`app_core.rs` 里 `tempfile::tempdir().unwrap()` 依赖 `TEMP`，同样要指到项目内可写目录（如 `src-tauri\target\testtmp`），否则测试直接 panic。
- **NSIS 安装包在含中文的路径下静默失败**：`exit=2`，既不安装也不报错；复制到纯 ASCII 路径（`C:\Temp\`）才行。现在 `bundle.active: false` 不会再打安装包，这个坑只在有人手动改回 `true` 时才出现。
- **`pwsh` 把 cargo/npm 的 stderr 当成错误**：`[exit code: 1]` 常出现在构建/测试明明成功的时候，判断成功与否要看输出里的 `Finished` / `test result: ok`，别只看退出码。

## 文档位置

- `README.md` 面向最终用户（功能/使用/快捷键/数据位置/FAQ），不写构建与排障；开发、构建、测试、目录结构、数据流约定、macOS 待办都在 `docs/DEVELOPMENT.md`。用户可见行为变化两边都要同步。
- 设计/计划/验收在 `docs/superpowers/`：`specs/` 是设计，`plans/` 是分步实现计划，`acceptance-*.md` 是验收记录。
- README 配图只有 `docs/images/bar.png`（340×64 折叠条，取自 `.superpowers/tmp/q0.png`，是 CDP 截图，背景为实色浅灰、非透明）。`.superpowers/` 被 gitignore，不能直接引用，要复制到 `docs/images/`。用户要求只展示未展开面板的形态。

## 远端仓库

- origin：`https://github.com/injoyai/radiobar.git`，默认分支 `master`（2026-11-17 推送）。本地分支 `ui-polish`、`task-6-seed` 未推。
- **沙箱下 push 会失败**：git 走 `credential.helper=manager-core`，凭据助手需要管道 stdio，受限模式报 `couldn't create signal pipe, Win32 error 5` 与 `could not read Username for 'https://github.com'`。用 `danger-full-access` 重跑同一条 push 即可（不是密码问题）。
- `AGENTS.md`、`MEMORY.md` 是未跟踪文件，未提交也未推送；`.superpowers/` 已在 `.gitignore`。
