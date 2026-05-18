pub type AppResult<T> = Result<T, String>;

pub fn stringify_io(err: std::io::Error) -> String {
    err.to_string()
}
