//! 视频媒体资源管理模块
//!
//! 统一处理视频相关的媒体资源操作，包括：
//! - NFO 元数据文件保存
//! - 封面图片下载/截取保存
//! - 视频帧截取（ffmpeg）
//! - 预览截图保存
//! - 文件回滚

use std::fs;
use std::path::{Path, PathBuf};

use crate::nfo::generator::NfoGenerator;
use crate::resource_scrape::types::ScrapeMetadata;

const EXTRAFANART_DIR_NAME: &str = "extrafanart";
const SUBTITLE_EXTENSIONS: &[&str] = &[
    "srt", "ass", "ssa", "vtt", "sub", "idx", "smi", "sup", "sbv", "dfxp", "ttml",
    "scc", "usf",
];

#[derive(Debug, Clone)]
pub struct RelocatedVideoAssets {
    pub original_video_path: String,
    pub video_path: String,
    pub dir_path: String,
    pub poster: Option<String>,
    pub thumb: Option<String>,
    pub fanart: Option<String>,
}

// ============================================================
// 元数据存储模式（跟随视频 / 独立目录）
// ============================================================

/// 元数据存储配置，从 `AppSettings.metadata` 派生
#[derive(Debug, Clone)]
pub struct MetadataStorageConfig {
    /// 是否启用独立目录模式
    pub independent: bool,
    /// 独立目录模式下的元数据根目录
    pub root_dir: String,
}

impl MetadataStorageConfig {
    pub fn from_settings(settings: &crate::settings::AppSettings) -> Self {
        Self {
            independent: settings.metadata.is_independent(),
            root_dir: settings.metadata.root_dir.trim().to_string(),
        }
    }
}

/// 独立目录模式下需写入的 .strm 规格
#[derive(Debug, Clone)]
pub struct StrmSpec {
    /// .strm 文件路径
    pub path: PathBuf,
    /// 单行内容：视频真实绝对路径
    pub video_abs_path: String,
}

/// 元数据资产落地目标：NFO / 图片 / extrafanart 写到哪里、用什么文件名 stem
#[derive(Debug, Clone)]
pub struct MediaAssetTarget {
    /// NFO / poster / fanart / extrafanart 的落地目录
    pub dir: PathBuf,
    /// 文件名 stem（NFO = `<stem>.nfo`，封面 = `<stem>-poster.jpg`）
    pub stem: String,
    /// 独立目录模式下需写入的 .strm（跟随视频模式为 None）
    pub strm: Option<StrmSpec>,
}

/// 清洗为合法路径片段：替换 Windows 非法字符、折叠空白、截断长度、去尾部点/空格。
/// 与 `sanitize_title_for_path` 不同：永不返回错误，空串回退到 `fallback`。
fn sanitize_path_component(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let folded = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    // 截断到安全长度（按字符），避免 Windows 路径过长
    let truncated: String = folded.chars().take(100).collect();
    let trimmed = truncated.trim().trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() {
        fallback.trim().trim_end_matches(['.', ' ']).to_string()
    } else {
        trimmed
    }
}

/// 构造独立目录名：`番号 标题`（标题为空时退化为纯番号）
fn build_independent_folder_name(local_id: &str, title: &str) -> String {
    let id = local_id.trim();
    let title = title.trim();
    let raw = if title.is_empty() {
        id.to_string()
    } else {
        format!("{} {}", id, title)
    };
    sanitize_path_component(&raw, id)
}

/// 解析元数据资产落地目标。
///
/// - 跟随视频模式：目录 = 视频父目录，stem = 视频文件名，无 .strm。
/// - 独立目录模式（开启 + 根目录非空 + 番号非空）：目录 = `<root>/<番号 标题>/`，
///   stem = 番号，并附带指向视频真实路径的 .strm。条件不满足时自动回退到跟随视频。
pub fn resolve_asset_target(
    video_path: &str,
    local_id: &str,
    title: &str,
    cfg: &MetadataStorageConfig,
) -> Result<MediaAssetTarget, String> {
    let video = Path::new(video_path);
    let video_parent = video.parent().ok_or("无效的视频路径")?;
    let video_stem = video
        .file_stem()
        .ok_or("无效的视频文件名")?
        .to_string_lossy()
        .to_string();

    let local_id = local_id.trim();

    if cfg.independent && !cfg.root_dir.is_empty() && !local_id.is_empty() {
        let folder = build_independent_folder_name(local_id, title);
        let dir = Path::new(&cfg.root_dir).join(folder);
        let stem = sanitize_path_component(local_id, &video_stem);
        let strm = StrmSpec {
            path: dir.join(format!("{}.strm", stem)),
            video_abs_path: video_path.to_string(),
        };
        Ok(MediaAssetTarget {
            dir,
            stem,
            strm: Some(strm),
        })
    } else {
        Ok(MediaAssetTarget {
            dir: video_parent.to_path_buf(),
            stem: video_stem,
            strm: None,
        })
    }
}

/// 确保目标目录存在；独立目录模式下写入（或更新）.strm（内容为视频真实绝对路径，幂等）。
pub fn ensure_asset_dir_and_strm(target: &MediaAssetTarget) -> Result<(), String> {
    fs::create_dir_all(&target.dir)
        .map_err(|e| format!("创建元数据目录失败 {}: {}", target.dir.display(), e))?;

    if let Some(strm) = &target.strm {
        let want = strm.video_abs_path.trim();
        let need_write = match fs::read_to_string(&strm.path) {
            Ok(existing) => existing.trim() != want,
            Err(_) => true,
        };
        if need_write {
            fs::write(&strm.path, want)
                .map_err(|e| format!("写入 .strm 文件失败 {}: {}", strm.path.display(), e))?;
        }
    }
    Ok(())
}

