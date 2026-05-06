use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChidoriError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("fetch failed: {0}")]
    FetchFailed(String),
    #[error("timed out fetching page after {0} ms")]
    Timeout(u64),
    #[error("page too large: {0} bytes exceeds limit of {1} bytes")]
    TooLarge(u64, u64),
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("no content could be extracted")]
    ExtractionFailed,
    #[error("output failed: {0}")]
    OutputFailed(String),
    #[error("{0}")]
    Unknown(String),
}

impl ChidoriError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Unknown(_) => 1,
            Self::InvalidUrl(_) => 2,
            Self::FetchFailed(_) => 3,
            Self::Timeout(_) => 4,
            Self::TooLarge(_, _) => 5,
            Self::UnsupportedContentType(_) => 6,
            Self::ExtractionFailed => 7,
            Self::OutputFailed(_) => 8,
        }
    }
}
