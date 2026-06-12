//! javmost.ws 数据源解析器
//!
//! 详情页：https://www.javmost.ws/{CODE}/（番号即路径，无需搜索跳转）
//! 详情字段集中在 `div.card-block` 内，按 href 分类法提取，避免抓到菜单/相关视频：
//! - 演员：a[href*="/star/"]
//! - 片商：a[href*="/maker/"]
//! - 类别：a[href*="/category/"] + a[href*="/tag/"]
//! - 日期：card-block 文本中的 YYYY-MM-DD
//! - 标题/封面：head 的 og:title / og:image

use super::common::{dedup_strings, extract_head_meta, select_all_text, select_text};
use super::{SearchResult, Source};
use regex::Regex;
use std::sync::LazyLock;

static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4})[-/](\d{2})[-/](\d{2})").unwrap());

pub struct JavMost;

/// 清理标题：去掉番号、站点名与首尾分隔符
fn clean_title(raw: &str, code_upper: &str) -> String {
    let code_lower = code_upper.to_lowercase();
    let mut t = raw.to_string();
    for marker in ["JAVMOST", "JavMost", "Javmost", "Watch JAV", "Online Free", "javmost"] {
        t = t.replace(marker, "");
    }
    t = t.replace(code_upper, "").replace(&code_lower, "");
    t.trim_matches(|c: char| c == '-' || c == '|' || c == ' ' || c == '　' || c == ':')
        .trim()
        .to_string()
}

impl Source for JavMost {
    fn name(&self) -> &str {
        "javmost"
    }

    fn build_url(&self, code: &str) -> String {
        format!("https://www.javmost.ws/{}/", code.to_uppercase())
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = scraper::Html::parse_document(html);
        let code_upper = code.to_uppercase();

        let head = extract_head_meta(&doc);
        let cover_url = head.cover_url.clone();
        let title = clean_title(&head.title, &code_upper);

        // 字段限定在 .card-block，避免抓到相关视频列表 / 菜单
        let actors = select_all_text(&doc, r#".card-block a[href*="/star/"]"#).join(", ");
        let studio = select_all_text(&doc, r#".card-block a[href*="/maker/"]"#)
            .into_iter()
            .next()
            .unwrap_or_default();

        let mut tag_items = select_all_text(&doc, r#".card-block a[href*="/category/"]"#);
        tag_items.extend(select_all_text(&doc, r#".card-block a[href*="/tag/"]"#));
        let tags = dedup_strings(tag_items).join(", ");

        // 发行日期：card-block 文本里的首个 YYYY-MM-DD
        let premiered = select_text(&doc, ".card-block")
            .and_then(|text| {
                DATE_RE
                    .captures(&text)
                    .map(|c| format!("{}-{}-{}", &c[1], &c[2], &c[3]))
            })
            .unwrap_or_default();

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code_upper,
            title,
            actors,
            studio,
            source: self.name().to_string(),
            cover_url,
            tags,
            premiered,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DETAIL: &str = r#"
    <html>
      <head>
        <meta property="og:title" content="SSIS-666 测试标题 | JAVMOST">
        <meta property="og:image" content="https://www.javmost.ws/cover/ssis-666.jpg">
      </head>
      <body>
        <div class="card-block">
          <p class="card-text">2021-08-10</p>
          <p class="card-text"><a href="/star/test-actress/">Test Actress</a></p>
          <p class="card-text"><a href="/maker/test-maker/">Test Maker</a></p>
          <p class="card-text"><a href="/category/drama/">Drama</a><a href="/tag/hd/">HD</a></p>
        </div>
        <div class="media-body"><a href="/star/other-actress/">Other</a></div>
      </body>
    </html>
    "#;

    #[test]
    fn parse_extracts_card_block_fields() {
        let r = JavMost.parse(DETAIL, "SSIS-666").expect("应解析成功");
        assert_eq!(r.code, "SSIS-666");
        assert_eq!(r.title, "测试标题");
        assert_eq!(r.cover_url, "https://www.javmost.ws/cover/ssis-666.jpg");
        assert_eq!(r.actors, "Test Actress"); // 不应抓到 .media-body 里的相关视频演员
        assert_eq!(r.studio, "Test Maker");
        assert_eq!(r.premiered, "2021-08-10");
        assert!(r.tags.contains("Drama") && r.tags.contains("HD"));
    }

    #[test]
    fn build_url_uppercases_code() {
        assert_eq!(JavMost.build_url("ssis-666"), "https://www.javmost.ws/SSIS-666/");
    }
}
