//! avsox 数据源解析器（无码专用源）
//!
//! moo 家族站点，结构与 javbus 类似，但走「搜索 → 详情」两步：
//! - 搜索 URL: `{BASE}/cn/search/{code}` → 列表页 `a.movie-box` 链接到详情页
//! - 详情页结构：
//!   - 封面: `a.bigImage` href / `.bigImage img` src
//!   - 番号/发行日期/长度/系列: `.info` 中的 `<span>` 字段
//!   - 类别: `span.genre a`
//!   - 女优: `a.avatar-box span`（回退 `.star-name a`）
//!   - 预览图: `#sample-waterfall a.sample-box` href

use scraper::{Html, Selector};
use super::common::{select_all_attr, select_all_text, select_attr, select_text};
use super::{SearchResult, Source, SourceCapability};

/// avsox 主域名（该站点有多个镜像，域名随版本更新）
const BASE: &str = "https://avsox.click";

pub struct Avsox;

impl Source for Avsox {
    fn name(&self) -> &str { "avsox" }

    fn capability(&self) -> SourceCapability {
        SourceCapability::UncensoredOnly
    }

    fn build_url(&self, code: &str) -> String {
        format!("{}/cn/search/{}", BASE, code)
    }

    /// 从搜索结果列表页提取详情页 URL（优先番号匹配，回退第一个）
    fn extract_detail_url(&self, html: &str, code: &str) -> Option<String> {
        let doc = Html::parse_document(html);
        let code_norm = normalize_code(code);
        let sel = Selector::parse("a.movie-box").ok()?;

        for el in doc.select(&sel) {
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            if normalize_code(&text).contains(&code_norm) {
                if let Some(href) = el.value().attr("href") {
                    if !href.is_empty() {
                        return Some(absolute(href));
                    }
                }
            }
        }

        // 回退：取第一个结果
        let first = doc.select(&sel).next()?;
        first
            .value()
            .attr("href")
            .filter(|h| !h.is_empty())
            .map(absolute)
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = Html::parse_document(html);

        // 封面：bigImage href 通常是大图，img src 通常是缩略图
        let cover_url = select_attr(&doc, "a.bigImage", "href")
            .or_else(|| select_attr(&doc, ".bigImage img", "src"))
            .map(|u| absolute(&u))
            .unwrap_or_default();
        let poster_url = select_attr(&doc, ".bigImage img", "src")
            .map(|u| absolute(&u))
            .unwrap_or_default();

        // 标题：h3，去掉番号部分
        let raw_title = select_text(&doc, "h3").unwrap_or_default();
        let title = if raw_title.is_empty() {
            String::new()
        } else {
            raw_title.replace(code, "").trim().to_string()
        };
        let sort_title = if raw_title.is_empty() {
            code.to_string()
        } else {
            format!("{} {}", code, raw_title)
        };

        let info_text = select_text(&doc, ".info").unwrap_or_default();
        let premiered = extract_field(&info_text, &["發行日期:", "发行日期:"]).unwrap_or_default();
        let duration_raw = extract_field(&info_text, &["長度:", "长度:"]).unwrap_or_default();
        let duration = if duration_raw.is_empty() {
            String::new()
        } else {
            duration_raw.replace("分鐘", "分钟")
        };
        let label = extract_field(&info_text, &["系列:"]).unwrap_or_default();
        let studio = extract_field(
            &info_text,
            &["製作商:", "制作商:", "發行商:", "发行商:"],
        )
        .unwrap_or_default();

        // 类别（只取 href 含 /genre/ 的链接）
        let tags = select_genre(&doc).join(", ");

        // 女优：avsox 详情页用 a.avatar-box span，回退 .star-name a
        let mut actor_list = select_all_text(&doc, "a.avatar-box span");
        if actor_list.is_empty() {
            actor_list = select_all_text(&doc, ".star-name a");
        }
        let actors = actor_list.join(", ");

        // 预览截图
        let thumbs = select_all_attr(&doc, "#sample-waterfall a.sample-box", "href")
            .into_iter()
            .map(|u| absolute(&u))
            .collect();

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code.to_string(),
            title,
            poster_url,
            actors,
            duration,
            studio: studio.clone(),
            source: self.name().to_string(),
            cover_url,
            tags: tags.clone(),
            premiered,
            rating: None,
            thumbs,
            sort_title,
            mpaa: "JP-18+ 无码".to_string(),
            custom_rating: "JP-18+".to_string(),
            country_code: "JP".to_string(),
            critic_rating: Some(0),
            maker: studio,
            label,
            genres: tags,
            is_uncensored: true,
            ..Default::default()
        })
    }
}

