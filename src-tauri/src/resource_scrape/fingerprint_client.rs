//! 基于 wreq 的 TLS 指纹伪装 HTTP 客户端
//!
//! 使用 wreq 库模拟 Chrome 浏览器的 TLS 指纹（JA4 + Akamai），
//! 可以绕过大部分 Cloudflare 等反爬检测，减少对 WebView 回退的依赖。

use std::time::Duration;
use wreq::Client;
use wreq_util::Emulation;

/// 请求超时（秒）
const TIMEOUT_SECS: u64 = 30;

/// 创建 wreq HTTP 客户端（Chrome TLS 指纹 + 代理）
pub fn create_client() -> Result<Client, String> {
    // 注意：刻意不跟随重定向（wreq 默认即如此）。本项目把 3xx（如 javbus 的
    // 年龄验证门 302）视为"该回退 WebView"的信号；若跟随重定向，会把年龄门/
    // 反爬页静默解析成垃圾，反而绕过了 WebView 回退。良性 trailing-slash 跳转
    // 改在各源 build_url 里直接用规范 URL 规避（见 3xplanet）。
    let mut builder = Client::builder()
        .emulation(Emulation::Chrome137)
        .timeout(Duration::from_secs(TIMEOUT_SECS));

    if let Some(proxy_url) = crate::utils::proxy::get_proxy_url() {
        let proxy = wreq::Proxy::all(proxy_url.as_str())
            .map_err(|e| format!("wreq 代理配置失败: {}", e))?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|e| format!("创建 wreq 客户端失败: {}", e))
}

/// 进程级共享 Client：复用连接池/TLS session，避免每次刮削/搜索/下载重建指纹
/// Client（BoringSSL 指纹构建较贵，且重建会丢掉同站点跨阶段的 keep-alive）。
/// 按当前代理设置缓存，代理变更（与缓存不一致）时自动重建。
pub fn shared_client() -> Result<Client, String> {
    use std::sync::RwLock;
    static SHARED: RwLock<Option<(Option<url::Url>, Client)>> = RwLock::new(None);

    let current_proxy = crate::utils::proxy::get_proxy_url();

    // 快路径：代理未变则直接复用
    if let Ok(guard) = SHARED.read() {
        if let Some((proxy, client)) = guard.as_ref() {
            if *proxy == current_proxy {
                return Ok(client.clone());
            }
        }
    }

    // 慢路径：重建并缓存
    let client = create_client()?;
    if let Ok(mut guard) = SHARED.write() {
        *guard = Some((current_proxy, client.clone()));
    }
    Ok(client)
}

/// 请求指定 URL 并返回 HTML 文本
pub async fn fetch_html(client: &Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status));
    }

    resp.text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))
}

/// 请求指定 URL 并返回原始字节（用于图片下载等）
pub async fn fetch_bytes(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    Ok(bytes.to_vec())
}
