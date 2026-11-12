use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::model::Station;

pub const USER_AGENT: &str = "radiobar/0.1";
pub const API_BASE: &str = "https://de1.api.radio-browser.info";
const PAGE_LIMIT: u32 = 50;

pub trait PageFetcher {
    fn fetch(&mut self, country: &str, limit: u32) -> Result<Vec<Station>, String>;
}

/// 逐页拉取每个国家 top 50；单国失败不中断整体同步，全部失败才返回 Err。
pub fn run<T: PageFetcher>(
    fetcher: &mut T,
    countries: &[String],
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Station>, String> {
    run_with_delay(fetcher, countries, Duration::from_millis(300), &mut progress)
}

pub fn run_with_delay<T: PageFetcher>(
    fetcher: &mut T,
    countries: &[String],
    delay: Duration,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Station>, String> {
    let total = countries.len();
    let mut seen_ids = HashSet::new();
    let mut stations = Vec::new();
    let mut failures = Vec::new();

    for (i, code) in countries.iter().enumerate() {
        match fetcher.fetch(code, PAGE_LIMIT) {
            Ok(fetched) => {
                for st in fetched {
                    if seen_ids.insert(st.id.clone()) {
                        stations.push(st);
                    }
                }
            }
            Err(err) => {
                eprintln!("sync: 国家 {code} 拉取失败：{err}");
                failures.push(code.clone());
            }
        }
        progress(i + 1, total);
        if i + 1 < total {
            std::thread::sleep(delay);
        }
    }

    if !countries.is_empty() && stations.is_empty() {
        return Err(format!("同步失败：{} 个国家均未返回数据", failures.len()));
    }
    Ok(stations)
}

pub struct HttpFetcher {
    client: reqwest::blocking::Client,
    base: String,
}

impl HttpFetcher {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            base: API_BASE.to_string(),
        })
    }

    pub fn with_base(base: &str) -> Result<Self, String> {
        let mut fetcher = Self::new()?;
        fetcher.base = base.to_string();
        Ok(fetcher)
    }
}

impl PageFetcher for HttpFetcher {
    fn fetch(&mut self, country: &str, limit: u32) -> Result<Vec<Station>, String> {
        let url = format!(
            "{}/json/stations/bycountrycodeexact/{}?limit={}&hidebroken=true&order=clickcount&reverse=true",
            self.base, country, limit
        );
        let mut last_err = String::new();
        for attempt in 0..3 {
            match self.client.get(&url).send() {
                Ok(resp) => match resp.json::<Vec<RawStation>>() {
                    Ok(raw) => return Ok(raw.into_iter().map(RawStation::into_station).collect()),
                    Err(e) => last_err = format!("解析失败：{e}"),
                },
                Err(e) => last_err = format!("请求失败：{e}"),
            }
            if attempt < 2 {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        Err(last_err)
    }
}

#[derive(Deserialize)]
struct RawStation {
    #[serde(default)]
    stationuuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    countrycode: String,
    #[serde(default)]
    url_resolved: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    codec: String,
    #[serde(default)]
    bitrate: u32,
    #[serde(default, deserialize_with = "de_boolish")]
    hls: bool,
    #[serde(default)]
    votes: u32,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    clickcount: u32,
    #[serde(default = "default_lastcheck")]
    lastcheckok: u8,
}

fn default_lastcheck() -> u8 {
    1
}

/// radio-browser 的 `hls` 字段在真实响应里是数字 0/1（布尔值只是历史/测试写法），
/// 两种形态都要能解析，否则整个国家的电台都会因反序列化失败而被丢弃。
fn de_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    use serde_json::Value;
    match Value::deserialize(deserializer)? {
        Value::Bool(b) => Ok(b),
        Value::Number(n) => Ok(n.as_f64().map(|f| f != 0.0).unwrap_or(false)),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "" | "0" | "false" | "no" => Ok(false),
            other => Err(D::Error::custom(format!("invalid hls value: {other}"))),
        },
        Value::Null => Ok(false),
        other => Err(D::Error::custom(format!("invalid hls value: {other}"))),
    }
}

impl RawStation {
    fn into_station(self) -> Station {
        let tags = self
            .tags
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .take(2)
            .map(|s| s.to_string())
            .collect();
        Station {
            id: self.stationuuid,
            name: self.name.trim().to_string(),
            country_code: self.countrycode,
            url_resolved: self.url_resolved,
            url: self.url,
            codec: self.codec,
            bitrate: self.bitrate,
            hls: self.hls,
            votes: self.votes,
            tags,
            clickcount: self.clickcount,
            last_ok: self.lastcheckok == 1,
        }
    }
}

pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (y, m, d) = civil_from_days(secs / 86400);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 days -> civil date 算法。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeFetcher {
        calls: Vec<String>,
        fail_on: Option<String>,
    }

    impl PageFetcher for FakeFetcher {
        fn fetch(&mut self, country: &str, _limit: u32) -> Result<Vec<Station>, String> {
            self.calls.push(country.to_string());
            if Some(country.to_string()) == self.fail_on {
                return Err("boom".into());
            }
            let base = match country {
                "JP" => 500,
                "CN" => 900,
                _ => 100,
            };
            Ok((0..2)
                .map(|i| Station {
                    id: format!("{country}-{i}"),
                    name: format!("{country} 电台 {i}"),
                    country_code: country.to_string(),
                    url_resolved: "https://a/x.mp3".into(),
                    url: "https://a/x.mp3".into(),
                    codec: "MP3".into(),
                    bitrate: 128,
                    hls: false,
                    votes: 1,
                    tags: vec![],
                    clickcount: base + i,
                    last_ok: true,
                })
                .collect())
        }
    }

    #[test]
    fn test_run_dedupes_and_reports_progress() {
        let mut fetcher = FakeFetcher {
            calls: vec![],
            fail_on: None,
        };
        let countries: Vec<String> = ["JP", "CN", "US"].iter().map(|s| s.to_string()).collect();
        let mut progress: Vec<(usize, usize)> = vec![];
        let stations = run_with_delay(
            &mut fetcher,
            &countries,
            Duration::ZERO,
            |done, total| progress.push((done, total)),
        )
        .unwrap();
        assert_eq!(stations.len(), 6);
        assert_eq!(fetcher.calls, vec!["JP", "CN", "US"]);
        assert_eq!(progress.len(), 3);
        assert_eq!(progress.last().unwrap(), &(3, 3));
    }

    #[test]
    fn test_run_single_country_failure_does_not_abort() {
        let mut fetcher = FakeFetcher {
            calls: vec![],
            fail_on: Some("CN".into()),
        };
        let countries: Vec<String> = ["JP", "CN", "US"].iter().map(|s| s.to_string()).collect();
        let stations =
            run_with_delay(&mut fetcher, &countries, Duration::ZERO, |_, _| {}).unwrap();
        assert_eq!(stations.len(), 4);
        assert_eq!(fetcher.calls.len(), 3);
    }

    #[test]
    fn test_run_all_failures_returns_err() {
        struct AlwaysFail;
        impl PageFetcher for AlwaysFail {
            fn fetch(&mut self, _: &str, _: u32) -> Result<Vec<Station>, String> {
                Err("nope".into())
            }
        }
        let countries = vec!["JP".to_string()];
        let result = run_with_delay(&mut AlwaysFail, &countries, Duration::ZERO, |_, _| {});
        assert!(result.is_err());
    }

    #[test]
    fn test_now_iso_format() {
        let s = now_iso();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
        assert!(s.starts_with("202") || s.starts_with("20"));
    }

    /// 真实 radio-browser 响应里 `hls` 是数字 0/1，不是布尔值。
    /// 夹具 `tests/fixtures/stations_api.json` 为 2026-11-11 从
    /// `https://de1.api.radio-browser.info/json/stations/bycountrycodeexact/JP` 抓取的原始响应。
    #[test]
    fn test_raw_station_parses_real_api_payload() {
        let raw: Vec<RawStation> =
            serde_json::from_str(include_str!("../tests/fixtures/stations_api.json")).unwrap();
        assert_eq!(raw.len(), 20);
        let stations: Vec<Station> = raw.into_iter().map(RawStation::into_station).collect();
        assert!(stations.iter().all(|s| !s.name.is_empty()));
        assert!(stations.iter().any(|s| s.hls), "夹具含 hls=1 的台");
        assert!(stations.iter().any(|s| !s.hls), "夹具含 hls=0 的台");
        assert_eq!(stations[0].country_code, "JP");
    }

    #[test]
    fn test_hls_accepts_number_and_bool() {
        for (json, expected) in [
            (r#"{"hls":0}"#, false),
            (r#"{"hls":1}"#, true),
            (r#"{"hls":true}"#, true),
            (r#"{"hls":false}"#, false),
        ] {
            let raw: RawStation = serde_json::from_str(json).unwrap();
            assert_eq!(raw.into_station().hls, expected, "payload {json}");
        }
    }

    #[test]
    fn test_raw_station_tags_trimmed_and_capped() {
        let raw: RawStation = serde_json::from_str(
            r#"{"stationuuid":"x","name":" Name ","countrycode":"CN","url":"http://a","url_resolved":"https://a","codec":"MP3","bitrate":64,"hls":false,"votes":1,"tags":" ,a,b,c ","clickcount":5,"lastcheckok":0}"#,
        )
        .unwrap();
        let st = raw.into_station();
        assert_eq!(st.name, "Name");
        assert_eq!(st.tags, vec!["a".to_string(), "b".to_string()]);
        assert!(!st.last_ok);
    }
}
