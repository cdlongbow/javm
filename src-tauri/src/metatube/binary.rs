//! MetaTube sidecar 二进制定位
//!
//! 二进制随 GitHub Actions 构建按平台打包进 `src-tauri/bin/`（`bundle.resources: ["bin/*"]`），
//! 运行时按多候选路径解析。**务必用 `is_file()`**：FFmpeg sidecar 曾因 `exists()` 在 arm64
//! 误选同名目录而失败（见 commit 584b3c1）。

use std::path::PathBuf;

/// sidecar 可执行文件名（统一固定名，GHA 下载后重命名为此）
#[cfg(windows)]
pub const BINARY_NAME: &str = "metatube-server.exe";
#[cfg(not(windows))]
pub const BINARY_NAME: &str = "metatube-server";

/// 解析 metatube-server 可执行文件路径。
///
/// 候选顺序：exe 同级目录 → exe 同级 `bin/` → 开发期 `target/{debug,release}/bin` → `src-tauri/bin`。
/// 全部用 `is_file()` 校验，避免误选同名目录。找不到返回 `None`（上层据此判定 sidecar 不可用 → 回退）。
pub fn resolve_binary_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(BINARY_NAME));
            candidates.push(dir.join("bin").join(BINARY_NAME));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        for profile in ["debug", "release"] {
            candidates.push(
                cwd.join("src-tauri")
                    .join("target")
                    .join(profile)
                    .join("bin")
                    .join(BINARY_NAME),
            );
        }
        candidates.push(cwd.join("src-tauri").join("bin").join(BINARY_NAME));
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}