/// 在元数据根目录下定位某番号现有的独立子目录（含 `<番号>.strm`）。
///
/// 子目录名为「番号 标题」可能随标题变化，故按番号前缀筛选、再以 `<番号>.strm` 存在性确认。
/// 返回 `(子目录路径, 文件名 stem)`；非独立模式 / 未配置 / 未找到时返回 None。
fn find_independent_dir(cfg: &MetadataStorageConfig, local_id: &str) -> Option<(PathBuf, String)> {
    if !cfg.independent {
        return None;
    }
    let root = cfg.root_dir.trim();
    let local_id = local_id.trim();
    if root.is_empty() || local_id.is_empty() {
        return None;
    }

    let stem = sanitize_path_component(local_id, local_id);
    let strm_name = format!("{}.strm", stem);
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false) {
            continue;
        }
        if !entry.file_name().to_string_lossy().starts_with(&stem) {
            continue;
        }
        let dir = entry.path();
        if dir.join(&strm_name).exists() {
            return Some((dir, stem));
        }
    }
    None
}

/// 视频移动/重命名后，同步独立目录里对应番号子目录的 `.strm` 内容为新的视频绝对路径。
/// 非独立模式 / 未找到 `.strm` 时静默跳过（不视为错误）。
pub fn sync_independent_strm(
    cfg: &MetadataStorageConfig,
    local_id: &str,
    new_video_path: &str,
) -> Result<(), String> {
    let Some((dir, stem)) = find_independent_dir(cfg, local_id) else {
        return Ok(());
    };
    let strm_path = dir.join(format!("{}.strm", stem));
    let want = new_video_path.trim();
    let need_write = match fs::read_to_string(&strm_path) {
        Ok(existing) => existing.trim() != want,
        Err(_) => true,
    };
    if need_write {
        fs::write(&strm_path, want)
            .map_err(|e| format!("更新 .strm 文件失败 {}: {}", strm_path.display(), e))?;
    }
    Ok(())
}

/// 独立目录模式下，把更新后的 NFO 写回视频对应的独立元数据目录（按番号定位现有子目录、
/// 引用该目录内已有图集的本地文件名）。
///
/// 返回 `Ok(true)` = 已写入独立目录；`Ok(false)` = 非独立模式或未找到独立目录（调用方应回退写视频同级）。
pub fn save_nfo_to_independent_dir(
    cfg: &MetadataStorageConfig,
    local_id: &str,
    metadata: &ScrapeMetadata,
) -> Result<bool, String> {
    let Some((dir, stem)) = find_independent_dir(cfg, local_id) else {
        return Ok(false);
    };
    save_nfo_to(&dir, &stem, metadata)?;
    Ok(true)
}

// ============================================================
// NFO 元数据
// ============================================================

/// 统一的 NFO 保存逻辑：检查本地封面是否存在，然后调用 NfoGenerator 生成 NFO 文件
///
/// 供 queue_manager、commands 等模块复用，避免重复实现。
pub fn save_nfo_for_video(video_path: &str, metadata: &ScrapeMetadata) -> Result<(), String> {
    let path = Path::new(video_path);
    let parent_dir = path.parent().ok_or("无效的视频路径")?;
    let file_stem = path
        .file_stem()
        .ok_or("无效的视频文件名")?
        .to_string_lossy()
        .to_string();

    save_nfo_to(parent_dir, &file_stem, metadata)
}

/// 将 NFO 保存到指定目录，文件名为 `<stem>.nfo`；按同目录已存在的标准图集文件
/// （`<stem>-poster/fanart/thumb.*`）引用相对文件名。
///
/// 供独立目录模式直接写入 `<root>/<番号 标题>/<番号>.nfo`。
pub fn save_nfo_to(dir: &Path, stem: &str, metadata: &ScrapeMetadata) -> Result<(), String> {
    let generator = NfoGenerator::new();
    let artwork = detect_local_artwork(dir, stem);
    let nfo_path = dir.join(format!("{}.nfo", stem));
    generator.save_to(metadata, &nfo_path, &artwork).map(|_| ())
}

/// 探测同目录已存在的标准图集文件，返回 NFO 引用的相对文件名（poster/fanart/thumb）。
pub fn detect_local_artwork(dir: &Path, stem: &str) -> crate::nfo::generator::NfoArtwork {
    crate::nfo::generator::NfoArtwork {
        poster: detect_artwork_filename(dir, stem, crate::media::artwork::POSTER_SUFFIX),
        fanart: detect_artwork_filename(dir, stem, crate::media::artwork::FANART_SUFFIX),
        thumb: detect_artwork_filename(dir, stem, crate::media::artwork::THUMB_SUFFIX),
    }
}

fn detect_artwork_filename(dir: &Path, stem: &str, suffix: &str) -> Option<String> {
    ["jpg", "jpeg", "png", "webp"]
        .iter()
        .map(|ext| format!("{}-{}.{}", stem, suffix, ext))
        .find(|name| dir.join(name).exists())
}

pub fn has_same_named_parent_dir(video_path: &Path) -> bool {
    let Some(parent_dir) = video_path.parent() else {
        return false;
    };
    let Some(parent_name) = parent_dir.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(file_stem) = video_path.file_stem().and_then(|name| name.to_str()) else {
        return false;
    };

    parent_name.eq_ignore_ascii_case(file_stem)
}

fn is_subtitle_suffix_separator(ch: char) -> bool {
    matches!(ch, '.' | '_' | '-' | ' ' | '[' | '(')
}

