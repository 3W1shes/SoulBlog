use crate::{
    error::{Result, AppError},
    state::AppState,
    services::auth::User,
    models::media::MediaUploadResponse,
};
use axum::{
    extract::{
        multipart::MultipartRejection, DefaultBodyLimit, Multipart, Path, Query, State,
    },
    response::{Json, Response},
    routing::{get, post, delete},
    Router,
    Extension,
    http::{StatusCode, header},
    body::Body,
};
use serde_json::{json, Value};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, error, debug};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/upload", post(upload_image))
        .route("/files/*path", get(serve_file))
        .route("/:file_id", delete(delete_file))
        .route("/", get(list_user_files))
        // Let business-layer size checks (MAX_UPLOAD_SIZE) decide upload limits.
        .layer(DefaultBodyLimit::disable())
}

#[derive(Debug, Deserialize)]
pub struct MediaListQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
}

/// 上传图片
/// POST /api/blog/media/upload
pub async fn upload_image(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    multipart: std::result::Result<Multipart, MultipartRejection>,
) -> Result<Json<MediaUploadResponse>> {
    debug!("Processing image upload for user: {}", user.id);

    let mut multipart = multipart.map_err(|e| {
        error!("Multipart extraction failed: {}", e);
        AppError::BadRequest(format!("上传请求格式错误: {}", e))
    })?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    // 处理multipart表单数据
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to process multipart field: {}", e);
        AppError::BadRequest("无法处理上传的文件".to_string())
    })? {
        let field_name = field.name().unwrap_or("");
        
        if field_name == "file" {
            // 获取文件信息
            filename = field.file_name().map(|s| s.to_string());
            content_type = field.content_type().map(|s| s.to_string());
            
            // 读取文件数据
            let data = field.bytes().await.map_err(|e| {
                error!("Failed to read file data: {}", e);
                AppError::BadRequest("无法读取文件数据".to_string())
            })?;
            
            file_data = Some(data.to_vec());
            break;
        }
    }

    // 验证必要的数据
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("未找到上传的文件".to_string()))?;
    let filename = filename.unwrap_or_else(|| "unnamed".to_string());
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    debug!("Uploading file: {} ({}), size: {} bytes", filename, content_type, file_data.len());

    // 调用媒体服务处理上传
    let upload_result = app_state.media_service
        .upload_image(&user.id, &filename, &content_type, file_data)
        .await
        .map_err(|e| {
            error!("Upload image failed for user {}: {}", user.id, e);
            e
        })?;

    info!("Successfully uploaded image for user: {}, filename: {}", user.id, filename);

    Ok(Json(upload_result))
}

/// 获取文件
/// GET /api/blog/media/files/*path
pub async fn serve_file(
    State(app_state): State<Arc<AppState>>,
    Path(file_path): Path<String>,
) -> Result<Response<Body>> {
    debug!("Serving file: {}", file_path);

    // 获取文件数据
    let file_data = app_state.media_service.get_file(&file_path).await?;

    // 根据文件扩展名确定内容类型
    let content_type = determine_content_type(&file_path);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // 缓存一年
        .body(Body::from(file_data))
        .map_err(|e| {
            error!("Failed to build file response: {}", e);
            AppError::Internal("构建文件响应失败".to_string())
        })?;

    Ok(response)
}

/// 删除文件
/// DELETE /api/blog/media/:file_id
pub async fn delete_file(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(file_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting file: {} for user: {}", file_id, user.id);

    app_state.media_service.delete_file(&user.id, &file_id).await?;

    info!("Successfully deleted file: {} for user: {}", file_id, user.id);

    Ok(Json(json!({
        "success": true,
        "message": "文件已删除"
    })))
}

/// 获取用户的文件列表
/// GET /api/blog/media/
pub async fn list_user_files(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<MediaListQuery>,
) -> Result<Json<Value>> {
    debug!("Listing files for user: {}", user.id);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100); // 限制最大每页100个

    let (files, total) = app_state.media_service
        .get_user_files(&user.id, page, limit)
        .await?;

    let total_pages = (total + limit - 1) / limit;

    Ok(Json(json!({
        "success": true,
        "data": {
            "files": files.iter().map(|f| f.to_response()).collect::<Vec<_>>(),
            "pagination": {
                "current_page": page,
                "total_pages": total_pages,
                "total_items": total,
                "items_per_page": limit,
                "has_next": page < total_pages,
                "has_prev": page > 1,
            }
        }
    })))
}

fn determine_content_type(file_path: &str) -> &'static str {
    let extension = file_path.split('.').last().unwrap_or("").to_lowercase();
    
    match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
