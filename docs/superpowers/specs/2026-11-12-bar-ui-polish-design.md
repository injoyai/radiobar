# radiobar 折叠条与面板 UI 打磨（2026-11-12）

## 意图与原因

用户在验收交付后实际使用便携版，提出 6 条界面问题。这些问题不影响功能，
但影响「愿不愿意把它常驻在桌面上」——对一个常驻小工具来说，视觉整洁度是核心体验。

本设计只处理这 6 条，不扩大范围。

## 目标

1. 圆角边缘干净，无矩形边线
2. 折叠条右侧无死白，布局饱满
3. 范围按钮与面板按钮图标可区分
4. 设置入口图标不再像日/夜间切换
5. 面板向下空间不足时向上展开，且折叠条位置不跳
6. 面板展开后再点按钮能收起

## 非目标

- 不改配色体系、不做主题切换、不加新功能入口
- 不改面板内部（搜索/列表/分页/设置页）的布局
- 不改窗口尺寸常量 `BAR_W`/`BAR_H`/`PANEL_H`
- 不动 macOS 相关待办

## 现状（实测证据）

| 问题 | 实测 | 根因位置 |
|------|------|----------|
| ① 矩形边线 | 窗口外框 356×72、客户区 340×64；在客户区之外 `x=348`、`y=64` 有 R163 深灰带，向外渐隐（194→201→224）；客户区内侧边缘是 R224 | DWM 矩形窗口阴影（`tauri.conf.json` 未设 `shadow: false`，默认开启） |
| ② 右侧空白 | CDP 量得控件行 `x=12..284`，客户区宽 340 ⇒ **右侧空 56px** | `src/style.css` `.volume-wrap` 的 `max-width: 78px` 封顶，其余按钮 `flex: none` |
| ③ 图标撞车 | `src/ui/bar.ts:79` 与 `:81` 同为 `icons.list`（仅当范围为「列表」时撞车） | 同左 |
| ④ 设置图标像日夜 | `src/ui/icons.ts:58-61` 是「圆 + 8 条放射线」，即太阳造型 | 同左 |
| ⑤ 展开方向 | 条在 `y=952`，开面板后窗口变 `576..1024`，**底边钉住向上长**（方向本身对）；但 `.bar` 在 DOM 里排在 `.panel` 之前，于是条被顶到窗口顶部 `y=576`，视觉上跳了 376px | `src/style.css` `#app` 的 `flex-direction: column` + 前端无「向上展开」状态 |
| ⑥ 不能收起 | `src/ui/bar.ts:211-213` 写死 `setPanelOpen(true)` | 同左 |

## 方案

用户对 ② 选了 A（音量条吃掉空位）；对 ① 选了「扩大窗口自画阴影」。

### ① 关掉 DWM 阴影，改为窗口留边 + CSS 自画圆角阴影（用户选定）

**为什么必须动 DWM 阴影**：R163 那条线在客户区**之外**（客户区 340×64 内是 R224，
`x=348`/`y=64` 处是 R163 并向外渐隐到 R224），属 DWM 给无边框窗口画的**矩形**
投影，CSS 无法裁掉它。

**做法**：窗口四周各留 `SHADOW_PAD` 像素透明边，阴影在留边内由 CSS 画成圆角。

1. `src-tauri/tauri.conf.json`：window 加 `"shadow": false`；
   `width` 340 → `340 + 2*SHADOW_PAD`，`height` 64 → `64 + 2*SHADOW_PAD`。
2. `src/style.css`：`#app` 由 `width:100vw; height:100vh` 改为
   `inset: SHADOW_PAD`（即四周各缩进留边），
   `box-shadow` 由内描边 `inset 0 0 0 1px` 改为外投影
   `0 6px 18px rgba(0,0,0,0.34)`（浅色主题换对应深色值）。
