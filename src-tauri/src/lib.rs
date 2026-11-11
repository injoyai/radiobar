pub mod app_core;
pub mod commands;
pub mod library;
pub mod model;
pub mod queue;
pub mod snap;
pub mod state;
pub mod storage;
pub mod sync;
pub mod window_platform;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
}

impl AppState {
    fn new(core: AppCore, paths: AppPaths) -> Self {
        Self {
            core: Mutex::new(core),
            paths,
            move_gen: AtomicU64::new(0),
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
            CoreEffect::ResizePanel(open) => apply_panel(app, open),
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
    window_platform::apply_panel(&window, bar, open);
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

/// 折叠条移动后延迟吸附：拖动期间不断自增代数，只让最后一跳生效。
fn schedule_snap(app: &AppHandle) {
    let generation = {
        let state = app.state::<AppState>();
        state.move_gen.fetch_add(1, Ordering::SeqCst) + 1
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let state = handle.state::<AppState>();
        if state.move_gen.load(Ordering::SeqCst) != generation {
            return;
        }
        let Some(window) = handle.get_webview_window(WINDOW_LABEL) else {
            return;
        };
        let (panel_open, previous) = {
            let core = state.core.lock().unwrap();
            (
                core.persisted.panel_open,
                core.persisted.bar_pos.map(|(x, y)| snap::Rect {
                    x,
                    y,
                    w: window_platform::BAR_W,
                    h: window_platform::BAR_H,
                }),
            )
        };

        let bar = if panel_open {
            previous
                .and_then(|prev| window_platform::bar_rect_while_open(&window, prev))
                .or_else(|| window_platform::current_bar_rect(&window))
        } else {
            window_platform::snap_bar_window(&window)
        };
        let Some(bar) = bar else { return };

        {
            let mut core = state.core.lock().unwrap();
            let pos = (bar.x, bar.y);
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

            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                let rect = window_platform::initial_bar_rect(&window, persisted.bar_pos);
                window_platform::apply_size(&window, rect.w, rect.h);
                window_platform::apply_position(&window, rect.x, rect.y);
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

            if persisted.autoplay_on_launch {
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
