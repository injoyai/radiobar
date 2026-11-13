# radiobar 折叠条 UI 打磨 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修掉用户提出的 6 条折叠条/面板 UI 问题（圆角边线、右侧空白、图标撞车、设置图标像日夜模式、面板向上展开时条子跳动、面板不能收起）。

**Architecture:** 纯前端改动为主（CSS + SVG 图标 + 点击逻辑），加三处后端配合：`UiState` 增 `panelGrowsUp` 字段、窗口四周留 `SHADOW_PAD` 透明边以自画圆角投影、几何换算改认这套留边。

**Tech Stack:** TypeScript（无前端框架）、原生 CSS、Tauri 2 / Rust、vitest、cargo test。

**Spec:** `docs/superpowers/specs/2026-11-12-bar-ui-polish-design.md`

## Global Constraints

- 不新增任何 npm / cargo 依赖。
- 逻辑条尺寸常量保持 `BAR_W = 340`、`BAR_H = 64`、`PANEL_H = 440` 不变；`src-tauri/src/snap.rs` 的几何计算一律在逻辑坐标里跑，不感知留边。
- `SHADOW_PAD = 16`（窗口四周透明留边，用于画圆角投影）。
- IPC 命名：Rust 侧 `panel_grows_up`（`#[serde(rename_all = "camelCase")]` 已作用于 `UiState`），前端类型侧 `panelGrowsUp`。
- 每步结束 `cargo test --lib`、`npx tsc --noEmit`、`npm test` 必须全绿。
- 跑 `cargo test` 前须设 `TEMP`/`TMP`/`TMPDIR` 指向 `.superpowers\sdd\2026-11-10-radio-player\tmp-test`（本机 `%TEMP%` 残留会导致 `tempfile` 报 PermissionDenied）。

## Review Focus

- **透明留边吞点击**：窗口比视觉条大一圈，留边区域仍接收鼠标事件。条浮在桌面中间时用户点条周围 16px 会打空到我们的窗口上。期望：至少吸附贴边时那一侧留边在屏幕外、无感。
- **吸附后视觉缝隙**：留边让外框比视觉条大，贴边后可能出现 16px 空隙。期望：视觉上仍紧贴边缘。
- **多显示器负坐标**：`snap.rs` 已支持负坐标显示器，加留边后换算若漏减 `SHADOW_PAD`，副屏上会整体偏移。
- **面板向上展开的判定与后端一致性**：前端 `panelGrowsUp` 与后端 `expanded_rect` 必须同一条判断，否则会出现条子跳但窗口不长、或反之。
- **面板关闭时留边状态复位**：`expand-up` 类若在关闭时未清除，下次打开会继承错误方向。

---

## File Structure

| 文件 | 职责 | 本次动作 |
|------|------|----------|
| `src-tauri/tauri.conf.json` | 窗口声明 | 改：`shadow: false`、尺寸加留边 |
| `src-tauri/src/window_platform.rs` | 窗口几何换算 | 改：常量留边替代运行时测量；`apply_panel` 返回方向 |
| `src-tauri/src/state.rs` | `UiState` 定义 | 改：增 `panel_grows_up`（**注意 `UiState` 在 `state.rs:55`，不在 `app_core.rs`**） |
| `src-tauri/src/app_core.rs` | 状态机 | 改：`AppCore` 增字段 + `ui_state()` 产出 |
| `src-tauri/src/lib.rs` | 生命周期/托盘 | 改：`apply_panel` 接住方向并回写 `core.panel_grows_up` |
| `src/style.css` | 全部样式 | 改：`#app` 留边 + 外投影、`expand-up`、音量条去上限 |
| `src/state.ts` | 前端类型 | 改：`UiState` 增 `panelGrowsUp` |
| `src/ui/icons.ts` | SVG 图标 | 改：新增 `panel-toggle`，重画 `settings` |
| `src/ui/bar.ts` | 折叠条 | 改：panel 按钮图标与 toggle 逻辑 |
| `src/main.ts` | 组装 | 改：按 state 切 `expand-up` 类 |