fn is_matching_subtitle_file(video_path: &Path, candidate: &Path) -> bool {
    let Some(video_parent) = video_path.parent() else {
        return false;
    };
    let Some(candidate_parent) = candidate.parent() else {
        return false;
    };
    if video_parent != candidate_parent {
        return false;
    }

    let Some(extension) = candidate.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    if !SUBTITLE_EXTENSIONS
        .iter()
        .any(|item| item.eq_ignore_ascii_case(extension))
    {
        return false;
    }

    let Some(video_stem) = video_path.file_stem().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(candidate_stem) = candidate.file_stem().and_then(|name| name.to_str()) else {
        return false;
    };

    let video_stem_lower = video_stem.to_ascii_lowercase();
    let candidate_stem_lower = candidate_stem.to_ascii_lowercase();

    candidate_stem_lower == video_stem_lower
        || candidate_stem_lower
            .strip_prefix(&video_stem_lower)
            .is_some_and(|suffix| {
                suffix
                    .chars()
                    .next()
                    .is_some_and(is_subtitle_suffix_separator)
            })
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn move_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
            Ok(())
        }
    }
}

fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_recursive(src, dst).map_err(|e| format!("复制目录失败: {}", e))?;
            fs::remove_dir_all(src).map_err(|e| format!("删除原目录失败: {}", e))?;
            Ok(())
        }
    }
}

fn resolve_asset_source(video_path: &Path, explicit_path: Option<&str>, suffix: &str) -> Option<PathBuf> {
    // 仅接受与视频同级的图：独立元数据目录里的图不随视频移动/重命名搬走，留在独立目录。
    let video_parent = video_path.parent();
    explicit_path
        .map(PathBuf::from)
        .filter(|path| path.exists() && path.is_file() && path.parent() == video_parent)
        .or_else(|| find_sibling_artwork(video_path, suffix).map(PathBuf::from))
}

fn move_optional_asset(source: Option<PathBuf>, target_dir: &Path, label: &str) -> Option<String> {
    let source = source?;
    let file_name = match source.file_name() {
        Some(file_name) => file_name,
        None => {
            log::error!(
                "[media_assets] event=move_optional_asset_invalid_filename label={} source={}",
                label,
                source.display()
            );
            return None;
        }
    };

    let target = target_dir.join(file_name);
    if target.exists() && !source.exists() {
        return Some(target.to_string_lossy().to_string());
    }

    if source == target {
        return Some(target.to_string_lossy().to_string());
    }

    if !source.exists() {
        return None;
    }

    match move_file(&source, &target) {
        Ok(()) => Some(target.to_string_lossy().to_string()),
        Err(error) => {
            log::error!(
                "[media_assets] event=move_optional_asset_failed label={} source={} target={} error={}",
                label,
                source.display(),
                target.display(),
                error
            );
            None
        }
    }
}

fn move_matching_subtitle_files(video_path: &Path, target_dir: &Path) {
    let Some(parent_dir) = video_path.parent() else {
        return;
    };

    let Ok(entries) = fs::read_dir(parent_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let candidate = entry.path();
        if !is_matching_subtitle_file(video_path, &candidate) {
            continue;
        }

        let Some(file_name) = candidate.file_name() else {
            continue;
        };
        let target = target_dir.join(file_name);
        if let Err(error) = move_file(&candidate, &target) {
            log::error!(
                "[media_assets] event=move_subtitle_failed source={} target={} error={}",
                candidate.display(),
                target.display(),
                error
            );
        }
    }
}

#[derive(Debug, Clone)]
struct PendingRenameOperation {
    source: PathBuf,
    target: PathBuf,
    is_dir: bool,
}

