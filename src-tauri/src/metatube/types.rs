//! MetaTube sidecar 配置、状态与 API 数据类型
//!
//! API 字段名已实测对齐（与 metatube 建表列一致）。结构体仍全部 `#[serde(default)]`，
//! 缺字段/多字段都不报错，最大化兼容服务端版本差异。

use serde::{Deserialize, Serialize};

/// sidecar 运行配置（由设置派生）
#[derive(Debug, Clone)]
pub struct MetaTubeConfig {
    /// 是否启用（关闭则不拉起进程，源被跳过）
    pub enabled: bool,
    /// 偏好的 provider 列表（搜索时优先；空 = 服务端默认全部）
    pub providers: Vec<String>,
    /// 额外启动参数（覆盖/追加，预留）
    pub extra_args: Vec<String>,
}

impl Default for MetaTubeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            providers: Vec::new(),
            extra_args: Vec::new(),
        }
    }
}

/// sidecar 运行状态（下发前端用于展示与回退判断）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetaTubeStatus {
    /// 未启用
    Disabled,
    /// 启动中（拉起进程 / 健康检查中）
    Starting,
    /// 就绪（健康检查通过，可用）
    Ready,
    /// 失败（达到最大重试仍不可用 → 该源回退跳过）
    Failed,
    /// 已停止（应用退出 / 手动停止）
    Stopped,
}

impl MetaTubeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// 状态快照（命令返回）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaTubeStatusSnapshot {
    pub status: MetaTubeStatus,
    /// 当前监听端口（就绪时有值）
    pub port: Option<u16>,
    /// 二进制是否已就位（缺失则无法启用）
    pub binary_present: bool,
    /// 累计重启次数
    pub restarts: u32,
    /// 最近一次错误信息
    pub last_error: Option<String>,
}

// ==================== API 响应类型 ====================

/// 搜索候选（`GET /v1/movies/search` 返回的精简条目）
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MovieSearchResult {
    pub provider: String,
    pub id: String,
    pub number: String,
    pub title: String,
    #[serde(alias = "thumb_url")]
    pub thumb_url: String,
    #[serde(alias = "cover_url")]
    pub cover_url: String,
    pub score: f64,
}

/// 影片详情（`GET /v1/movies/:provider/:id` 返回的完整 MovieInfo）
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MovieInfo {
    pub provider: String,
    pub id: String,
    pub number: String,
    pub title: String,
    pub summary: String,
    pub director: String,
    pub actors: Vec<String>,
    pub maker: String,
    pub label: String,
    pub series: String,
    pub genres: Vec<String>,
    pub score: f64,
    /// 时长（分钟）
    pub runtime: i64,
    pub release_date: String,
    pub cover_url: String,
    pub big_cover_url: String,
    pub thumb_url: String,
    pub big_thumb_url: String,
    pub preview_images: Vec<String>,
    pub preview_video_url: String,
    pub homepage: String,
}