---

### Task 1: 音量条吃掉右侧空白（问题 ②）

**Files:**
- Modify: `src/style.css:246-253`（`.volume-wrap`）

**Interfaces:**
- Consumes: 无
- Produces: 无（纯样式）

- [ ] **Step 1: 改 `.volume-wrap`**

把 `max-width: 78px` 删掉，保留 `min-width: 58px`。改后该规则为：

```css
.volume-wrap {
  flex: 1 1 auto;
  min-width: 58px;
  display: flex;
  align-items: center;
  gap: 1px;
}
```

- [ ] **Step 2: 构建并重启，量右侧是否铺满**

Run:
```powershell
npm run build
```
然后用带 CDP 的方式启动应用，执行：
```powershell
node geo-probe.mjs 9226
```
Expected: `buttons` 里 `pin.r` 从 `284` 变成 `>= 320`，`lastRight` 接近 `328`（`appRight` 减 12 的右 padding）。

- [ ] **Step 3: 目视确认**

截折叠条 `356x72` 区域，确认右侧不再有 56px 死白，音量滑块明显变长。

- [ ] **Step 4: Commit**

```bash
git add src/style.css
git commit -m "style: let volume slider fill the collapsed bar's trailing space"
```

---

### Task 2: 面板按钮换独立图标 + 可收起（问题 ③ 与 ⑥）

**Files:**
- Modify: `src/ui/icons.ts`（新增 `panel-toggle`）
- Modify: `src/ui/bar.ts:81`（按钮图标）、`:211-213`（点击逻辑）、`:236-239`（双击逻辑）、`render()` 内同步图标/标题

**Interfaces:**
- Consumes: `icons` 常量对象、`ipc.setPanelOpen(open: boolean)`
- Produces: `icons["panel-toggle"]`（`string`，SVG 标记）

- [ ] **Step 1: 加图标**

在 `src/ui/icons.ts` 的 `icons` 对象里，`chevron` 之后加一条：

```ts
  "panel-toggle": svg('<path d="m7 14 5-5 5 5"/>'),
```

与已有 `chevron`（`m7 9.5 5 5 5-5`，向下）区分：`panel-toggle` 是向上箭头。

- [ ] **Step 2: 改按钮初始图标**

`src/ui/bar.ts:81` 把 `icons.list` 换成 `icons["panel-toggle"]`：

```html
<button class="icon-btn" type="button" data-act="panel" title="展开面板" aria-label="展开面板">${icons["panel-toggle"]}</button>
```

- [ ] **Step 3: 取 panel 按钮引用**

在 `src/ui/bar.ts:92-100` 那批 `element<HTMLButtonElement>(...)` 里、`pinBtn` 之前加：

```ts
  const panelBtn = element<HTMLButtonElement>(bar, '[data-act="panel"]');
```

- [ ] **Step 4: 在 `render()` 里按 `panelOpen` 切朝向与文案**

在 `render(state: UiState)` 内、`pinBtn` 那两行之前插入（`panelBtn` 已在 Step 3 取到）：

```ts
    panelBtn.innerHTML = state.panelOpen ? icons.chevron : icons["panel-toggle"];
    panelBtn.title = state.panelOpen ? "收起面板" : "展开面板";
    panelBtn.setAttribute("aria-label", state.panelOpen ? "收起面板" : "展开面板");
    panelBtn.setAttribute("aria-expanded", String(state.panelOpen));
```

- [ ] **Step 5: 点击改为 toggle**

`src/ui/bar.ts:211-213` 改成：

```ts
      case "panel":
        if (state) run(ipc.setPanelOpen(!state.panelOpen));
        break;
```

- [ ] **Step 6: 双击也改为 toggle**

`src/ui/bar.ts:236-239` 改成：