fn is_same_path_for_fs(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn sanitize_title_for_path(title: &str) -> Result<String, String> {
    let sanitized = title
        .trim()
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_string();

    if sanitized.is_empty() {
        return Err("标题为空或包含非法字符，无法作为文件名".to_string());
    }

    Ok(sanitized)
}

fn remap_path_after_dir_rename(path: &Path, old_dir: &Path, new_dir: &Path) -> PathBuf {
    match path.strip_prefix(old_dir) {
        Ok(relative) => new_dir.join(relative),
        Err(_) => path.to_path_buf(),
    }
}

fn build_artwork_target_path(source: &Path, target_dir: &Path, new_stem: &str, suffix: &str) -> Option<PathBuf> {
    let extension = source.extension()?.to_str()?;
    Some(target_dir.join(format!("{}-{}.{}", new_stem, suffix, extension)))
}

fn queue_optional_file_rename(
    operations: &mut Vec<PendingRenameOperation>,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
) {
    let (Some(source), Some(target)) = (source, target) else {
        return;
    };

    if is_same_path_for_fs(&source, &target) {
        return;
    }

    operations.push(PendingRenameOperation {
        source,
        target,
        is_dir: false,
    });
}

fn queue_matching_subtitle_renames(
    operations: &mut Vec<PendingRenameOperation>,
    video_path: &Path,
    old_dir: &Path,
    target_dir: &Path,
    new_stem: &str,
    rename_parent_dir: bool,
) {
    let Some(parent_dir) = video_path.parent() else {
        return;
    };

    let Some(video_stem) = video_path.file_stem().and_then(|name| name.to_str()) else {
        return;
    };

    let Ok(entries) = fs::read_dir(parent_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let source = entry.path();
        if !is_matching_subtitle_file(video_path, &source) {
            continue;
        }

        let extension = match source.extension().and_then(|ext| ext.to_str()) {
            Some(extension) => extension.to_string(),
            None => continue,
        };

        let candidate_stem = match source.file_stem().and_then(|name| name.to_str()) {
            Some(stem) => stem,
            None => continue,
        };

        let suffix = candidate_stem
            .strip_prefix(video_stem)
            .map(str::to_string)
            .or_else(|| {
                let video_stem_lower = video_stem.to_ascii_lowercase();
                let candidate_stem_lower = candidate_stem.to_ascii_lowercase();
                candidate_stem_lower
                    .strip_prefix(&video_stem_lower)
                    .map(|rest| candidate_stem[candidate_stem.len() - rest.len()..].to_string())
            })
            .unwrap_or_default();

        let remapped_source = if rename_parent_dir {
            remap_path_after_dir_rename(&source, old_dir, target_dir)
        } else {
            source.clone()
        };
        let target = target_dir.join(format!("{}{}.{}", new_stem, suffix, extension));

        if is_same_path_for_fs(&remapped_source, &target) {
            continue;
        }

        operations.push(PendingRenameOperation {
            source: remapped_source,
            target,
            is_dir: false,
        });
    }
}

fn rollback_rename_operations(completed: &[PendingRenameOperation]) {
    for operation in completed.iter().rev() {
        if !operation.target.exists() {
            continue;
        }

        if let Some(parent) = operation.source.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let rollback_result = if operation.is_dir {
            move_dir(&operation.target, &operation.source)
        } else {
            move_file(&operation.target, &operation.source)
                .map_err(|error| format!("回滚文件失败: {}", error))
        };

        if let Err(error) = rollback_result {
            log::error!(
                "[media_assets] event=rollback_rename_failed source={} target={} error={}",
                operation.source.display(),
                operation.target.display(),
                error
            );
        }
    }
}

fn execute_rename_operations(operations: &[PendingRenameOperation]) -> Result<(), String> {
    let mut completed = Vec::new();

    for operation in operations {
        if let Some(parent) = operation.target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }

        if operation.target.exists() && !is_same_path_for_fs(&operation.source, &operation.target) {
            rollback_rename_operations(&completed);
            return Err(format!("目标已存在，无法重命名: {}", operation.target.display()));
        }

        let result = if operation.is_dir {
            move_dir(&operation.source, &operation.target)
        } else {
            move_file(&operation.source, &operation.target)
                .map_err(|e| format!("重命名文件失败: {}", e))
        };

        if let Err(error) = result {
            rollback_rename_operations(&completed);
            return Err(format!(
                "重命名失败: {} -> {}: {}",
                operation.source.display(),
                operation.target.display(),
                error
            ));
        }

        completed.push(operation.clone());
    }

    Ok(())
}

pub fn rename_video_assets_with_title(
    video_path: &str,
    new_title: &str,
    poster: Option<&str>,
    thumb: Option<&str>,
    fanart: Option<&str>,
) -> Result<Option<RelocatedVideoAssets>, String> {
    let video_path_obj = Path::new(video_path);
    if !video_path_obj.exists() {
        return Err("源视频文件不存在".to_string());
    }

    let old_dir = video_path_obj.parent().ok_or("无效的视频路径")?;
    let old_stem = video_path_obj
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("无效的视频文件名")?;
    let new_stem = sanitize_title_for_path(new_title)?;
    let current_parent_name = old_dir.file_name().and_then(|name| name.to_str());
    let already_in_target_parent = current_parent_name
        .is_some_and(|name| name.eq_ignore_ascii_case(&new_stem));

    let rename_parent_dir = old_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(old_stem));

    let target_dir = if already_in_target_parent {
        old_dir.to_path_buf()
    } else if rename_parent_dir {
        let parent_of_parent = old_dir.parent().ok_or("无效的父目录")?;
        parent_of_parent.join(&new_stem)
    } else {
        old_dir.join(&new_stem)
    };

    let current_video_path = if rename_parent_dir {
        remap_path_after_dir_rename(video_path_obj, old_dir, &target_dir)
    } else {
        video_path_obj.to_path_buf()
    };

    let new_file_name = match video_path_obj.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{}.{}", new_stem, ext),
        _ => new_stem.clone(),
    };
    let new_video_path = target_dir.join(new_file_name);

    if already_in_target_parent && is_same_path_for_fs(&current_video_path, &new_video_path) {
        return Ok(None);
    }

    let actual_poster_source = resolve_asset_source(video_path_obj, poster, "poster");
    let actual_thumb_source = resolve_asset_source(video_path_obj, thumb, "thumb");
    let actual_fanart_source = resolve_asset_source(video_path_obj, fanart, "fanart");

    let poster_source = actual_poster_source
        .as_ref()
        .map(|path| remap_path_after_dir_rename(path, old_dir, &target_dir));
    let thumb_source = actual_thumb_source
        .as_ref()
        .map(|path| remap_path_after_dir_rename(path, old_dir, &target_dir));
    let fanart_source = actual_fanart_source
        .as_ref()
        .map(|path| remap_path_after_dir_rename(path, old_dir, &target_dir));

    let poster_target = poster_source
        .as_ref()
        .and_then(|source| build_artwork_target_path(source, &target_dir, &new_stem, "poster"));
    let thumb_target = thumb_source
        .as_ref()
        .and_then(|source| build_artwork_target_path(source, &target_dir, &new_stem, "thumb"));
    let fanart_target = fanart_source
        .as_ref()
        .and_then(|source| build_artwork_target_path(source, &target_dir, &new_stem, "fanart"));

    let mut operations = Vec::new();
    if rename_parent_dir && !is_same_path_for_fs(old_dir, &target_dir) {
        operations.push(PendingRenameOperation {
            source: old_dir.to_path_buf(),
            target: target_dir.clone(),
            is_dir: true,
        });
    }

    if !is_same_path_for_fs(&current_video_path, &new_video_path) {
        operations.push(PendingRenameOperation {
            source: current_video_path.clone(),
            target: new_video_path.clone(),
            is_dir: false,
        });
    }

    let actual_nfo_source = video_path_obj.with_extension("nfo");
    let current_nfo = if rename_parent_dir {
        remap_path_after_dir_rename(&actual_nfo_source, old_dir, &target_dir)
    } else {
        actual_nfo_source.clone()
    };
    let new_nfo = new_video_path.with_extension("nfo");
    if actual_nfo_source.exists() && !is_same_path_for_fs(&current_nfo, &new_nfo) {
        operations.push(PendingRenameOperation {
            source: current_nfo,
            target: new_nfo,
            is_dir: false,
        });
    }

    queue_optional_file_rename(&mut operations, poster_source.clone(), poster_target.clone());
    queue_optional_file_rename(&mut operations, thumb_source.clone(), thumb_target.clone());
    queue_optional_file_rename(&mut operations, fanart_source.clone(), fanart_target.clone());

    if !rename_parent_dir {
        let extrafanart_source = old_dir.join(EXTRAFANART_DIR_NAME);
        let extrafanart_target = target_dir.join(EXTRAFANART_DIR_NAME);
        if extrafanart_source.exists() && extrafanart_source.is_dir()
            && !is_same_path_for_fs(&extrafanart_source, &extrafanart_target)
        {
            operations.push(PendingRenameOperation {
                source: extrafanart_source,
                target: extrafanart_target,
                is_dir: true,
            });
        }
    }

    queue_matching_subtitle_renames(
        &mut operations,
        video_path_obj,
        old_dir,
        &target_dir,
        &new_stem,
        rename_parent_dir,
    );

    if operations.is_empty() {
        return Ok(None);
    }

    execute_rename_operations(&operations)?;

    Ok(Some(RelocatedVideoAssets {
        original_video_path: video_path.to_string(),
        video_path: new_video_path.to_string_lossy().to_string(),
        dir_path: target_dir.to_string_lossy().to_string(),
        // 未随视频搬动的图（如独立目录里的图）保留其原路径，避免被写库清空
        poster: poster_target
            .or(poster_source)
            .map(|path| path.to_string_lossy().to_string())
            .or_else(|| poster.map(|p| p.to_string())),
        thumb: thumb_target
            .or(thumb_source)
            .map(|path| path.to_string_lossy().to_string())
            .or_else(|| thumb.map(|p| p.to_string())),
        fanart: fanart_target
            .or(fanart_source)
            .map(|path| path.to_string_lossy().to_string())
            .or_else(|| fanart.map(|p| p.to_string())),
    }))
}