3. `src-tauri/src/window_platform.rs`：把运行时测量的 `fn shadow_delta`
   换成常量（留边是我们自己定的，不必再测），并让几何换算认这套留边：
   - `shadow_delta` → 返回 `(-SHADOW_PAD, -SHADOW_PAD, 2*SHADOW_PAD, 2*SHADOW_PAD)`
   - `apply_position(x, y)` → `x - SHADOW_PAD`, `y - SHADOW_PAD`（沿用现有 `x + dx` 写法即可）
   - `apply_size(w, h)` → 需要写 `w + 2*SHADOW_PAD`, `h + 2*SHADOW_PAD`（**现有实现没收这层补偿**）
   - `current_bar_rect` → `inner_position + SHADOW_PAD`、`inner_size - 2*SHADOW_PAD`

`BAR_W`(340) / `BAR_H`(64) / `PANEL_H`(440) 仍是**逻辑条尺寸**，不变；
`snap.rs` 的纯几何计算、吸附、`expanded_rect` 全部照旧工作，因为它们在逻辑坐标里算。

**已知代价（需要接受）**：窗口外那圈留边是透明的但**仍可被点中**（Tauri 默认不做
命中穿透），所以条浮在桌面中间时会挡住它周围约 `SHADOW_PAD` 宽的点击。
条吸附到屏幕边缘时，那一侧的留边在屏幕外，无影响。
`SHADOW_PAD` 取 **16**：够放下 `0 6px 18px` 的投影尾部（底部略削 4px，肉眼不可见），
又不至于让留边过宽。

> 备选（若日后觉得挡点击）：`SHADOW_PAD` 降到 8～10 并改用更紧的
> `0 3px 12px`；或改用方案 A（直接 `"shadow": false` 且不留边，无投影、无留边）。

### ② 音量条吃掉空位（A 案）

`src/style.css` 的 `.volume-wrap` 去掉 `max-width: 78px`（保留 `min-width`）。
56px 死白全部给音量滑块，滑块约从 78px 增至 ~126px，整行铺满。

其余按钮尺寸、顺序、间距一律不动。

### ③ 面板按钮换独立图标

新增 `icons["panel-toggle"]`：向上的 chevron（收起态）/ 向下（展开态），
在 `render()` 里按 `state.panelOpen` 切换，并同步 `title` 与 `aria-label`
（「展开面板」/「收起面板」）。

理由：chevron 与 `icons.list` 视觉差异明显，同时天然表达「开/关」语义，
顺带解决 ⑥ 的可见反馈。

### ④ 设置图标换成滑杆

`src/ui/icons.ts` 的 `settings` 改为三横线 + 错位滑块（sliders），
不再用「圆 + 放射线」。

理由：齿轮在 13px 下容易糊成一团，且当前造型已被读成亮度切换；
滑杆造型在同样尺寸下更清晰，也是常见的设置入口。

### ⑤ 向上展开时折叠条钉在原位

后端把「本次是向上还是向下展开」告诉前端：
- `src-tauri/src/app_core.rs` 的 `UiState` 增加 `panel_grows_up: bool`
  （序列化为 `panelGrowsUp`），与 `expanded_rect` 用同一条判断
  （`bar_center >= work_center`），避免前后端各算一套。
- `src/style.css` 加 `#app.expand-up { flex-direction: column-reverse; }`
- `src/main.ts`（或 `bar/panel` 的 render 处）按
  `state.panelOpen && state.panelGrowsUp` 切该类。

效果：窗口仍向上长、底边钉住，但 DOM 顺序反转 ⇒ 面板在上、条在下，
条回到原来的 `y=952..1016`，不再跳动。

### ⑥ 再点收起

`src/ui/bar.ts` 的 `case "panel"` 改为
`run(ipc.setPanelOpen(!state.panelOpen))`；
`topRow` 的 `dblclick` 同样改为按 `state.panelOpen` 取反。

## 约束

- 不新增依赖
- 保持现有 IPC 命名风格（camelCase 在 JS 侧，snake_case 在 Rust 侧）
- 改动后 `cargo test --lib`、`npx tsc --noEmit`、`npm test` 必须全绿
- `UiState` 加字段属 IPC 契约变更，需同步 `src/state.ts` 的类型定义

## 留给实现者决定

- chevron 的具体路径数据与旋转角度
- 滑杆图标的三条线/滑块具体坐标
- `expand-up` 类挂载点放在 `main.ts` 还是 `bar.ts` 的 render（两处都合理，
  择改动更小者）

