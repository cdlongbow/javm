use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use super::proxy;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub configured: bool,
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percentage: Option<f64>,
}

const UPDATER_NOT_CONFIGURED: &str = "UPDATER_NOT_CONFIGURED";

fn is_updater_not_configured_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("pubkey")
        || lower.contains("endpoint")
        || lower.contains("endpoints")
        || lower.contains("updater") && lower.contains("config")
}

const GITHUB_REPO: &str = "ddmoyu/javm";

/// 判断某个 release tag 是否属于指定更新通道。
/// 正式版（无预发布后缀）对所有通道可见；rc 通道额外接收 `-rc`；
/// beta 通道额外接收 `-beta` 和 `-rc`（即越靠前的通道收到越多，alpha 不在任何通道内）。
fn tag_matches_channel(tag: &str, channel: &str) -> bool {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    if !version.contains('-') {
        return true; // 正式版
    }
    let lower = version.to_ascii_lowercase();
    match channel {
        "rc" => lower.contains("-rc"),
        "beta" => lower.contains("-beta") || lower.contains("-rc"),
        _ => false,
    }
}

/// 预发布通道：调用 GitHub Releases API，按创建时间倒序找到首个符合通道的版本，
/// 返回其 `latest.json` 资产地址。失败或无匹配时返回 None（回退到默认正式版端点）。
/// 客户端走代理，保证仅代理可达 GitHub 的网络下预发布通道也能解析。
async fn resolve_prerelease_endpoint(channel: &str) -> Option<String> {
    let client = proxy::apply_proxy_auto(wreq::Client::builder())
        .ok()?
        .build()
        .ok()?;
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page=30");
    let resp = client
        .get(&url)
        .header("User-Agent", "javm-updater")
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    let releases: serde_json::Value = resp.json().await.ok()?;
    for release in releases.as_array()? {
        if release.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
            continue;
        };
        if tag_matches_channel(tag, channel) {
            return Some(format!(
                "https://github.com/{GITHUB_REPO}/releases/download/{tag}/latest.json"
            ));
        }
    }
    None
}

async fn build_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app
        .updater_builder()
        .header("User-Agent", "tauri-updater")
        .map_err(|e| format!("设置请求头失败: {e}"))?
        .timeout(Duration::from_secs(60));

    // 读取更新通道：rc/beta 时改用对应预发布版本的端点，stable 保持默认（仅正式版）
    let channel = crate::settings::get_settings(app.clone())
        .await
        .map(|settings| settings.update.channel)
        .unwrap_or_else(|_| "stable".to_string());
    if channel == "rc" || channel == "beta" {
        if let Some(endpoint) = resolve_prerelease_endpoint(&channel).await {
            if let Ok(parsed) = url::Url::parse(&endpoint) {
                builder = builder
                    .endpoints(vec![parsed])
                    .map_err(|e| format!("设置更新端点失败: {e}"))?;
            }
        }
    }

    if let Some(config_dir) = app.path().app_config_dir().ok() {
        if let Some(proxy_url) = proxy::resolve_proxy_url(&config_dir) {
            builder = builder.proxy(proxy_url);
        }
    }

    builder
        .build()
        .map_err(|e| format!("初始化更新器失败: {e}"))
}

/// 从 GitHub Release API 获取指定版本的发布说明（兜底方案）
async fn fetch_release_body_from_github(version: &str) -> Option<String> {
    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    let url = format!(
        "https://api.github.com/repos/ddmoyu/javm/releases/tags/{tag}"
    );
    let resp = wreq::Client::new()
        .get(&url)
        .header("User-Agent", "javm-updater")
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("body")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|s| !s.trim().is_empty())
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<AppUpdateInfo, String> {
    let current_version = app.package_info().version.to_string();

    let updater = match build_updater(&app).await {
        Ok(updater) => updater,
        Err(error) if is_updater_not_configured_error(&error) => {
            return Err(UPDATER_NOT_CONFIGURED.to_string());
        }
        Err(error) => return Err(error),
    };
    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败: {e:?}"))?;

    if let Some(update) = update {
        // 优先使用 update.body，若为空则从 raw_json 的 notes 字段兜底
        let mut body = update
            .body
            .filter(|b| !b.trim().is_empty())
            .or_else(|| {
                update
                    .raw_json
                    .get("notes")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .filter(|s| !s.trim().is_empty())
            });

        // 终极兜底：如果 latest.json 的 notes 也为空，从 GitHub Release API 获取
        if body.is_none() {
            body = fetch_release_body_from_github(&update.version).await;
        }

        Ok(AppUpdateInfo {
            configured: true,
            available: true,
            current_version: update.current_version,
            version: Some(update.version),
            body,
            date: update.date.map(|date| date.to_string()),
            target: Some(update.target),
        })
    } else {
        Ok(AppUpdateInfo {
            configured: true,
            available: false,
            current_version,
            version: None,
            body: None,
            date: None,
            target: None,
        })
    }
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<String, String> {
    let updater = build_updater(&app).await?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败: {e}"))?
        .ok_or_else(|| "当前没有可用更新".to_string())?;

    let downloaded_bytes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let progress_app = app.clone();
    let finish_app = app.clone();
    let download_counter = downloaded_bytes.clone();
    let finish_counter = downloaded_bytes.clone();

    update
        .download_and_install(
            move |chunk_length, content_length| {
                let downloaded = download_counter
                    .fetch_add(chunk_length as u64, std::sync::atomic::Ordering::Relaxed)
                    + chunk_length as u64;
                let total = content_length.filter(|value| *value > 0);
                let percentage = total.map(|value| (downloaded as f64 / value as f64) * 100.0);

                let _ = progress_app.emit(
                    "app-update-download-progress",
                    AppUpdateProgress {
                        phase: "downloading".to_string(),
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                        percentage,
                    },
                );
            },
            move || {
                let downloaded = finish_counter.load(std::sync::atomic::Ordering::Relaxed);
                let _ = finish_app.emit(
                    "app-update-download-progress",
                    AppUpdateProgress {
                        phase: "installing".to_string(),
                        downloaded_bytes: downloaded,
                        total_bytes: Some(downloaded).filter(|value| *value > 0),
                        percentage: Some(100.0),
                    },
                );
            },
        )
        .await
        .map_err(|e| format!("安装更新失败: {e}"))?;

    #[cfg(target_os = "windows")]
    {
        Ok("更新安装程序已启动，应用会自动退出完成安装。".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok("更新已安装，请重启应用以完成切换。".to_string())
    }
}