pub fn ensure_video_in_named_parent_dir(
    video_path: &str,
    poster: Option<&str>,
    thumb: Option<&str>,
    fanart: Option<&str>,
) -> Result<Option<RelocatedVideoAssets>, String> {
    let video_path_obj = Path::new(video_path);
    if has_same_named_parent_dir(video_path_obj) {
        return Ok(None);
    }

    let parent_dir = video_path_obj.parent().ok_or("无效的视频路径")?;
    let file_stem = video_path_obj
        .file_stem()
        .ok_or("无效的视频文件名")?
        .to_string_lossy()
        .to_string();
    let file_name = video_path_obj.file_name().ok_or("无效的视频文件名")?;

    let target_dir = parent_dir.join(&file_stem);
    fs::create_dir_all(&target_dir).map_err(|e| format!("创建同名目录失败: {}", e))?;

    let new_video_path = target_dir.join(file_name);
    if new_video_path.exists() {
        return Err(format!(
            "目标目录已存在同名视频文件: {}",
            new_video_path.display()
        ));
    }

    move_file(video_path_obj, &new_video_path).map_err(|e| format!("移动视频文件失败: {}", e))?;

    let current_nfo = video_path_obj.with_extension("nfo");
    if current_nfo.exists() {
        let new_nfo = new_video_path.with_extension("nfo");
        if let Err(error) = move_file(&current_nfo, &new_nfo) {
            log::error!(
                "[media_assets] event=move_nfo_failed source={} target={} error={}",
                current_nfo.display(),
                new_nfo.display(),
                error
            );
        }
    }

    let new_poster = move_optional_asset(
        resolve_asset_source(video_path_obj, poster, "poster"),
        &target_dir,
        "poster",
    );
    let new_thumb = move_optional_asset(
        resolve_asset_source(video_path_obj, thumb, "thumb"),
        &target_dir,
        "thumb",
    );
    let new_fanart = move_optional_asset(
        resolve_asset_source(video_path_obj, fanart, "fanart"),
        &target_dir,
        "fanart",
    );

    let extrafanart_dir = parent_dir.join(EXTRAFANART_DIR_NAME);
    if extrafanart_dir.exists() && extrafanart_dir.is_dir() {
        let target_extrafanart_dir = target_dir.join(EXTRAFANART_DIR_NAME);
        if let Err(error) = move_dir(&extrafanart_dir, &target_extrafanart_dir) {
            log::error!(
                "[media_assets] event=move_extrafanart_dir_failed source={} target={} error={}",
                extrafanart_dir.display(),
                target_extrafanart_dir.display(),
                error
            );
        }
    }

    move_matching_subtitle_files(video_path_obj, &target_dir);

    Ok(Some(RelocatedVideoAssets {
        original_video_path: video_path.to_string(),
        video_path: new_video_path.to_string_lossy().to_string(),
        dir_path: target_dir.to_string_lossy().to_string(),
        poster: new_poster,
        thumb: new_thumb,
        fanart: new_fanart,
    }))
}

/// 在指定资产目录下定位 extrafanart 子目录
pub fn extrafanart_dir_in(asset_dir: &Path) -> PathBuf {
    asset_dir.join(EXTRAFANART_DIR_NAME)
}

