//! Agent API 统一响应格式
//! 
//! 为 OpenClaw / Agent 调用提供统一的响应信封

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use super::request_id::{generate_request_id, RequestId};

/// Agent API 统一响应信封
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse<T> {
    /// 是否成功
    pub ok: bool,
    
    /// 响应数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentError>,
    
    /// 请求追踪 ID
    pub request_id: String,
}

/// Agent API 错误结构
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentError {
    /// 错误代码
    pub code: String,
    
    /// 错误消息
    pub message: String,
    
    /// 额外详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AgentResponse<()> {
    /// 生成新的请求 ID
    pub fn generate_request_id() -> String {
        generate_request_id()
    }
}

impl<T> AgentResponse<T> {
    pub fn success_with_request_id(data: T, request_id: Option<RequestId>) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
            request_id: request_id
                .map(|request_id| request_id.0)
                .unwrap_or_else(AgentResponse::generate_request_id),
        }
    }

    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self::success_with_request_id(data, None)
    }
    
    pub fn error_with_request_id(
        code: impl Into<String>,
        message: impl Into<String>,
        request_id: Option<RequestId>,
    ) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(AgentError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
            request_id: request_id
                .map(|request_id| request_id.0)
                .unwrap_or_else(AgentResponse::generate_request_id),
        }
    }

    /// 创建错误响应
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::error_with_request_id(code, message, None)
    }
    
    /// 创建带详情的错误响应
    pub fn error_with_details(
        code: impl Into<String>, 
        message: impl Into<String>,
        details: serde_json::Value
    ) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(AgentError {
                code: code.into(),
                message: message.into(),
                details: Some(details),
            }),
            request_id: AgentResponse::generate_request_id(),
        }
    }
}

impl<T: Serialize> IntoResponse for AgentResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.ok {
            StatusCode::OK
        } else {
            match self.error.as_ref().map(|e| e.code.as_str()) {
                Some("unauthorized") => StatusCode::UNAUTHORIZED,
                Some("forbidden") => StatusCode::FORBIDDEN,
                Some("not_found") => StatusCode::NOT_FOUND,
                Some("bad_request") => StatusCode::BAD_REQUEST,
                Some("conflict") => StatusCode::CONFLICT,
                Some("too_many_requests") => StatusCode::TOO_MANY_REQUESTS,
                Some("bad_gateway") => StatusCode::BAD_GATEWAY,
                Some("service_unavailable") => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        };
        
        (status, Json(self)).into_response()
    }
}

/// 从 AppError 转换
impl From<crate::error::AppError> for AgentError {
    fn from(err: crate::error::AppError) -> Self {
        match err {
            crate::error::AppError::Authentication(msg) => Self {
                code: "unauthorized".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::Authorization(msg) => Self {
                code: "forbidden".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::NotFound(msg) => Self {
                code: "not_found".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::BadRequest(msg) => Self {
                code: "bad_request".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::Conflict(msg) => Self {
                code: "conflict".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::Validation(msg) => Self {
                code: "bad_request".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::RateLimitExceeded => Self {
                code: "too_many_requests".to_string(),
                message: "Rate limit exceeded".to_string(),
                details: None,
            },
            crate::error::AppError::ServiceUnavailable(msg) => Self {
                code: "service_unavailable".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::ExternalService(msg) => Self {
                code: "bad_gateway".to_string(),
                message: msg,
                details: None,
            },
            crate::error::AppError::Internal(_)
            | crate::error::AppError::Database(_)
            | crate::error::AppError::FileUpload(_)
            | crate::error::AppError::ImageProcessing(_)
            | crate::error::AppError::Email(_)
            | crate::error::AppError::Serialization(_)
            | crate::error::AppError::Request(_)
            | crate::error::AppError::Io(_)
            | crate::error::AppError::Utf8(_)
            | crate::error::AppError::Uuid(_)
            | crate::error::AppError::Jwt(_)
            | crate::error::AppError::ValidatorError(_)
            | crate::error::AppError::Parse(_) => Self {
                code: "internal_error".to_string(),
                message: "Internal server error".to_string(),
                details: None,
            },
        }
    }
}
