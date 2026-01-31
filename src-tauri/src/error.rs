//! App error type for Tauri commands. Implements Display and Serialize for frontend.

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    FfmpegNotFound(String),

    #[error("FFmpeg failed (code {code}): {stderr}")]
    FfmpegFailed { code: i32, stderr: String },

    #[error("Aborted")]
    Aborted,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        if s == "Aborted" {
            AppError::Aborted
        } else {
            AppError::FfmpegFailed {
                code: -1,
                stderr: s,
            }
        }
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_aborted_string() {
        let e = AppError::from("Aborted");
        assert!(matches!(e, AppError::Aborted));
    }

    #[test]
    fn from_other_string() {
        let e = AppError::from("some error message");
        match &e {
            AppError::FfmpegFailed { code, stderr } => {
                assert_eq!(*code, -1);
                assert_eq!(stderr, "some error message");
            }
            _ => panic!("expected FfmpegFailed"),
        }
    }

    #[test]
    fn from_str_works() {
        let e: AppError = "Aborted".into();
        assert!(matches!(e, AppError::Aborted));
    }
}
