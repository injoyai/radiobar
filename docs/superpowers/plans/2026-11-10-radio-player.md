# 电台播放器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个 Windows 可安装的 Tauri 2 电台播放器：340×64 折叠条 + 340×440 面板、托盘常驻、边缘吸附、置顶、精选 1 万台内置电台库 + 增量同步、三种播放模式 × 三种切换范围、收藏。

**Architecture:** Rust 核（`src-tauri/src/`）是唯一真相源：`model`/`state`/`library`/`queue`/`snap`/`sync`/`storage` 为纯逻辑（可 `cargo test` 单测），`app_core.rs` 把一次操作编译成 `Vec<CoreEffect>`，`commands.rs` 负责 Tauri 薄封装与事件推送。前端（`src/`）无框架 TS：只渲染 `UiState` 并把音频错误上报；播放用 `<audio>` + hls.js（仅 `.m3u8`）。

**Tech Stack:** Tauri 2（tray-icon/image-png、tauri-plugin-single-instance）、serde/serde_json、reqwest(rustls-tls)、tokio、rand、thiserror、windows crate（工作区矩形）；vite + typescript + hls.js + vitest；NSIS 打包；`identifier: com.local.radiobar`。

**Spec:** `docs/superpowers/specs/2026-11-10-radio-player-design.md`（行为以该文档 §9/§16 为准，本计划只补实现决策）

## Global Constraints

- 平台：Windows 10/11 x64 为 v1 验收目标；所有 `cfg(windows)` 只允许出现在 `window_platform.rs` 与 `snap.rs` 的调用处，其余模块保持跨平台可编译（M8 为 macOS 留口）。
- 数据源：radio-browser（`https://de1.api.radio-browser.info`），请求必须带 `User-Agent: radiobar/0.1`。数据 CC0。
- 性能预算：安装包 < 15 MB、常驻内存 < 120 MB、冷启动可操作 < 1.5 s、空闲 CPU < 1%。
- 内置快照约 1 万台（每国 top50 + 全球 top500 去重），`stations.local.json` 首次启动从 seed 复制，之后只增量合并，不主动扩到 52k 全量。
- 界面：折叠条 340×64，面板 340×440；无系统标题栏；圆角 14；深色 `#141a1f`/`#e8eef2`/`#3ecfb2`，浅色跟随系统；中文 UI。
- 播放模式三态：`sequential`（到末尾停住）/ `loop`（回绕）/ `shuffle`（本轮不重复，prev 回放历史）；范围三态：`favorites` / `list` / `all`。
- 失败策略：单台最多两次尝试（`urlResolved` 一次 → 原始 `url` 一次）；连续失败 3 台停播；`lastcheckok == false` 的台不进队列但进列表。
- 持久化只写 `app_data_dir`（`%APPDATA%\com.local.radiobar\`）：`state.json` 与 `stations.local.json`，均为临时文件 + rename 原子写。
- 每次提交只包含该任务的文件；`main` 分支；提交信息用 `feat:`/`fix:`/`chore:` 前缀。
- 构建与 `cargo`/`rustup` 需要完全权限（受限 shell 的 schannel 被沙箱挡住）；npm 用 `registry.npmmirror.com`，crates 用 `rsproxy.cn` sparse 镜像。

## Review Focus

以下输入/失败模式不在任何单测正文里，但必须各由某个任务的测试钉住（已在对应任务列出）：

1. **能连上但永远不出声的流**（HTTP 200、无数据、不报错）：8 s `stalled` 必须走失败流程，否则界面显示"播放中"却无声。→ Task 8 `test_stall_triggers_retry_then_switch`。
2. **HLS 判定**：`https://x/y.m3u8?token=...`、大写 `.M3U8`、以及无后缀的 HLS 流；判错会把 m3u8 当 MP3 交给 `<audio>`。→ Task 8 `test_is_hls_url`。
3. **台名含 emoji/中日韩/组合字符**：截断必须按字符（grapheme）而非 UTF-16 单元，否则会把 emoji 劈成乱码。→ Task 11 `truncate.test.ts`。
4. **同步中途失败**：`changed` 分页到第 3 页网络断了，必须保留旧库、不写半成品、`last_sync` 不前移。→ Task 14 `test_sync_aborts_without_consuming_cursor`。
5. **面板贴底展开**：工作区比面板矮（小屏/任务栏占位）时，向上生长仍要整窗可见；否则窗口上半截跑到屏外，用户按不到按钮。→ Task 5 `test_expanded_clamped_in_short_work_area`。

---

## File Structure

```
F:\桌面\项目\电台播放器\
├─ package.json / vite.config.ts / tsconfig.json / index.html
├─ scripts/
│  ├─ build-seed.mjs          # 生成 stations.seed.json（8 并发，失败国家跳过）
│  └─ build-countries.mjs     # 生成 countries.zh.json（Node Intl.DisplayNames）
├─ src/                       # 前端（无框架 TS）
│  ├─ main.ts                 # 启动、事件订阅、状态镜像
│  ├─ state.ts                # 前端 UiState 类型 + 渲染入口
│  ├─ ipc.ts                  # invoke 封装 + 事件监听
│  ├─ ui/bar.ts               # 折叠条渲染与绑定
│  ├─ ui/panel.ts             # 面板：搜索/国家/列表/收藏/同步/设置
│  ├─ ui/icons.ts             # 内联 SVG 图标常量
│  ├─ format.ts               # 国旗、截断、码率文案、进度（纯函数，vitest）
│  ├─ audio.ts                # <audio> + hls.js，错误/stall 上报
│  └─ style.css
├─ src-tauri/
│  ├─ Cargo.toml / build.rs / tauri.conf.json
│  ├─ capabilities/default.json
│  ├─ icons/                  # 由 tauri icon 生成
│  ├─ resources/stations.seed.json / countries.zh.json
│  └─ src/
│     ├─ main.rs              # 仅调用 radiobar_lib::run()
│     ├─ lib.rs               # Tauri Builder、托盘、单实例、插件注册
│     ├─ model.rs             # Station / PlayMode / Scope / ListFilter
│     ├─ state.rs             # PersistedState / UiState / 默认值
│     ├─ storage.rs           # 原子读写 JSON
│     ├─ library.rs           # 载入/搜索/国家/收藏/合并
│     ├─ queue.rs             # 模式×范围的 next/prev（纯函数）
│     ├─ snap.rs              # 吸附与展开几何（纯函数）
│     ├─ app_core.rs          # 状态机：一次操作 → Vec<CoreEffect>
│     ├─ commands.rs          # #[tauri::command] 薄封装 + 事件
│     ├─ window_platform.rs   # 托盘、工作区、置顶等平台细节
│     └─ sync.rs              # reqwest changed 拉取 + 合并
└─ tests（Rust 模块内 #[cfg(test)]；前端 vitest 同目录 *.test.ts）
```

