/// Stable negative return codes for the C ABI. Do not renumber.
pub const ERR_ARGUMENT: i32 = -1;
pub const ERR_CONFIG: i32 = -2;
pub const ERR_MODEL: i32 = -3;
pub const ERR_STATE: i32 = -4;
pub const ERR_BUSY: i32 = -5;
pub const ERR_IO: i32 = -6;
pub const ERR_UNAVAILABLE: i32 = -7;
pub const ERR_EVENT_TOO_SMALL: i32 = -8;

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("invalid argument: {0}")]
    Argument(String),
    #[error("invalid config: {0}")]
    Config(String),
    #[error("model error: {0}")]
    Model(String),
    #[error("invalid state: {0}")]
    State(String),
    #[error("busy: {0}")]
    Busy(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("runtime unavailable: {0}")]
    Unavailable(String),
}

impl AsrError {
    pub fn code(&self) -> i32 {
        match self {
            Self::Argument(_) => ERR_ARGUMENT,
            Self::Config(_) => ERR_CONFIG,
            Self::Model(_) => ERR_MODEL,
            Self::State(_) => ERR_STATE,
            Self::Busy(_) => ERR_BUSY,
            Self::Io(_) => ERR_IO,
            Self::Unavailable(_) => ERR_UNAVAILABLE,
        }
    }
}

impl From<std::io::Error> for AsrError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub(crate) fn io(path: &std::path::Path, e: std::io::Error) -> AsrError {
    AsrError::Io(format!("{}: {e}", path.display()))
}
