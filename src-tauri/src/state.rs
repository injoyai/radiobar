use serde::{Deserialize, Serialize};

use crate::model::{ListFilter, PlayMode, Scope, Station};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedState {
    pub volume: u8,
    pub muted: bool,
    pub mode: PlayMode,
    pub scope: Scope,
    pub current_id: Option<String>,
    pub always_on_top: bool,
    /// 折叠条左上角坐标（canonical rect）。
    pub bar_pos: Option<(i32, i32)>,
    pub panel_open: bool,
    pub last_sync: Option<String>,
    pub favorites: Vec<String>,
    pub history: Vec<String>,
    pub filter: ListFilter,
    pub autoplay_on_launch: bool,
    pub auto_next_on_failure: bool,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            volume: 70,
            muted: false,
            mode: PlayMode::Sequential,
            scope: Scope::All,
            current_id: None,
            always_on_top: true,
            bar_pos: None,
            panel_open: false,
            last_sync: None,
            favorites: Vec::new(),
            history: Vec::new(),
            filter: ListFilter::default(),
            autoplay_on_launch: false,
            auto_next_on_failure: true,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackRequest {
    pub token: u64,
    pub url: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub current: Option<Station>,
    pub playing: bool,
    pub volume: u8,
    pub muted: bool,
    pub mode: PlayMode,
    pub scope: Scope,
    pub always_on_top: bool,
    pub panel_open: bool,
    pub favorite_ids: Vec<String>,
    pub last_sync: Option<String>,
    pub filter: ListFilter,
    pub playback: PlaybackRequest,
    pub can_next: bool,
    pub can_prev: bool,
    pub autoplay_on_launch: bool,
    pub auto_next_on_failure: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let s = PersistedState::default();
        assert_eq!(s.volume, 70);
        assert!(!s.muted);
        assert!(s.always_on_top);
        assert!(!s.panel_open);
        assert!(!s.autoplay_on_launch);
        assert!(s.auto_next_on_failure);
        assert_eq!(s.mode, PlayMode::Sequential);
        assert_eq!(s.scope, Scope::All);
        assert!(s.favorites.is_empty());
        assert!(s.history.is_empty());
    }

    #[test]
    fn test_persisted_state_reads_partial_json() {
        let s: PersistedState = serde_json::from_str(r#"{"volume": 30}"#).unwrap();
        assert_eq!(s.volume, 30);
        assert!(s.always_on_top);
    }
}
