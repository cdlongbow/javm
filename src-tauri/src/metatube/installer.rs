//! 运行时下载安装 metatube-server：按当前 OS/ARCH 取 GitHub 最新 release 资产，
//! 解压落地到应用数据 `bin/`（[`binary::resolve_binary_path`](super::binary::resolve_binary_path) 优先候选）。

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::utils::proxy;

/// metatube 官方预编译 releases 仓库
const RELEASES_REPO: &str = "metatube-community/metatube-server-releases";

/// 当前系统/架构 → 资产 os-arch 标识（与官方资产命名一致）。不支持的平台返回 None。
pub fn current_os_arch() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-amd64"),
        ("windows", "aarch64") => Some("windows-arm64"),
        ("macos", "x86_64") => Some("darwin-amd64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("linux", "x86_64") => Some("linux-amd64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        _ => None,
    }
}

/// 下载客户端：**跟随重定向**（GitHub 资产地址会 302 到 objects.githubusercontent.com，
/// 项目默认 wreq 客户端不跟随重定向）。复用全局代理。
fn download_client() -> Result<wreq::Client, String> {
    proxy::apply_proxy_auto(wreq::Client::builder().redirect(wreq::redirect::Policy::limited(10)))?
        .build()
        .map_err(|e| format!("构建网络客户端失败: {e}"))
}

/// 下载并安装最新 metatube-server 到 `bin_dir`，返回落地的可执行文件路径。
/// 走全局代理设置，保证仅代理可达 GitHub 的网络下也能下载。
pub async fn download_latest(bin_dir: &Path) -> Result<PathBuf, String> {
    let os_arch = current_os_arch().ok_or_else(|| {
        format!(
            "当前系统/架构暂无 MetaTube 预编译版本: {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let asset_name = format!("metatube-server-{os_arch}.zip");
    let client = download_client()?;

    // 直接走 GitHub「最新版」资产重定向地址下载，**避开有限流的 REST API**；
    // 该地址 302 跳到实际下载源，由 download_client 跟随。
    let url = format!("https://github.com/{RELEASES_REPO}/releases/latest/download/{asset_name}");
    log::info!("[metatube] event=download_start url={url}");

    let resp = client
        .get(&url)
        .header("User-Agent", "javm")
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| format!("下载 MetaTube 失败（网络/代理是否可达 GitHub）: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "下载 MetaTube 失败: HTTP {}（资产 {} 可能不存在，或网络/代理不可达 GitHub）",
            resp.status().as_u16(),
            asset_name
        ));
    }
    let zip_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取 MetaTube 下载内容失败: {e}"))?;

    // 3. 解压取出 metatube-server(.exe)：先写临时文件，再**原子重命名**落地，
    //    避免下载/写入中断在目标路径留下半截损坏二进制。
    std::fs::create_dir_all(bin_dir).map_err(|e| format!("创建 bin 目录失败: {e}"))?;
    let target = bin_dir.join(super::binary::BINARY_NAME);
    let tmp = bin_dir.join(format!("{}.downloading", super::binary::BINARY_NAME));
    let _ = std::fs::remove_file(&tmp); // 清理可能残留的旧临时文件

    extract_server_binary(&zip_bytes, &tmp)?;

    #[cfg(unix)]
    set_executable(&tmp)?;

    std::fs::rename(&tmp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("替换二进制失败（旧文件可能正在运行）: {e}")
    })?;

    log::info!("[metatube] event=download_done path={}", target.display());
    Ok(target)
}

/// 从 zip 字节中找出 metatube-server(.exe) 写入 `target`。
fn extract_server_binary(zip_bytes: &[u8], target: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(zip_bytes)).map_err(|e| format!("打开压缩包失败: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("读取压缩条目失败: {e}"))?;
        if !entry.is_file() {
            continue;
        }
        let fname = entry.name().rsplit(['/', '\\']).next().unwrap_or("");
        if fname == "metatube-server" || fname == "metatube-server.exe" {
            // read_to_end 读满时 zip 会校验 CRC32，传输损坏会在此报错
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf).map_err(|e| format!("解压/校验失败: {e}"))?;
            if buf.is_empty() {
                return Err("解压得到空文件，下载可能损坏".to_string());
            }
            std::fs::write(target, &buf).map_err(|e| format!("写入二进制失败: {e}"))?;
            return Ok(());
        }
    }
    Err("压缩包内未找到 metatube-server 可执行文件".to_string())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).map_err(|e| format!("设置可执行权限失败: {e}"))
}