// ============ 辅助函数 ============

/// 相对路径补全为绝对 URL
fn absolute(url: &str) -> String {
    if url.starts_with("http") {
        url.to_string()
    } else {
        format!("{}{}", BASE, url)
    }
}

/// 番号归一：去除非字母数字并大写，用于宽松匹配
fn normalize_code(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// 选取 span.genre 下 href 含 /genre/ 的类别文本
fn select_genre(doc: &Html) -> Vec<String> {
    let sel = match Selector::parse("span.genre a") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| {
            let href = el.value().attr("href").unwrap_or("");
            if !href.contains("/genre/") {
                return None;
            }
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cleaned.is_empty() { None } else { Some(cleaned) }
        })
        .collect()
}

/// 从信息文本中提取指定字段的值
fn extract_field(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        if let Some(pos) = text.find(label) {
            let after = &text[pos + label.len()..];
            let value = after.trim().split_whitespace().next()?;
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_is_uncensored_only() {
        assert_eq!(Avsox.capability(), SourceCapability::UncensoredOnly);
        assert!(Avsox.capability().handles(true)); // 无码作品：查
        assert!(!Avsox.capability().handles(false)); // 有码作品：跳过
    }

    #[test]
    fn extract_detail_url_prefers_code_match() {
        let html = r#"
            <a class="movie-box" href="/cn/movie/aaa"><span>OTHER-001</span></a>
            <a class="movie-box" href="/cn/movie/bbb"><span>HEYZO-1234 标题</span></a>
        "#;
        let url = Avsox.extract_detail_url(html, "HEYZO-1234").unwrap();
        assert_eq!(url, "https://avsox.click/cn/movie/bbb");
    }

    #[test]
    fn parse_extracts_moo_family_fields() {
        // moo 家族详情页结构（与 javbus 类似），验证选择器拼写正确
        let html = r#"
            <a class="bigImage" href="/cover/big.jpg"><img src="/cover/small.jpg"></a>
            <h3>HEYZO-1234 测试标题</h3>
            <div class="info">
              <p><span class="header">發行日期:</span> 2020-01-01</p>
              <p><span class="header">長度:</span> 60分鐘</p>
              <p><span class="header">系列:</span> 测试系列</p>
            </div>
            <span class="genre"><a href="/cn/genre/xxx">巨乳</a></span>
            <a class="avatar-box"><div class="photo-info"><span>测试演员</span></div></a>
            <div id="sample-waterfall"><a class="sample-box" href="/preview/1.jpg"></a></div>
        "#;
        let r = Avsox.parse(html, "HEYZO-1234").unwrap();
        assert!(r.is_uncensored);
        assert_eq!(r.source, "avsox");
        assert_eq!(r.title, "测试标题");
        assert_eq!(r.premiered, "2020-01-01");
        assert_eq!(r.duration, "60分钟");
        assert_eq!(r.label, "测试系列");
        assert_eq!(r.cover_url, "https://avsox.click/cover/big.jpg");
        assert_eq!(r.actors, "测试演员");
        assert_eq!(r.tags, "巨乳");
        assert_eq!(r.thumbs, vec!["https://avsox.click/preview/1.jpg"]);
    }
}
