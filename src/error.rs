use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use soulcore::error::SoulCoreError;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Authorization error: {0}")]
    Authorization(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Conflict: {0}")]
    Conflict(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("File upload error: {0}")]
    FileUpload(String),
    
    #[error("Image processing error: {0}")]
    ImageProcessing(String),
    
    #[error("Email error: {0}")]
    Email(String),
    
    #[error("External service error: {0}")]
    ExternalService(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),
    
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    
    #[error("Validation error: {0}")]
    ValidatorError(#[from] validator::ValidationErrors),
    
    #[error("Parse error: {0}")]
    Parse(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, error_code) = match &self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string(), "DATABASE_ERROR")
            }
            AppError::Authentication(msg) => {
                (StatusCode::UNAUTHORIZED, msg.clone(), "AUTHENTICATION_ERROR")
            }
            AppError::Authorization(msg) => {
                (StatusCode::FORBIDDEN, msg.clone(), "AUTHORIZATION_ERROR")
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "VALIDATION_ERROR")
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, msg.clone(), "NOT_FOUND")
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, msg.clone(), "CONFLICT")
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "BAD_REQUEST")
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), "INTERNAL_ERROR")
            }
            AppError::ServiceUnavailable(msg) => {
                (StatusCode::SERVICE_UNAVAILABLE, msg.clone(), "SERVICE_UNAVAILABLE")
            }
            AppError::RateLimitExceeded => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string(), "RATE_LIMIT_EXCEEDED")
            }
            AppError::FileUpload(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "FILE_UPLOAD_ERROR")
            }
            AppError::ImageProcessing(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "IMAGE_PROCESSING_ERROR")
            }
            AppError::Email(msg) => {
                tracing::error!("Email error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Email service error".to_string(), "EMAIL_ERROR")
            }
            AppError::ExternalService(msg) => {
                tracing::error!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External service error".to_string(), "EXTERNAL_SERVICE_ERROR")
            }
            AppError::Serialization(e) => {
                tracing::error!("Serialization error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Serialization error".to_string(), "SERIALIZATION_ERROR")
            }
            AppError::Request(e) => {
                tracing::error!("Request error: {}", e);
                (StatusCode::BAD_REQUEST, "Request error".to_string(), "REQUEST_ERROR")
            }
            AppError::Io(e) => {
                tracing::error!("IO error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "IO error".to_string(), "IO_ERROR")
            }
            AppError::Utf8(e) => {
                tracing::error!("UTF-8 error: {}", e);
                (StatusCode::BAD_REQUEST, "Invalid UTF-8".to_string(), "UTF8_ERROR")
            }
            AppError::Uuid(e) => {
                tracing::error!("UUID error: {}", e);
                (StatusCode::BAD_REQUEST, "Invalid UUID".to_string(), "UUID_ERROR")
            }
            AppError::Jwt(e) => {
                tracing::debug!("JWT error: {}", e);
                (StatusCode::UNAUTHORIZED, "Invalid token".to_string(), "JWT_ERROR")
            }
            AppError::ValidatorError(e) => {
                tracing::warn!("Validation failed: {:?}", e);
                let validation_errors = e
                    .field_errors()
                    .iter()
                    .map(|(field, errors)| {
                        (
                            field.to_string(),
                            errors.iter().map(|e| e.message.as_ref().unwrap_or(&"Invalid value".into()).to_string()).collect::<Vec<_>>()
                        )
                    })
                    .collect::<std::collections::HashMap<String, Vec<String>>>();
                    
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": {
                            "code": "VALIDATION_ERROR",
                            "message": "Validation failed",
                            "details": validation_errors
                        }
                    }))
                ).into_response();
            }
            AppError::Parse(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "PARSE_ERROR")
            }
        };

        let body = Json(json!({
            "error": {
                "code": error_code,
                "message": error_message
            }
        }));

        (status, body).into_response()
    }
}

// 便利函数，用于创建常见错误
impl AppError {
    pub fn not_found(resource: &str) -> Self {
        Self::NotFound(format!("{} not found", resource))
    }

    pub fn unauthorized(msg: &str) -> Self {
        Self::Authentication(msg.to_string())
    }

    pub fn forbidden(msg: &str) -> Self {
        Self::Authorization(msg.to_string())
    }

    pub fn bad_request(msg: &str) -> Self {
        Self::BadRequest(msg.to_string())
    }

    pub fn internal(msg: &str) -> Self {
        Self::Internal(msg.to_string())
    }

    pub fn conflict(msg: &str) -> Self {
        Self::Conflict(msg.to_string())
    }

    pub fn validation(msg: &str) -> Self {
        Self::Validation(msg.to_string())
    }
}

// 从其他错误类型转换
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<SoulCoreError> for AppError {
    fn from(err: SoulCoreError) -> Self {
        AppError::Database(surrealdb::Error::thrown(err.to_string()))
    }
}