---

## Task 1: 工具链与 Tauri 脚手架

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.ts`, `src/style.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json`
- Create: `.gitignore`, `README.md`

**Interfaces:**
- Produces: `npm run tauri dev` 可启动空白 340×64 无边框窗口；`cargo test`（在 `src-tauri/`）可运行；git 仓库初始化完成。

- [ ] **Step 1: 安装 Rust（完全权限 shell）**

```powershell
$env:RUSTUP_DIST_SERVER="https://rsproxy.cn"; $env:RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
curl.exe -L -o "$env:TEMP\rustup-init.exe" "https://rsproxy.cn/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
& "$env:TEMP\rustup-init.exe" -y --default-toolchain stable --profile minimal
```
Expected: `cargo --version` 与 `rustc --version` 输出 1.8x 或更新。失败则回退 `https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe`（较慢）。

- [ ] **Step 2: 配置 crates 镜像**

写 `%USERPROFILE%\.cargo\config.toml`（若已存在则追加缺失段，不覆盖）：

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[net]
git-fetch-with-cli = true
```

- [ ] **Step 3: 写脚手架文件**

- `package.json`：`"type": "module"`；scripts `dev: vite`、`build: vite build`、`preview`、`tauri: tauri`、`test: vitest run`；dependencies `@tauri-apps/api@^2`、`hls.js@^1.7`；devDependencies `@tauri-apps/cli@^2`、`vite@^7`、`typescript@^5.7`、`vitest@^3`。（vite 若需 ^8 再升，配置只用 port/target，兼容。）
- `vite.config.ts`：`server.port = 1420`、`strictPort: true`、`clearScreen: false`；`build.target = "chrome110"`。
- `index.html`：`<div id="app">`，`<script type="module" src="/src/main.ts">`；`<meta name="color-scheme" content="dark light">`。
- `src-tauri/tauri.conf.json`：

```json
{
  "productName": "radiobar",
  "version": "0.1.0",
  "identifier": "com.local.radiobar",
  "build": { "beforeDevCommand": "npm run dev", "devUrl": "http://localhost:1420",
             "beforeBuildCommand": "npm run build", "frontendDist": "../dist" },
  "app": {
    "windows": [{ "label": "main", "title": "radiobar", "width": 340, "height": 64,
                  "decorations": false, "transparent": true, "resizable": false,
                  "alwaysOnTop": true, "skipTaskbar": false, "visible": true }],
    "security": { "csp": null }
  },
  "bundle": { "active": true, "targets": ["nsis"], "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"] }
}
```
- `src-tauri/Cargo.toml`：`[lib] name = "radiobar_lib" crate-type = ["staticlib","cdylib","rlib"]`；deps `tauri = { version = "2", features = ["tray-icon","image-png"] }`、`tauri-plugin-single-instance = "2"`、`serde = { version = "1", features = ["derive"] }`、`serde_json = "1"`、`serde` 已含；`[target.'cfg(windows)'.dependencies] windows = { version = "0.58", features = ["Win32_Foundation","Win32_UI_WindowsAndMessaging","Win32_Graphics_Gdi"] }`。其余依赖在后续任务加入。
- `src-tauri/build.rs`：`fn main() { tauri_build::build() }`。
- `src-tauri/src/main.rs`：`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] fn main() { radiobar_lib::run() }`。
- `src-tauri/src/lib.rs`：`pub fn run()` 内 `tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|_,_,_| {})).run(tauri::generate_context!()).expect("error while running tauri application")`。
- `capabilities/default.json`：`{ "identifier": "default", "windows": ["main"], "permissions": ["core:default", "core:window:allow-start-dragging"] }`。
- 图标：用 Python/Pillow 生成 1024×1024 青绿圆角方块+波形的 `app-icon.png` 到临时目录，再 `npx tauri icon app-icon.png` 生成 `src-tauri/icons/`。
- `.gitignore`：`node_modules/`、`dist/`、`src-tauri/target/`、`app-icon.png`、`.DS_Store`。

- [ ] **Step 4: 安装依赖并验证构建**

```powershell
npm install
npm run tauri dev
```
Expected: Vite 起在 1420，Rust 首次编译完成，出现空白 340×64 无边框窗口。记录首编译耗时（后续冷启动预算另测）。关闭窗口/`Ctrl+C` 结束。

- [ ] **Step 5: git 初始化并提交**

```powershell
git init -b main
git add .gitignore README.md package.json package-lock.json vite.config.ts tsconfig.json index.html src src-tauri docs
git commit -m "chore: scaffold Tauri 2 app with design doc and plan"
```
Expected: 工作树干净；`docs/superpowers/` 中文档已入库。

## Task 2: storage.rs 原子持久化

**Files:**
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/lib.rs`（`pub mod storage;`）

**Interfaces:**
- Produces:
```rust
pub fn load_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T; // 缺失/损坏 → T::default()
pub fn save_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()>;
```

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_atomic_save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        save_json(&path, &serde_json::json!({"a": 1})).unwrap();
        let v: serde_json::Value = load_json(&path);
        assert_eq!(v["a"], 1);
        assert!(!path.with_extension("json.tmp").exists());
    }
    #[test]
    fn test_corrupt_json_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"{not json").unwrap();
        let v: serde_json::Value = load_json(&path);
        assert_eq!(v, serde_json::Value::default());
    }
}
```
- [ ] **Step 2: 加 `tempfile` dev-dependency，跑测试确认失败**

Run: `cargo test storage::tests -q`
Expected: FAIL（`load_json`/`save_json` 不存在）。

- [ ] **Step 3: 实现**

`save_json`：确保父目录 `create_dir_all`；序列化到同目录 `format!("{}.tmp", name)`，`write_all` + `sync_all`，再 `fs::rename` 覆盖；Windows rename 覆盖已存在文件用 `fs::rename`（同卷可行）。`load_json`：读文件，`serde_json::from_slice` 成功则返回，否则 `T::default()`（并 `eprintln!` 记录）。