## 验证方式

逐项用真实运行实例复核（截图 + CDP 量尺寸），不以「代码看起来对」为准：

| 项 | 判据 |
|----|------|
| ① | 窗口外框应为 `340+2*SHADOW_PAD` × `64+2*SHADOW_PAD`；四角放大截图检查：圆角外侧是无投影的透明，`SHADOW_PAD` 内是**圆角**柔和投影，**不再出现 R163 直角带** |
| ② | CDP 量控件行右端 ≈ 328（逻辑宽 340 减 12 padding） |
| ③ | DOM 中 scope 与 panel 按钮的 `innerHTML` 不同 |
| ④ | `icons.settings` 不再含 8 条放射线 |
| ⑤ | 在右下角开/合面板，折叠条的逻辑 client 坐标不变（`GetWindowRect` 减去 `SHADOW_PAD` 后比对） |
| ⑥ | 连点两次 panel 按钮，`state.panelOpen` 依次 true → false |
| 回归 | 吸附五例（左/右/上/下/自由）仍与屏幕边缘对齐到逻辑坐标；`cargo test --lib` 50 passed；`tsc` exit 0；`vitest` 21 passed |

> ①的验证必须做吸附后的截图：留边让窗口外框比视觉条大一圈，
> 吸附贴边时要确认留边在屏幕外、视觉上仍贴边，没有凭空多出 `SHADOW_PAD` 的缝。


## 实测结果（2026-11-13，发行版 `radiobar.exe` CDP 9231）

| 项 | 结果 | 证据 |
|----|------|------|
| ① | 通过 | 窗口外框 372×96（= 340+32 × 64+32）；四角放大截图 `pad-tl.png`/`pad-br.png` 显示圆角外侧为柔和投影，原 R163 直角带消失；`#app` 为 `inset:16px` 的圆角矩形 |
| ② | 通过 | `check-bar-fill.mjs`：`expectedRight=344`、`lastChildRight=344`、`trailingGapPx=0`（逻辑宽 340 − 12 padding；视口 372 含两侧留边） |
| ③ | 通过 | `check-panel-ui.mjs`：panel 按钮与 scope 按钮 `innerHTML` 不同（`DIFFERENT`）；折叠时显示向上 chevron，展开时显示向下 chevron |
| ④ | 通过 | `check-panel-ui.mjs`：`settings` 图标的 8 条放射线段已消失（`GONE`），改为三横线 + 三个错位滑块圆 |
| ⑤ | 通过 | 窗口在屏幕中部打开面板 → `panelGrowsUp=false`、向下展开、条在顶部；窗口移到底部重新打开 → `panelGrowsUp=true`、`expand-up` 类已挂、条底边与 `#app` 底边对齐（`bar=392..456`，`app=16..456`） |
| ⑥ | 通过 | `check-panel-ui.mjs`：连点两次 panel 按钮，`panelOpen` 依次 `true → false` |
| 回归 | 通过 | `snap-test.ps1`：`SNAP: all 5 cases pass`（left `5,400→0,400`、right `1575,400→1580,400`、top `500,5→500,0`、bottom `500,971→500,976`、free `500,300→500,300`）；`cargo test` 55 passed；`tsc` exit 0；`vitest` 21 passed |

截图：折叠态 `%TEMP%\v1-collapsed.png`（372×96）、中部分开面板 `%TEMP%\v1-panel-down.png`（372×472，条在上）、底部分开面板 `%TEMP%\v1-panel-up.png`（372×472，条在下）。

### 与设计的偏差

- 吸附脚本 `snap-test.ps1` 原先靠 `GetWindowRect` 与 `ClientToScreen` 的差值推算 DWM 阴影偏移；关掉 DWM 阴影后该差值为 0，脚本需改用常量 `-SHADOW_PAD`。这是**验证脚本**的适配，产品代码的 `current_bar_rect`/`outer_position` 一直用 `inner_rect` 换算，无需改动。
- `check-bar-fill.mjs` 的判据从「视口宽 − 12」改为「`.bar` 内容盒右边缘 − padding-right」，因为视口现在含 32px 留边，不再等价于逻辑宽。
- 「留给实现者决定」的三项落地选择：chevron 用 `m7 14.5 5-5 5 5`（向上）与既有 `m7 9.5 5 5 5-5`（向下）成对；滑杆图标用三条横线 `M3.4 7h17.2`/`M3.4 12h17.2`/`M3.4 17h17.2` + `circle r=2.1` 三个错位滑块；`expand-up` 挂在 `src/main.ts` 的 `render`（面板状态本来就在那里统一处理）。

