//! MetaTube sidecar Tauri 命令：状态查询与手动重启。

use tauri::{AppHandle, Manager};

use super::types::MetaTubeStatusSnapshot;
use super::MetaTubeManager;

/// 查询 sidecar 状态（前端展示 + 判断是否就绪）。
#[tauri::command]
pub async fn metatube_status(app: AppHandle) -> Result<MetaTubeStatusSnapshot, String> {
    let manager = app
        .try_state::<MetaTubeManager>()
        .ok_or_else(|| "MetaTube 管理器未初始化".to_string())?;
    Ok(manager.snapshot())
}

/// 手动重启 sidecar（启动失败/放弃后重试）。
#[tauri::command]
pub async fn metatube_restart(app: AppHandle) -> Result<MetaTubeStatusSnapshot, String> {
    let manager = app
        .try_state::<MetaTubeManager>()
        .ok_or_else(|| "MetaTube 管理器未初始化".to_string())?;
    manager.restart();
    Ok(manager.snapshot())
}
