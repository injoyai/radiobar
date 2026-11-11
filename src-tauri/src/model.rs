use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub id: String,
    pub name: String,
    #[serde(rename = "country")]
    pub country_code: String,
    pub url_resolved: String,
    pub url: String,
    pub codec: String,
    pub bitrate: u32,
    pub hls: bool,
    pub votes: u32,
    pub tags: Vec<String>,
    pub clickcount: u32,
    pub last_ok: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode {
    #[default]
    Sequential,
    Loop,
    Shuffle,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Favorites,
    List,
    #[default]
    All,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub query: String,
    pub country: Option<String>,
}

/// 去掉 query / fragment 后，路径部分以 `.m3u8` 结尾（大小写不敏感）。
pub fn is_hls_url(url: &str) -> bool {
    let without_fragment = url.split('#').next().unwrap_or("");
    let without_query = without_fragment.split('?').next().unwrap_or("");
    without_query.to_ascii_lowercase().ends_with(".m3u8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hls_url() {
        assert!(is_hls_url("https://a.com/live.m3u8"));
        assert!(is_hls_url("https://a.com/live.M3U8?token=1"));
        assert!(is_hls_url("https://a.com/live.m3u8#frag"));
        assert!(!is_hls_url("https://a.com/live.mp3"));
        assert!(!is_hls_url("https://a.com/m3u8/stream"));
    }

    #[test]
    fn test_play_mode_and_scope_serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&PlayMode::Shuffle).unwrap(),
            "\"shuffle\""
        );
        assert_eq!(serde_json::to_string(&Scope::Favorites).unwrap(), "\"favorites\"");
        assert_eq!(PlayMode::default(), PlayMode::Sequential);
        assert_eq!(Scope::default(), Scope::All);
    }

    #[test]
    fn test_station_json_field_names() {
        let st = Station {
            id: "1".into(),
            name: "测试".into(),
            country_code: "CN".into(),
            url_resolved: "https://a/b.mp3".into(),
            url: "https://a/b.mp3".into(),
            codec: "MP3".into(),
            bitrate: 64,
            hls: false,
            votes: 1,
            tags: vec![],
            clickcount: 2,
            last_ok: true,
        };
        let v = serde_json::to_value(&st).unwrap();
        assert_eq!(v["country"], "CN");
        assert_eq!(v["urlResolved"], "https://a/b.mp3");
        assert_eq!(v["lastOk"], true);
    }
}
