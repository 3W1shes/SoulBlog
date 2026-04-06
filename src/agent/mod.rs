//! SoulBlog Agent API v1
//! 
//! 面向 OpenClaw / Agent 的最小能力面接口
//! 提供对出版物和文章的只读访问能力

pub mod auth;
pub mod handlers;
pub mod request_id;
pub mod response;
pub mod router;

pub use router::agent_router;
