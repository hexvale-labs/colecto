use colecto_core::ErrorCategory;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Authentication,
    Forbidden,
    BadRequest,
    NotFound,
    Conflict,
    TooManyRequests,
    BadGateway,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceError {
    pub kind: ErrorKind,
    pub message: String,
}

impl ServiceError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn forbidden() -> Self {
        Self::new(ErrorKind::Forbidden, "This request is not granted access")
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Authentication, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadRequest, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, message)
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::TooManyRequests, message)
    }

    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadGateway, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn category(&self) -> ErrorCategory {
        if self.message.contains("uncertain outcome") {
            ErrorCategory::DeliveryUncertain
        } else if self
            .message
            .starts_with("Mail server rejected authentication")
            || self.message.starts_with("JMAP HTTP 401:")
            || self.message.starts_with("JMAP HTTP 403:")
        {
            ErrorCategory::ProviderAuthentication
        } else if self.message.starts_with("Mail server timed out") {
            ErrorCategory::ProviderTimeout
        } else if self.message.starts_with("Mail server TLS") {
            ErrorCategory::ProviderTls
        } else if self.message.starts_with("JMAP HTTP 429:") {
            ErrorCategory::ProviderRateLimited
        } else {
            match self.kind {
                ErrorKind::Authentication => ErrorCategory::Authentication,
                ErrorKind::Forbidden => ErrorCategory::Forbidden,
                ErrorKind::BadRequest => ErrorCategory::InvalidRequest,
                ErrorKind::NotFound => ErrorCategory::NotFound,
                ErrorKind::Conflict => ErrorCategory::Conflict,
                ErrorKind::TooManyRequests | ErrorKind::BadGateway => ErrorCategory::Provider,
                ErrorKind::Internal => ErrorCategory::StorageOrService,
            }
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ServiceError {}

impl From<colecto_core::Error> for ServiceError {
    fn from(error: colecto_core::Error) -> Self {
        match error {
            colecto_core::Error::Transport(_) => Self::bad_gateway(error.to_string()),
            colecto_core::Error::Missing => Self::not_found(error.to_string()),
            colecto_core::Error::Invalid(_) => Self::bad_request(error.to_string()),
            colecto_core::Error::Storage(_) => {
                tracing::error!(%error, "Local mail storage error");
                Self::internal(
                    "Could not access local mail storage. Your unsaved changes are still in the editor.",
                )
            }
        }
    }
}

/// Read an HTTP response body up to `max` bytes, failing if the stream errors or exceeds `max`.
pub(crate) async fn read_bounded(
    mut response: reqwest::Response,
    max: usize,
) -> Result<Vec<u8>, ()> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if body.len() + chunk.len() > max {
            return Err(());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
