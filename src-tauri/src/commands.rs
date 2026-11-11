use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::app_core::AppCore;
use crate::library::{CountryInfo, SearchPage};
use crate::model::{ListFilter, PlayMode, Scope};
use crate::state::UiState;
use crate::sync::{self, HttpFetcher};
use crate::{apply_effects, dispatch, persist_runtime, AppState};

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> UiState {
    state.core.lock().unwrap().ui_state()
}

#[tauri::command]
pub fn toggle_play(app: AppHandle) {
    dispatch(&app, |core| core.toggle_play());
}

#[tauri::command]
pub fn play_station(app: AppHandle, id: String) {
    dispatch(&app, move |core| core.play_station(&id));
}

#[tauri::command]
pub fn next(app: AppHandle) {
    dispatch(&app, AppCore::next);
}

#[tauri::command]
pub fn prev(app: AppHandle) {
    dispatch(&app, AppCore::prev);
}

#[tauri::command]
pub fn set_volume(app: AppHandle, volume: u8) {
    dispatch(&app, move |core| core.set_volume(volume));
}

#[tauri::command]
pub fn toggle_mute(app: AppHandle) {
    dispatch(&app, AppCore::toggle_mute);
}

#[tauri::command]
pub fn set_mode(app: AppHandle, mode: PlayMode) {
    dispatch(&app, move |core| core.set_mode(mode));
}

#[tauri::command]
pub fn set_scope(app: AppHandle, scope: Scope) {
    dispatch(&app, move |core| core.set_scope(scope));
}

#[tauri::command]
pub fn set_always_on_top(app: AppHandle, value: bool) {
    dispatch(&app, move |core| core.set_always_on_top(value));
}

#[tauri::command]
pub fn set_panel_open(app: AppHandle, open: bool) {
    dispatch(&app, move |core| core.set_panel_open(open));
}

#[tauri::command]
pub fn set_filter(app: AppHandle, filter: ListFilter) {
    dispatch(&app, move |core| core.set_filter(filter));
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: bool) {
    dispatch(&app, move |core| core.set_setting(&key, value));
}

#[tauri::command]
pub fn search_stations(state: State<'_, AppState>, offset: usize, limit: usize) -> SearchPage {
    let limit = limit.min(100);
    state.core.lock().unwrap().search(offset, limit)
}

#[tauri::command]
pub fn get_countries(state: State<'_, AppState>) -> Vec<CountryInfo> {
    state.core.lock().unwrap().countries().to_vec()
}

#[tauri::command]
pub fn toggle_favorite(app: AppHandle, id: String) {
    dispatch(&app, move |core| core.toggle_favorite(&id));
}

#[tauri::command]
pub fn report_playback_error(app: AppHandle, message: String) {
    dispatch(&app, move |core| core.report_playback_error(&message));
}

#[tauri::command]
pub fn report_playback_started(app: AppHandle) {
    dispatch(&app, |core| core.playback_started());
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    persist_runtime(&app);
    app.exit(0);
}

#[tauri::command]
pub fn sync_library(app: AppHandle) {
    static SYNCING: AtomicBool = AtomicBool::new(false);
    if SYNCING.swap(true, Ordering::SeqCst) {
        let _ = app.emit(
            "toast",
            serde_json::json!({ "level": "warn", "text": "同步进行中，请稍候" }),
        );
        return;
    }

    let (countries, local_path) = {
        let state = app.state::<AppState>();
        let core = state.core.lock().unwrap();
        (
            core.countries()
                .iter()
                .map(|c| c.code.clone())
                .collect::<Vec<String>>(),
            state.paths.local.clone(),
        )
    };

    let handle = app.clone();
    std::thread::spawn(move || {
        let outcome = (|| -> Result<(usize, usize), String> {
            let mut fetcher = HttpFetcher::new()?;
            let stations = sync::run(&mut fetcher, &countries, |done, total| {
                let _ = handle.emit(
                    "sync-progress",
                    serde_json::json!({ "phase": "fetching", "done": done, "total": total }),
                );
            })?;

            let state = handle.state::<AppState>();
            let persisted;
            let effects;
            let ui;
            let added;
            {
                let mut core = state.core.lock().unwrap();
                added = core.merge_sync(stations);
                let _ = core.library().write_local(&local_path);
                effects = core.sync_finished(sync::now_iso());
                ui = core.ui_state();
                persisted = Some(core.persisted.clone());
            }
            apply_effects(&handle, effects, ui, persisted);
            Ok((added, countries.len()))
        })();

        match outcome {
            Ok((added, total)) => {
                let _ = handle.emit(
                    "toast",
                    serde_json::json!({
                        "level": "info",
                        "text": format!("更新完成：覆盖 {total} 个国家，新增 {added} 台")
                    }),
                );
            }
            Err(err) => {
                let _ = handle.emit(
                    "toast",
                    serde_json::json!({ "level": "error", "text": format!("更新失败：{err}") }),
                );
            }
        }
        SYNCING.store(false, Ordering::SeqCst);
    });
}
