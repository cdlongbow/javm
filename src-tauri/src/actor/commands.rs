//! 演员中心命令：抓取档案 + 作品全集（star 页），演员详情查询。

use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::db::{ActorWorkInput, Database};
use crate::error::{AppError, AppResult};
use crate::resource_scrape::actor_provider;
use crate::resource_scrape::fetcher::{FetchOptions, Fetcher};
use crate::resource_scrape::sources::ResourceSite;
use crate::settings;
use crate::utils::designation_recognizer;

/// 分页抓取上限，防止异常分页导致空转
const MAX_STAR_PAGES: u32 = 50;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorFetchResult {
    pub profile_updated: bool,
    pub works_total: usize,
    pub works_local: i64,
}

fn opt(s: &str) -> Option<&str> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// 从 "B88/W58/H85" 之类的三围原文提取 bust/waist/hip。识别不出则 None。
fn parse_measurements(s: &str) -> (Option<i32>, Option<i32>, Option<i32>) {
    let upper = s.to_uppercase();
    let grab = |prefix: char| -> Option<i32> {
        let pos = upper.find(prefix)?;
        let digits: String = upper[pos + prefix.len_utf8()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().ok()
    };
    (grab('B'), grab('W'), grab('H'))
}

/// MetaTube ActorInfo → 演员档案入参（头像取首图，三围从 measurements 解析）。
fn actor_info_to_profile(info: &crate::metatube::types::ActorInfo) -> crate::db::ActorProfileInput {
    let (bust, waist, hip) = parse_measurements(&info.measurements);
    crate::db::ActorProfileInput {
        avatar_url: info.images.iter().find(|s| !s.trim().is_empty()).cloned(),
        birthday: non_empty(&info.birthday),
        height: (info.height > 0).then_some(info.height as i32),
        cup: non_empty(&info.cup_size),
        bust,
        waist,
        hip,
    }
}

/// 抓取演员档案 + 作品全集：解析 star 页（分页爬全）→ 落库 + 本地番号匹配。
///
/// star code 优先用库里已收割的（刮削时从详情页 `.avatar-box` 取）；没有则按演员名走
/// `searchstar` 搜索解析得到并回填。所有抓取经 `Fetcher`（HTTP→WebView 回退），
/// 过 javbus 的年龄门 / Cloudflare（与详情刮削同路径）。
#[tauri::command]
pub async fn fetch_actor_profile(
    app: AppHandle,
    actor_id: i64,
    db: State<'_, Database>,
) -> AppResult<ActorFetchResult> {
    // javbus 站点 + 抓取选项（含 WebView 回退），与详情刮削一致，用于过年龄门 / CF
    let app_settings = settings::get_settings(app.clone()).await.unwrap_or_default();
    let fetch_settings = settings::resolve_scrape_fetch_settings(&app_settings.scrape);
    let options = FetchOptions {
        webview_enabled: fetch_settings.webview_enabled,
        webview_fallback_enabled: fetch_settings.webview_fallback_enabled,
        show_webview: fetch_settings.dev_show_webview,
        max_webview_windows: fetch_settings.max_webview_windows,
    };
    let site = ResourceSite {
        id: "javbus".to_string(),
        name: "javbus".to_string(),
        enabled: true,
        avg_score: None,
        scrape_count: None,
    };
    let token = CancellationToken::new();
    let fetcher = Fetcher::new();

    // 1. star code：库里有就用；没有则按演员名 searchstar 搜索并回填
    let (name, mut star_code) = {
        let conn = db.get_connection()?;
        let name: String = conn
            .query_row(
                "SELECT name FROM actors WHERE id = ?",
                rusqlite::params![actor_id],
                |r| r.get(0),
            )
            .map_err(|e| AppError::Business(format!("演员不存在: {e}")))?;
        let code = Database::get_actor_star_code(&conn, actor_id)?;
        (name, code)
    };

    // 1.5 MetaTube 档案（就绪时优先）：结构化 JSON 拿头像/身高/三围/生日，比抓 star 页可靠、免年龄门；
    //      并尽量取 JavBus provider 的演员 id 当 star code（用于后续全集抓取）。
    let mut mt_profile: Option<crate::db::ActorProfileInput> = None;
    if let Some(client) = app
        .try_state::<crate::metatube::MetaTubeManager>()
        .and_then(|m| m.client())
    {
        if let Ok(cands) = client.search_actor(&name, &[]).await {
            let want = name.trim();
            let pick = cands
                .iter()
                .find(|c| c.name.trim() == want && c.provider.eq_ignore_ascii_case("javbus"))
                .or_else(|| cands.iter().find(|c| c.name.trim() == want))
                .or_else(|| cands.first());
            if let Some(c) = pick {
                if let Ok(info) = client.get_actor(&c.provider, &c.id).await {
                    mt_profile = Some(actor_info_to_profile(&info));
                }
                if star_code.is_none()
                    && c.provider.eq_ignore_ascii_case("javbus")
                    && !c.id.trim().is_empty()
                {
                    let conn = db.get_connection()?;
                    let avatar = c.images.first().map(|s| s.as_str()).unwrap_or("");
                    let _ = Database::update_actor_avatar(&conn, &name, avatar, &c.id);
                    star_code = Some(c.id.clone());
                }
            }
        }
    }

    // star code 仍无 → JavBus searchstar 兜底（经 fetcher 过年龄门）
    if star_code.is_none() {
        let search_url = actor_provider::build_search_url(&name);
        if let Ok(html) = fetcher.fetch(&app, &search_url, &site, options, &token).await {
            if let Some(hit) = actor_provider::pick_star_from_search(&html, &name) {
                let conn = db.get_connection()?;
                let _ = Database::update_actor_avatar(&conn, &name, &hit.avatar_url, &hit.star_code);
                star_code = Some(hit.star_code);
            }
        }
    }

    // 既无 star code 又无 MetaTube 档案 → 确实搜不到
    if star_code.is_none() && mt_profile.is_none() {
        return Err(AppError::Business(format!(
            "未在数据源搜到演员「{name}」，无法获取档案/全集"
        )));
    }

    // 2. 全集：有 star code 才分页抓 star 页（MetaTube 不提供作品全集）。经 fetcher 过年龄门。
    let mut profile: Option<crate::db::ActorProfileInput> = None;
    let mut works: Vec<actor_provider::StarWork> = Vec::new();
    if let Some(code) = &star_code {
        let mut page = 1u32;
        loop {
            let url = actor_provider::build_star_url(code, page);
            let html = fetcher
                .fetch(&app, &url, &site, options, &token)
                .await
                .map_err(AppError::Business)?;

            if page == 1 {
                profile = Some(actor_provider::parse_profile(&html));
            }
            let page_works = actor_provider::parse_works(&html);
            let has_next = actor_provider::parse_has_next_page(&html);
            if page_works.is_empty() {
                break;
            }
            works.extend(page_works);
            if !has_next || page >= MAX_STAR_PAGES {
                break;
            }
            page += 1;
        }
    }

    // 3. 落库（单事务：档案 + 作品 upsert + 本地匹配 + 作品数）
    let profile_updated = profile.is_some() || mt_profile.is_some();
    let works_total = works.len();
    let db_inner = db.inner().clone();
    let works_local = tokio::task::spawn_blocking(move || -> AppResult<i64> {
        let mut conn = db_inner.get_connection()?;
        let tx = conn.transaction()?;

        // 先写 star 页解析档案，再写 MetaTube 档案（COALESCE：MetaTube 覆盖冲突项、补空缺）
        if let Some(p) = &profile {
            Database::update_actor_profile(&tx, actor_id, p)?;
        }
        if let Some(p) = &mt_profile {
            Database::update_actor_profile(&tx, actor_id, p)?;
        }
        for w in &works {
            let is_unc = designation_recognizer::is_uncensored_designation(&w.code);
            Database::upsert_actor_work(
                &tx,
                &ActorWorkInput {
                    actor_id,
                    code: &w.code,
                    title: opt(&w.title),
                    cover_url: opt(&w.cover_url),
                    release_date: opt(&w.release_date),
                    source: Some("javbus"),
                    is_uncensored: is_unc,
                },
            )?;
        }
        Database::relink_actor_works_local(&tx, actor_id)?;
        Database::set_actor_work_count(&tx, actor_id, works_total as i64)?;

        let local: i64 = tx.query_row(
            "SELECT COUNT(*) FROM actor_works WHERE actor_id = ?1 AND status = 'local'",
            rusqlite::params![actor_id],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(local)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))??;

    Ok(ActorFetchResult {
        profile_updated,
        works_total,
        works_local,
    })
}

/// 演员详情：档案 + 作品全集（本地有/缺失），供演员详情页渲染。
#[tauri::command]
pub async fn get_actor_detail(
    actor_id: i64,
    db: State<'_, Database>,
) -> AppResult<serde_json::Value> {
    let conn = db.get_connection()?;
    tokio::task::spawn_blocking(move || -> AppResult<serde_json::Value> {
        let profile = conn.query_row(
            "SELECT id, name, avatar_path, avatar_url, birthday, height, cup, bust, waist, hip, work_count
             FROM actors WHERE id = ?",
            rusqlite::params![actor_id],
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "avatarPath": r.get::<_, Option<String>>(2)?,
                    "avatarUrl": r.get::<_, Option<String>>(3)?,
                    "birthday": r.get::<_, Option<String>>(4)?,
                    "height": r.get::<_, Option<i64>>(5)?,
                    "cup": r.get::<_, Option<String>>(6)?,
                    "bust": r.get::<_, Option<i64>>(7)?,
                    "waist": r.get::<_, Option<i64>>(8)?,
                    "hip": r.get::<_, Option<i64>>(9)?,
                    "workCount": r.get::<_, Option<i64>>(10)?,
                }))
            },
        )?;

        let mut stmt = conn.prepare(
            "SELECT code, title, cover_url, release_date, status, local_video_id, is_uncensored
             FROM actor_works WHERE actor_id = ? ORDER BY release_date DESC",
        )?;
        let works: Vec<serde_json::Value> = stmt
            .query_map(rusqlite::params![actor_id], |r| {
                Ok(serde_json::json!({
                    "code": r.get::<_, String>(0)?,
                    "title": r.get::<_, Option<String>>(1)?,
                    "coverUrl": r.get::<_, Option<String>>(2)?,
                    "releaseDate": r.get::<_, Option<String>>(3)?,
                    "status": r.get::<_, String>(4)?,
                    "localVideoId": r.get::<_, Option<String>>(5)?,
                    "isUncensored": r.get::<_, Option<i64>>(6)?.unwrap_or(0) != 0,
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(serde_json::json!({ "profile": profile, "works": works }))
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}