### 已知限制

- 窗口四周 16px 留边**仍是窗口的一部分**，会接收鼠标点击。条浮在桌面中间时，视觉圆角外侧 16px 会挡住下面的桌面点击；吸附到屏幕边缘时那一侧留边在屏幕外，无感。这是「自画阴影」方案的固有代价，已与用户确认后采用。

## 后续修复：吸顶后与屏幕上边缘的 16px 缝隙（2026-11-13）

`## Review Focus` 里列出的「吸附后视觉缝隙」在本设计的验收中只截图验了左右两例，
上边未验，实际是**吸不上**——用户报告「贴边功能 和上方贴边有空隙」。

**根因**：鼠标拖动走 `data-tauri-drag-region` → `startDragging()`（即 HTCAPTION），
Windows 在这条路径上把窗口**外框**顶边钳在屏幕顶（外框 `y >= 0`），而外框比可见条
高出 `SHADOW_PAD` 一圈，于是逻辑 `bar.y` 最低只能到 `16`。`snap.rs` 原阈值
`SNAP_THRESHOLD = 12`，`top_dist(16) < 12` 为假，上边永远不吸附，条静止在 `y=16`，
视觉上正好留 16px 缝。左右/下边系统允许拖出屏幕外，不触发钳制，所以只有上边出问题。

**证据**：
- `%APPDATA%\com.local.radiobar\state.json` 里拖动后落盘 `"barPos":[1422,16]`，`y` 恰好等于 `SHADOW_PAD`。
- 程序化定位不受拖拽钳制：置顶无边框测试窗口请求 `top=-16`，`GetWindowRect` 读回 `top=-16`，
  说明吸附目标 `apply_position(y - 16)` 可达。
- 几何换算本身无残差：外框位置 = `bar - 16`，CSS `#app { inset: 16px }` 再抵消留边，
  可见内容左上精确等于逻辑 `(bar.x, bar.y)`。故逻辑 `y=0` 即视觉贴顶。

**改动**：
- `src-tauri/src/snap.rs`：`SNAP_THRESHOLD` `12 → 20`，注释写明必须大于 `SHADOW_PAD`；
  新增回归测试 `test_snap_top_from_drag_clamped_position`（`y=16` 的条须吸到 `0`）。
- `src-tauri/src/window_platform.rs`：`initial_bar_rect` 恢复记忆位置前先按所在屏工作区
  `snap_bar` 一次，修掉旧 `barPos` 里 `y=16` 在重启后被原样恢复、缝隙永久复现的问题；
  新增测试 `test_snap_threshold_covers_shadow_pad`（`SNAP_THRESHOLD > SHADOW_PAD`）。

**验证**：`cargo test --lib` 61 passed（含两条新测试）。`npm run build` 与
`npm run tauri build` 在受限沙箱下会因 vite 用管道拉起子进程（`exec("net use")`）报
`spawn EPERM`，需放宽权限后执行；构建已通过，release `radiobar.exe` 已产出。
**端到端验证（2026-11-16，真实桌面）**：从仓库外目录启动便携版，用 `SetWindowPos`
把窗口外框移到 `y=0`（等价于用户拖动时被系统钳住的极限位置，逻辑 `bar.y=16`），
等 150ms 去抖后 `GetWindowRect` 读回 `1405,-16,372x96`——吸附把外框拉到 `y=-16`，
可见条顶正好落在屏幕 `y=0`；截屏首行像素即为条的背景色 `#F7F9FA`，无缝隙。
对照：外框 `y=24`（逻辑 `y=40`，超出阈值 20）不会被吸附，位置保持不变。

## 后续修复：展开态与打开面板瞬间同样要吸附（2026-11-16）

