use std::{fmt, io};

#[derive(Debug)]
pub enum CoreError {
    Io(io::Error),
    Json(serde_json::Error),
    Invalid(String),
    NotFound(String),
    Conflict(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "檔案操作失敗：{error}"),
            Self::Json(error) => write!(f, "Manifest 格式錯誤：{error}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::NotFound(message) => write!(f, "{message}"),
            Self::Conflict(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<io::Error> for CoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
