use crate::{
    error::{Result, AppError},
    config::Config,
    models::media::{MediaFile, MediaUploadResponse},
    utils::image::ImageProcessor,
    services::database::Database,
};
use std::path::Path;
use std::sync::Arc;
use chrono::{Utc, Datelike};
use uuid::Uuid;
use tokio::fs;
use surrealdb::sql::Thing;

#[derive(Clone)]
pub struct MediaService {
    config: Config,
    db: Arc<Database>,
}

impl MediaService {
    pub async fn new(config: &Config, db: Arc<Database>) -> Result<Self> {
        Ok(Self { 
            config: config.clone(),
            db,
        })
    }

    pub async fn upload_image(&self, user_id: &str, filename: &str, content_type: &str, data: Vec<u8>) -> Result<MediaUploadResponse> {
        // 验证文件类型
        self.validate_image_type(content_type)?;
        
        // 验证文件大小
        if data.len() as u64 > self.config.max_upload_size {
            return Err(AppError::BadRequest("文件大小超出限制".to_string()));
        }

        // 使用图片处理器验证和获取图片信息
        let image_processor = ImageProcessor::new();
        
        // 验证图片格式
        if !image_processor.is_valid_image(&data) {
            return Err(AppError::BadRequest("无效的图片格式".to_string()));
        }

        // 获取图片尺寸
        let dimensions = image_processor.get_dimensions(&data).map_err(|e| AppError::BadRequest(e))?;
        let (width, height) = (dimensions.width, dimensions.height);

        // 生成文件名和存储路径
        let file_extension = self.get_file_extension(content_type);
        let file_id = Uuid::new_v4().to_string();
        let stored_filename = format!("{}.{}", file_id, file_extension);
        
        // 创建存储目录结构 (按日期分组)
        let now = Utc::now();
        let date_path = format!("{}/{:02}/{:02}", now.year(), now.month(), now.day());
        let storage_dir = format!("uploads/images/{}", date_path);
        let storage_path = format!("{}/{}", storage_dir, stored_filename);
        
        // 确保目录存在
        if let Err(e) = fs::create_dir_all(&storage_dir).await {
            tracing::error!("Failed to create upload directory: {}", e);
            return Err(AppError::Internal("创建上传目录失败".to_string()));
        }

        // 保存文件到磁盘
        if let Err(e) = fs::write(&storage_path, &data).await {
            tracing::error!("Failed to write file: {}", e);
            return Err(AppError::Internal("保存文件失败".to_string()));
        }

        // 生成公开访问URL
        let public_url = format!("/api/blog/media/files/{}", storage_path.replace("uploads/", ""));

        // 创建数据库记录
        let media_file = MediaFile {
            id: surrealdb::sql::Thing::from(("media_file".to_string(), file_id.clone())),
            user_id: user_id.to_string(),
            filename: stored_filename.clone(),
            original_filename: filename.to_string(),
            content_type: content_type.to_string(),
            size: data.len() as i64,
            width: Some(width),
            height: Some(height),
            storage_path: storage_path.clone(),
            public_url: public_url.clone(),
            created_at: now,
        };

        // 保存到数据库
        let _: MediaFile = self.db
            .create("media_file", media_file.clone())
            .await
            .map_err(|e| {
                tracing::error!("Failed to save media file to database: {}", e);
                AppError::Internal("保存文件信息到数据库失败".to_string())
            })?;

        tracing::info!("Successfully uploaded image: {} for user: {}", stored_filename, user_id);

        Ok(media_file.to_response())
    }

    pub async fn get_file(&self, file_path: &str) -> Result<Vec<u8>> {
        let full_path = format!("uploads/{}", file_path);
        
        // 验证路径安全性
        let canonical_path = Path::new(&full_path).canonicalize()
            .map_err(|_| AppError::NotFound("文件不存在".to_string()))?;
        
        let uploads_dir = Path::new("uploads").canonicalize()
            .map_err(|_| AppError::Internal("上传目录配置错误".to_string()))?;
        
        if !canonical_path.starts_with(&uploads_dir) {
            return Err(AppError::BadRequest("非法的文件路径".to_string()));
        }

        // 读取文件
        fs::read(&full_path).await
            .map_err(|_| AppError::NotFound("文件不存在".to_string()))
    }

    pub async fn delete_file(&self, user_id: &str, file_id: &str) -> Result<()> {
        // 查找文件记录
        let media_file: Option<MediaFile> = self.db
            .get_by_id("media_file", file_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to query media file: {}", e);
                AppError::Internal("查询文件失败".to_string())
            })?;

        let media_file = media_file.ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

        // 验证所有权
        if media_file.user_id != user_id {
            return Err(AppError::Authorization("无权限删除此文件".to_string()));
        }

        // 删除物理文件
        if let Err(e) = fs::remove_file(&media_file.storage_path).await {
            tracing::warn!("Failed to delete physical file: {}", e);
        }

        // 删除数据库记录
        self.db
            .delete_by_id("media_file", file_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to delete media file from database: {}", e);
                AppError::Internal("删除文件记录失败".to_string())
            })?;

        tracing::info!("Successfully deleted media file: {} for user: {}", file_id, user_id);

        Ok(())
    }

    pub async fn get_user_files(&self, user_id: &str, page: usize, limit: usize) -> Result<(Vec<MediaFile>, usize)> {
        let offset = (page - 1) * limit;

        // 查询用户的所有文件
        let query = format!(
            "SELECT * FROM media_file WHERE user_id = '{}' ORDER BY created_at DESC LIMIT {} START {}",
            user_id, limit, offset
        );

        let mut response = self.db
            .query(&query)
            .await
            .map_err(|e| {
                tracing::error!("Failed to query user media files: {}", e);
                AppError::Internal("查询用户文件失败".to_string())
            })?;

        let files: Vec<MediaFile> = response.take(0)
            .map_err(|e| {
                tracing::error!("Failed to parse media files: {}", e);
                AppError::Internal("解析文件数据失败".to_string())
            })?;

        // 获取总数
        let count_query = format!("SELECT count() AS total FROM media_file WHERE user_id = '{}'", user_id);
        let mut count_response = self.db
            .query(&count_query)
            .await
            .map_err(|e| {
                tracing::error!("Failed to count user media files: {}", e);
                AppError::Internal("统计文件数量失败".to_string())
            })?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            total: i64,
        }

        let count_result: Option<CountResult> = count_response.take(0)
            .map_err(|e| {
                tracing::error!("Failed to parse count: {}", e);
                AppError::Internal("解析计数失败".to_string())
            })?;

        let total = count_result.map(|r| r.total as usize).unwrap_or(0);

        Ok((files, total))
    }

    fn validate_image_type(&self, content_type: &str) -> Result<()> {
        let allowed_types: Vec<&str> = self.config.allowed_image_types
            .split(',')
            .map(|s| s.trim())
            .collect();

        if !allowed_types.contains(&content_type) {
            return Err(AppError::BadRequest(format!(
                "不支持的图片格式: {}。支持的格式: {}",
                content_type,
                self.config.allowed_image_types
            )));
        }

        Ok(())
    }

    fn get_file_extension(&self, content_type: &str) -> &str {
        match content_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "bin",
        }
    }
}
