//! MetaTube HTTP 客户端（访问本地 sidecar）
//!
//! 端点依据计划 §4.3（源自 metatube `route/route.go`，已确认）：
//! - `GET /v1/providers`（public）—— 兼作健康检查
//! - `GET /v1/movies/search?q=&provider=&fallback=`（private，需 token）
//! - `GET /v1/movies/:provider/:id`（private）
//!
//! 鉴权已实测：私有端点需 `Authorization: Bearer <token>`（query token 被拒）；
//! 响应统一包一层 `{"data": ...}`（见 [`Envelope`]）。

use serde::Deserialize;

use crate::resource_scrape::types::SearchResult;

use super::types::{ActorInfo, ActorSearchResult, MovieInfo, MovieSearchResult};

/// MetaTube 所有响应统一包一层 `{"data": ...}`（已实测确认）。
#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

/// 本地 sidecar HTTP 客户端
#[derive(Clone)]
pub struct MetaTubeClient {
    base_url: String,
    token: String,
    http: wreq::Client,
}

impl MetaTubeClient {
    pub fn new(base_url: String, token: String) -> Result<Self, String> {
        let http = wreq::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| format!("创建 MetaTube 客户端失败: {}", e))?;
        Ok(Self {
            base_url,
            token,
            http,
        })
    }

    /// 健康检查：列 provider（public 端点）。返回 true 表示 sidecar 就绪。
    pub async fn health(&self) -> bool {
        let url = format!("{}/v1/providers", self.base_url);
        matches!(self.http.get(&url).send().await, Ok(resp) if resp.status().is_success())
    }

    /// 影片聚合搜索。`providers` 为空则服务端默认查全部。
    /// 鉴权用 `Authorization: Bearer`（已实测：query token 不被接受）。
    pub async fn search(
        &self,
        code: &str,
        providers: &[String],
    ) -> Result<Vec<MovieSearchResult>, String> {
        let mut url = format!(
            "{}/v1/movies/search?q={}&fallback=true",
            self.base_url,
            urlencoding(code),
        );
        if !providers.is_empty() {
            url.push_str("&provider=");
            url.push_str(&urlencoding(&providers.join(",")));
        }

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("MetaTube 搜索请求失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("MetaTube 搜索 HTTP {}", resp.status()));
        }
        resp.json::<Envelope<Vec<MovieSearchResult>>>()
            .await
            .map(|env| env.data)
            .map_err(|e| format!("MetaTube 搜索响应解析失败: {}", e))
    }

    /// 演员聚合搜索（`GET /v1/actors/search`）。`providers` 为空则服务端默认查全部演员源。
    pub async fn search_actor(
        &self,
        name: &str,
        providers: &[String],
    ) -> Result<Vec<ActorSearchResult>, String> {
        let mut url = format!(
            "{}/v1/actors/search?q={}&fallback=true",
            self.base_url,
            urlencoding(name),
        );
        if !providers.is_empty() {
            url.push_str("&provider=");
            url.push_str(&urlencoding(&providers.join(",")));
        }
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("MetaTube 演员搜索请求失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("MetaTube 演员搜索 HTTP {}", resp.status()));
        }
        resp.json::<Envelope<Vec<ActorSearchResult>>>()
            .await
            .map(|env| env.data)
            .map_err(|e| format!("MetaTube 演员搜索响应解析失败: {}", e))
    }

    /// 取演员完整档案（`GET /v1/actors/:provider/:id`）。
    pub async fn get_actor(&self, provider: &str, id: &str) -> Result<ActorInfo, String> {
        let url = format!(
            "{}/v1/actors/{}/{}",
            self.base_url,
            urlencoding(provider),
            urlencoding(id),
        );
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("MetaTube 演员详情请求失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("MetaTube 演员详情 HTTP {}", resp.status()));
        }
        resp.json::<Envelope<ActorInfo>>()
            .await
            .map(|env| env.data)
            .map_err(|e| format!("MetaTube 演员详情响应解析失败: {}", e))
    }

    /// 取影片完整详情。
    pub async fn get_movie(&self, provider: &str, id: &str) -> Result<MovieInfo, String> {
        let url = format!(
            "{}/v1/movies/{}/{}",
            self.base_url,
            urlencoding(provider),
            urlencoding(id),
        );
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| format!("MetaTube 详情请求失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("MetaTube 详情 HTTP {}", resp.status()));
        }
        resp.json::<Envelope<MovieInfo>>()
            .await
            .map(|env| env.data)
            .map_err(|e| format!("MetaTube 详情响应解析失败: {}", e))
    }
}

/// 极简 URL 编码（番号/provider/id 一般为 ASCII，仅转义少量保留字符）。
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// MovieInfo → 项目统一的 SearchResult（字段映射见计划 §4.4）。
pub fn movie_info_to_search_result(info: MovieInfo) -> SearchResult {
    let duration = if info.runtime > 0 {
        format!("{}分钟", info.runtime)
    } else {
        String::new()
    };
    let cover_url = if !info.big_cover_url.is_empty() {
        info.big_cover_url.clone()
    } else {
        info.cover_url.clone()
    };
    let poster_url = if !info.big_thumb_url.is_empty() {
        info.big_thumb_url.clone()
    } else {
        info.thumb_url.clone()
    };
    let genres = info.genres.join(", ");
    let rating = if info.score > 0.0 {
        Some(info.score)
    } else {
        None
    };

    SearchResult {
        code: info.number.clone(),
        title: info.title.clone(),
        actors: info.actors.join(", "),
        duration,
        studio: info.maker.clone(),
        source: super::SOURCE_ID.to_string(),
        page_url: info.homepage.clone(),
        cover_url,
        poster_url,
        director: info.director.clone(),
        tags: genres.clone(),
        premiered: info.release_date.clone(),
        rating,
        thumbs: info.preview_images.clone(),
        plot: info.summary.clone(),
        outline: info.summary.clone(),
        maker: info.maker.clone(),
        label: info.label.clone(),
        set_name: info.series.clone(),
        genres,
        mpaa: "JP-18+".to_string(),
        custom_rating: "JP-18+".to_string(),
        country_code: "JP".to_string(),
        ..Default::default()
    }
}