```ts
  topRow.addEventListener("dblclick", (event) => {
    if ((event.target as Element).closest("[data-act]")) return;
    const state = currentState;
    if (state) run(ipc.setPanelOpen(!state.panelOpen));
  });
```

- [ ] **Step 7: 类型检查 + 构建**

Run:
```powershell
npx tsc --noEmit
npm run build
```
Expected: `tsc` exit 0；build 成功。

- [ ] **Step 8: 实测连点两次**

用 CDP 连点 panel 按钮两次，每次后读 `get_state`：

Run:
```powershell
node acceptance.mjs 9226 state
```
Expected: 第一次点击后 `panelOpen: true`，第二次后 `panelOpen: false`。

同时确认 DOM 中 scope 与 panel 按钮的 `innerHTML` 不同（`node geo-probe.mjs 9226` 里可加一条断言，或直接读 `document.querySelector('[data-act="scope"]').innerHTML !== document.querySelector('[data-act="panel"]').innerHTML`）。

- [ ] **Step 9: Commit**

```bash
git add src/ui/icons.ts src/ui/bar.ts
git commit -m "feat: give the panel button its own chevron and make it toggle"
```

---

### Task 3: 设置图标换成滑杆（问题 ④）

**Files:**
- Modify: `src/ui/icons.ts:58-61`（`settings`）

**Interfaces:**
- Consumes: `svg(body)` 工厂
- Produces: `icons.settings`（`string`）

- [ ] **Step 1: 重画 `settings`**

把 `settings` 的 body 从「圆 + 8 条放射线」换成三横线 + 错位滑块：

```ts
  settings: svg(
    '<path d="M4 7h16"/><path d="M4 12h16"/><path d="M4 17h16"/>' +
      '<circle cx="9" cy="7" r="2.2"/><circle cx="15" cy="12" r="2.2"/><circle cx="8" cy="17" r="2.2"/>',
  ),
```

- [ ] **Step 2: 构建 + 目视**

Run:
```powershell
npm run build
```
打开面板，目视设置按钮（`.panel-foot` 右侧），确认不再是太阳造型，三条线 + 三个滑块点在 13px 下清晰可辨。

- [ ] **Step 3: 确认不再含放射线**

Run:
```powershell
node -e "const s=require('fs').readFileSync('src/ui/icons.ts','utf8'); const m=s.match(/settings: svg\(([\s\S]*?)\),/); console.log(m[1]);"
```
Expected: 输出里没有 `M12 3.2v2.4` 这类放射线段，只有 `h16` 横线与 `circle`。

- [ ] **Step 4: Commit**

```bash
git add src/ui/icons.ts
git commit -m "style: replace the sun-like settings icon with sliders"
```

---

### Task 4: 窗口留边 + CSS 自画圆角投影（问题 ①，后端几何）

**Files:**
- Modify: `src-tauri/tauri.conf.json`（window 配置）
- Modify: `src-tauri/src/window_platform.rs:5-6`（常量）、`:41-58`（`shadow_delta`）、`:72-79`（`apply_position`）、`:81-86`（`apply_size`）、`:60-70`（`current_bar_rect`）

**Interfaces:**
- Consumes: 无
- Produces: `pub const SHADOW_PAD: i32 = 16`；`shadow_delta` 改为常量返回；`apply_size` / `apply_position` / `current_bar_rect` 全部认留边

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/window_platform.rs` 末尾加 `#[cfg(test)] mod tests`（该文件当前没有测试）。测试只覆盖不依赖窗口实例的纯计算部分——把留边换算抽成纯函数：

先在文件里加：

```rust
/// 逻辑尺寸 → 外框尺寸（四周各加 SHADOW_PAD）。
pub fn outer_size(w: i32, h: i32) -> (i32, i32) {
    (w + SHADOW_PAD * 2, h + SHADOW_PAD * 2)
}

/// 逻辑位置 → 外框位置（左上各减 SHADOW_PAD）。
pub fn outer_position(x: i32, y: i32) -> (i32, i32) {
    (x - SHADOW_PAD, y - SHADOW_PAD)
}

/// 外框矩形 → 逻辑矩形（去掉四周留边）。
pub fn inner_rect(x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
    (
        x + SHADOW_PAD,
        y + SHADOW_PAD,
        w - SHADOW_PAD * 2,
        h - SHADOW_PAD * 2,
    )
}
```