pub fn extrafanart_dir_for_video(video_path: &Path) -> Result<PathBuf, String> {
    let parent_dir = video_path.parent().ok_or("无效的视频路径")?;
    Ok(extrafanart_dir_in(parent_dir))
}

pub fn find_sibling_artwork(video_path: &Path, suffix: &str) -> Option<String> {
    let parent_dir = video_path.parent()?;
    let file_stem = video_path.file_stem()?.to_string_lossy();

    ["jpg", "jpeg", "png", "webp"]
        .iter()
        .map(|ext| parent_dir.join(format!("{}-{}.{}", file_stem, suffix, ext)))
        .find(|path| path.exists() && path.is_file())
        .map(|path| path.to_string_lossy().to_string())
}

fn is_supported_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp"))
        .unwrap_or(false)
}

fn parse_fanart_index(path: &Path) -> Option<usize> {
    let stem = path.file_stem()?.to_str()?;
    let suffix = stem.strip_prefix("fanart")?;
    suffix.parse::<usize>().ok()
}

pub fn collect_extrafanart_paths(video_path: &Path) -> Vec<(usize, String)> {
    let extrafanart_dir = match extrafanart_dir_for_video(video_path) {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    if !extrafanart_dir.exists() || !extrafanart_dir.is_dir() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    if let Ok(entries) = fs::read_dir(&extrafanart_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_supported_image_file(&path) {
                continue;
            }

            if let Some(index) = parse_fanart_index(&path) {
                paths.push((index, path.to_string_lossy().to_string()));
            }
        }
    }
    paths.sort_by_key(|(index, _)| *index);
    paths
}

pub fn next_extrafanart_index(video_path: &Path) -> usize {
    collect_extrafanart_paths(video_path)
        .into_iter()
        .map(|(index, _)| index)
        .max()
        .unwrap_or(0)
        + 1
}

pub async fn sync_extrafanart_from_urls(
    video_path: &str,
    images: Vec<(usize, String)>,
) -> Result<Vec<String>, String> {
    let video_parent = Path::new(video_path).parent().ok_or("无效的视频路径")?;
    sync_extrafanart_to_dir(video_parent, images).await
}

/// 下载预览图到 `<asset_dir>/extrafanart/`，文件名 `fanart{N}.jpg`，有界并发。
///
/// 供独立目录模式将预览图写入 `<root>/<番号 标题>/extrafanart/`。
pub async fn sync_extrafanart_to_dir(
    asset_dir: &Path,
    images: Vec<(usize, String)>,
) -> Result<Vec<String>, String> {
    if images.is_empty() {
        return Ok(Vec::new());
    }

    let extrafanart_dir = extrafanart_dir_in(asset_dir);
    fs::create_dir_all(&extrafanart_dir).map_err(|e| format!("创建 extrafanart 目录失败: {}", e))?;

    let client = std::sync::Arc::new(crate::resource_scrape::fingerprint_client::shared_client()?);
    // 有界并发下载预览图（原先串行逐张，10-20 张时是主要耗时）
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
    let mut handles = Vec::new();

    for (index, url) in images {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let save_path = extrafanart_dir.join(format!("fanart{}.jpg", index));
        let client = client.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            if save_path.exists() {
                return Some((index, save_path.to_string_lossy().to_string()));
            }
            let _permit = sem.acquire_owned().await.ok()?;
            match crate::download::image::download_image(&client, &trimmed, &save_path).await {
                Ok(path) => Some((index, path)),
                Err(e) => {
                    log::error!(
                        "[media_assets] event=download_extrafanart_failed index={} url={} error={}",
                        index, trimmed, e
                    );
                    None
                }
            }
        }));
    }

    // 收集结果并按 index 恢复顺序
    let mut indexed: Vec<(usize, String)> = Vec::new();
    for handle in handles {
        if let Ok(Some(pair)) = handle.await {
            indexed.push(pair);
        }
    }
    indexed.sort_by_key(|(i, _)| *i);
    let saved_paths = indexed.into_iter().map(|(_, p)| p).collect();

    Ok(saved_paths)
}

// ============================================================
// 封面图片
// ============================================================

/// 将截取的视频帧保存为封面图片
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `frame_path` - 截取的帧图片路径
///
/// # 返回
/// * `Ok(String)` - 保存的封面图片路径
/// * `Err(String)` - 保存失败的错误信息
pub fn save_frame_as_cover_assets(
    video_path: &str,
    frame_path: &str,
) -> Result<crate::media::artwork::ArtworkResult, String> {
    let video_path_obj = Path::new(video_path);
    let parent_dir = video_path_obj.parent().ok_or("无效的视频路径")?;
    let file_stem = video_path_obj
        .file_stem()
        .ok_or("无效的文件名")?
        .to_string_lossy()
        .to_string();

    // 截帧为横版 → fanart + thumb，并右裁出竖版 poster，产出标准图集
    let artwork = crate::media::artwork::produce_artwork_from_local_image(
        parent_dir,
        &file_stem,
        Path::new(frame_path),
    );
    if artwork.fanart.is_none() && artwork.poster.is_none() {
        return Err("保存封面失败".to_string());
    }
    Ok(artwork)
}

