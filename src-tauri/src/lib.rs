pub mod app_core;
pub mod commands;
pub mod library;
pub mod model;
pub mod queue;
pub mod shortcuts;
pub mod snap;
pub mod state;
pub mod storage;
pub mod sync;
pub mod window_platform;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use app_core::{AppCore, CoreEffect, ToastLevel};
use library::Library;
use state::{PersistedState, UiState};

const WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main";
const SEED_JSON: &str = include_str!("../resources/stations.seed.json");
const COUNTRIES_ZH: &str = include_str!("../resources/countries.zh.json");
const COUNTRIES_EN: &str = include_str!("../resources/countries.en.json");

pub struct AppPaths {
    pub state: PathBuf,
    pub local: PathBuf,
}

pub struct AppState {
    pub core: Mutex<AppCore>,
    pub paths: AppPaths,
    /// 拖动防抖代数：每次 Moved 自增，只有最新一代的定时器执行吸附。
    move_gen: AtomicU64,
    /// 吸附缓动进行中：它自己每帧 set_position 会触发 Moved，须跳过这些回响，
    /// 否则缓动线程会去抖出一代新吸附，反过来打断自己。
    snap_animating: AtomicBool,
}

impl AppState {
    fn new(core: AppCore, paths: AppPaths) -> Self {
        Self {
            core: Mutex::new(core),
            paths,
            move_gen: AtomicU64::new(0),
            snap_animating: AtomicBool::new(false),
        }
    }
}

fn level_str(level: ToastLevel) -> &'static str {
    match level {
        ToastLevel::Info => "info",
        ToastLevel::Warn => "warn",
        ToastLevel::Error => "error",
    }
}

/// 执行核心返回的副作用：事件推送、置顶、面板几何、托盘提示、落盘。
pub fn apply_effects(
    app: &AppHandle,
    effects: Vec<CoreEffect>,
    ui: UiState,
    persisted: Option<PersistedState>,
) {
    let mut need_persist = false;

    // 面板几何要在 emit 之前处理：展开方向（panel_grows_up）是 apply_panel 写进
    // core 的，晚于 emit 的话前端收到的 state 里方向还是旧值，DOM 顺序不会反转。
    let mut ui = ui;
    // apply_panel 会把吸附后的位置写回 core.persisted，调用方传来的快照已经过时，
    // 落盘前必须刷新，否则刚吸附掉的缝隙又会被写回磁盘。
    let mut persisted = persisted;
    for effect in effects.iter() {
        if let CoreEffect::ResizePanel(open) = effect {
            apply_panel(app, *open);
            let state = app.state::<AppState>();
            let core = state.core.lock().unwrap();
            ui.panel_grows_up = core.panel_grows_up;
            if persisted.is_some() {
                persisted = Some(core.persisted.clone());
            }
        }
    }

    for effect in effects {
        match effect {
            CoreEffect::EmitState => {
                let _ = app.emit("state-changed", &ui);
            }
            CoreEffect::Toast { level, text } => {
                let _ = app.emit(
                    "toast",
                    serde_json::json!({ "level": level_str(level), "text": text }),
                );
            }
            CoreEffect::Persist => need_persist = true,
            CoreEffect::ApplyAlwaysOnTop(value) => {
                if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                    let _ = window.set_always_on_top(value);
                }
            }
            // 已在上面提前处理（必须在 EmitState 之前）。
            CoreEffect::ResizePanel(_) => {}
            CoreEffect::SetTrayTooltip(text) => {
                if let Some(tray) = app.tray_by_id(TRAY_ID) {
                    let _ = tray.set_tooltip(text.as_deref());
                }
            }
        }
    }

    if need_persist {
        if let Some(persisted) = persisted {
            let state = app.state::<AppState>();
            let _ = storage::save_json(&state.paths.state, &persisted);
        }
    }
}

fn apply_panel(app: &AppHandle, open: bool) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    let bar = {
        let state = app.state::<AppState>();
        let core = state.core.lock().unwrap();
        core.persisted.bar_pos
    };
    let bar = match bar {
        Some((x, y)) => snap::Rect {
            x,
            y,
            w: window_platform::BAR_W,
            h: window_platform::BAR_H,
        },
        None => match window_platform::current_bar_rect(&window) {
            Some(rect) => rect,
            None => return,
        },
    };
    // 展开前先吸附：拖动吸顶时外框被系统钳在 y=0，逻辑 y 停在 16，
    // 若直接按 16 算展开矩形，缝隙会被原样带进展开态。
    let bar = window_platform::snap_bar_to_work(&window, bar);
    let (bar, grows_up) = window_platform::apply_panel(&window, bar, open);
    let state = app.state::<AppState>();
    let mut core = state.core.lock().unwrap();
    core.panel_grows_up = grows_up;
    // 吸附后的位置是新的 canonical 位置，落回持久化状态，
    // 否则下次 `bar_rect_while_open` 还会从旧的带缝坐标推算。
    core.persisted.bar_pos = Some((bar.x, bar.y));
}

