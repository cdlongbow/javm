//! 视频链接探测框架（仅 debug 构建编译）。
//!
//! 目的：让 AI / 开发者无需手动点 UI，就能用应用真实的「隐藏 WebView」流程批量测试
//! 候选视频站能否抓到视频链接，从而把站点筛选工作自动化。
//!
//! 为什么放进主程序而不是 example：隐藏 WebView 需要 Tauri + WebView2 运行时，而
//! `tauri-build` 只把 Windows 清单/WebView2 资源嵌进主二进制 `javm.exe`，example 跑不起来
//! （STATUS_ENTRYPOINT_NOT_FOUND）。这里复用与正式「资源链接查找」完全相同的注入脚本
//! `video_finder::INTERCEPT_JS` 与反检测脚本，保证测试结果与线上行为一致。
//!
//! 用法（dev，无需启动前端）：
//! ```
//! JAVM_LINK_PROBE=link_probe_targets.json cargo run
//! ```
//! 可选 `JAVM_LINK_PROBE_OUT=自定义结果路径`（默认 link_probe_results.json）。
//! 跑完会把结果写入 JSON 并自动退出。配置文件格式见 link_probe_targets.sample.json。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};

use super::video_finder::{INTERCEPT_JS, VIDEO_FINDER_LABEL};
use super::webview_support;

fn default_code() -> String {
    "SSIS-666".to_string()
}
fn default_secs() -> u64 {
    18
}

#[derive(Deserialize)]
struct ProbeConfig {
    #[serde(default = "default_code")]
    code: String,
    #[serde(default = "default_secs")]
    per_site_secs: u64,
    targets: Vec<Target>,
}

#[derive(Deserialize, Clone)]
struct Target {
    name: String,
    /// 可含 {C}=大写番号 {c}=小写番号
    url: String,
    /// 搜索页置 true：加载后自动跳转到首个含番号的结果详情页再抓取
    #[serde(default)]
    follow: bool,
}

#[derive(Serialize)]
struct ProbeResult {
    name: String,
    url: String,
    ok: bool,
    total: usize,
    m3u8: usize,
    mp4: usize,
    other: usize,
    /// 抓到的若干样例链接（最多 8 条），供人工核对
    samples: Vec<String>,
    /// 抓取结束时 WebView 的实际 URL（用于诊断：是否成功跳转到详情页）
    final_url: String,
    note: String,
}

impl ProbeResult {
    /// 构造失败结果（建窗/解析等阶段出错）
    fn fail(name: &str, url: &str, note: String) -> Self {
        ProbeResult {
            name: name.to_string(),
            url: url.to_string(),
            ok: false,
            total: 0,
            m3u8: 0,
            mp4: 0,
            other: 0,
            samples: vec![],
            final_url: url.to_string(),
            note,
        }
    }
}

/// 入口：从 JAVM_LINK_PROBE 指向的配置文件读取目标，逐个用隐藏 WebView 探测后写结果并退出。
pub async fn run(app: AppHandle) {
    let cfg_path = match std::env::var("JAVM_LINK_PROBE") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => {
            log::error!("[link_probe] 未设置 JAVM_LINK_PROBE 配置文件路径");
            app.exit(1);
            return;
        }
    };
    let out_path =
        std::env::var("JAVM_LINK_PROBE_OUT").unwrap_or_else(|_| "link_probe_results.json".into());

    let cfg: ProbeConfig = match std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("读取配置失败: {e}"))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("解析配置失败: {e}")))
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("[link_probe] {e} (path={cfg_path})");
            app.exit(1);
            return;
        }
    };

    println!("\n[link_probe] 番号={} 每站={}s 目标数={}", cfg.code, cfg.per_site_secs, cfg.targets.len());

    // 全局监听捕获事件，按站点清空
    let captured: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(BTreeSet::new()));
    let cap_for_listener = captured.clone();
    app.listen("video-finder-link", move |event| {
        if let Ok(url) = serde_json::from_str::<String>(event.payload()) {
            if let Ok(mut set) = cap_for_listener.lock() {
                set.insert(url);
            }
        }
    });

    let anti = webview_support::build_anti_detection_script();
    let lc = cfg.code.to_lowercase();
    let mut results: Vec<ProbeResult> = Vec::new();

    println!("{:<16} {:<5} {:<6} {:<5} {:<5} {}", "站点", "判定", "总数", "m3u8", "mp4", "URL");
    println!("{}", "-".repeat(72));

    for target in &cfg.targets {
        let url = target.url.replace("{C}", &cfg.code).replace("{c}", &lc);
        if let Ok(mut set) = captured.lock() {
            set.clear();
        }

        let result = probe_one(&app, target, &url, &lc, &anti, cfg.per_site_secs, &captured).await;
        println!(
            "{:<16} {:<5} {:<6} {:<5} {:<5} {}",
            target.name,
            if result.ok && result.total > 0 { "☑️" } else { "❌" },
            result.total,
            result.m3u8,
            result.mp4,
            url
        );
        results.push(result);
    }

    // 写结果文件（含样例链接，供人工核对；不在终端打印具体链接）
    match serde_json::to_string_pretty(&results) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&out_path, json) {
                log::error!("[link_probe] 写结果失败: {e}");
            } else {
                println!("\n[link_probe] 结果已写入 {out_path}（含样例链接）");
            }
        }
        Err(e) => log::error!("[link_probe] 序列化结果失败: {e}"),
    }

    let ok = results.iter().filter(|r| r.ok && r.total > 0).count();
    println!("[link_probe] 完成：{ok}/{} 个站点抓到视频链接", results.len());
    app.exit(0);
}