上面的修复只覆盖了**折叠态**。用户随后报告：折叠条吸到屏幕上边缘后，**展开列表（面板打开）
条与屏幕顶端仍有 16px 缝隙**。

**根因（三处，同一类错误：吸附只挂在折叠分支上）**：

1. `src-tauri/src/lib.rs` 的 `schedule_snap` 里，`panel_open` 为真时走
   `bar_rect_while_open` 只**重算**展开矩形，**从不调用 `snap_bar`**；只有 `panel_open` 为假
   才走吸附。于是「拖到顶（外框被钳在 `y=0`，逻辑 `bar.y=16`）→ 立刻双击展开」把缝隙原样带进
   展开态，而且面板开着时继续拖动完全不吸附。
2. `apply_panel`（`lib.rs`）与 `run()` 里恢复 `panel_open` 的那次 `apply_panel(&handle, true)`
   都直接拿 `persisted.bar_pos` 算 `expanded_rect`，**没有先 `snap_bar`**。所以打开面板的
   *那一瞬间* 缝隙就已经产生，与去抖线程无关。
3. 顺带发现 `bar_rect_while_open` 用 `previous.y + BAR_H <= rect.y + rect.h && previous.y != rect.y`
   **猜**展开方向。面板开着拖动时窗口矩形每帧都在变，这个推断必然误判（例：向下展开、
   条拖到 `y=100`，会反推出 `y=476`），反推的条位置也就错了。

**改动（几何层未重写，只补吸附调用点与方向来源）**：

- `src-tauri/src/window_platform.rs`：
  - `bar_rect_while_open(window, previous: Rect)` → `bar_rect_while_open(window, grows_up: bool)`，
    方向改用 `AppCore::panel_grows_up` 这个权威值；反推逻辑抽成 `bar_rect_from_window`。
  - 新增 `pub fn snap_bar_to_work(window, bar) -> Rect`（取不到工作区时原样返回），
    折叠/展开两个分支共用同一条吸附路径。
  - 删除已无调用者的 `snap_bar_window`。
- `src-tauri/src/lib.rs`：
  - `apply_panel`：算展开矩形**之前**先 `snap_bar_to_work`，并把吸附后的位置写回
    `core.persisted.bar_pos`（否则下次反推还会从带缝坐标出发）。
  - `apply_effects`：`apply_panel` 会改 `core.persisted`，而 `dispatch` 的落盘快照在它之前克隆，
    所以在 `ResizePanel` 预循环里同步刷新 `persisted`，避免刚吸附掉的缝隙被写回磁盘。
  - `schedule_snap`：两个分支统一 `raw = 反推或读取 → snap_bar_to_work(raw)`；仅当吸附
    真的改变了位置才重写窗口（`apply_panel` 会再触发 `Moved`，无条件重写会让去抖线程自我喂食）。
- `src-tauri/src/snap.rs`：新增回归测试 `test_expanded_after_snap_is_flush_with_top`
  （`bar.y=16` 经 `snap_bar` 后 `expanded_rect.y == 0`）与反向对照
  `test_expanded_without_snap_keeps_gap`（不吸附则 `y == 16`，即被修掉的旧行为）。

**验证**：
- `cargo test --lib`（`TMP`/`TEMP`/`TMPDIR` → `src-tauri\target\testtmp`）`67 passed`（改前 61）。
- `npm run tauri build -- --no-bundle` 通过，release `radiobar.exe` 与交付的便携版 SHA256 一致。
- **端到端（2026-11-16，真实桌面，便携版）**：
  - 折叠态 `SetWindowPos(y=0)`（逻辑 `bar.y=16`）→ **在 150ms 去抖窗口内**双击条顶行 →
    `GetWindowRect` 读回 `1419,-16,372x472`，即外框顶 `y=-16`、可见条顶落在屏幕 `y=0`。
    截屏 `y=0..32` 全为 `#F7F9FA`（条背景），`y=48` 起才出现面板内容，无缝隙。
  - 面板**开着**拖到 `y=0` → 去抖后同样读回 `y=-16`（此前完全不吸附）。
  - 向上展开回归：条放屏幕底部展开得 `T=584`（`1000 - (440-64)`），方向正确；
    面板开着拖到 `y=300` 后 `barPos` 反推为 `[1435,692]`（`300 + 472 - 64 - 16`），无误判。
  - 对照：面板开着拖到外框 `y=24`（逻辑 `y=40` > 阈值 20）不吸附，位置保持不变。