- [ ] **Step 4: 跑测试**

Run: `cargo test storage::tests -q`
Expected: PASS 2 tests。

- [ ] **Step 5: 提交** `git add src-tauri/src/storage.rs src-tauri/Cargo.toml src-tauri/src/lib.rs; git commit -m "feat: atomic JSON storage with corrupt-file fallback"`

## Task 3: model 与 state 类型

**Files:**
- Create: `src-tauri/src/model.rs`, `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces（其余任务只依赖这些名字）：

```rust
// model.rs
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub id: String, pub name: String,
    #[serde(rename = "country")] pub country_code: String,
    pub url_resolved: String, pub url: String,
    pub codec: String, pub bitrate: u32, pub hls: bool,
    pub votes: u32, pub tags: Vec<String>, pub clickcount: u32,
    pub last_ok: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode { #[default] Sequential, Loop, Shuffle }

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope { Favorites, List, #[default] All }

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter { pub query: String, pub country: Option<String> }

pub fn is_hls_url(url: &str) -> bool; // path 部分（去 query/fragment）以 .m3u8 结尾，大小写不敏感

// state.rs
pub struct PersistedState { volume: u8, muted: bool, mode: PlayMode, scope: Scope,
  current_id: Option<String>, always_on_top: bool, bar_pos: Option<(i32,i32)>,
  panel_open: bool, last_sync: Option<String>, favorites: Vec<String>,
  history: Vec<String>, filter: ListFilter, autoplay_on_launch: bool,
  auto_next_on_failure: bool }
impl Default for PersistedState; // volume 70, muted false, mode Sequential, scope All,
// always_on_top true, panel_open false, autoplay false, auto_next true, favorites/history 空
pub struct UiState { /* 序列化给前端，camelCase */ current: Option<Station>,
  playing: bool, volume: u8, muted: bool, mode: PlayMode, scope: Scope,
  always_on_top: bool, panel_open: bool, favorite_ids: Vec<String>,
  last_sync: Option<String>, filter: ListFilter,
  playback: PlaybackRequest, can_next: bool, can_prev: bool,
  autoplay_on_launch: bool, auto_next_on_failure: bool }
pub struct PlaybackRequest { pub token: u64, pub url: String }
```

- [ ] **Step 1: 写失败测试（model.rs）**

```rust
#[test]
fn test_is_hls_url() {
    assert!(is_hls_url("https://a.com/live.m3u8"));
    assert!(is_hls_url("https://a.com/live.M3U8?token=1"));
    assert!(!is_hls_url("https://a.com/live.mp3"));
    assert!(!is_hls_url("https://a.com/m3u8/stream"));
}
#[test]
fn test_defaults() {
    let s = PersistedState::default();
    assert_eq!(s.volume, 70); assert!(s.always_on_top); assert!(!s.autoplay_on_launch);
    assert!(s.auto_next_on_failure); assert_eq!(s.mode, PlayMode::Sequential);
}
```
- [ ] **Step 2: 跑测试确认失败** `cargo test model::tests -q` → FAIL。
- [ ] **Step 3: 实现类型与默认值**；`UiState` 增加 `#[serde(rename_all = "camelCase")]`；`PlaybackRequest` 同。`is_hls_url` 用 `url` 解析或手写：截 `#`/`?`、转小写、`ends_with(".m3u8")`。
- [ ] **Step 4: 跑测试** `cargo test model::tests -q` → PASS。
- [ ] **Step 5: 提交** `feat: station and app state models`

## Task 4: queue.rs 播放队列

**Files:**
- Create: `src-tauri/src/queue.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `model::{PlayMode, is_hls_url}` 不需要；只依赖 `PlayMode`。
- Produces:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum QueueError { Empty, AtEnd }

pub struct Queue { /* ids, current: Option<usize>, mode, played: HashSet<usize>, history: Vec<String> */ }

impl Queue {
    pub fn new(mode: PlayMode) -> Self;
    pub fn set_mode(&mut self, mode: PlayMode);
    /// 替换范围快照；current_id 在新列表里则保留位置，否则 current=None。
    pub fn set_ids(&mut self, ids: Vec<String>, current_id: Option<&str>);
    pub fn ids(&self) -> &[String];
    pub fn current_id(&self) -> Option<&str>;
    pub fn position_of(&self, id: &str) -> Option<usize>;
    pub fn set_current(&mut self, id: &str);
    /// 记录一次成功开播：追加 history（去重相邻，保留最后 100 条），shuffle 的 played 集合加标记。
    pub fn mark_played(&mut self, id: &str);
    pub fn history(&self) -> &[String];
    pub fn set_history(&mut self, history: Vec<String>);
    pub fn next(&mut self, rng: &mut impl rand::Rng) -> Result<usize, QueueError>;
    pub fn prev(&mut self) -> Result<usize, QueueError>;
}
```

规则（§16）：`sequential` 末尾 `AtEnd`；`loop` 回绕；`shuffle` next 从未在本轮 played 里的非当前项随机取（本轮取完先重置 played 再取），prev 从 `history` 反向找第一个仍在 ids 里且不等于当前的 id（找到则把 current 移到它，`AtEnd` 表示没有更早的）。`ids` 为空 → `Empty`。

- [ ] **Step 1: 写失败测试**（`rand::rngs::StdRng::seed_from_u64(42)`）

```rust
fn q(ids: &[&str], mode: PlayMode) -> Queue {
    let mut q = Queue::new(mode);
    q.set_ids(ids.iter().map(|s| s.to_string()).collect(), None);
    q
}
#[test] fn test_sequential_end_and_prev() { /* 3 台：next→1,2 后第 3 次 AtEnd；prev 回 0 后 AtEnd */ }
#[test] fn test_loop_wraps_both_ends() { /* next 从 2 → 0，prev 从 0 → 2 */ }
#[test] fn test_shuffle_no_repeat_until_round_done() { /* 10 台连续 next 得到 10 个不同下标 */ }
#[test] fn test_shuffle_prev_uses_history() { /* 播 A,B,C 后 prev → B，再 prev → A，再 prev AtEnd */ }
#[test] fn test_empty_scope() { /* ids 空 → Empty */ }
#[test] fn test_set_ids_keeps_current_when_present() { /* 当前在列表里则索引更新，不在则 None */ }
#[test] fn test_history_capped_at_100() { /* mark_played 150 次后 history.len()==100 */ }
```
- [ ] **Step 2: 加 `rand` 依赖（`rand = "0.8"`）与 `tempfile` dev-dependency，跑测试确认失败**
- [ ] **Step 3: 实现**（用 `rand::seq::SliceRandom` 选候选；`played` 存索引，`set_ids` 时清空）
- [ ] **Step 4: 跑测试** `cargo test queue::tests -q` → PASS 7 tests。
- [ ] **Step 5: 提交** `feat: play queue with sequential/loop/shuffle semantics`

## Task 5: snap.rs 吸附与展开几何

**Files:**
- Create: `src-tauri/src/snap.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

pub const SNAP_THRESHOLD: i32 = 12;
pub const PANEL_H: i32 = 440;

/// 折叠条贴边：与 work 四边距离 < SNAP_THRESHOLD 时贴齐，返回新 Rect。
pub fn snap_bar(bar: Rect, work: Rect) -> Rect;
/// 面板展开位置：默认向下；bar 中心在工作区下半部时向上；结果整窗 clamp 进 work。
pub fn expanded_rect(bar: Rect, work: Rect, panel_h: i32) -> Rect;
/// 位置记忆校验：bar 是否与任一显示器工作区有交集。
pub fn is_visible_on(rect: Rect, work_areas: &[Rect]) -> bool;
```

- [ ] **Step 1: 写失败测试**

```rust
#[test] fn test_snap_each_edge() { /* bar 离左 5px → x=work.x；离右 5px → x=work.x+work.w-bar.w；上/下同理 */ }
#[test] fn test_no_snap_beyond_threshold() { /* 距边 20px 不变 */ }
#[test] fn test_negative_coordinate_monitor() { /* work = x:-1920 的副屏，贴右边正确 */ }
#[test] fn test_expanded_grows_down_normally() { /* bar 在上半部 → 返回 y=bar.y, h=panel_h */ }
#[test] fn test_expanded_grows_up_near_bottom() { /* bar 在下半部 → y = bar.y+bar.h-panel_h, h=panel_h, 且 y>=work.y */ }
#[test] fn test_expanded_clamped_in_short_work_area() { /* work.h=300 < panel_h → 整窗 clamp 进 work */ }
#[test] fn test_is_visible_on() { /* 部分在屏内 true；完全在屏外 false */ }
```
- [ ] **Step 2: 跑测试确认失败** `cargo test snap::tests -q`。
- [ ] **Step 3: 实现**（全部整数运算；注意 `work.w - bar.w` 为负时 clamp 到 `work.x`）
- [ ] **Step 4: 跑测试** `cargo test snap::tests -q` → PASS 7 tests。
- [ ] **Step 5: 提交** `feat: edge snapping and panel expansion geometry`

## Task 6: seed 与中文国家名生成脚本

**Files:**
- Create: `scripts/build-seed.mjs`, `scripts/build-countries.mjs`
- Create（生成产物）: `src-tauri/resources/stations.seed.json`, `src-tauri/resources/countries.zh.json`

**Interfaces:**
- Produces: seed 文件 `{ "generated_at": "<ISO>", "stations": [ { i,n,c,u,o,k,b,h,v,t,x } ] }`；国家映射 `{ "CN": "中国", ... }`（225 条）。

- [ ] **Step 1: 写 `build-countries.mjs`**

读 `https://de1.api.radio-browser.info/json/countries`（UA header），取 `iso_3166_1`，用 `new Intl.DisplayNames(["zh"], {type:"region"}).of(code)` 生成映射，写 `src-tauri/resources/countries.zh.json`（排序、2 空格缩进）。校验：条目数 ≥ 200，`CN === "中国"`，`JP === "日本"`。

- [ ] **Step 2: 写 `build-seed.mjs`**

流程：GET `/json/countries` → 对每个国家 GET `/json/stations/bycountrycodeexact/{cc}?limit=50&hidebroken=true&order=clickcount&reverse=true`（8 并发，2 次重试，超时 20 s）→ 再 GET `/json/stations/topclick/500?hidebroken=true` → 按 uuid 去重、国家 top50 优先保留 → 映射成短键（`t` 取 tags 前 2 个，`x` = clickcount）→ 写 `{generated_at: new Date().toISOString(), stations}`。打印统计：国家数/总台数/失败国家数/文件大小。要求：失败国家跳过但记录；单次失败不影响其他；总台数在 8,000–15,000 之间。

- [ ] **Step 3: 运行脚本**（需完全权限网络）

```powershell
node scripts/build-countries.mjs
node scripts/build-seed.mjs
```
Expected: `countries.zh.json` ≥ 200 条、约 6 KB；seed 约 1–2 MB、8k–15k 台。若网络失败，重试；仍失败则记录并以 HTTP 直连 + `curl.exe --ssl-no-revoke` 兜底。

- [ ] **Step 4: 抽查数据**

Node 单行检查：seed 里 `CN` 台数在 40–50 之间（top50 去重后可能略少）、存在 `hls:true` 的台、存在 `x` 字段、url 均以 http/https 开头。

- [ ] **Step 5: 提交** `git add scripts src-tauri/resources; git commit -m "feat: generate curated seed and Chinese country-name map"`

## Task 7: library.rs 电台库

**Files:**
- Create: `src-tauri/src/library.rs`
- Create（测试夹具）: `src-tauri/tests/fixtures/seed.json`, `src-tauri/tests/fixtures/countries.zh.json`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `model::Station`、`countries.zh.json`。
- Produces:

```rust
pub struct CountryInfo { pub code: String, pub name_zh: String, pub name_en: String, pub count: usize }
pub struct SearchPage { pub total: usize, pub stations: Vec<Station> }

pub struct Library { /* stations: Vec<Station>, by_id: HashMap<String,usize>, countries: Vec<CountryInfo> */ }
impl Library {
    /// local 存在则读 local，否则读 seed；两者都带短键，需展开为 Station。
    pub fn load(seed_path: &Path, local_path: &Path, countries_zh: &Path) -> Library;
    pub fn len(&self) -> usize;
    pub fn get(&self, id: &str) -> Option<&Station>;
    pub fn snapshot_ids(&self, scope: Scope, favorites: &[String], filter: &ListFilter) -> Vec<String>;
    pub fn search(&self, filter: &ListFilter, offset: usize, limit: usize) -> SearchPage;
    pub fn countries(&self) -> &[CountryInfo];
    pub fn favorites(&self, favorites: &[String]) -> Vec<Station>;
    pub fn upsert_all(&mut self, stations: Vec<Station>) -> usize; // 返回新增数
    pub fn to_json(&self) -> String; // 存盘用
    pub fn write_local(&self, path: &Path) -> std::io::Result<()>;
}
```

排序：`search`/`snapshot_ids` 结果按 `clickcount` 降序，同分按 `name`；`favorites` 按传入 id 顺序。搜索匹配字段：台名、英文国名、中文国名、tags（子串、大小写不敏感）。`last_ok == false` 的台 `snapshot_ids` 跳过，但 `search` 仍返回。

- [ ] **Step 1: 建小而全的夹具**：约 8 台（JP 2、CN 2、US 2、DE 1、FR 1），含 1 台 `lastOk:false`、1 台 HLS、1 台 tags 含 "jazz"；`countries.zh.json` 含 JP 日本 / CN 中国 / US 美国 / DE 德国 / FR 法国。
- [ ] **Step 2: 写失败测试**

```rust
#[test] fn test_search_japanese_country_name_hits_jp() { /* "日本" 返回 2 台 JP */ }
#[test] fn test_search_case_insensitive() { /* "JAZZ" 命中 tag */ }
#[test] fn test_country_filter() { /* Some("JP") 只返回 JP */ }
#[test] fn test_all_sorted_by_clickcount_desc() {}
#[test] fn test_favorites_sorted_in_added_order() {}
#[test] fn test_snapshot_ids_skips_not_ok_stations() {}
#[test] fn test_search_still_shows_not_ok_station() {}
#[test] fn test_upsert_updates_and_adds() { /* 同 id 改 name 生效；新 id 返回 1 */ }
#[test] fn test_load_prefers_local_copy() {}
```
- [ ] **Step 3: 跑测试确认失败** `cargo test library::tests -q`。
- [ ] **Step 4: 实现**（seed 短键 → Station 展开；countries 计数按库内实际台数；`write_local` 用 `storage::save_json` 写短键格式）
- [ ] **Step 5: 跑测试** → PASS 9 tests。
- [ ] **Step 6: 提交** `feat: station library with search, filters and merge`

## Task 8: app_core.rs 状态机

**Files:**
- Create: `src-tauri/src/app_core.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `model::*`、`state::{PersistedState, UiState, PlaybackRequest}`、`queue::Queue`、`library::Library`、`snap` 不需要。
- Produces:

```rust
#[derive(Debug, PartialEq)]
pub enum CoreEffect {
    EmitState,                    // 推送 UiState
    Toast { level: ToastLevel, text: String },
    Persist,                      // 写 state.json
    ApplyAlwaysOnTop(bool),       // 调用窗口 API
    ResizePanel(bool),            // true=展开
    SetTrayTooltip(Option<String>),
}
#[derive(Debug, PartialEq, Clone, Copy)] pub enum ToastLevel { Info, Warn, Error }

pub struct AppCore {
    pub persisted: PersistedState,
    library: Library,
    queue: Queue,
    playing: bool,
    playback_token: u64,
    consecutive_failures: u32,
    try_original_url: bool, // 下一次尝试用 Station.url
}

impl AppCore {
    pub fn new(persisted: PersistedState, library: Library) -> Self;
    pub fn ui_state(&self) -> UiState;
    pub fn search(&self, offset: usize, limit: usize) -> SearchPage;
    pub fn countries(&self) -> &[CountryInfo];
    pub fn set_volume(&mut self, v: u8) -> Vec<CoreEffect>;
    pub fn toggle_mute(&mut self) -> Vec<CoreEffect>;
    pub fn set_mode(&mut self, m: PlayMode) -> Vec<CoreEffect>;
    pub fn set_scope(&mut self, s: Scope) -> Vec<CoreEffect>;
    pub fn set_always_on_top(&mut self, b: bool) -> Vec<CoreEffect>;
    pub fn set_panel_open(&mut self, b: bool) -> Vec<CoreEffect>;
    pub fn set_filter(&mut self, filter: ListFilter) -> Vec<CoreEffect>;
    pub fn toggle_favorite(&mut self, id: &str) -> Vec<CoreEffect>;
    pub fn set_setting(&mut self, key: &str, value: bool) -> Vec<CoreEffect>; // 仅 "autoplayOnLaunch" | "autoNextOnFailure"
    pub fn toggle_play(&mut self) -> Vec<CoreEffect>;
    pub fn play_station(&mut self, id: &str) -> Vec<CoreEffect>;
    pub fn playback_started(&mut self) -> Vec<CoreEffect>; // 清零连败与重试标记
    pub fn next(&mut self) -> Vec<CoreEffect>;
    pub fn prev(&mut self) -> Vec<CoreEffect>;
    pub fn report_playback_error(&mut self, msg: &str) -> Vec<CoreEffect>; // 含重试/切台/连败
    pub fn sync_finished(&mut self, last_sync: String) -> Vec<CoreEffect>;
    pub fn rebuild_queue(&mut self); // set_ids(snapshot_ids(scope,favs,filter), current_id)
}
```

行为（§16 全部规则）：
- `next/prev`：`queue.next(&mut rng)`；`Empty` → toast「收藏为空…」/「列表为空」按 scope；`AtEnd` → toast「已到列表末尾」；成功 → 设 current、`playing=true`、`mark_played`、新 `playback_token`、EmitState+Persist。
- `play_station`：校验 id 存在，设 current、playing=true、mark_played、新 token、rebuild_queue、EmitState+Persist。
- `toggle_play`：playing 取反；false 时 `playback_token` 不变。
- `report_playback_error`：若 `try_original_url == false` 且当前台 `url_resolved != url` → 置 `try_original_url = true`、`playback_token += 1`，EmitState（前端用新 token 重载原始 url）。否则计一次失败；`consecutive_failures += 1`；若 <3 → 自动按当前 mode/scope 切台（同 next 逻辑，但 `playing=true`）并 toast「{name} 播放失败，已切到 {next_name}」；若 ==3 → `playing=false`、toast「连续 3 台播放失败，已暂停」。
- 成功开始播放的确认：前端 `report_playback_started`（新命令）→ 清零 `consecutive_failures`、`try_original_url=false`。
- `set_filter`：持久化 filter，不重建队列（下次切台重建）；toast 无。
- `sync_finished`：写 `last_sync`，rebuild_queue，EmitState+Persist。
- RNG：`use rand::rngs::StdRng`，AppCore 持 `rng: StdRng`（测试用 `seed_from_u64`）。

- [ ] **Step 1: 写失败测试**（用 Task 7 夹具构造 Library）

```rust
#[test] fn test_stall_triggers_retry_then_switch() { /* report_playback_error 两次后 current 变化、playing 仍 true、consecutive_failures==0 */ }
#[test] fn test_three_consecutive_failures_stop() { /* 连续 3 台各报错 2 次（urlResolved+url）→ playing false、toast 含「连续 3 台」 */ }
#[test] fn test_retry_uses_original_url_second_time() { /* 首次 error 后 UiState.playback.url == station.url */ }
#[test] fn test_sequential_next_at_end_toasts() { /* toast「已到列表末尾」，current 不变 */ }
#[test] fn test_favorites_empty_toast() { /* scope=Favorites 无收藏 → toast 含「收藏为空」，current 不变 */ }
#[test] fn test_toggle_play_pause() {}
#[test] fn test_play_station_sets_playing_and_token_increments() {}
#[test] fn test_toggle_favorite_adds_and_removes() {} // 收藏后 UiState.favoriteIds 含 id；再点移除
#[test] fn test_set_setting_updates_and_persists() {}
#[test] fn test_mode_and_scope_persist_and_rebuild_on_next() {}
```
- [ ] **Step 2: 跑测试确认失败** `cargo test app_core::tests -q`。
- [ ] **Step 3: 实现**（所有变更返回 effects；不在 core 内碰窗口/磁盘）。
- [ ] **Step 4: 跑测试** → PASS 8 tests。
- [ ] **Step 5: 提交** `feat: playback state machine with retry and failure cascade`

## Task 9: Tauri 集成（托盘、窗口、IPC）

**Files:**
- Modify: `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/commands.rs`, `src-tauri/src/window_platform.rs`

**Interfaces:**
- Consumes: `AppCore`、`CoreEffect`、`storage`、`Library`。
- Produces: 前端可 `invoke` 的命令（snake_case）：`get_state`、`toggle_play`、`play_station`、`next`、`prev`、`set_volume`、`toggle_mute`、`set_mode`、`set_scope`、`set_always_on_top`、`set_panel_open`、`set_filter`、`set_setting`、`search_stations`、`get_countries`、`toggle_favorite`、`report_playback_error`、`report_playback_started`、`sync_library`、`quit`；事件：`state-changed`(UiState)、`sync-progress`、`toast`。

- [ ] **Step 1: 写 `commands.rs` 骨架**

全局 `struct AppHandleState { core: Mutex<AppCore>, app_dir: PathBuf }` 放入 `app.manage(...)`。每个命令：`fn apply(app: &AppHandle, effects: Vec<CoreEffect>)` 依次执行——`EmitState` → `app.emit("state-changed", core.ui_state())`；`Toast` → `emit("toast")`；`Persist` → `storage::save_json(app_data/state.json)`；`ApplyAlwaysOnTop` → `window_platform::set_always_on_top`；`ResizePanel` → `window_platform::set_panel`；`SetTrayTooltip` → 托盘句柄。命令返回 `UiState`（前端可直接用）。

- [ ] **Step 2: 托盘**

`tauri::tray::TrayIconBuilder::new()`，图标用 `app.default_window_icon()` 克隆；左键 `TrayIconEvent::Click { button: Left, button_state: Up }` → 显隐窗口并 `set_focus`；右键 `MenuBuilder`：显示窗口 / 播放·暂停 / 上一台 / 下一台 / 置顶(CheckMenuItem, checked 初始 true) / 更新电台库 / 退出；`menu` 事件按 `id` 派发到对应命令函数。

- [ ] **Step 3: 窗口行为**

- `WindowEvent::CloseRequested` → `api.prevent_close(); window.hide();`
- 启动：读 state，`bar_pos` 有效则 `set_position`（否则右下角：work_area 右下留 24px），`set_always_on_top(persisted.always_on_top)`，`ResizePanel(persisted.panel_open)`。
- 单实例插件回调：`window.show(); window.set_focus();`。
- `quit` 命令：先 `Persist`，再 `app.exit(0)`。

- [ ] **Step 4: 编译检查**

Run: `cargo check`（src-tauri）
Expected: 0 error；warning 可接受但不得有 `unused` 大量堆积。

- [ ] **Step 5: 手动冒烟** `npm run tauri dev`：窗口出现、托盘图标在、左键能隐藏/显示、右键菜单「退出」能结束进程。若托盘图标不可见，用 `image-png` feature + 32px png 重新加载（记录到 M7 图标任务）。

- [ ] **Step 6: 提交** `feat: Tauri tray, window lifecycle and IPC commands`

## Task 10: 窗口吸附与几何持久化

**Files:**
- Modify: `src-tauri/src/window_platform.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `snap::{snap_bar, expanded_rect, is_visible_on, Rect, SNAP_THRESHOLD, PANEL_H}`。
- Produces: `window_platform::work_area_for(window: &Window) -> Rect`；`on_moved(window)` 防抖 150 ms 后吸附并持久化 bar 坐标。

- [ ] **Step 1: 实现工作区查询**：Windows 用 `windows` crate：`MonitorFromWindow(MONITOR_DEFAULTTONEAREST)` + `GetMonitorInfoW`，`rcWork` 转 `Rect`（含负坐标）。
- [ ] **Step 2: 实现 `on_moved`**：`WindowEvent::Moved` 里记录 `(x,y)` 并重启 150 ms 定时器（`tokio::spawn` + `Instant`/`sleep`，用 generation 计数丢弃旧定时器）；到点后 `snap_bar`，若 rect 变化则 `set_position`；通过 command 内部接口把坐标写入 `persisted.bar_pos` 并 Persist。
- [ ] **Step 3: 实现 `set_panel`**：折叠条 rect = 当前 `outer_position`+340×64；展开取 `expanded_rect(bar, work, PANEL_H)`；收起恢复到 bar 原位置。窗口 min/max 尺寸保持一致。
- [ ] **Step 4: 启动位置校验**：`bar_pos` 不在任何 `is_visible_on` 工作区 → 右下角回落。
- [ ] **Step 5: 测试**：Task 5 已覆盖纯几何；此处加 `window_platform` 内 `#[cfg(test)]` 的 `test_restore_falls_back_when_offscreen`（用 `is_visible_on` 驱动，不依赖真实窗口）。
- [ ] **Step 6: 手动冒烟**：拖到四边贴齐；贴底后面板向上展开；重启位置复原；置顶按钮生效并被其他窗口盖住时仍在上层。
- [ ] **Step 7: 提交** `feat: edge snapping, panel geometry and position persistence`

## Task 11: 前端外壳与折叠条

**Files:**
- Create: `src/state.ts`, `src/ipc.ts`, `src/ui/icons.ts`, `src/ui/bar.ts`, `src/format.ts`, `src/format.test.ts`
- Modify: `src/main.ts`, `src/style.css`, `index.html`

**Interfaces:**
- Consumes: `state-changed` 事件、全部命令。
- Produces: `src/format.ts` 纯函数：

```ts
export function flagEmoji(code: string): string;      // "CN" → 🇨🇳（非法码返回 "🏳️"）
export function truncateName(name: string, max = 22): string; // 按 grapheme 截断 + "…"
export function codecLine(st: Station): string;       // "MP3 · 64 kbps" / "HLS · AAC" / "MP3"
export function progressText(done: number, total: number): string; // "更新中 3/12"
```

- [ ] **Step 1: 写失败测试**（`format.test.ts`，vitest）

```ts
import { describe, expect, it } from "vitest";
it("flagEmoji maps CN", () => expect(flagEmoji("CN")).toBe("🇨🇳"));
it("flagEmoji falls back", () => expect(flagEmoji("ZZ")).toBe("🏳️"));
it("truncate keeps emoji whole", () => expect(truncateName("🎵".repeat(30), 4)).toBe("🎵🎵🎵🎵…"));
it("truncate short name unchanged", () => expect(truncateName("KEXP", 22)).toBe("KEXP"));
it("codecLine with bitrate", () => expect(codecLine({codec:"MP3",bitrate:64,hls:false} as any)).toBe("MP3 · 64 kbps"));
it("codecLine hls without bitrate", () => expect(codecLine({codec:"AAC",bitrate:0,hls:true} as any)).toBe("HLS · AAC"));
it("progressText", () => expect(progressText(3, 12)).toBe("更新中 3/12"));
```
- [ ] **Step 2: 跑测试确认失败** `npm test`。
- [ ] **Step 3: 实现 format.ts**（grapheme 用 `Intl.Segmenter`）。
- [ ] **Step 4: 写 `ipc.ts`**：`invoke` 的 typed 封装 + `listen("state-changed"|"toast"|"sync-progress")`。
- [ ] **Step 5: 写 `bar.ts` + `style.css`**：结构按设计 §9.1；`data-tauri-drag-region` 在整行空白区；按钮 SVG 来自 `icons.ts`；绑定：`⏮ prev`、`▶/⏸ toggle_play`、`⏭ next`、音量滑块 `input → set_volume` 防抖 100ms、🔇 `toggle_mute`、📋 循环 `set_scope`、☰ 循环 `set_mode`、📌 `set_always_on_top(!state.alwaysOnTop)`；台名与国家文本由 `render(state)` 重绘。
- [ ] **Step 6: 渲染逻辑**：`state.ts` 保存最近 UiState；`main.ts` 启动 `invoke("get_state")`，订阅事件后调用 `renderBar`；`prefers-color-scheme` 用 CSS。
- [ ] **Step 7: 手动冒烟**：条上按钮调命令并在 Rust 侧改变状态后回显；模式/范围图标随状态变化。
- [ ] **Step 8: 提交** `feat: collapsed bar UI with playback controls`

## Task 12: 音频播放与失败上报

**Files:**
- Create: `src/audio.ts`
- Modify: `src/main.ts`

**Interfaces:**
- Consumes: `UiState.playback {token, url}`、命令 `report_playback_error` / `report_playback_started`。
- Produces: `class AudioController { sync(playback: PlaybackRequest, playing: boolean, volume: number, muted: boolean): void }`。

- [ ] **Step 1: 实现 AudioController**

- 单例 `<audio>`（`preload="none"`），音量/静音同步；`playing=false` 时 `pause()`。
- `token` 变化 → 若 `isHlsUrl(url)`（同 Task 3 的规则，前端重写一份）用动态 `import("hls.js")` 创建 `Hls`，`hls.loadSource(url)`、`attachMedia`，监听 `ERROR` → report；否则 `audio.src = url; audio.play()`。
- `playing` 事件 → `report_playback_started`；`error` 事件 → `report_playback_error(audio.error?.message)`；watchdog：`loadstart` 后 8 s 仍 `readyState < 3 && !paused` → report "stalled"（只报一次，切台 token 变化重置）。
- 卸载旧 hls 实例（`destroy()`）避免泄漏。
- `navigator.mediaSession` 填标题/国家，`play/pause/nexttrack/previoustrack` action 映射到命令（尽力而为）。

- [ ] **Step 2: 测试（vitest，jsdom 不可用则纯函数）**：把 `isHlsUrl` 与 watchdog 的 `shouldReportStall(readyState, elapsedMs, reported)` 抽成纯函数并单测：
```ts
it("isHlsUrl handles query strings", () => expect(isHlsUrl("https://a/b.M3U8?x=1")).toBe(true));
it("shouldReportStall true after 8s", () => expect(shouldReportStall(2, 8100, false)).toBe(true));
it("shouldReportStall false when reported", () => expect(shouldReportStall(2, 9000, true)).toBe(false));
```
- [ ] **Step 3: 跑测试** `npm test` → PASS。
- [ ] **Step 4: 手动冒烟**：点▶有声音；切台换流；HLS 台能播（在 seed 里挑 3 个 `h:true` 的台验证）；断开网络后 `<audio>` 报错触发 Rust 切台与 toast。
- [ ] **Step 5: 提交** `feat: audio playback with HLS support and failure reporting`

## Task 13: 面板 UI

**Files:**
- Create: `src/ui/panel.ts`
- Modify: `src/main.ts`, `src/style.css`, `src/ipc.ts`

**Interfaces:**
- Consumes: `search_stations(offset)` → `{total, stations}`、`get_countries()`、`toggle_favorite`、`set_filter`、`sync_library`、`set_panel_open`、同步进度事件。
- Produces: `renderPanel(state, page)`；面板展开时 `set_panel_open(true)`。

- [ ] **Step 1: 实现结构**：搜索框（输入 200 ms 防抖 → `set_filter` + 重新 `search_stations(0)`）、国家下拉（全部 + `国家 (count)`，选择即过滤）、列表（50 行/页，行内 国旗/台名/tags/编码/⭐，当前播放行高亮，点击行 `play_station`，⭐ `toggle_favorite`）、分页（上一页/下一页）、页脚（共 N 台 / 上次同步 / 🗘 更新 / ⚙ 设置）。
- [ ] **Step 2: 更新按钮**：点击 `invoke("sync_library")`，禁用按钮并监听 `sync-progress` 显示 `progressText`；完成/失败用 toast 显示。
- [ ] **Step 3: 设置视图**：⚙ 切换面板内的设置区（两个 checkbox：启动自动播放、失败自动切台）→ `invoke("set_setting", {key, value})`；返回按钮回列表。设置区渲染当前值（来自 UiState）。
- [ ] **Step 4: 展开/收起**：条右侧双击或点击台名区展开；面板右上 × 收起；`set_panel_open` 由 Rust 改窗口尺寸并回推 UiState。
- [ ] **Step 5: 手动冒烟**：搜 "jazz"、选日本、收藏 3 台、收藏范围切台；重启后收藏与筛选保持；列表滚到底翻页。
- [ ] **Step 6: 提交** `feat: searchable station panel with favorites and sync`

## Task 14: sync.rs 增量同步

**Files:**
- Create: `src-tauri/src/sync.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `library::Library::upsert_all`、`storage::save_json`、`AppCore::sync_finished`。
- Produces:

```rust
pub struct SyncClient { /* reqwest::Client with UA, base_url: String */ }
impl SyncClient {
    pub fn new(base_url: &str) -> Self;
    pub async fn fetch_changed(&self, since: &str, page: u32) -> Result<Vec<Station>, SyncError>;
    pub async fn fetch_countries(&self) -> Result<(), SyncError>; // 仅用于校验网络/更新缓存
}
pub fn parse_changed(json: &serde_json::Value) -> Result<Vec<Station>, SyncError>; // 纯函数，测试用
```

- [ ] **Step 1: 写失败测试**（夹具 `tests/fixtures/changed.json`，含 3 台：1 新增、1 修改既有、1 `lastcheckok:0`）

```rust
#[test] fn test_parse_changed_maps_fields() {}
#[test] fn test_parse_changed_rejects_non_array() {}
#[test] fn test_sync_aborts_without_consuming_cursor() { /* 用 mock：第二页返回 Err → 库不变、last_sync 不变 */ }
#[test] fn test_merge_keeps_not_ok_station_out_of_queue() {}
```
（mock 用 trait `PageFetcher` 注入，生产实现走 reqwest；`sync_library` 内部循环调用它。）
- [ ] **Step 2: 跑测试确认失败** `cargo test sync::tests -q`。
- [ ] **Step 3: 实现**：先给 `Cargo.toml` 加 `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls","json"] }`、`tokio = { version = "1", features = ["rt-multi-thread","time","sync"] }`、`thiserror = "2"`。`sync.rs` 提供 `sync_library(core: &mut AppCore, fetcher: &dyn PageFetcher, progress: impl Fn(SyncProgress)) -> Result<usize, SyncError>`：循环 `offset` 0,1000,2000…（上限 50 页），每页 `fetch_changed(since, page)`，`upsert_all`，中途 Err → 立即返回且不写盘；全部成功 → `core.sync_finished(now)`、`library.write_local`、`save_json(state)`。进度回调发 `sync-progress`。
- [ ] **Step 4: 接命令行命令** `sync_library`（async command，`tokio::spawn` 避免阻塞 UI；同一时间只允许一个同步，用 `AtomicBool`）。
- [ ] **Step 5: 跑测试** `cargo test sync::tests -q` → PASS。
- [ ] **Step 6: 手动冒烟**（需网络）：点更新，进度出现，数量/`last_sync` 变化；播放中不中断；断网点更新 → toast 失败且旧库可用。
- [ ] **Step 7: 提交** `feat: incremental radio-browser sync with progress`

## Task 15: 打包、验收与跨平台收口

**Files:**
- Modify: `src-tauri/tauri.conf.json`, `README.md`, `src-tauri/src/window_platform.rs`
- Create: `docs/superpowers/acceptance-2026-11-10.md`（验收记录）

**Interfaces:**
- Consumes: 所有前序任务。
- Produces: `src-tauri/target/release/bundle/nsis/radiobar_0.1.0_x64-setup.exe` 与免安装 `radiobar.exe`。

- [ ] **Step 1: 图标与托盘双版本**：确认 `tauri icon` 产物齐全；浅色任务栏用深色描边版托盘图（`resources/tray-dark.png`），启动时按系统主题选择。
- [ ] **Step 2: 打包**

```powershell
npm run tauri build
```
Expected: bundle 生成；记录 `.exe` 体积（目标 < 15 MB）。若 NSIS 下载失败，用 `tauri build --no-bundle` 先产出免安装 exe 并记录。

- [ ] **Step 3: 跑手工验收清单**（设计 §12 的 10 条），逐条记录到 `docs/superpowers/acceptance-2026-11-10.md`：结果 + 实测数值（冷启动时间、任务管理器内存/CPU）。
- [ ] **Step 4: 跨平台收口检查**：`grep -rn "cfg(windows)" src-tauri/src` 只应出现在 `window_platform.rs`；`cargo check --target x86_64-pc-windows-msvc` 通过；README 写清 macOS 待办（托盘模板图、`tauri://localhost` 混合内容验证、签名）。
- [ ] **Step 5: 提交** `chore: package NSIS installer and record acceptance results`

---

## Self-check notes（写完计划后核对）

- 设计 §2 的 9 条目标 → Task 1/9/10（窗口托盘）、Task 8/11/12（播放）、Task 4/8（模式范围）、Task 6/7/14（库与同步）、Task 13（收藏）、Task 15（体积与验收）、Task 15 Step 4（跨平台）。
- §12 自动测试：queue→T4、snap→T5、library→T7、storage→T2、sync→T14、前端 format/audio→T11/T12。
- §16 行为：模式语义→T4、列表快照→T7+T8、失败级联→T8、窗口几何→T5+T10、国家中文名→T6+T7、设置两项→T13。