async fn probe_one(
    app: &AppHandle,
    target: &Target,
    url: &str,
    code_lc: &str,
    anti: &str,
    per_site_secs: u64,
    captured: &Arc<Mutex<BTreeSet<String>>>,
) -> ProbeResult {
    let mut note = String::new();
    let parsed: url::Url = match url.parse() {
        Ok(u) => u,
        Err(e) => return ProbeResult::fail(&target.name, url, format!("URL 无效: {e}")),
    };

    // 关闭可能残留的窗口
    if let Some(existing) = app.get_webview_window(VIDEO_FINDER_LABEL) {
        let _ = existing.close();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let data_directory = match webview_support::persistent_data_directory(app) {
        Ok(d) => d,
        Err(e) => return ProbeResult::fail(&target.name, url, format!("数据目录失败: {e}")),
    };

    let builder = WebviewWindowBuilder::new(app, VIDEO_FINDER_LABEL, WebviewUrl::External(parsed))
        .title("link probe")
        .inner_size(1920.0, 1080.0)
        .visible(false)
        .user_agent(webview_support::WEBVIEW_USER_AGENT)
        .initialization_script(anti)
        .data_directory(data_directory);

    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args(webview_support::WEBVIEW_BROWSER_ARGS);

    let window = match builder.build() {
        Ok(w) => w,
        Err(e) => return ProbeResult::fail(&target.name, url, format!("建窗失败: {e}")),
    };

    // 注入循环：每 400ms eval 一次拦截脚本；follow 站点在预热后每轮尝试跳转首个结果
    // （JS 渲染的搜索结果可能晚出现，故反复尝试；follow JS 自带"仅在搜索页执行一次"守卫）
    const FOLLOW_WARMUP_CYCLES: u64 = 5; // 约 2s 预热，等首屏渲染
    let cycles = (per_site_secs * 1000 / 400).max(1);
    let follow_js = if target.follow {
        note = "搜索页：自动跳转首个结果详情后抓取".into();
        build_follow_js(code_lc)
    } else {
        String::new()
    };
    for i in 0..cycles {
        let _ = window.eval(INTERCEPT_JS);
        // 触发播放：很多详情页要点击/调用 play() 才会发起 m3u8 请求，主动 kickstart
        let _ = window.eval(PLAYBACK_KICK_JS);
        if target.follow && i >= FOLLOW_WARMUP_CYCLES {
            let _ = window.eval(&follow_js);
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    let final_url = window.url().map(|u| u.to_string()).unwrap_or_default();

    let (total, m3u8, mp4, other, samples) = {
        let set = captured.lock().unwrap();
        let m: Vec<&String> = set.iter().filter(|u| u.contains(".m3u8")).collect();
        let p: Vec<&String> = set.iter().filter(|u| u.contains(".mp4")).collect();
        let m3u8 = m.len();
        let mp4 = p.len();
        let other = set.len() - m3u8 - mp4;
        let samples: Vec<String> = set.iter().take(8).cloned().collect();
        (set.len(), m3u8, mp4, other, samples)
    };

    let _ = window.close();
    tokio::time::sleep(Duration::from_millis(400)).await;

    ProbeResult {
        name: target.name.clone(),
        url: url.to_string(),
        ok: true,
        total,
        m3u8,
        mp4,
        other,
        samples,
        final_url,
        note,
    }
}

/// 尝试触发页面内（同源）播放器开始播放，促使其发起 m3u8/mp4 请求。
/// 无法触及跨域 iframe 内的播放器（顶层注入的固有限制）。
const PLAYBACK_KICK_JS: &str = r#"(function(){try{
  document.querySelectorAll('video').forEach(function(v){try{v.muted=true;var p=v.play&&v.play();if(p&&p.catch)p.catch(function(){});}catch(e){}});
  var sels=['.vjs-big-play-button','.jw-icon-display','.plyr__control--overlaid','.dplayer-play-icon','[class*="play-button"]','[class*="playButton"]','.play','#play','button[aria-label*="lay"]'];
  for(var i=0;i<sels.length;i++){var e=document.querySelector(sels[i]);if(e&&e.click){try{e.click();}catch(_){}}}
}catch(e){}})();"#;

/// 在搜索结果页跳转到首个站内详情链接。
/// 守卫：仅当当前仍是搜索页（URL 含 s=/q=/keyword=/search_query=/value= 或 /search/）时执行，
/// 跳转到详情页后 URL 不再匹配 → 不会从详情页再次跳走。
/// 匹配优先级：路径含 /videos|/watch|/movie|/play 或 href 含番号 > 数字 id 详情（/12345 或 12345.html）。
/// 排除分类/标签/分页/登录等非详情链接。
fn build_follow_js(code_lc: &str) -> String {
    format!(
        r#"(function(){{try{{
  if(!/[?&](s|q|keyword|value|search_query)=|\/search\//i.test(location.href)) return;
  var c="{code}";
  var as=document.querySelectorAll('a[href]');
  var fallback=null;
  for(var i=0;i<as.length;i++){{
    var el=as[i]; var abs=el.href||''; var h=(el.getAttribute('href')||'').toLowerCase();
    if(!abs||h.indexOf('#')===0||h.indexOf('javascript')===0) continue;
    if(abs.indexOf(location.host)<0) continue;
    if(/\/(category|categories|tag|tags|page|author|search|login|register|sign|genre|actress|maker|studio|2257|dmca|lander|privacy|terms|abuse|contact|latest|trending|popular|random|static)\b/i.test(h)) continue;
    if(/\/(videos?|watch|movie|play)\//i.test(h) || h.indexOf(c)>=0){{ window.location.href=abs; return; }}
    if(!fallback && /\/\d{{5,}}(\.html)?\/?$/.test(h)) fallback=abs;
  }}
  if(fallback){{ window.location.href=fallback; }}
}}catch(e){{}}}})();"#,
        code = code_lc
    )
}
