pub type AppResult<T> = Result<T, String>;

pub async fn run_blocking<T, F>(work: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("后台任务执行失败：{err}"))?
}

pub fn stringify_io(err: std::io::Error) -> String {
    err.to_string()
}