/// 将截取的多个视频帧保存到 extrafanart 目录
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `frame_paths` - 截取的帧图片路径列表
///
/// # 返回
/// * `Ok(Vec<String>)` - 保存的预览图路径列表
/// * `Err(String)` - 保存失败的错误信息
pub fn save_frames_to_extrafanart(
    video_path: &str,
    frame_paths: &[String],
) -> Result<Vec<String>, String> {
    let video_path_obj = Path::new(video_path);
    let extrafanart_dir = extrafanart_dir_for_video(video_path_obj)?;
    fs::create_dir_all(&extrafanart_dir).map_err(|e| format!("创建 extrafanart 目录失败: {}", e))?;

    let mut next_index = next_extrafanart_index(video_path_obj);
    let mut thumb_paths = Vec::new();

    for frame_path in frame_paths {
        let thumb_filename = format!("fanart{}.jpg", next_index);
        let thumb_path = extrafanart_dir.join(&thumb_filename);

        fs::copy(frame_path, &thumb_path)
            .map_err(|e| format!("保存预览图 {} 失败: {}", next_index, e))?;

        thumb_paths.push(thumb_path.to_string_lossy().to_string());
        next_index += 1;
    }

    Ok(thumb_paths)
}

// ============================================================
// 视频帧截取 (ffmpeg)
// ============================================================

/// 从视频中随机截取指定数量的帧
///
/// 将视频时长均匀分段，在每段内随机选择时间点，覆盖 0%~100% 范围。
/// 需要系统安装 ffmpeg。
///
/// # 参数
/// * `video_path` - 视频文件路径
/// * `count` - 要截取的帧数量
// 已抽离至 crate::media::ffmpeg

// ============================================================
// 文件回滚
// ============================================================

