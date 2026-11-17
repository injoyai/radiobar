use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::library::{CountryInfo, Library, SearchPage};
use crate::model::{ListFilter, PlayMode, Scope, Station};
use crate::queue::{Queue, QueueError};
use crate::state::{PersistedState, PlaybackRequest, UiState};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ToastLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, PartialEq)]
pub enum CoreEffect {
    EmitState,
    Toast { level: ToastLevel, text: String },
    Persist,
    ApplyAlwaysOnTop(bool),
    ResizePanel(bool),
    SetTrayTooltip(Option<String>),
}

pub struct AppCore {
    pub persisted: PersistedState,
    library: Library,
    queue: Queue,
    playing: bool,
    playback_token: u64,
    consecutive_failures: u32,
    try_original_url: bool,
    rng: StdRng,
    /// 上次 `apply_panel` 判定的展开方向（由窗口层回写，供前端决定 DOM 顺序）。
    pub panel_grows_up: bool,
}

impl AppCore {
    pub fn new(persisted: PersistedState, library: Library) -> Self {
        Self::new_seeded(persisted, library, rand::random())
    }

    pub fn new_seeded(persisted: PersistedState, library: Library, seed: u64) -> Self {
        let mut queue = Queue::new(persisted.mode);
        queue.set_history(persisted.history.clone());
        let mut core = Self {
            persisted,
            library,
            queue,
            playing: false,
            playback_token: 0,
            consecutive_failures: 0,
            try_original_url: false,
            rng: StdRng::seed_from_u64(seed),
            panel_grows_up: false,
        };
        core.rebuild_queue();
        core
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn ui_state(&self) -> UiState {
        let current = self.current_station();
        let playback = PlaybackRequest {
            token: self.playback_token,
            url: current
                .as_ref()
                .map(|st| {
                    if self.try_original_url {
                        st.url.clone()
                    } else {
                        st.url_resolved.clone()
                    }
                })
                .unwrap_or_default(),
        };
        UiState {
            current,
            playing: self.playing,
            volume: self.persisted.volume,
            muted: self.persisted.muted,
            mode: self.persisted.mode,
            scope: self.persisted.scope,
            always_on_top: self.persisted.always_on_top,
            panel_open: self.persisted.panel_open,
            favorite_ids: self.persisted.favorites.clone(),
            last_sync: self
                .persisted
                .last_sync
                .clone()
                .or_else(|| self.library.generated_at().map(|s| s.to_string())),
            filter: self.persisted.filter.clone(),
            playback,
            can_next: self.can_step(true),
            can_prev: self.can_step(false),
            autoplay_on_launch: self.persisted.autoplay_on_launch,
            auto_next_on_failure: self.persisted.auto_next_on_failure,
            panel_grows_up: self.panel_grows_up,
            shortcuts: self.persisted.shortcuts.clone(),
        }
    }

    pub fn search(&self, offset: usize, limit: usize) -> SearchPage {
        if self.persisted.scope == Scope::Favorites {
            return self.library.search_favorites(
                &self.persisted.filter,
                &self.persisted.favorites,
                offset,
                limit,
            );
        }
        self.library.search(&self.persisted.filter, offset, limit)
    }

    pub fn countries(&self) -> &[CountryInfo] {
        self.library.countries()
    }

    pub fn set_volume(&mut self, v: u8) -> Vec<CoreEffect> {
        self.persisted.volume = v.min(100);
        if self.persisted.volume > 0 {
            self.persisted.muted = false;
        }
        self.state_effects(true)
    }

    pub fn toggle_mute(&mut self) -> Vec<CoreEffect> {
        self.persisted.muted = !self.persisted.muted;
        self.state_effects(true)
    }

    pub fn set_mode(&mut self, m: PlayMode) -> Vec<CoreEffect> {
        self.persisted.mode = m;
        self.queue.set_mode(m);
        self.state_effects(true)
    }

    pub fn set_scope(&mut self, s: Scope) -> Vec<CoreEffect> {
        self.persisted.scope = s;
        // 设计 §16「当前列表」：换范围不立刻重建队列，避免正在播的台从队列里被踢掉
        // 导致「声音在放、条上却显示未选择电台」。队列在下次切台/播放时按新范围重建。
        self.state_effects(true)
    }

    pub fn set_always_on_top(&mut self, b: bool) -> Vec<CoreEffect> {
        self.persisted.always_on_top = b;
        let mut effects = self.state_effects(true);
        effects.push(CoreEffect::ApplyAlwaysOnTop(b));
        effects
    }

    pub fn set_panel_open(&mut self, b: bool) -> Vec<CoreEffect> {
        self.persisted.panel_open = b;
        let mut effects = self.state_effects(true);
        effects.push(CoreEffect::ResizePanel(b));
        effects
    }

    pub fn set_filter(&mut self, filter: ListFilter) -> Vec<CoreEffect> {
        self.persisted.filter = filter;
        self.state_effects(true)
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Vec<CoreEffect> {
        if self.library.get(id).is_none() {
            return self.toast(ToastLevel::Warn, "找不到该电台");
        }
        if let Some(pos) = self.persisted.favorites.iter().position(|f| f == id) {
            self.persisted.favorites.remove(pos);
        } else {
            self.persisted.favorites.push(id.to_string());
        }
        self.state_effects(true)
    }

    pub fn set_setting(&mut self, key: &str, value: bool) -> Vec<CoreEffect> {
        match key {
            "autoplayOnLaunch" => self.persisted.autoplay_on_launch = value,
            "autoNextOnFailure" => self.persisted.auto_next_on_failure = value,
            _ => return self.toast(ToastLevel::Warn, "未知设置项"),
        }
        self.state_effects(true)
    }

    /// 改键成功后落盘新的完整键位表（注册动作由 shortcuts 模块负责）。
    pub fn set_shortcuts(&mut self, map: BTreeMap<String, String>) -> Vec<CoreEffect> {
        self.persisted.shortcuts = map;
        self.state_effects(true)
    }

    pub fn toggle_play(&mut self) -> Vec<CoreEffect> {
        if self.playing {
            self.playing = false;
            return self.state_effects(false);
        }
        if self.queue.current_id().is_none() {
            self.rebuild_queue();
            match self.queue.next(&mut self.rng) {
                Ok(_) => {
                    self.playing = true;
                    self.playback_token += 1;
                    if let Some(id) = self.queue.current_id().map(|s| s.to_string()) {
                        self.queue.mark_played(&id);
                    }
                    return self.state_effects(true);
                }
                Err(QueueError::Empty) => return self.empty_toast(),
                Err(QueueError::AtEnd) => return self.toast(ToastLevel::Info, "已到列表末尾"),
            }
        }
        self.playing = true;
        self.state_effects(false)
    }

    pub fn play_station(&mut self, id: &str) -> Vec<CoreEffect> {
        if self.library.get(id).is_none() {
            return self.toast(ToastLevel::Warn, "找不到该电台");
        }
        self.rebuild_queue();
        self.queue.set_current(id);
        self.playing = true;
        self.consecutive_failures = 0;
        self.try_original_url = false;
        self.playback_token += 1;
        self.queue.mark_played(id);
        self.state_effects(true)
    }

    pub fn playback_started(&mut self) -> Vec<CoreEffect> {
        self.consecutive_failures = 0;
        self.try_original_url = false;
        Vec::new()
    }

    pub fn next(&mut self) -> Vec<CoreEffect> {
        self.step(true)
    }

    pub fn prev(&mut self) -> Vec<CoreEffect> {
        self.step(false)
    }

    pub fn report_playback_error(&mut self, msg: &str) -> Vec<CoreEffect> {
        eprintln!("playback error: {msg}");
        let Some(id) = self.queue.current_id().map(|s| s.to_string()) else {
            return self.state_effects(false);
        };
        let Some(st) = self.library.get(&id) else {
            return self.state_effects(false);
        };
        let name = st.name.clone();

        // 第一层重试：换原始 url（仅当与 url_resolved 不同）。
        if !self.try_original_url && st.url_resolved != st.url {
            self.try_original_url = true;
            self.playback_token += 1;
            return self.state_effects(false);
        }

        self.consecutive_failures += 1;
        self.try_original_url = false;

        if self.consecutive_failures >= 3 {
            self.playing = false;
            return self.toast_with_state(ToastLevel::Error, "连续 3 台播放失败，已暂停");
        }

        if !self.persisted.auto_next_on_failure {
            self.playing = false;
            return self.toast_with_state(ToastLevel::Warn, &format!("{name} 播放失败，已暂停"));
        }

        self.rebuild_queue();
        match self.queue.next(&mut self.rng) {
            Ok(_) => {
                let next_name = self
                    .queue
                    .current_id()
                    .map(|s| s.to_string())
                    .and_then(|id| {
                        self.queue.mark_played(&id);
                        self.library.get(&id).map(|st| st.name.clone())
                    })
                    .unwrap_or_default();
                self.playing = true;
                self.playback_token += 1;
                let mut effects = self.state_effects(true);
                effects.push(CoreEffect::Toast {
                    level: ToastLevel::Warn,
                    text: format!("{name} 播放失败，已切到 {next_name}"),
                });
                effects
            }
            Err(_) => {
                self.playing = false;
                self.toast_with_state(ToastLevel::Warn, &format!("{name} 播放失败，已暂停"))
            }
        }
    }

    pub fn sync_finished(&mut self, last_sync: String) -> Vec<CoreEffect> {
        self.persisted.last_sync = Some(last_sync);
        self.rebuild_queue();
        self.state_effects(true)
    }

    /// 增量同步合并进库，返回新增台数。
    pub fn merge_sync(&mut self, stations: Vec<Station>) -> usize {
        let added = self.library.upsert_all(stations);
        self.rebuild_queue();
        added
    }

    pub fn mark_library_synced(&mut self) {
        self.rebuild_queue();
    }

    /// 用当前范围 + 筛选 + 收藏重建队列快照；列表未变化时保留 shuffle 已播集合。
    pub fn rebuild_queue(&mut self) {
        // 队列还没定位过（首次启动）时用持久化的上次电台补上，
        // 否则 current 一直是 None，按播放会走 next() 跳到别的台。
        let current_id = self
            .queue
            .current_id()
            .map(|s| s.to_string())
            .or_else(|| self.persisted.current_id.clone());
        let ids = self.library.snapshot_ids(
            self.persisted.scope,
            &self.persisted.favorites,
            &self.persisted.filter,
        );
        if ids != self.queue.ids() {
            self.queue.set_ids(ids, current_id.as_deref());
        }
    }

    fn step(&mut self, forward: bool) -> Vec<CoreEffect> {
        self.rebuild_queue();
        let result = if forward {
            self.queue.next(&mut self.rng)
        } else {
            self.queue.prev()
        };
        match result {
            Ok(_) => {
                if let Some(id) = self.queue.current_id().map(|s| s.to_string()) {
                    self.queue.mark_played(&id);
                }
                self.playing = true;
                self.consecutive_failures = 0;
                self.try_original_url = false;
                self.playback_token += 1;
                self.state_effects(true)
            }
            Err(QueueError::Empty) => self.empty_toast(),
            Err(QueueError::AtEnd) => self.toast_with_state(ToastLevel::Info, "已到列表末尾"),
        }
    }

    fn current_station(&self) -> Option<Station> {
        self.queue
            .current_id()
            .and_then(|id| self.library.get(id))
            .cloned()
            .or_else(|| {
                self.persisted
                    .current_id
                    .as_deref()
                    .and_then(|id| self.library.get(id))
                    .cloned()
            })
    }

    fn can_step(&self, forward: bool) -> bool {
        let len = self.queue.ids().len();
        if len == 0 {
            return false;
        }
        if self.persisted.mode != PlayMode::Sequential {
            return true;
        }
        match self.queue.current_id().and_then(|id| self.queue.position_of(id)) {
            Some(pos) => {
                if forward {
                    pos + 1 < len
                } else {
                    pos > 0
                }
            }
            None => true,
        }
    }

    fn tray_tooltip(&self) -> Option<String> {
        if !self.playing {
            return None;
        }
        let st = self.current_station()?;
        let country = self
            .library
            .country_name_zh(&st.country_code)
            .unwrap_or(&st.country_code)
            .to_string();
        Some(format!("{} · {}", st.name, country))
    }

    fn state_effects(&mut self, persist: bool) -> Vec<CoreEffect> {
        self.persisted.current_id = self.queue.current_id().map(|s| s.to_string());
        self.persisted.history = self.queue.history().to_vec();
        self.persisted.was_playing = self.playing;
        let mut effects = vec![
            CoreEffect::EmitState,
            CoreEffect::SetTrayTooltip(self.tray_tooltip()),
        ];
        if persist {
            effects.push(CoreEffect::Persist);
        }
        effects
    }

    fn empty_toast(&mut self) -> Vec<CoreEffect> {
        let text = match self.persisted.scope {
            Scope::Favorites => "收藏为空，先去面板里加几台",
            Scope::List => "列表为空",
            Scope::All => "电台库为空",
        };
        self.toast_with_state(ToastLevel::Warn, text)
    }

    fn toast(&mut self, level: ToastLevel, text: &str) -> Vec<CoreEffect> {
        let mut effects = self.state_effects(false);
        effects.push(CoreEffect::Toast {
            level,
            text: text.to_string(),
        });
        effects
    }

    fn toast_with_state(&mut self, level: ToastLevel, text: &str) -> Vec<CoreEffect> {
        self.toast(level, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATIONS: &str = r#"{
        "generated_at": "2026-11-01T00:00:00Z",
        "stations": [
            {"i":"a","n":"A 电台","c":"CN","u":"https://a/a.mp3","o":"https://a/a-orig.mp3","k":"MP3","b":64,"h":false,"v":1,"t":[],"x":900},
            {"i":"b","n":"B 电台","c":"JP","u":"https://a/b.mp3","o":"https://a/b.mp3","k":"MP3","b":128,"h":false,"v":1,"t":[],"x":800},
            {"i":"c","n":"C 电台","c":"US","u":"https://a/c.mp3","o":"https://a/c.mp3","k":"AAC","b":128,"h":false,"v":1,"t":[],"x":700},
            {"i":"d","n":"D 电台","c":"US","u":"https://a/d.mp3","o":"https://a/d.mp3","k":"MP3","b":128,"h":false,"v":1,"t":[],"x":600}
        ]
    }"#;

    fn fixture() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("seed.json"), STATIONS).unwrap();
        std::fs::write(dir.path().join("zh.json"), r#"{"CN":"中国","JP":"日本","US":"美国"}"#).unwrap();
        std::fs::write(dir.path().join("en.json"), r#"{"CN":"China","JP":"Japan","US":"United States"}"#).unwrap();
        let lib = Library::load(
            &dir.path().join("seed.json"),
            &dir.path().join("local.json"),
            &dir.path().join("zh.json"),
            &dir.path().join("en.json"),
        );
        (dir, lib)
    }

    fn core_with(scope: Scope) -> AppCore {
        let (_dir, lib) = fixture();
        let mut persisted = PersistedState::default();
        persisted.scope = scope;
        AppCore::new_seeded(persisted, lib, 42)
    }

    fn toast_text(effects: &[CoreEffect]) -> Option<String> {
        effects.iter().find_map(|e| match e {
            CoreEffect::Toast { text, .. } => Some(text.clone()),
            _ => None,
        })
    }

    #[test]
    fn test_ui_state_reports_panel_grows_up() {
        let mut core = core_with(Scope::All);
        assert!(!core.ui_state().panel_grows_up);
        core.panel_grows_up = true;
        assert!(core.ui_state().panel_grows_up);
        core.panel_grows_up = false;
        assert!(!core.ui_state().panel_grows_up);
    }

    #[test]
    fn test_stall_triggers_retry_then_switch() {
        let mut core = core_with(Scope::All);
        let _ = core.play_station("a");
        assert_eq!(core.ui_state().current.unwrap().id, "a");

        // 第一次失败：改试原始 url。
        let effects = core.report_playback_error("stalled");
        assert_eq!(core.ui_state().playback.url, "https://a/a-orig.mp3");
        assert!(core.playing());
        assert!(!effects.iter().any(|e| matches!(e, CoreEffect::Toast { .. })));

        // 第二次失败：切到下一台。
        let effects = core.report_playback_error("stalled");
        assert!(core.playing());
        assert_ne!(core.ui_state().current.unwrap().id, "a");
        assert!(toast_text(&effects).unwrap().contains("已切到"));
    }

    #[test]
    fn test_three_consecutive_failures_stop() {
        let mut core = core_with(Scope::All);
        core.play_station("a");
        let mut last_toast = String::new();
        for _ in 0..3 {
            core.report_playback_error("boom");
            let effects = core.report_playback_error("boom");
            last_toast = toast_text(&effects).unwrap_or_default();
        }
        assert!(!core.playing());
        assert!(last_toast.contains("连续 3 台"), "toast was {last_toast}");
    }

    #[test]
    fn test_retry_uses_original_url_second_time() {
        let mut core = core_with(Scope::All);
        core.play_station("a");
        let effects = core.report_playback_error("x");
        let state = core.ui_state();
        assert_eq!(state.playback.url, "https://a/a-orig.mp3");
        assert!(effects.iter().any(|e| matches!(e, CoreEffect::EmitState)));
    }

    #[test]
    fn test_sequential_next_at_end_toasts() {
        let mut core = core_with(Scope::All);
        let mut persisted = PersistedState::default();
        persisted.scope = Scope::All;
        persisted.mode = PlayMode::Sequential;
        // 走到末尾
        for _ in 0..4 {
            core.next();
        }
        let current_before = core.ui_state().current.map(|s| s.id);
        let effects = core.next();
        assert_eq!(toast_text(&effects).unwrap(), "已到列表末尾");
        assert_eq!(core.ui_state().current.map(|s| s.id), current_before);
        drop(persisted);
    }

    #[test]
    fn test_favorites_empty_toast() {
        let mut core = core_with(Scope::Favorites);
        let effects = core.next();
        assert!(toast_text(&effects).unwrap().contains("收藏为空"));
        assert!(core.ui_state().current.is_none());
    }

    #[test]
    fn test_toggle_play_pause() {
        let mut core = core_with(Scope::All);
        core.toggle_play();
        assert!(core.playing());
        let token = core.ui_state().playback.token;
        core.toggle_play();
        assert!(!core.playing());
        assert_eq!(core.ui_state().playback.token, token);
    }

    #[test]
    fn test_startup_keeps_persisted_station_when_play_pressed() {
        let (_dir, lib) = fixture();
        let mut persisted = PersistedState::default();
        persisted.scope = Scope::All;
        persisted.current_id = Some("b".to_string());
        let mut core = AppCore::new_seeded(persisted, lib, 42);

        // 界面显示的电台来自持久化；队列当前项也必须是它，
        // 否则点播放会走 next() 换到另一台。
        assert_eq!(core.ui_state().current.unwrap().id, "b");

        core.toggle_play();
        assert!(core.playing());
        assert_eq!(core.ui_state().current.unwrap().id, "b");
    }

    #[test]
    fn test_persisted_was_playing_tracks_play_state() {
        let mut core = core_with(Scope::All);
        assert!(!core.persisted.was_playing);
        core.toggle_play();
        assert!(core.persisted.was_playing);
        core.toggle_play();
        assert!(!core.persisted.was_playing);
    }

    #[test]
    fn test_play_station_sets_playing_and_token_increments() {
        let mut core = core_with(Scope::All);
        core.play_station("c");
        assert!(core.playing());
        let t1 = core.ui_state().playback.token;
        core.play_station("b");
        assert_eq!(core.ui_state().playback.token, t1 + 1);
        assert_eq!(core.ui_state().current.unwrap().id, "b");
    }

    #[test]
    fn test_search_respects_favorites_scope() {
        let mut core = core_with(Scope::All);
        core.toggle_favorite("b");
        let all = core.search(0, 50);
        assert_eq!(all.total, 4);

        core.set_scope(Scope::Favorites);
        let favs = core.search(0, 50);
        assert_eq!(favs.total, 1);
        assert_eq!(favs.stations[0].id, "b");
    }

    #[test]
    fn test_toggle_favorite_adds_and_removes() {
        let mut core = core_with(Scope::All);
        core.toggle_favorite("a");
        assert_eq!(core.ui_state().favorite_ids, vec!["a".to_string()]);
        core.toggle_favorite("a");
        assert!(core.ui_state().favorite_ids.is_empty());
    }

    #[test]
    fn test_set_setting_updates_and_persists() {
        let mut core = core_with(Scope::All);
        let effects = core.set_setting("autoplayOnLaunch", true);
        assert!(core.ui_state().autoplay_on_launch);
        assert!(effects.iter().any(|e| matches!(e, CoreEffect::Persist)));
    }

    #[test]
    fn test_mode_and_scope_persist_and_rebuild_on_next() {
        let mut core = core_with(Scope::All);
        core.set_mode(PlayMode::Loop);
        core.set_scope(Scope::List);
        let state = core.ui_state();
        assert_eq!(state.mode, PlayMode::Loop);
        assert_eq!(state.scope, Scope::List);
        core.next();
        assert!(core.ui_state().current.is_some());
    }

    #[test]
    fn test_scope_switch_keeps_playing_station() {
        // 设计 §16：播放中改范围不应把正在播的台从队列里踢掉（否则声音在放、条上却显示未选择电台）。
        let mut core = core_with(Scope::All);
        core.play_station("c");
        let before = core.ui_state().current.expect("current station");
        assert_eq!(before.id, "c");

        core.set_scope(Scope::Favorites);

        let after = core.ui_state();
        assert_eq!(after.scope, Scope::Favorites);
        assert_eq!(after.current.as_ref().map(|s| s.id.as_str()), Some("c"));
        assert!(after.playing);

        // 下一次切台才按新范围重建队列；收藏为空时给出空范围提示。
        let effects = core.next();
        assert_eq!(toast_text(&effects).as_deref(), Some("收藏为空，先去面板里加几台"));
    }
}
