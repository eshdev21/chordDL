use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    AuthRequired,
    RateLimited,
    Unavailable,
    BrowserLock,
    DependencyMissing,
    Generic,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[allow(dead_code)]
    #[error("yt-dlp error: {0}")]
    YtDlp(String), // Fallback generic error

    #[error("Login required. Use Firefox Auth.")]
    YtDlpLoginRequired,

    #[error("Too many requests (429). Try again later.")]
    YtDlpRateLimited,

    #[error("Video unavailable (Geo-block/Private/Deleted).")]
    YtDlpUnavailable,

    #[error("Network error during download.")]
    YtDlpNetworkError,

    #[error("Browser cookies locked (Firefox is running). Please close Firefox and retry.")]
    YtDlpBrowserLock,

    #[error("Dependency missing: {0}")]
    DependencyMissing(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[allow(dead_code)]
    #[error("Cancelled")]
    Cancelled,

    #[error("Playlist URL detected. Please switch to Playlist mode.")]
    PlaylistUrlInSingleMode,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("message", &self.user_message())?;
        map.serialize_entry("category", &self.category())?;
        map.serialize_entry("severity", &self.severity())?;
        map.end()
    }
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl AppError {
    #[allow(dead_code)]
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AppError::YtDlpLoginRequired
            | AppError::YtDlpBrowserLock
            | AppError::PlaylistUrlInSingleMode
            | AppError::YtDlpRateLimited => ErrorSeverity::Warning,
            AppError::YtDlpUnavailable
            | AppError::YtDlpNetworkError
            | AppError::YtDlp(_)
            | AppError::Io(_) => ErrorSeverity::Error,
            AppError::Database(_) | AppError::DependencyMissing(_) | AppError::Internal(_) => {
                ErrorSeverity::Critical
            }
            AppError::Cancelled => ErrorSeverity::Info,
        }
    }

    #[allow(dead_code)]
    pub fn user_message(&self) -> String {
        match self {
            AppError::YtDlpLoginRequired => {
                "Bot detection triggered. Please sign in to Firefox.".to_string()
            }
            AppError::YtDlpBrowserLock => {
                "Firefox cookies are locked. Close Firefox and retry.".to_string()
            }
            AppError::YtDlpRateLimited => "YouTube rate limited. Try again in an hour.".to_string(),
            AppError::YtDlpUnavailable => {
                "This video is unavailable (region-locked or private).".to_string()
            }
            AppError::PlaylistUrlInSingleMode => {
                "This is a playlist URL — switch to Playlist mode.".to_string()
            }
            _ => self.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn recovery_action(&self) -> Option<&'static str> {
        match self {
            AppError::YtDlpLoginRequired => Some("open_settings"),
            AppError::YtDlpBrowserLock | AppError::YtDlpRateLimited => Some("retry"),
            AppError::DependencyMissing(_) => Some("open_setup"),
            _ => None,
        }
    }

    pub fn category(&self) -> ErrorCategory {
        match self {
            AppError::YtDlpLoginRequired => ErrorCategory::AuthRequired,
            AppError::YtDlpRateLimited => ErrorCategory::RateLimited,
            AppError::YtDlpUnavailable => ErrorCategory::Unavailable,
            AppError::YtDlpBrowserLock => ErrorCategory::BrowserLock,
            AppError::DependencyMissing(_) => ErrorCategory::DependencyMissing,
            _ => ErrorCategory::Generic,
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