测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outer_size_adds_padding_on_all_sides() {
        assert_eq!(outer_size(BAR_W, BAR_H), (340 + 32, 64 + 32));
        assert_eq!(outer_size(BAR_W, PANEL_H_FOR_TEST), (372, 472));
    }

    #[test]
    fn test_outer_position_subtracts_padding() {
        assert_eq!(outer_position(100, 200), (84, 184));
        assert_eq!(outer_position(0, 0), (-16, -16));
    }

    #[test]
    fn test_inner_rect_roundtrips_with_outer_helpers() {
        let (ox, oy) = outer_position(1234, 952);
        let (ow, oh) = outer_size(BAR_W, BAR_H);
        assert_eq!(inner_rect(ox, oy, ow, oh), (1234, 952, BAR_W, BAR_H));
    }
}
```

`PANEL_H_FOR_TEST` 用 `use crate::snap::PANEL_H;` 引入后直接写 `PANEL_H`（440），不要新增常量——测试里写成 `outer_size(BAR_W, PANEL_H) == (372, 472)` 即可。

- [ ] **Step 2: 跑测试确认失败**

Run:
```powershell
cd src-tauri; cargo test window_platform -q
```
Expected: 编译失败 `cannot find function outer_size`（还没实现时若已加上函数则改为先删实现再跑；关键是先看到红）。

- [ ] **Step 3: 实现三个纯函数并把现有函数改为用它们**

- `src-tauri/src/window_platform.rs:5-6` 后加 `pub const SHADOW_PAD: i32 = 16;`
- `fn shadow_delta(window: &WebviewWindow) -> (i32, i32, i32, i32)` 改为返回 `(-SHADOW_PAD, -SHADOW_PAD, SHADOW_PAD * 2, SHADOW_PAD * 2)`，去掉运行时测量（保留函数签名以免调用点全改）。
- `apply_position`：用 `outer_position(x, y)`。
- `apply_size`：改用 `outer_size(w, h)`（**这是当前缺失的补偿**）。
- `current_bar_rect`：用 `inner_rect(...)` 包住 `inner_position` + `inner_size` 的结果。

- [ ] **Step 4: 跑测试确认通过**

Run:
```powershell
cd src-tauri; cargo test window_platform -q
```
Expected: 3 passed。

- [ ] **Step 5: 改 `tauri.conf.json`**

window 配置加 `"shadow": false`，并把尺寸改为含留边：

```json
"width": 372,
"height": 96,
```

（`340+32 = 372`、`64+32 = 96`。）其余字段不动。

- [ ] **Step 6: 跑全量 Rust 测试**

Run:
```powershell
cd src-tauri; cargo test -q
```
Expected: 53 passed（原 50 + 新增 3）。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/window_platform.rs
git commit -m "feat: reserve a transparent margin so the window shadow can be rounded"
```

---

### Task 5: `#app` 留边 + 外投影（问题 ①，前端样式）

**Files:**
- Modify: `src/style.css:79-89`（`#app`）

**Interfaces:**
- Consumes: `SHADOW_PAD = 16`（与 Task 4 同值，CSS 里写字面量）
- Produces: `#app` 的 `inset: 16px` 与圆角外投影

- [ ] **Step 1: 改 `#app`**

```css
#app {
  position: absolute;
  inset: 16px;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  border-radius: 14px;
  overflow: hidden;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.34);
}
```

去掉原来的内描边 `inset 0 0 0 1px var(--stroke)`。`--shadow` 变量保持原样不动（toast 还在用）。

- [ ] **Step 2: 浅色主题换对应投影**

在已有的 `@media (prefers-color-scheme: light)` 块里加：

