//! 临时「视频下载链接」候选站点体检 harness（用完即删，不属于正式构建）。
//!
//! 与 parser_check（元数据刮削体检）互不影响，专门测「能否拿到视频/HLS 链接」。
//!
//! 用法：HARNESS_PROXY=http://127.0.0.1:7897 cargo run --example link_check
//!
//! 流程：对每个候选站点按番号 ssis-666 / ssis-777 / ssis-888（任一命中即有效）请求，
//! 搜索页会二跳到首个含番号的详情页，再扫描页面内的视频链接信号
//! （.m3u8 / .mp4 / <source> / 播放器库 / iframe 内嵌）。
//! 仅输出响应元信息（状态 / 长度 / 命中的信号类别 / 命中番号），不打印页面正文。
//!
//! 局限：纯 HTTP 不执行 JS，若站点的播放地址由前端脚本动态注入（多数在线站如此），
//! HTTP 抓不到，需用应用内的 WebView 资源链接查找；本表 ❌ 不代表 WebView 也拿不到。

use std::time::Duration;

use javm_lib::resource_scrape::fingerprint_client;
use regex::Regex;
use scraper::{Html, Selector};
use wreq_util::Emulation;

const CODES: [&str; 3] = ["SSIS-666", "SSIS-777", "SSIS-888"];

/// 候选站点：(展示名, URL 模板, 是否搜索页[需二跳])
/// 模板中 {C}=大写番号 {c}=小写番号
fn candidates() -> Vec<(&'static str, &'static str, bool)> {
    vec![
        // ---- 已接入锚点（校验 harness/代理是否正常） ----
        ("jable(锚点)", "https://jable.tv/videos/{c}/", false),
        ("missav(锚点)", "https://missav.ws/{c}", false),
        // ---- 在线观看 / 视频链接候选 ----
        ("supjav", "https://supjav.com/zh/?s={C}", true),
        ("javdock", "https://javdock.com/?s={C}", true),
        ("javhaven", "https://javhaven.com/?s={C}", true),
        ("tktube", "https://tktube.com/search/{c}/", true),
        ("javraveclub", "https://javrave.club/?s={C}", true),
        ("javpornhd", "https://javpornhd.net/?s={C}", true),
        ("new-jav", "https://new-jav.net/?s={C}", true),
        ("javhd.today", "https://javhd.today/?s={C}", true),
        ("javhdporn", "https://javhdporn.net/?s={C}", true),
        ("javenglish", "https://javenglish.cc/?s={C}", true),
        ("javtsunami", "https://javtsunami.com/?s={C}", true),
        ("vjav", "https://vjav.com/search/{c}/", true),
        ("hpjav", "https://hpjav.tv/?s={C}", true),
        ("javhub", "https://javhub.net/?s={C}", true),
        ("24av", "https://24av.net/zh/search?keyword={C}", true),
        ("javsub", "https://javsubtitle.com/?s={C}", true),
        ("javhd.icu", "https://javhd.icu/?s={C}", true),
        ("javangel", "https://javangel.com/?s={C}", true),
        ("javpub", "https://javpub.net/?s={C}", true),
        ("ichiav", "https://ichiav.com/?s={C}", true),
        ("pussyav", "https://pussyav.com/?s={C}", true),
        ("javplatform", "https://javplatform.com/?s={C}", true),
        ("javfor.me", "https://javfor.me/?s={C}", true),
        ("avgle", "https://avgle.com/search/{C}", true),
        ("javdoe", "https://javdoe.sh/?s={C}", true),
        ("javwhores", "https://javwhores.com/?s={C}", true),
        ("erito", "https://www.erito.com/search?q={C}", true),
        ("tokyomotion", "https://www.tokyomotion.net/search/videos?search_query={C}", true),
        ("netflav", "https://netflav.com/search?type=title&value={C}", true),
        ("freejavonline", "https://freejavonline.com/?s={C}", true),
        ("fyav", "https://fyav.com/?s={C}", true),
        ("fujiav", "https://fujiav.com/?s={C}", true),
        ("javfree", "https://javfree.sh/?s={C}", true),
        ("javmix", "https://javmix.tv/?s={C}", true),
        ("bteat", "https://bteat.net/?s={C}", true),
        ("javme.xyz", "https://javme.xyz/?s={C}", true),
        ("javhihi", "https://javhihi.com/?s={C}", true),
        ("javbangers", "https://javbangers.com/?s={C}", true),
        ("javfull", "https://javfull.net/?s={C}", true),
        ("javbests", "https://javbests.com/?s={C}", true),
        ("javplayer", "https://javplayer.me/?s={C}", true),
        ("javdaddy", "https://javdaddy.tv/?s={C}", true),
        // ---- 之前归类为「流媒体/视频链接站」的 ❌（重测视频链接） ----
        ("thisav2", "https://thisav2.com/cn/search/videos?search_query={C}", true),
        ("njav", "https://www.njav.com/zh/v/{c}", false),
        ("javct", "https://javct.net/{c}", false),
        ("javeng", "https://javeng.tv/{c}/", false),
        ("javquick", "https://javquick.com/search/{C}", true),
        ("jav.wine", "https://jav.wine/{c}", false),
        ("jav.spa", "https://jav.spa/{c}", false),
    ]
}