/// 回滚文件操作，删除已创建的文件
///
/// 当数据库操作失败时调用此函数，以确保文件系统和数据库之间的数据一致性
#[allow(dead_code)]
pub fn rollback_files(
    nfo_path: Option<&std::path::PathBuf>,
    cover_path: Option<&str>,
    thumbs_dir: Option<&std::path::PathBuf>,
) {
    if let Some(nfo) = nfo_path {
        if nfo.exists() {
            match fs::remove_file(nfo) {
                Ok(_) => log::info!("[media_assets] event=rollback_nfo_deleted path={}", nfo.display()),
                Err(e) => log::error!("[media_assets] event=rollback_nfo_delete_failed path={} error={}", nfo.display(), e),
            }
        }
    }

    if let Some(cover) = cover_path {
        if !cover.trim().is_empty() {
            let cover_path_obj = Path::new(cover);
            if cover_path_obj.exists() {
                match fs::remove_file(cover_path_obj) {
                    Ok(_) => log::info!("[media_assets] event=rollback_cover_deleted path={}", cover),
                    Err(e) => log::error!("[media_assets] event=rollback_cover_delete_failed path={} error={}", cover, e),
                }
            }
        }
    }

    if let Some(thumbs) = thumbs_dir {
        if thumbs.exists() {
            match fs::remove_dir_all(thumbs) {
                Ok(_) => log::info!("[media_assets] event=rollback_thumbs_deleted path={}", thumbs.display()),
                Err(e) => log::error!("[media_assets] event=rollback_thumbs_delete_failed path={} error={}", thumbs.display(), e),
            }
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_rollback_files_deletes_nfo() {
        let temp_dir = std::env::temp_dir();
        let nfo_path = temp_dir.join("test_video.nfo");

        let mut file = fs::File::create(&nfo_path).unwrap();
        file.write_all(b"test nfo content").unwrap();
        drop(file);

        assert!(nfo_path.exists());
        rollback_files(Some(&nfo_path), None, None);
        assert!(!nfo_path.exists());
    }

    #[test]
    fn test_rollback_files_deletes_cover() {
        let temp_dir = std::env::temp_dir();
        let cover_path = temp_dir.join("test_video-poster.jpg");

        let mut file = fs::File::create(&cover_path).unwrap();
        file.write_all(b"fake image data").unwrap();
        drop(file);

        assert!(cover_path.exists());
        let cover_str = cover_path.to_string_lossy().to_string();
        rollback_files(None, Some(&cover_str), None);
        assert!(!cover_path.exists());
    }

    #[test]
    fn test_rollback_files_deletes_thumbs_directory() {
        let temp_dir = std::env::temp_dir();
        let thumbs_dir = temp_dir.join("test_thumbs");
        fs::create_dir_all(&thumbs_dir).unwrap();

        for i in 1..=3 {
            let thumb_path = thumbs_dir.join(format!("thumb_{:03}.jpg", i));
            let mut file = fs::File::create(&thumb_path).unwrap();
            file.write_all(b"fake thumb data").unwrap();
        }

        assert!(thumbs_dir.exists());
        assert_eq!(fs::read_dir(&thumbs_dir).unwrap().count(), 3);

        rollback_files(None, None, Some(&thumbs_dir));
        assert!(!thumbs_dir.exists());
    }

    #[test]
    fn test_rollback_files_deletes_all() {
        let temp_dir = std::env::temp_dir();
        let nfo_path = temp_dir.join("test_all.nfo");
        let cover_path = temp_dir.join("test_all-poster.jpg");
        let thumbs_dir = temp_dir.join("test_all_thumbs");

        fs::File::create(&nfo_path)
            .unwrap()
            .write_all(b"nfo")
            .unwrap();
        fs::File::create(&cover_path)
            .unwrap()
            .write_all(b"cover")
            .unwrap();
        fs::create_dir_all(&thumbs_dir).unwrap();
        fs::File::create(thumbs_dir.join("thumb_001.jpg"))
            .unwrap()
            .write_all(b"thumb")
            .unwrap();

        assert!(nfo_path.exists());
        assert!(cover_path.exists());
        assert!(thumbs_dir.exists());

        let cover_str = cover_path.to_string_lossy().to_string();
        rollback_files(Some(&nfo_path), Some(&cover_str), Some(&thumbs_dir));

        assert!(!nfo_path.exists());
        assert!(!cover_path.exists());
        assert!(!thumbs_dir.exists());
    }

    #[test]
    fn test_rollback_files_handles_nonexistent_files() {
        let temp_dir = std::env::temp_dir();
        let nonexistent_nfo = temp_dir.join("nonexistent.nfo");
        let nonexistent_cover = temp_dir.join("nonexistent-poster.jpg");
        let nonexistent_thumbs = temp_dir.join("nonexistent_thumbs");

        assert!(!nonexistent_nfo.exists());
        assert!(!nonexistent_cover.exists());
        assert!(!nonexistent_thumbs.exists());

        let cover_str = nonexistent_cover.to_string_lossy().to_string();
        rollback_files(
            Some(&nonexistent_nfo),
            Some(&cover_str),
            Some(&nonexistent_thumbs),
        );

        assert!(!nonexistent_nfo.exists());
        assert!(!nonexistent_cover.exists());
        assert!(!nonexistent_thumbs.exists());
    }

    #[test]
    fn resolve_asset_target_follow_video_mode() {
        let cfg = MetadataStorageConfig { independent: false, root_dir: String::new() };
        let target = resolve_asset_target("/videos/ABC-123.mp4", "ABC-123", "标题", &cfg).unwrap();
        assert_eq!(target.dir, Path::new("/videos"));
        assert_eq!(target.stem, "ABC-123");
        assert!(target.strm.is_none());
    }

    #[test]
    fn resolve_asset_target_independent_mode() {
        let cfg = MetadataStorageConfig { independent: true, root_dir: "/meta".to_string() };
        let target = resolve_asset_target("/videos/raw_name.mp4", "ABC-123", "标题 X", &cfg).unwrap();
        assert_eq!(target.dir, Path::new("/meta").join("ABC-123 标题 X"));
        assert_eq!(target.stem, "ABC-123");
        let strm = target.strm.expect("独立模式应生成 .strm");
        assert_eq!(strm.path, Path::new("/meta").join("ABC-123 标题 X").join("ABC-123.strm"));
        assert_eq!(strm.video_abs_path, "/videos/raw_name.mp4");
    }

    #[test]
    fn resolve_asset_target_independent_falls_back_without_root_or_id() {
        // 根目录为空 → 回退跟随视频
        let cfg_no_root = MetadataStorageConfig { independent: true, root_dir: String::new() };
        let t1 = resolve_asset_target("/videos/ABC-123.mp4", "ABC-123", "标题", &cfg_no_root).unwrap();
        assert!(t1.strm.is_none());
        assert_eq!(t1.dir, Path::new("/videos"));

        // 番号为空 → 回退跟随视频
        let cfg = MetadataStorageConfig { independent: true, root_dir: "/meta".to_string() };
        let t2 = resolve_asset_target("/videos/ABC-123.mp4", "  ", "标题", &cfg).unwrap();
        assert!(t2.strm.is_none());
        assert_eq!(t2.stem, "ABC-123");
    }

    #[test]
    fn build_independent_folder_name_handles_empty_title() {
        assert_eq!(build_independent_folder_name("ABC-123", ""), "ABC-123");
        assert_eq!(build_independent_folder_name("ABC-123", "  "), "ABC-123");
        assert_eq!(build_independent_folder_name("ABC-123", "Hello"), "ABC-123 Hello");
    }

    #[test]
    fn sanitize_path_component_replaces_illegal_chars_and_folds_space() {
        assert_eq!(sanitize_path_component("a/b:c*d?", "fallback"), "a_b_c_d_");
        assert_eq!(sanitize_path_component("   ", "fallback"), "fallback");
        assert_eq!(sanitize_path_component("a   b", "fb"), "a b");
    }

    #[test]
    fn sync_independent_strm_rewrites_matching_strm() {
        let root = std::env::temp_dir().join(format!("javm-strm-test-{}", std::process::id()));
        let sub = root.join("ABC-123 标题文字");
        fs::create_dir_all(&sub).unwrap();
        let strm = sub.join("ABC-123.strm");
        fs::write(&strm, "D:\\old\\ABC-123.mp4").unwrap();

        let cfg = MetadataStorageConfig {
            independent: true,
            root_dir: root.to_string_lossy().to_string(),
        };
        sync_independent_strm(&cfg, "ABC-123", "E:\\new\\ABC-123.mp4").unwrap();
        assert_eq!(fs::read_to_string(&strm).unwrap().trim(), "E:\\new\\ABC-123.mp4");

        // 非独立模式：跳过，不改动
        let cfg_off = MetadataStorageConfig {
            independent: false,
            root_dir: root.to_string_lossy().to_string(),
        };
        sync_independent_strm(&cfg_off, "ABC-123", "X").unwrap();
        assert_eq!(fs::read_to_string(&strm).unwrap().trim(), "E:\\new\\ABC-123.mp4");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn save_nfo_to_independent_dir_writes_into_located_dir() {
        let root = std::env::temp_dir().join(format!("javm-indnfo-test-{}", std::process::id()));
        let sub = root.join("ABC-123 标题");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("ABC-123.strm"), "D:\\v\\ABC-123.mp4").unwrap();

        let cfg = MetadataStorageConfig {
            independent: true,
            root_dir: root.to_string_lossy().to_string(),
        };
        let meta = ScrapeMetadata {
            local_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            ..Default::default()
        };

        // 独立目录存在 → 写入 <番号>.nfo 并返回 true
        assert!(save_nfo_to_independent_dir(&cfg, "ABC-123", &meta).unwrap());
        assert!(sub.join("ABC-123.nfo").exists());

        // 非独立模式 → 返回 false，不写
        let cfg_off = MetadataStorageConfig {
            independent: false,
            root_dir: root.to_string_lossy().to_string(),
        };
        assert!(!save_nfo_to_independent_dir(&cfg_off, "ABC-123", &meta).unwrap());

        let _ = fs::remove_dir_all(&root);
    }
}