```css
  #app {
    box-shadow: 0 6px 18px rgba(20, 26, 31, 0.18);
  }
```

- [ ] **Step 3: 构建并截图四角**

Run:
```powershell
npm run build
```
启动后截窗口外框（应为 372×96），放大四角，预期：

- 圆角外侧是透明（桌面可见）
- 投影沿圆角弯曲，**不再有 R163 的直角带**

- [ ] **Step 4: 确认内部布局未受影响**

Run:
```powershell
node geo-probe.mjs 9226
```
Expected: `viewport.w` 仍为 `340`（`#app` 缩小不影响 webview 视口，但 `.bar` 宽度需仍为 340-24 padding 的关系成立），`controls.r` 仍是 `328`。

若 `viewport` 变成 372 说明窗口尺寸换算有误，回 Task 4 检查 `apply_size`。

- [ ] **Step 5: Commit**

```bash
git add src/style.css
git commit -m "style: draw a rounded shadow inside the reserved margin"
```

---

### Task 6: 向上展开时折叠条钉住 + `panelGrowsUp` 状态（问题 ⑤）

**Files:**
- Modify: `src-tauri/src/state.rs:53-72`（`UiState` 结构体——**注意它在这里，不在 `app_core.rs`**）
- Modify: `src-tauri/src/app_core.rs`（`AppCore` 字段、`new_seeded`、`ui_state()`）
- Modify: `src-tauri/src/window_platform.rs`（`apply_panel` 返回方向）
- Modify: `src-tauri/src/lib.rs:104-126`（`apply_panel` 函数接住方向并回写）
- Modify: `src/state.ts`（`UiState` 类型）
- Modify: `src/style.css`（`#app.expand-up`）
- Modify: `src/main.ts`（切类）

**Interfaces:**
- Consumes: `snap::expanded_rect` 的判断条件；`BAR_H`；`current_work_rect(window) -> Option<Rect>`
- Produces: `UiState.panel_grows_up: bool`（JSON `panelGrowsUp`）；`#app.expand-up` 类；`window_platform::apply_panel(...) -> (Rect, bool)`

- [ ] **Step 1: 写失败测试（Rust）**

`src-tauri/src/app_core.rs` 的 `#[cfg(test)] mod tests` 里已有辅助 `fn core_with(scope: Scope) -> AppCore`（`src-tauri/src/app_core.rs:454`）。用它写：

```rust
#[test]
fn test_ui_state_reports_panel_grows_up() {
    use crate::model::Scope;
    let mut core = core_with(Scope::All);
    core.panel_grows_up = true;
    assert!(core.ui_state().panel_grows_up);
    core.panel_grows_up = false;
    assert!(!core.ui_state().panel_grows_up);
}
```

- [ ] **Step 2: 跑测试确认失败**

Run:
```powershell
cd src-tauri; cargo test panel_grows_up -q
```
Expected: 编译失败，`no field panel_grows_up`。

- [ ] **Step 3: 加字段并产出**

`src-tauri/src/state.rs:53-72` 的 `UiState` 加（该结构体已有 `#[serde(rename_all = "camelCase")]`，JSON 自动输出 `panelGrowsUp`）：

```rust
    pub panel_grows_up: bool,
```

`src-tauri/src/app_core.rs:26` 的 `AppCore` 加同名字段，`new_seeded`（`:42`）里初始化为 `false`。

`ui_state()`（`src-tauri/src/app_core.rs:67-104`）的 `UiState { ... }` 字面量里加：

```rust
            panel_grows_up: self.panel_grows_up,
```

- [ ] **Step 4: 跑测试确认通过**

Run:
```powershell
cd src-tauri; cargo test panel_grows_up -q
```
Expected: PASS。

- [ ] **Step 5: 在后端算出方向并回写**

`src-tauri/src/window_platform.rs:131` 的 `apply_panel` 现在签名是 `pub fn apply_panel(window: &WebviewWindow, bar: Rect, open: bool) -> Rect`。改为：

