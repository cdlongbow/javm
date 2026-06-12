//! sextb.net 数据源解析器
//!
//! 详情页：https://sextb.net/{CODE}（番号即路径，无需搜索跳转）
//! 详情字段集中在 `div.col-7 > div.description` 内，按 href 分类法提取，
//! 避免抓到顶栏菜单里的同类链接：
//! - 演员：a[href*="/actress/"]
//! - 片商：a[href*="/studio/"]
//! - 导演：a[href*="/director/"]
//! - 类别：a[href*="/genre/"]
//! - 日期：description 文本中的 YYYY-MM-DD
//! - 标题/封面：head 的 og:title / og:image

use super::common::{dedup_strings, extract_head_meta, select_all_text, select_text};
use super::{SearchResult, Source};
use regex::Regex;
use std::sync::LazyLock;

static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4})[-/](\d{2})[-/](\d{2})").unwrap());

pub struct SexTB;

/// 清理标题：去掉番号、站点名与首尾分隔符
fn clean_title(raw: &str, code_upper: &str) -> String {
    let code_lower = code_upper.to_lowercase();
    let mut t = raw.to_string();
    for marker in ["SexTB", "SEXTB", "Sextb", "JAV Torrent", "Torrent", "Download", "sextb.net"] {
        t = t.replace(marker, "");
    }
    t = t.replace(code_upper, "").replace(&code_lower, "");
    t.trim_matches(|c: char| c == '-' || c == '|' || c == ' ' || c == '　' || c == ':')
        .trim()
        .to_string()
}

impl Source for SexTB {
    fn name(&self) -> &str {
        "sextb"
    }

    fn build_url(&self, code: &str) -> String {
        format!("https://sextb.net/{}", code.to_uppercase())
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = scraper::Html::parse_document(html);
        let code_upper = code.to_uppercase();

        let head = extract_head_meta(&doc);
        let cover_url = head.cover_url.clone();
        let title = clean_title(&head.title, &code_upper);

        // 字段限定在 .col-7 .description，排除顶栏菜单里的同类链接
        let actors = select_all_text(&doc, r#".col-7 .description a[href*="/actress/"]"#).join(", ");
        let studio = select_all_text(&doc, r#".col-7 .description a[href*="/studio/"]"#)
            .into_iter()
            .next()
            .unwrap_or_default();
        let director = select_all_text(&doc, r#".col-7 .description a[href*="/director/"]"#)
            .into_iter()
            .next()
            .unwrap_or_default();
        let tags = dedup_strings(select_all_text(
            &doc,
            r#".col-7 .description a[href*="/genre/"]"#,
        ))
        .join(", ");

        let premiered = select_text(&doc, ".col-7 .description")
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
            director,
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
        <meta property="og:title" content="SSIS-666 测试标题 - SexTB">
        <meta property="og:image" content="https://sextb.net/cover/ssis-666.jpg">
      </head>
      <body>
        <nav><ul class="navbar-submenu"><li class="navbar-submenu-item"><a href="/actress/menu/">菜单演员</a></li></ul></nav>
        <div class="col-7">
          <div class="description">
            Release Date: 2021-08-10
            <a href="/actress/test-actress/">Test Actress</a>
            <a href="/studio/test-studio/">Test Studio</a>
            <a href="/director/test-director/">Test Director</a>
            <a href="/genre/drama/">Drama</a>
            <a href="/genre/hd/">HD</a>
          </div>
        </div>
      </body>
    </html>
    "#;

    #[test]
    fn parse_extracts_scoped_description_fields() {
        let r = SexTB.parse(DETAIL, "SSIS-666").expect("应解析成功");
        assert_eq!(r.code, "SSIS-666");
        assert_eq!(r.title, "测试标题");
        assert_eq!(r.cover_url, "https://sextb.net/cover/ssis-666.jpg");
        assert_eq!(r.actors, "Test Actress"); // 不应抓到菜单里的演员
        assert_eq!(r.studio, "Test Studio");
        assert_eq!(r.director, "Test Director");
        assert_eq!(r.premiered, "2021-08-10");
        assert!(r.tags.contains("Drama") && r.tags.contains("HD"));
    }

    #[test]
    fn build_url_uppercases_code() {
        assert_eq!(SexTB.build_url("ssis-666"), "https://sextb.net/SSIS-666");
    }
}
