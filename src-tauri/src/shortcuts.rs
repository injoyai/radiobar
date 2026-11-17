use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{dispatch, AppState};

/// 动作 id 与默认键位。
pub const DEFAULT_SHORTCUTS: [(&str, &str); 5] = [
    ("prev", "control+ArrowLeft"),
    ("next", "control+ArrowRight"),
    ("volumeUp", "control+ArrowUp"),
    ("volumeDown", "control+ArrowDown"),
    ("playPause", "control+KeyP"),
];

/// 音量键步长（折叠条滚轮是 2）。
const VOLUME_STEP: u8 = 5;

/// 注册/注销要跳主线程并阻塞等待，两个线程并发操作会互相踩，串行化。
static LOCK: Mutex<()> = Mutex::new(());

pub fn defaults() -> BTreeMap<String, String> {
    DEFAULT_SHORTCUTS
        .iter()
        .map(|(action, accel)| (action.to_string(), accel.to_string()))
        .collect()
}

pub fn label(action: &str) -> &'static str {
    match action {
        "prev" => "上一台",
        "next" => "下一台",
        "volumeUp" => "音量 +",
        "volumeDown" => "音量 −",
        "playPause" => "播放 / 暂停",
        _ => "未知动作",
    }
}

fn is_action(action: &str) -> bool {
    DEFAULT_SHORTCUTS.iter().any(|(id, _)| *id == action)
}

/// 校验并规范化组合键：必须带至少一个修饰键，且能被插件解析。
fn normalize(raw: &str) -> Option<String> {
    let text = raw.trim();
    if !text.contains('+') {
        return None;
    }
    Shortcut::from_str(text).ok()?;
    Some(text.to_string())
}

fn parse(accel: &str) -> Option<Shortcut> {
    Shortcut::from_str(accel).ok()
}

fn same_shortcut(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// 用持久化里的值覆盖默认键位；非法值丢弃，未知动作丢弃。
/// 因此返回的表始终是完整的动作集合。
pub fn resolved(saved: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut map = defaults();
    for (action, accel) in saved {
        if !is_action(action) {
            continue;
        }
        if let Some(accel) = normalize(accel) {
            map.insert(action.clone(), accel);
        }
    }
    map
}

fn current(app: &AppHandle) -> BTreeMap<String, String> {
    let state = app.state::<AppState>();
    let map = state.core.lock().unwrap().persisted.shortcuts.clone();
    map
}

/// 全量换绑：先清空再逐条注册。返回每个注册失败的键（空表示全部成功）。
///
/// 必须从后台线程调用：插件内部 `run_on_main_thread` + `rx.recv()`，
/// 在主线程里直接调用会等自己而自锁。
pub fn install(app: &AppHandle, map: &BTreeMap<String, String>) -> Vec<String> {
    let _guard = LOCK.lock().unwrap();
    let shortcuts = app.global_shortcut();
    let mut errors = Vec::new();
    if let Err(err) = shortcuts.unregister_all() {
        errors.push(format!("解除旧快捷键失败：{err}"));
    }
    for (action, accel) in map {
        let id = action.clone();
        let outcome = shortcuts.on_shortcut(accel.as_str(), move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                run(app, &id);
            }
        });
        if let Err(err) = outcome {
            errors.push(format!("{}（{accel}）注册失败：{err}", label(action)));
        }
    }
    errors
}

/// 录制改键期间临时释放所有全局键，否则按下已注册的组合会被系统吞掉，
/// 设置页根本收不到 keydown。
pub fn suspend(app: &AppHandle) {
    let _guard = LOCK.lock().unwrap();
    let _ = app.global_shortcut().unregister_all();
}

/// 按持久化里当前的键位表重新注册（录制结束或改键失败时恢复）。
pub fn restore(app: &AppHandle) -> Vec<String> {
    let map = current(app);
    install(app, &map)
}

fn run(app: &AppHandle, action: &str) {
    dispatch(app, move |core| match action {
        "prev" => core.prev(),
        "next" => core.next(),
        "volumeUp" => {
            let volume = core.persisted.volume.saturating_add(VOLUME_STEP);
            core.set_volume(volume)
        }
        "volumeDown" => {
            let volume = core.persisted.volume.saturating_sub(VOLUME_STEP);
            core.set_volume(volume)
        }
        "playPause" => core.toggle_play(),
        _ => Vec::new(),
    });
}

/// 改绑一个动作：校验 → 注册 → 失败则恢复原键位。成功返回新的完整键位表。
pub fn rebind(
    app: &AppHandle,
    action: &str,
    accel: &str,
) -> Result<BTreeMap<String, String>, String> {
    if !is_action(action) {
        return Err("未知的快捷键动作".to_string());
    }
    let accel =
        normalize(accel).ok_or_else(|| "组合键无效：需要至少一个 Ctrl / Alt / Shift / Win".to_string())?;

    let previous = current(app);
    if let Some((other, _)) = previous
        .iter()
        .find(|(id, existing)| id.as_str() != action && same_shortcut(existing, &accel))
    {
        return Err(format!("{accel} 已经分配给了「{}」", label(other)));
    }

    let mut next = previous.clone();
    next.insert(action.to_string(), accel.clone());
    let errors = install(app, &next);
    if !errors.is_empty() {
        let _ = install(app, &previous);
        return Err(errors.join("；"));
    }
    Ok(next)
}

/// 启动时在后台线程注册，避免阻塞主线程 setup。
pub fn spawn_install(app: &AppHandle, map: BTreeMap<String, String>) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let errors = install(&handle, &map);
        if !errors.is_empty() {
            let _ = handle.emit(
                "toast",
                serde_json::json!({ "level": "warn", "text": errors.join("；") }),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_match_requested_keys() {
        let map = defaults();
        assert_eq!(map["prev"], "control+ArrowLeft");
        assert_eq!(map["next"], "control+ArrowRight");
        assert_eq!(map["volumeUp"], "control+ArrowUp");
        assert_eq!(map["volumeDown"], "control+ArrowDown");
        assert_eq!(map["playPause"], "control+KeyP");
    }

    #[test]
    fn test_resolved_overlays_saved_and_drops_garbage() {
        let saved = BTreeMap::from([
            ("prev".to_string(), "control+shift+ArrowLeft".to_string()),
            ("playPause".to_string(), "not-a-key".to_string()),
            ("bogus".to_string(), "control+KeyQ".to_string()),
        ]);
        let map = resolved(&saved);
        assert_eq!(map["prev"], "control+shift+ArrowLeft");
        // 非法值回落到默认，未知动作被丢弃
        assert_eq!(map["playPause"], "control+KeyP");
        assert!(!map.contains_key("bogus"));
        assert_eq!(map.len(), DEFAULT_SHORTCUTS.len());
    }

    #[test]
    fn test_normalize_requires_modifier_and_valid_key() {
        assert_eq!(
            normalize(" control+ArrowUp ").as_deref(),
            Some("control+ArrowUp")
        );
        // 无修饰键：不接管裸按键
        assert!(normalize("ArrowUp").is_none());
        assert!(normalize("control+NotAKey").is_none());
    }

    #[test]
    fn test_same_shortcut_ignores_alias_and_case() {
        assert!(same_shortcut("ctrl+Left", "control+ArrowLeft"));
        assert!(same_shortcut("control+keyp", "control+KeyP"));
        assert!(!same_shortcut("control+ArrowLeft", "control+ArrowRight"));
        assert!(!same_shortcut("control+ArrowLeft", "control+shift+ArrowLeft"));
    }
}
