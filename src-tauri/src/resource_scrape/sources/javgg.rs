//! javgg.net 数据源解析器
//!
//! 搜索型网站：https://javgg.net/?s={CODE}
//! 搜索结果链接到详情页 /jav/{slug}/（slug 通常以番号开头）。
//! 详情页（WordPress starstruck 主题）字段集中在 `div#Cast.sgeneros3` 内，
//! 按 href 分类法提取，避免抓到页头菜单/页脚里的同类链接：
//! - 演员：a[href*="/star/"]
//! - 类别：a[href*="/genre/"]
//! - 片商：a[href*="/maker/"]
//! - 标签：a[href*="/tag/"]
//! - 日期：[itemprop="datecreated"] / .date
//! - 时长：[itemprop="duration"] / .runtime
//! - 标题/封面：head 的 og:title / og:image

use super::common::{dedup_strings, extract_head_meta, select_all_text, select_text};
use super::{SearchResult, Source};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static A_HREF_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4})[-/](\d{2})[-/](\d{2})").unwrap());

pub struct JavGG;

/// 将相对链接补全为绝对地址
fn absolutize(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("https://javgg.net{}", href)
    } else {
        format!("https://javgg.net/{}", href)
    }
}

/// 规范化日期为 YYYY-MM-DD（兼容 YYYY/MM/DD），无法识别则返回去空白原串
fn normalize_date(raw: &str) -> String {
    if let Some(caps) = DATE_RE.captures(raw) {
        return format!("{}-{}-{}", &caps[1], &caps[2], &caps[3]);
    }
    raw.trim().to_string()
}

/// 清理标题：去掉番号、站点名与首尾分隔符
fn clean_title(raw: &str, code_upper: &str) -> String {
    let code_lower = code_upper.to_lowercase();
    let mut t = raw.to_string();
    for marker in ["JavGG", "JAVGG", "Javgg", "javgg.net", "- Watch JAV Online Free", "Watch JAV Online"] {
        t = t.replace(marker, "");
    }
    t = t.replace(code_upper, "").replace(&code_lower, "");
    t.trim_matches(|c: char| c == '-' || c == '|' || c == ' ' || c == '　' || c == ':')
        .trim()
        .to_string()
}

impl Source for JavGG {
    fn name(&self) -> &str {
        "javgg"
    }

    fn build_url(&self, code: &str) -> String {
        format!("https://javgg.net/?s={}", code)
    }

    /// 从搜索结果页找到与番号匹配的详情页 /jav/{slug}/
    fn extract_detail_url(&self, html: &str, code: &str) -> Option<String> {
        let doc = Html::parse_document(html);
        let code_l = code.to_lowercase();
        let code_nodash = code_l.replace('-', "");
        for el in doc.select(&A_HREF_SEL) {
            let href = el.value().attr("href").unwrap_or("");
            let lower = href.to_lowercase();
            if let Some(idx) = lower.find("/jav/") {
                let slug = &lower[idx + 5..];
                if slug.starts_with(&code_l) || slug.starts_with(&code_nodash) {
                    return Some(absolutize(href));
                }
            }
        }
        None
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = Html::parse_document(html);
        let code_upper = code.to_uppercase();

        let head = extract_head_meta(&doc);
        let cover_url = head.cover_url.clone();
        let title = clean_title(&head.title, &code_upper);

        // 详情字段限定在 #Cast 容器内，避免抓到菜单/页脚同类链接
        let actors = select_all_text(&doc, r#"#Cast a[href*="/star/"]"#).join(", ");
        let studio = select_all_text(&doc, r#"#Cast a[href*="/maker/"]"#)
            .into_iter()
            .next()
            .unwrap_or_default();

        let mut tag_items = select_all_text(&doc, r#"#Cast a[href*="/genre/"]"#);
        tag_items.extend(select_all_text(&doc, r#"#Cast a[href*="/tag/"]"#));
        let tags = dedup_strings(tag_items).join(", ");

        let premiered = select_text(&doc, r#"[itemprop="datecreated"]"#)
            .or_else(|| select_text(&doc, ".data .date"))
            .map(|s| normalize_date(&s))
            .unwrap_or_default();

        let duration = select_text(&doc, r#"[itemprop="duration"]"#)
            .or_else(|| select_text(&doc, ".data .runtime"))
            .unwrap_or_default();

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code_upper,
            title,
            actors,
            duration,
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
        <meta property="og:title" content="SSIS-001 测试标题 - JavGG">
        <meta property="og:image" content="https://javgg.net/cover/ssis-001.jpg">
      </head>
      <body>
        <div class="data">
          <span class="date" itemprop="datecreated">2021-04-24</span>
          <span class="runtime" itemprop="duration">120 min</span>
          <div class="boxye2">
            <div id="Cast" class="sgeneros3">
              <a href="/star/test-actress/">Test Actress</a>
              <a href="/maker/test-maker/">Test Maker</a>
              <a href="/genre/drama/">Drama</a>
              <a href="/tag/hd/">HD</a>
            </div>
          </div>
        </div>
      </body>
    </html>
    "#;

    #[test]
    fn parse_extracts_scoped_fields() {
        let r = JavGG.parse(DETAIL, "SSIS-001").expect("应解析成功");
        assert_eq!(r.code, "SSIS-001");
        assert_eq!(r.title, "测试标题");
        assert_eq!(r.cover_url, "https://javgg.net/cover/ssis-001.jpg");
        assert_eq!(r.actors, "Test Actress");
        assert_eq!(r.studio, "Test Maker");
        assert_eq!(r.premiered, "2021-04-24");
        assert_eq!(r.duration, "120 min");
        assert!(r.tags.contains("Drama") && r.tags.contains("HD"));
    }

    #[test]
    fn extract_detail_url_matches_code_slug() {
        let search = r#"<a href="/jav/ssis-001-test/">x</a><a href="/jav/other-999/">y</a>"#;
        let url = JavGG.extract_detail_url(search, "SSIS-001").unwrap();
        assert_eq!(url, "https://javgg.net/jav/ssis-001-test/");
    }
}
