use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// 读取 JSON；文件缺失或内容损坏时回退到 `T::default()`。
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<T>(&bytes) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("load_json: 解析 {} 失败（使用默认值）：{err}", path.display());
                T::default()
            }
        },
        Err(err) => {
            if err.kind() != io::ErrorKind::NotFound {
                eprintln!("load_json: 读取 {} 失败（使用默认值）：{err}", path.display());
            }
            T::default()
        }
    }
}

/// 原子写入：先写同目录临时文件并 fsync，再 rename 覆盖目标。
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let tmp = tmp_path(path);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".tmp");
    PathBuf::from(os)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        save_json(&path, &serde_json::json!({ "a": 1 })).unwrap();
        let v: serde_json::Value = load_json(&path);
        assert_eq!(v["a"], 1);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn test_corrupt_json_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, b"{not json").unwrap();
        let v: serde_json::Value = load_json(&path);
        assert_eq!(v, serde_json::Value::default());
    }

    #[test]
    fn test_missing_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let v: serde_json::Value = load_json(&dir.path().join("absent.json"));
        assert_eq!(v, serde_json::Value::default());
    }
}
