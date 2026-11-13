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