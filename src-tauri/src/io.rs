use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{stringify_io, AppResult};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let text = fs::read_to_string(path).map_err(stringify_io)?;
    serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败 {}：{err}", path.display()))
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    atomic_write_text(path, &format!("{text}\n"))
}

pub fn atomic_write_text(path: &Path, text: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(stringify_io)?;
    }
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|item| item.to_str())
            .unwrap_or("file")
    ));
    {
        let mut file = fs::File::create(&temp).map_err(stringify_io)?;
        file.write_all(text.as_bytes()).map_err(stringify_io)?;
        file.sync_all().map_err(stringify_io)?;
    }
    match fs::rename(&temp, path) {
        Ok(_) => Ok(()),
        Err(first_err) if path.exists() => {
            fs::remove_file(path).map_err(|err| {
                let _ = fs::remove_file(&temp);
                format!(
                    "替换 {} 失败：{first_err}；删除旧文件也失败：{err}",
                    path.display()
                )
            })?;
            fs::rename(&temp, path).map_err(stringify_io)
        }
        Err(err) => Err(stringify_io(err)),
    }
}
