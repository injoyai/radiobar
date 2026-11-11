use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{ListFilter, Scope, Station};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CountryInfo {
    pub code: String,
    pub name_zh: String,
    pub name_en: String,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub total: usize,
    pub stations: Vec<Station>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ShortStation {
    i: String,
    n: String,
    #[serde(default)]
    c: String,
    #[serde(default)]
    u: String,
    #[serde(default)]
    o: String,
    #[serde(default)]
    k: String,
    #[serde(default)]
    b: u32,
    #[serde(default)]
    h: bool,
    #[serde(default)]
    v: u32,
    #[serde(default)]
    t: Vec<String>,
    #[serde(default)]
    x: u32,
    #[serde(default = "default_true")]
    ok: bool,
}

fn default_true() -> bool {
    true
}

impl ShortStation {
    fn into_station(self) -> Station {
        Station {
            id: self.i,
            name: self.n,
            country_code: self.c,
            url_resolved: self.u,
            url: self.o,
            codec: self.k,
            bitrate: self.b,
            hls: self.h,
            votes: self.v,
            tags: self.t,
            clickcount: self.x,
            last_ok: self.ok,
        }
    }

    fn from_station(st: &Station) -> Self {
        Self {
            i: st.id.clone(),
            n: st.name.clone(),
            c: st.country_code.clone(),
            u: st.url_resolved.clone(),
            o: st.url.clone(),
            k: st.codec.clone(),
            b: st.bitrate,
            h: st.hls,
            v: st.votes,
            t: st.tags.iter().take(2).cloned().collect(),
            x: st.clickcount,
            ok: st.last_ok,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct SeedFile {
    #[serde(default)]
    generated_at: Option<String>,
    #[serde(default)]
    stations: Vec<ShortStation>,
}

pub struct Library {
    stations: Vec<Station>,
    by_id: HashMap<String, usize>,
    countries: Vec<CountryInfo>,
    zh_names: HashMap<String, String>,
    en_names: HashMap<String, String>,
    generated_at: Option<String>,
}

impl Library {
    /// local 存在则读 local，否则读 seed；两者都带短键，需展开为 Station。
    pub fn load(
        seed_path: &Path,
        local_path: &Path,
        countries_zh: &Path,
        countries_en: &Path,
    ) -> Library {
        let zh_names = read_name_map(countries_zh);
        let en_names = read_name_map(countries_en);

        let (file, from_local) = match read_seed(local_path) {
            Some(f) if !f.stations.is_empty() => (f, true),
            _ => (read_seed(seed_path).unwrap_or_default(), false),
        };
        let _ = from_local;

        let mut library = Library {
            stations: file.stations.into_iter().map(|s| s.into_station()).collect(),
            by_id: HashMap::new(),
            countries: Vec::new(),
            zh_names,
            en_names,
            generated_at: file.generated_at,
        };
        library.rebuild_index();
        library
    }

    /// 空库（资源缺失时的兜底）。
    pub fn empty(countries_zh: &Path, countries_en: &Path) -> Library {
        Library {
            stations: Vec::new(),
            by_id: HashMap::new(),
            countries: Vec::new(),
            zh_names: read_name_map(countries_zh),
            en_names: read_name_map(countries_en),
            generated_at: None,
        }
    }

    /// 内置资源（编译期内嵌）版本：local 存在且非空时优先。
    pub fn load_embedded(
        seed_json: &str,
        countries_zh_json: &str,
        countries_en_json: &str,
        local_path: &Path,
    ) -> Library {
        let zh_names = serde_json::from_str(countries_zh_json).unwrap_or_default();
        let en_names = serde_json::from_str(countries_en_json).unwrap_or_default();
        let file = match read_seed(local_path) {
            Some(f) if !f.stations.is_empty() => f,
            _ => serde_json::from_str(seed_json).unwrap_or_default(),
        };
        let mut library = Library {
            stations: file.stations.into_iter().map(|s| s.into_station()).collect(),
            by_id: HashMap::new(),
            countries: Vec::new(),
            zh_names,
            en_names,
            generated_at: file.generated_at,
        };
        library.rebuild_index();
        library
    }

    pub fn len(&self) -> usize {
        self.stations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stations.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Station> {
        self.by_id.get(id).map(|&i| &self.stations[i])
    }

    pub fn countries(&self) -> &[CountryInfo] {
        &self.countries
    }

    pub fn country_name_zh(&self, code: &str) -> Option<&str> {
        self.zh_names.get(code).map(|s| s.as_str())
    }

    pub fn generated_at(&self) -> Option<&str> {
        self.generated_at.as_deref()
    }

    pub fn set_generated_at(&mut self, value: String) {
        self.generated_at = Some(value);
    }

    /// 队列快照：按范围 + 筛选取 id，跳过 lastcheckok=false 的台。
    pub fn snapshot_ids(&self, scope: Scope, favorites: &[String], filter: &ListFilter) -> Vec<String> {
        match scope {
            Scope::Favorites => favorites
                .iter()
                .filter_map(|id| self.get(id))
                .filter(|st| st.last_ok)
                .map(|st| st.id.clone())
                .collect(),
            Scope::List => self
                .filtered_sorted(filter)
                .into_iter()
                .filter(|st| st.last_ok)
                .map(|st| st.id.clone())
                .collect(),
            Scope::All => self
                .sorted_all()
                .into_iter()
                .filter(|st| st.last_ok)
                .map(|st| st.id.clone())
                .collect(),
        }
    }

    /// 列表页：包含 lastcheckok=false 的台（列表里标注「暂不可用」）。
    pub fn search(&self, filter: &ListFilter, offset: usize, limit: usize) -> SearchPage {
        let all = self.filtered_sorted(filter);
        let total = all.len();
        let stations = all
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        SearchPage { total, stations }
    }

    pub fn favorites(&self, favorites: &[String]) -> Vec<Station> {
        favorites
            .iter()
            .filter_map(|id| self.get(id).cloned())
            .collect()
    }

    /// 增量合并：同 id 覆盖、新 id 追加；返回新增数。
    pub fn upsert_all(&mut self, stations: Vec<Station>) -> usize {
        let mut added = 0;
        for st in stations {
            match self.by_id.get(&st.id) {
                Some(&idx) => self.stations[idx] = st,
                None => {
                    self.stations.push(st);
                    added += 1;
                }
            }
        }
        self.rebuild_index();
        added
    }

    pub fn write_local(&self, path: &Path) -> std::io::Result<()> {
        let file = SeedFile {
            generated_at: self.generated_at.clone(),
            stations: self
                .stations
                .iter()
                .map(ShortStation::from_station)
                .collect(),
        };
        crate::storage::save_json(path, &file)
    }

    fn rebuild_index(&mut self) {
        self.stations
            .sort_by(|a, b| b.clickcount.cmp(&a.clickcount).then_with(|| a.name.cmp(&b.name)));
        self.by_id = self
            .stations
            .iter()
            .enumerate()
            .map(|(i, st)| (st.id.clone(), i))
            .collect();
        self.rebuild_countries();
    }

    fn rebuild_countries(&mut self) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for st in &self.stations {
            if st.country_code.is_empty() {
                continue;
            }
            *counts.entry(st.country_code.clone()).or_insert(0) += 1;
        }
        let mut countries: Vec<CountryInfo> = counts
            .into_iter()
            .map(|(code, count)| CountryInfo {
                name_zh: self
                    .zh_names
                    .get(&code)
                    .cloned()
                    .unwrap_or_else(|| code.clone()),
                name_en: self
                    .en_names
                    .get(&code)
                    .cloned()
                    .unwrap_or_else(|| code.clone()),
                code,
                count,
            })
            .collect();
        countries.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name_zh.cmp(&b.name_zh)));
        self.countries = countries;
    }

    fn sorted_all(&self) -> Vec<&Station> {
        let mut list: Vec<&Station> = self.stations.iter().collect();
        list.sort_by(|a, b| b.clickcount.cmp(&a.clickcount).then_with(|| a.name.cmp(&b.name)));
        list
    }

    fn filtered_sorted(&self, filter: &ListFilter) -> Vec<&Station> {
        let query = filter.query.trim().to_lowercase();
        self.sorted_all()
            .into_iter()
            .filter(|st| {
                if let Some(code) = filter.country.as_deref() {
                    if st.country_code != code {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                if st.name.to_lowercase().contains(&query) {
                    return true;
                }
                if st.country_code.to_lowercase().contains(&query) {
                    return true;
                }
                if let Some(name) = self.zh_names.get(&st.country_code) {
                    if name.to_lowercase().contains(&query) {
                        return true;
                    }
                }
                if let Some(name) = self.en_names.get(&st.country_code) {
                    if name.to_lowercase().contains(&query) {
                        return true;
                    }
                }
                st.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
            })
            .collect()
    }
}

fn read_seed(path: &Path) -> Option<SeedFile> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_name_map(path: &Path) -> HashMap<String, String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATIONS: &str = r#"{
        "generated_at": "2026-11-01T00:00:00Z",
        "stations": [
            {"i":"jp1","n":"J-Wave","c":"JP","u":"https://a/jp1.mp3","o":"https://a/jp1.mp3","k":"MP3","b":128,"h":false,"v":10,"t":["jazz","music"],"x":900},
            {"i":"jp2","n":"NHK","c":"JP","u":"https://a/jp2.m3u8","o":"https://a/jp2.m3u8","k":"AAC","b":0,"h":true,"v":5,"t":["news"],"x":800},
            {"i":"cn1","n":"中国之声","c":"CN","u":"https://a/cn1.mp3","o":"https://a/cn1.mp3","k":"MP3","b":64,"h":false,"v":3,"t":["news"],"x":700},
            {"i":"cn2","n":"音乐广播","c":"CN","u":"https://a/cn2.mp3","o":"https://a/cn2.mp3","k":"MP3","b":128,"h":false,"v":2,"t":["music"],"x":650},
            {"i":"us1","n":"KEXP","c":"US","u":"https://a/us1.mp3","o":"https://a/us1.mp3","k":"MP3","b":160,"h":false,"v":9,"t":["indie"],"x":600},
            {"i":"us2","n":"Broken FM","c":"US","u":"https://a/us2.mp3","o":"https://a/us2.mp3","k":"MP3","b":128,"h":false,"v":0,"t":[],"x":100,"ok":false},
            {"i":"de1","n":"Deutschlandfunk","c":"DE","u":"https://a/de1.mp3","o":"https://a/de1.mp3","k":"MP3","b":128,"h":false,"v":4,"t":["news"],"x":500},
            {"i":"fr1","n":"France Inter","c":"FR","u":"https://a/fr1.mp3","o":"https://a/fr1.mp3","k":"MP3","b":128,"h":false,"v":1,"t":["talk"],"x":400}
        ]
    }"#;

    const ZH: &str = r#"{"JP":"日本","CN":"中国","US":"美国","DE":"德国","FR":"法国"}"#;
    const EN: &str = r#"{"JP":"Japan","CN":"China","US":"United States","DE":"Germany","FR":"France"}"#;

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("seed.json"), STATIONS).unwrap();
            std::fs::write(dir.path().join("countries.zh.json"), ZH).unwrap();
            std::fs::write(dir.path().join("countries.en.json"), EN).unwrap();
            Fixture { dir }
        }

        fn load(&self) -> Library {
            Library::load(
                &self.dir.path().join("seed.json"),
                &self.dir.path().join("local.json"),
                &self.dir.path().join("countries.zh.json"),
                &self.dir.path().join("countries.en.json"),
            )
        }
    }

    #[test]
    fn test_search_japanese_country_name_hits_jp() {
        let f = Fixture::new();
        let lib = f.load();
        let page = lib.search(
            &ListFilter {
                query: "日本".into(),
                country: None,
            },
            0,
            50,
        );
        assert_eq!(page.total, 2);
        assert!(page.stations.iter().all(|s| s.country_code == "JP"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let f = Fixture::new();
        let lib = f.load();
        let page = lib.search(
            &ListFilter {
                query: "JAZZ".into(),
                country: None,
            },
            0,
            50,
        );
        assert_eq!(page.total, 1);
        assert_eq!(page.stations[0].id, "jp1");
    }

    #[test]
    fn test_country_filter() {
        let f = Fixture::new();
        let lib = f.load();
        let page = lib.search(
            &ListFilter {
                query: String::new(),
                country: Some("JP".into()),
            },
            0,
            50,
        );
        assert_eq!(page.total, 2);
    }

    #[test]
    fn test_all_sorted_by_clickcount_desc() {
        let f = Fixture::new();
        let lib = f.load();
        let page = lib.search(&ListFilter::default(), 0, 50);
        assert_eq!(page.total, 8);
        assert_eq!(page.stations[0].id, "jp1");
        let clicks: Vec<u32> = page.stations.iter().map(|s| s.clickcount).collect();
        assert!(clicks.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn test_favorites_sorted_in_added_order() {
        let f = Fixture::new();
        let lib = f.load();
        let favs = lib.favorites(&["us1".into(), "cn1".into()]);
        assert_eq!(favs[0].id, "us1");
        assert_eq!(favs[1].id, "cn1");
    }

    #[test]
    fn test_snapshot_ids_skips_not_ok_stations() {
        let f = Fixture::new();
        let lib = f.load();
        let ids = lib.snapshot_ids(Scope::All, &[], &ListFilter::default());
        assert!(!ids.contains(&"us2".to_string()));
        assert_eq!(ids.len(), 7);
        let fav_ids = lib.snapshot_ids(Scope::Favorites, &["us2".into()], &ListFilter::default());
        assert!(fav_ids.is_empty());
    }

    #[test]
    fn test_search_still_shows_not_ok_station() {
        let f = Fixture::new();
        let lib = f.load();
        let page = lib.search(
            &ListFilter {
                query: "Broken".into(),
                country: None,
            },
            0,
            50,
        );
        assert_eq!(page.total, 1);
        assert!(!page.stations[0].last_ok);
    }

    #[test]
    fn test_upsert_updates_and_adds() {
        let f = Fixture::new();
        let mut lib = f.load();
        let mut updated = lib.get("jp1").unwrap().clone();
        updated.name = "J-Wave 改".into();
        let added = lib.upsert_all(vec![
            updated,
            Station {
                id: "new1".into(),
                name: "New".into(),
                country_code: "JP".into(),
                url_resolved: "https://a/new.mp3".into(),
                url: "https://a/new.mp3".into(),
                codec: "MP3".into(),
                bitrate: 128,
                hls: false,
                votes: 0,
                tags: vec![],
                clickcount: 1000,
                last_ok: true,
            },
        ]);
        assert_eq!(added, 1);
        assert_eq!(lib.len(), 9);
        assert_eq!(lib.get("jp1").unwrap().name, "J-Wave 改");
        assert_eq!(lib.search(&ListFilter::default(), 0, 1).stations[0].id, "new1");

        let local = f.dir.path().join("local.json");
        lib.write_local(&local).unwrap();
        assert!(local.exists());
    }

    #[test]
    fn test_load_prefers_local_copy() {
        let f = Fixture::new();
        let lib = f.load();
        lib.write_local(&f.dir.path().join("local.json")).unwrap();
        // 改写 local，去掉一台，验证优先读取 local。
        std::fs::write(
            f.dir.path().join("local.json"),
            r#"{"generated_at":"2026-11-02T00:00:00Z","stations":[{"i":"only","n":"Only","c":"US","u":"https://a/o.mp3","o":"https://a/o.mp3","k":"MP3","b":128,"h":false,"v":1,"t":[],"x":5}]}"#,
        )
        .unwrap();
        let loaded = f.load();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.generated_at(), Some("2026-11-02T00:00:00Z"));
    }
}
