use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    User {
        code: &'static str,
        message: String,
        detail: Option<String>,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
}

impl AppError {
    pub fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self::User {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(
        code: &'static str,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::User {
            code,
            message: message.into(),
            detail: Some(detail.into()),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::User { code, .. } => code,
            Self::Io(_) => "io_error",
            Self::Json(_) => "json_error",
            Self::Yaml(_) => "yaml_error",
            Self::Url(_) => "url_error",
            Self::Http(_) => "network_error",
            Self::Keyring(_) => "keychain_error",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct WireError<'a> {
            code: &'a str,
            message: String,
            detail: Option<String>,
        }

        let (message, detail) = match self {
            Self::User {
                message, detail, ..
            } => (message.clone(), detail.clone()),
            other => (other.to_string(), None),
        };

        WireError {
            code: self.code(),
            message,
            detail,
        }
        .serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