```rust
pub fn apply_panel(window: &WebviewWindow, bar: Rect, open: bool) -> (Rect, bool) {
```

`open == false` 时把三处 `return`/结尾都返回 `(bar, false)`。`open == true` 时用与 `snap::expanded_rect` **完全相同**的判断：

```rust
let grows_up = match current_work_rect(window) {
    Some(work) => bar.y + BAR_H / 2 >= work.y + work.h / 2,
    None => false,
};
```

结尾返回 `(bar, grows_up)`。

`src-tauri/src/lib.rs:125` 的调用点接住返回值：

```rust
    let (_bar, grows_up) = window_platform::apply_panel(&window, bar, open);
    let state = app.state::<AppState>();
    let mut core = state.core.lock().unwrap();
    core.panel_grows_up = grows_up;
```

（`open == false` 时 `grows_up` 为 `false`，自动复位；这一步同时覆盖了 Review Focus 里「面板关闭时留边状态复位」。）

`src-tauri/src/lib.rs:324` 的 `apply_panel(&handle, true)` 是启动时恢复面板，走同一个 `apply_panel` 函数，无需另改。

- [ ] **Step 6: 前端类型补字段**

`src/state.ts` 的 `UiState` interface 加：

```ts
  panelGrowsUp: boolean;
```

- [ ] **Step 7: 加 CSS 类**

`src/style.css` 在 `#app` 规则之后加：

```css
#app.expand-up {
  flex-direction: column-reverse;
}
```

- [ ] **Step 8: `main.ts` 切类**

`src/main.ts:22-33` 的 `render(state)` 里，`bar.render(state)` **之前**加：

```ts
  app.classList.toggle("expand-up", state.panelOpen && state.panelGrowsUp);
```

- [ ] **Step 9: 类型检查 + 全量测试**

Run:
```powershell
npx tsc --noEmit
cd src-tauri; cargo test -q
cd ..; npm test
```
Expected: tsc exit 0；cargo 54 passed；vitest 21 passed。

- [ ] **Step 10: 实测条子不再跳**

应用在右下角（`y=952` 附近）时开面板。用 `GetWindowRect` 记录前后窗口矩形，按 `inner_rect` 换算成逻辑坐标：

Expected: 折叠条的**逻辑** y 在开/合前后一致（仍是 952 附近）；窗口外框底边钉住不变；面板出现在条的上方。

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/app_core.rs src-tauri/src/lib.rs src-tauri/src/window_platform.rs src/state.ts src/style.css src/main.ts
git commit -m "feat: keep the bar anchored and grow the panel upward when near the bottom"
```

---

### Task 7: 回归验证与交付记录

**Files:**
- Modify: `docs/superpowers/specs/2026-11-12-bar-ui-polish-design.md`（补实测结果；如与设计不符则记录差异）

**Interfaces:**
- Consumes: 前 6 个任务的全部产出
- Produces: 验证记录

- [ ] **Step 1: 吸附五例回归**

Run:
```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .superpowers\sdd\2026-11-10-radio-player\snap-test.ps1
```
Expected: `SNAP: all 5 cases pass`，且贴边后视觉上无 16px 缝隙（截图确认左右两例）。

- [ ] **Step 2: 六条问题逐项目视验收**

截三张图：折叠态、面板展开（向下展开，窗口在中部）、面板展开（向上展开，窗口在底部）。逐条对照设计与 Review Focus。

- [ ] **Step 3: 全量测试**

Run:
```powershell
npx tsc --noEmit
cd src-tauri; cargo test -q
cd ..; npm test
```
Expected: 全绿。

- [ ] **Step 4: 记录实测结果**

在 spec 的「验证方式」表格后追加一小节，写实际数字（控件行右端、窗口外框尺寸、条子逻辑 y 前后值、截图路径）。

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-11-12-bar-ui-polish-design.md
git commit -m "docs: record UI polish verification results"
```