## 后续修复：吸附时机改为「松手后」（2026-11-16）

上一版虽然把缝隙修掉了，但交互手感不对。用户报告：「靠近边缘松开后，会先远离边缘一段
距离，然后闪到边缘吸附」。

**根因**：`schedule_snap` 的去抖线程只判断「位置是否还在变」，不判断**拖动是否已结束**。
鼠标还按着时，`Moved` 停止 150ms 后吸附就执行了，于是：

1. 系统拖动循环（HTCAPTION）每帧按光标位置摆窗口，我们此时用 `set_position` 把窗口
   拽到贴边位置——两者抢同一个窗口位置，观感是窗口从光标下被拽走又被拽回来。
2. 松手后残余的那次吸附再补一刀贴边，就是用户看到的「闪一下」。

**证据（程序化复现，便携版）**：按住左键不放（`mouse_event(0x0002)`，
`GetAsyncKeyState(1)` 确认按下）后用 `SetWindowPos` 把外框摆到 `y=0`，
`GetWindowRect` 每 40ms 采样：`hold 0ms T=0` → `hold 90ms T=-16`。
**左键仍按着，吸附就已经生效**。松手后保持 `-16`。

**改动**：

- `src-tauri/src/lib.rs`：新增 `primary_button_down()`（`GetAsyncKeyState(VK_LBUTTON)` 最高位，
  非 Windows 恒 `false`）。`schedule_snap` 去抖从 150ms 降到 40ms，之后**先等左键松开**
  （每 16ms 轮询一次；期间若代数变了说明有更新的一代在等，本代直接退出，所以同时最多
  一条线程在等），再做吸附计算与写回。吸附逻辑本身（`snap_bar_to_work` / `apply_panel`）
  一行没动。
- `src-tauri/Cargo.toml`：`windows` crate 增加 feature `Win32_UI_Input_KeyboardAndMouse`。

**为什么不用别的做法**：`Resized`/`Moved` 事件里判断不出拖动是否结束；Windows 没有可靠的
「拖动结束」回调（`WM_EXITSIZEMOVE` 只有接管整个拖动循环才收得到，那要重写 `WM_NCHITTEST`，
代价远大于收益）。轮询左键是唯一低成本且准确的信号，且松手后第一次轮询即命中，无感知延迟。