/// 锁住核心、执行一次动作、把副作用派发出去。
pub fn dispatch(app: &AppHandle, action: impl FnOnce(&mut AppCore) -> Vec<CoreEffect>) {
    let state = app.state::<AppState>();
    let (effects, ui, persisted) = {
        let mut core = state.core.lock().unwrap();
        let effects = action(&mut core);
        let ui = core.ui_state();
        let persisted = effects
            .iter()
            .any(|effect| matches!(effect, CoreEffect::Persist))
            .then(|| core.persisted.clone());
        (effects, ui, persisted)
    };
    apply_effects(app, effects, ui, persisted);
}

/// 把当前持久化状态立即写盘（退出前调用）。
pub fn persist_runtime(app: &AppHandle) {
    let state = app.state::<AppState>();
    let core = state.core.lock().unwrap();
    let _ = storage::save_json(&state.paths.state, &core.persisted);
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            let _ = window.hide();
        }
        _ => show_window(app),
    }
}

/// 左键是否仍按着。拖动期间系统拖动循环每帧按光标摆窗口，此时若我们吸附，
/// 窗口会被从光标下拽走、下一帧又被拽回来，观感是「先离开边缘、松手再闪一下贴边」。
#[cfg(windows)]
fn primary_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    // 最高位为 1 表示当前按下。
    unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000 != 0 }
}

#[cfg(not(windows))]
fn primary_button_down() -> bool {
    false
}

/// 吸附缓动：把整窗从 `from` 逐帧滑到 `to`，而不是整窗瞬移 16px。
///
/// 只平移不改尺寸，所以面板展开态也能用（展开矩形跟着折叠条同步平移）；
/// 滑完后再 `apply_panel` 一次性对齐尺寸与方向。期间 `snap_animating` 为真，
/// 让自身 `Moved` 回响不去开启新一代去抖、把自己打断在半路。
///
/// 返回窗口真正停下的矩形：被用户重新按住打断时是半路的最后一帧，调用方拿它落盘，
/// 记忆位置才不会跳到从没到过的终点。
fn glide_to(
    window: &tauri::WebviewWindow,
    state: &AppState,
    from: snap::Rect,
    to: snap::Rect,
    panel_open: bool,
) -> snap::Rect {
    const STEPS: u32 = 6;
    let delta = (to.x - from.x, to.y - from.y);
    state.snap_animating.store(true, Ordering::SeqCst);
    let mut landed = from;
    let mut interrupted = false;
    for step in 1..=STEPS {
        // 缓动只有 ~96ms，但用户可能已经重新按住拖动：立刻让位，不要跟他抢窗口。
        if primary_button_down() {
            interrupted = true;
            break;
        }
        let (dx, dy) = snap::eased_offset(delta, step, STEPS);
        landed = snap::Rect {
            x: from.x + dx,
            y: from.y + dy,
            ..to
        };
        window_platform::apply_position(window, landed.x, landed.y);
        std::thread::sleep(Duration::from_millis(16));
    }
    // 收尾对齐会写一次尺寸，被打断时不做：窗口停在半路，尺寸仍是原样。
    // 面板态还得重新对齐展开方向，否则下次展开会朝错的一边。
    if !interrupted {
        if panel_open {
            window_platform::apply_panel(window, to, true);
        }
        landed = to;
    }
    state.snap_animating.store(false, Ordering::SeqCst);
    landed
}