/// 扫描 HTML 里的视频链接信号，返回命中的类别列表
fn video_signals(html: &str, re_m3u8: &Regex, re_mp4: &Regex, re_player: &Regex, re_iframe: &Regex) -> Vec<&'static str> {
    let mut hits = Vec::new();
    if re_m3u8.is_match(html) {
        hits.push("m3u8");
    }
    if re_mp4.is_match(html) {
        hits.push("mp4");
    }
    if html.contains("<source") || html.contains("<video") {
        hits.push("source/video");
    }
    if re_player.is_match(html) {
        hits.push("播放器库");
    }
    if re_iframe.is_match(html) {
        hits.push("iframe");
    }
    hits
}

/// 从搜索结果页提取首个「疑似详情页」链接（含番号或 /video(s)/ 路径）
fn extract_detail_link(html: &str, base: &str, code_lc: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("a[href]").ok()?;
    let mut first_video_path: Option<String> = None;
    for a in doc.select(&sel) {
        let href = match a.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        if href.is_empty() || href.starts_with('#') || href.starts_with("javascript") {
            continue;
        }
        let lower = href.to_lowercase();
        // 优先：链接里直接含番号
        if lower.contains(code_lc) {
            return resolve(base, href);
        }
        // 兜底：第一个像观看页的链接
        if first_video_path.is_none()
            && (lower.contains("/video") || lower.contains("/watch") || lower.contains("/v/"))
        {
            first_video_path = resolve(base, href);
        }
    }
    first_video_path
}

fn resolve(base: &str, href: &str) -> Option<String> {
    if href.starts_with("http") {
        return Some(href.to_string());
    }
    url::Url::parse(base).ok()?.join(href).ok().map(|u| u.to_string())
}

#[tokio::main]
async fn main() {
    let proxy = std::env::var("HARNESS_PROXY").ok().filter(|s| !s.is_empty());
    let mut builder = wreq::Client::builder()
        .emulation(Emulation::Chrome137)
        .timeout(Duration::from_secs(12));
    if let Some(p) = &proxy {
        println!("（使用代理: {p}）\n");
        builder = builder.proxy(wreq::Proxy::all(p.as_str()).expect("代理地址无效"));
    } else {
        println!("（未设置代理，HARNESS_PROXY 为空——多数站点可能不可达）\n");
    }
    let client = builder.build().expect("创建 wreq 客户端失败");

    let re_m3u8 = Regex::new(r"\.m3u8").unwrap();
    let re_mp4 = Regex::new(r"\.mp4").unwrap();
    let re_player = Regex::new(r"hls\.js|video\.?js|jwplayer|clappr|dplayer|plyr|playerjs").unwrap();
    let re_iframe = Regex::new(r"<iframe[^>]+src").unwrap();

    println!("番号: {}\n", CODES.join(" / "));
    println!("{:<14} {:<6} {:<6} {:<8} {:<8} {}", "站点", "判定", "状态", "长度", "命中码", "视频信号");
    println!("{}", "-".repeat(78));

    for (name, tmpl, is_search) in candidates() {
        let mut best_status = String::from("-");
        let mut best_len = 0usize;
        let mut hit_code: Option<&str> = None;
        let mut signals: Vec<&'static str> = Vec::new();

        'outer: for code in CODES {
            let lc = code.to_lowercase();
            let url = tmpl.replace("{C}", code).replace("{c}", &lc);
            let html = match fingerprint_client::fetch_html(&client, &url).await {
                Ok(h) => {
                    best_status = "200".into();
                    if h.len() > best_len {
                        best_len = h.len();
                    }
                    h
                }
                Err(e) => {
                    if best_status != "200" {
                        best_status = e.chars().take(14).collect();
                    }
                    // 传输失败（DNS/拒绝/超时）或被硬拦（403/503）时，换番号也没用，直接跳过该站
                    if e.contains("请求失败") || e.contains("403") || e.contains("503") {
                        break 'outer;
                    }
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    continue;
                }
            };

            // 详情页：直接扫；搜索页：先扫一遍，没信号再二跳
            let mut sig = video_signals(&html, &re_m3u8, &re_mp4, &re_player, &re_iframe);
            if sig.is_empty() && is_search {
                if let Some(detail) = extract_detail_link(&html, &url, &lc) {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    if let Ok(dh) = fingerprint_client::fetch_html(&client, &detail).await {
                        if dh.len() > best_len {
                            best_len = dh.len();
                        }
                        sig = video_signals(&dh, &re_m3u8, &re_mp4, &re_player, &re_iframe);
                    }
                }
            }

            if !sig.is_empty() {
                hit_code = Some(code);
                signals = sig;
                break 'outer;
            }
            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        let verdict = if hit_code.is_some() { "☑️" } else { "❌" };
        println!(
            "{:<14} {:<6} {:<6} {:<8} {:<8} {}",
            name,
            verdict,
            best_status,
            best_len,
            hit_code.unwrap_or("-"),
            signals.join(",")
        );
    }

    println!("\n判定：☑️ = HTTP 抓到的页面里含视频链接信号（m3u8/mp4/source/播放器/iframe），任一番号命中即可。");
    println!("❌ 可能是不可达、被拦、或播放地址由 JS 动态注入（需应用内 WebView 查找，HTTP 测不出）。");
}