**验证**：
- `cargo test --lib`（`TMP`/`TEMP`/`TMPDIR` → `src-tauri\target\testtmp`）`67 passed`。
- `npm run tauri build -- --no-bundle` 通过；`target\release\radiobar.exe` 与便携版
  SHA256 一致（`9592C588…C3363`）；`bundle\nsis\` 为空，未生成安装包。
- **端到端（2026-11-16，真实桌面，便携版）**：按住左键把外框摆到 `y=0`，
  40ms 间隔采样 480ms——全程 `T=0`，拖动中不再被吸附拉走；松开后首次采样即 `T=-16`。
  四边矩阵（按住中 / 松手后）：上 `0 → -16`、下 `944 → 960`、左 `L=-16`、右 `L=1564`；
  无边缘 `900,400` 松手后不动。面板展开态同样：按住中 `T=0`，松手后 `T=-16`，
  `state.json` 落盘 `barPos=[916,0]`、`panelOpen=true`。
- 未覆盖：吸附仍是瞬移（无过渡动画）。成熟 dock 会有 100~200ms 的缓动，本设计选择不加：
  `set_position` 逐帧插值会让窗口在拖动结束时「滑行」，与系统的无边框拖动观感冲突，
  且需要额外的动画状态机。若后续要做，应在 `snap_bar_to_work` 之后插值，而不是改几何层。

## 后续修复：吸附不再闪（2026-11-17）

用户继续报告：「看起来吸附会闪一下，动画从固定的地方开始，不是我鼠标的位置开始」。
上一节写明的「吸附仍是瞬移」与另一处此前没查到的中间帧撕裂，都是成因。

**根因（两条，都已定位到具体调用）**：

1. **面板态 size / position 分两次写**。`apply_panel` 与启动段原本连着调
   `apply_size` + `apply_position`。tao 0.37.2 的这两条路径各自发一条 `SetWindowPos`
   （`platform_impl/windows/window.rs` 的 `set_outer_position` 带 `SWP_NOSIZE`，
   `util.rs` 的 `set_inner_size_physical` 带 `SWP_NOMOVE`），于是中间必然存在
   「新高度 + 旧位置」的一帧，展开/收起时撕一下。Tauri 的 `Window` 没有合成两者的 API
   （`Webview::set_bounds` 设的是 webview 在窗口内的范围，不是窗口本身）。
2. **16px 瞬跳**。外框被系统钳在屏幕边，逻辑条位置从 `y=16` 跳到 `y=0`，可见条整块瞬移。
   终点只由工作区几何决定，所以「从固定的地方开始」——那正是吸附目标，不是鼠标位置。

**改动**：

- `src-tauri/src/window_platform.rs`：新增 `apply_frame(window, rect)`，直接发一条
  `SetWindowPos(hwnd, None, x, y, cx, cy, SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOACTIVATE)`
  同时写尺寸与位置。坐标用外框物理像素（`outer_position` / `outer_size` × `scale_factor`）；
  `HWND` 从 tauri 依赖的 windows 0.62 转成本项目的 0.58（`HWND(hwnd.0)`）。
  `apply_panel` 展开/收起两分支与 `lib.rs` 启动段改用它。顺带删掉从未被调用的
  `current_outer_position` / `set_outer_position`。
- `src-tauri/src/lib.rs`：新增 `glide_to(window, state, from, to, panel_open)`，
  `const STEPS: u32 = 6`（6 × 16ms ≈ 96ms），用 `snap::eased_offset` 逐帧 `apply_position`。
  期间 `snap_animating` 置真，让缓动自己触发的 `Moved` 回响不去顶掉代数（否则会把自己打断）；
  每帧前检查 `primary_button_down()`，用户重新按住拖动就立刻停在原地让位。
  面板态滑完后再 `apply_panel` 一次对齐尺寸与展开方向；折叠态最后一帧已落在终点，不再重写。
  `glide_to` 返回窗口**真正停下的矩形**，`schedule_snap` 拿它落盘——被打断时停在半路，
  若仍写吸附终点，下次启动会跳到从没到过的位置。

**为什么是 6 帧 × 16ms**：位移只有 16px，96ms 的 ease-out 已经能读出「滑过去」而不是「跳过去」，
再长就会和系统的无边框拖动观感打架（这正是上一节当初拒绝加缓动的原因）。上限由
`SNAP_THRESHOLD`(20) 决定，故时长可以硬编码。

**验证**：
- `cargo test --lib`（`TMP`/`TEMP`/`TMPDIR` → `src-tauri\target\testtmp`）`68 passed`，
  含既有 `test_eased_offset_is_monotonic_and_bounded`、`test_snap_threshold_covers_shadow_pad`。
- `npm run tauri build -- --no-bundle` 通过。
- **端到端（2026-11-17，真实桌面，便携版）**，`GetWindowRect` 2ms 采样：
  - 按住左键把外框摆到 `y=0`、持续采样：`T` 恒为 `0`，拖动中不吸。
  - 松手后逐帧：`0 → -5 → -9 → -12 → -14 → -16`（约 97ms 到底），缓动生效。
  - 面板态（`H=472`）同上：`0 → -5 → -9 → -12 → -14 → -16`，尺寸全程 `472` 不变。
  - 展开：`T=300/H=96` → `T=300/H=472` 单帧跳变，中间无「新高度 + 旧位置」帧；收起同理。
  - 缓动中途按住打断：窗口停在 `T=-12`，`state.json` 落盘 `barPos=[616,4]`
    （= 逻辑 `y=4` + `SHADOW_PAD`，与停下位置一致），不再写吸附终点。

## 全局快捷键（2026-11-16 补充文档）

播放器常驻托盘、多数时间不在前台，所以控制入口不能只有窗口内的按钮。全局快捷键
（`tauri-plugin-global-shortcut`）让折叠条隐藏时也能换台、调音量、播放暂停。

**默认键位**（`src-tauri/src/shortcuts.rs` 的 `DEFAULT_SHORTCUTS`）：

| 动作 | action | 默认键位 |
|------|--------|----------|
| 上一台 | `prev` | `control+ArrowLeft` |
| 下一台 | `next` | `control+ArrowRight` |
| 音量 + | `volumeUp` | `control+ArrowUp` |
| 音量 − | `volumeDown` | `control+ArrowDown` |
| 播放 / 暂停 | `playPause` | `control+KeyP` |

音量步长 `VOLUME_STEP = 5`（与折叠条滑块的 5 步一致）。加速键字符串是
`修饰键 + event.code`（`control+shift+F9`、`control+ArrowLeft`），**不是显示文本**；
显示用的 `Ctrl + ←` 由 `src/ui/panel.ts` 的 `formatAccel` 从 `event.code` 渲染。

**约束**：

- 组合键必须带修饰键（`Ctrl`/`Alt`/`Shift`/`Win` 至少一个），`normalize` 要求含 `+` 且
  `Shortcut::from_str` 能解析；否则拒绝。
- 五个动作之间不允许重复，`rebind` 查重失败时只 emit `toast`（`level: "warn"`），
  不改 `state.json`、不回滚已生效的注册。
- 改绑结果立即生效并写入 `state.json` 的 `shortcuts`，重启后由 `resolved()` 叠加到默认表；
  存量的非法值回落默认，未知动作丢弃。
- 注册（`install`）必须跑在后台线程：插件内部 `run_on_main_thread` + `rx.recv()`，
  主线程调用会自锁。
- 录制改键期间必须 `suspend()`（`unregister_all`）再 `restore()`，否则按下的组合会被
  全局钩子先吃掉，页面收不到 `keydown`，录制永远失败。

**真机验证（2026-11-16，便携版 + CDP 9231）**：

1. 录制链路：点设置页 `[data-shortcut="prev"]` → 按钮变「按下组合键…」→ 发 `Ctrl+Shift+F9`
   → `shortcuts.prev = control+shift+F9`，按钮显示 `Ctrl + Shift + F9`。
2. 重复键位：把 prev 改成已被 next 占用的 `control+ArrowRight` → `shortcuts` 完全不变，
   收到 toast「control+ArrowRight 已经分配给了「下一台」」，按钮回到原显示。
3. `Esc` 取消：键位不变，按钮恢复。
4. 录制态释放全局键：录制 next 时页面收到 `keydown`（`code=ArrowRight`），且当前电台
   **未变化**——证明按键没被系统吞掉也没触发动作；完成后 next 显示 `Ctrl + →`。
5. 改绑后系统级生效：`keybd_event` 发 `Ctrl+Shift+F9`，焦点已 `MinimizeAll` 到桌面，
   当前电台由 Classic Vinyl HD 变为 Radio Armisa。
6. 默认键位未被破坏：`Ctrl+→` 换台成功；`Ctrl+↑` 音量 20 → 25。
7. 持久化：`state.json` 的 `shortcuts` 含 `"prev": "control+shift+F9"`；重启便携版后
   `get_state` 仍返回该值，且再发 `Ctrl+Shift+F9` 仍能换台。

验证脚本在 `.superpowers/`：`send-hotkey.ps1`（`keybd_event` 系统级发键，参数走
`RB_PORT`/`RB_COMBO`/`RB_WAITMS`/`RB_FIELD` 环境变量，先 `MinimizeAll()` 抢走焦点）、
`capture-suspend.mjs`、`duplicate-rebind.mjs`、`read-state.mjs`、`press-key.mjs`。
`.ps1` 被执行策略拒绝时用 `Get-Content $path -Raw | Invoke-Expression` 跑，
且脚本内容必须是纯 ASCII（`Invoke-Expression` 按 ANSI 解码，中文注释会乱码）。