/// 折叠条移动后吸附：先去抖，再等拖动结束（松开左键）才吸，避免拖动中与系统抢窗口位置。
fn schedule_snap(app: &AppHandle) {
    let state = app.state::<AppState>();
    // 缓动逐帧 set_position 会触发 Moved；这些回响不能开启新一代去抖，
    // 否则缓动会把自己的代数顶掉、被打断在半路。
    if state.snap_animating.load(Ordering::SeqCst) {
        return;
    }
    let generation = state.move_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    std::thread::spawn(move || {
        // 拖动期间 Moved 连续到来，只让最后一跳继续。
        std::thread::sleep(Duration::from_millis(40));
        let state = handle.state::<AppState>();
        if state.move_gen.load(Ordering::SeqCst) != generation {
            return;
        }
        // 还按着左键就说明拖拽没结束：等松手再吸。期间若来了新的 Moved，
        // 说明有更新的一代在等，本代直接退出——所以最多只有一条线程在等。
        while primary_button_down() {
            std::thread::sleep(Duration::from_millis(16));
            if state.move_gen.load(Ordering::SeqCst) != generation {
                return;
            }
        }
        let Some(window) = handle.get_webview_window(WINDOW_LABEL) else {
            return;
        };
        let (panel_open, grows_up) = {
            let core = state.core.lock().unwrap();
            (core.persisted.panel_open, core.panel_grows_up)
        };

        // 两种状态都要吸附：面板开着拖动同样会把外框钳在屏幕顶。
        let raw = if panel_open {
            window_platform::bar_rect_while_open(&window, grows_up)
                .or_else(|| window_platform::current_bar_rect(&window))
        } else {
            window_platform::current_bar_rect(&window)
        };
        let Some(raw) = raw else { return };
        let bar = window_platform::snap_bar_to_work(&window, raw);

        // 只有吸附真的改变了位置才重写窗口：apply_panel 会再触发一次 Moved，
        // 无条件重写会让去抖线程自己喂自己。
        // 缓动被用户重新按住打断时窗口停在半路，落盘的必须是它真正停下的位置，
        // 否则下次启动会跳到从没到过的终点。
        let landed = if (bar.x, bar.y) != (raw.x, raw.y) {
            glide_to(&window, &state, raw, bar, panel_open)
        } else {
            raw
        };

        {
            let mut core = state.core.lock().unwrap();
            let pos = (landed.x, landed.y);
            if core.persisted.bar_pos == Some(pos) {
                return;
            }
            core.persisted.bar_pos = Some(pos);
            let _ = storage::save_json(&state.paths.state, &core.persisted);
        }
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play_pause", "播放 / 暂停", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "上一台", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一台", true, None::<&str>)?;
    let always_on_top =
        CheckMenuItem::with_id(app, "always_on_top", "置顶", true, true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "更新电台库", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &separator,
            &play,
            &prev,
            &next,
            &separator2,
            &always_on_top,
            &sync,
            &quit,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("radiobar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "play_pause" => dispatch(app, |core| core.toggle_play()),
            "prev" => dispatch(app, |core| core.prev()),
            "next" => dispatch(app, |core| core.next()),
            "always_on_top" => {
                let current = {
                    let state = app.state::<AppState>();
                    let core = state.core.lock().unwrap();
                    core.persisted.always_on_top
                };
                dispatch(app, move |core| core.set_always_on_top(!current));
            }
            "sync" => commands::sync_library(app.clone()),
            "quit" => {
                persist_runtime(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let paths = AppPaths {
                state: data_dir.join("state.json"),
                local: data_dir.join("stations.local.json"),
            };

            let persisted: PersistedState = storage::load_json(&paths.state);
            let library =
                Library::load_embedded(SEED_JSON, COUNTRIES_ZH, COUNTRIES_EN, &paths.local);
            let core = AppCore::new_seeded(persisted.clone(), library, rand::random());
            app.manage(AppState::new(core, paths));

            // 已存的键位过一遍校验（丢弃非法值），保证持久化与注册用同一份表。
            let shortcut_map = {
                let state = app.state::<AppState>();
                let mut core = state.core.lock().unwrap();
                core.persisted.shortcuts = shortcuts::resolved(&core.persisted.shortcuts);
                core.persisted.shortcuts.clone()
            };
            shortcuts::spawn_install(&handle, shortcut_map);

            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                let rect = window_platform::initial_bar_rect(&window, persisted.bar_pos);
                window_platform::apply_frame(&window, rect);
                let _ = window.set_always_on_top(persisted.always_on_top);
                {
                    let state = app.state::<AppState>();
                    let mut core = state.core.lock().unwrap();
                    core.persisted.bar_pos = Some((rect.x, rect.y));
                }
                if persisted.panel_open {
                    apply_panel(&handle, true);
                }

                let event_handle = handle.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(window) = event_handle.get_webview_window(WINDOW_LABEL) {
                            let _ = window.hide();
                        }
                    }
                    WindowEvent::Moved(_) => schedule_snap(&event_handle),
                    _ => {}
                });
            }

            build_tray(&handle)?;

            // 上次退出时在播放（或设置了开机自动播）则续播上次电台。
            if persisted.autoplay_on_launch || persisted.was_playing {
                dispatch(&handle, |core| core.toggle_play());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::toggle_play,
            commands::play_station,
            commands::next,
            commands::prev,
            commands::set_volume,
            commands::toggle_mute,
            commands::set_mode,
            commands::set_scope,
            commands::set_always_on_top,
            commands::set_panel_open,
            commands::set_filter,
            commands::set_setting,
            commands::set_shortcut,
            commands::set_shortcut_capture,
            commands::search_stations,
            commands::get_countries,
            commands::toggle_favorite,
            commands::report_playback_error,
            commands::report_playback_started,
            commands::sync_library,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
