//! 跨语言别名 Tauri 命令
//!
//! - 读：`entity_alias_expand`（中文输入→展开日文名查源）、`entity_alias_inspect`（查实体来源证据）。
//! - 洗：`entity_alias_purge_source`（删某网站脏数据）、`entity_alias_rebuild`（整体重建）、
//!   `entity_alias_block` / `entity_alias_force_merge` / `entity_alias_pin_canonical`（人工校正）。
//!   所有写操作内部都会重建投影簇，保证修正立即生效且重刮不复活。

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::{AliasRow, EvidenceRow};

fn db_path(app: &AppHandle) -> PathBuf {
    app.state::<crate::db::Database>()
        .get_database_path()
        .clone()
}

/// 在阻塞线程里打开连接执行 DB 闭包
async fn with_conn<T, F>(app: AppHandle, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
{
    let path = db_path(&app);
    tokio::task::spawn_blocking(move || {
        let conn =
            rusqlite::Connection::open(&path).map_err(|e| format!("打开数据库失败: {}", e))?;
        f(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// ==================== 读 ====================

/// 别名展开结果
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasExpansion {
    pub entity_id: Option<i64>,
    pub aliases: Vec<AliasRow>,
    /// 仅名字列表，按查询偏好排序（日文优先），探索可直接逐个用于查源
    pub query_names: Vec<String>,
}

/// 展开某名字所属实体的全部别名（探索/演员模块用）。
#[tauri::command]
pub async fn entity_alias_expand(
    app: AppHandle,
    entity_type: String,
    name: String,
) -> Result<AliasExpansion, String> {
    with_conn(app, move |conn| {
        let entity_id = super::resolve_entity(conn, &entity_type, &name)?;
        let aliases = super::expand(conn, &entity_type, &name)?;
        let query_names = aliases.iter().map(|a| a.name.clone()).collect();
        Ok(AliasExpansion {
            entity_id,
            aliases,
            query_names,
        })
    })
    .await
}

/// 列出某类型下所有「多名字实体簇」：前端列表据此把同一实体的多个名字合并为一条、显示主名。
#[tauri::command]
pub async fn entity_alias_clusters(
    app: AppHandle,
    entity_type: String,
) -> Result<Vec<super::AliasCluster>, String> {
    with_conn(app, move |conn| super::clusters(conn, &entity_type)).await
}

/// 查看某实体背后的原始证据（清洗 UI 据此判断要清掉哪个源/番号）。
#[tauri::command]
pub async fn entity_alias_inspect(
    app: AppHandle,
    entity_type: String,
    name: String,
) -> Result<Vec<EvidenceRow>, String> {
    with_conn(app, move |conn| {
        super::evidence_for_entity(conn, &entity_type, &name)
    })
    .await
}

// ==================== 洗 / 校正（均内置重建） ====================

/// 删除某数据源贡献的全部别名证据并重建（「某网站弄错了」时用）。返回删除证据条数。
#[tauri::command]
pub async fn entity_alias_purge_source(app: AppHandle, source: String) -> Result<usize, String> {
    with_conn(app, move |conn| {
        let n = super::purge_source(conn, &source)?;
        super::rebuild(conn)?;
        Ok(n)
    })
    .await
}

/// 从证据 + 规则整体重建别名簇。
#[tauri::command]
pub async fn entity_alias_rebuild(app: AppHandle) -> Result<(), String> {
    with_conn(app, move |conn| super::rebuild(conn)).await
}

/// 拉黑一个名字（永不入簇，重刮也不复活）并重建。
#[tauri::command]
pub async fn entity_alias_block(
    app: AppHandle,
    entity_type: String,
    name: String,
) -> Result<(), String> {
    with_conn(app, move |conn| {
        super::add_block(conn, &entity_type, &name)?;
        super::rebuild(conn)
    })
    .await
}

/// 强制把一组名字归并为同一实体并重建。
#[tauri::command]
pub async fn entity_alias_force_merge(
    app: AppHandle,
    entity_type: String,
    names: Vec<String>,
) -> Result<(), String> {
    with_conn(app, move |conn| {
        super::add_force_merge(conn, &entity_type, &names)?;
        super::rebuild(conn)
    })
    .await
}

/// 锁定某名字为该实体展示名并重建。
#[tauri::command]
pub async fn entity_alias_pin_canonical(
    app: AppHandle,
    entity_type: String,
    name: String,
) -> Result<(), String> {
    with_conn(app, move |conn| {
        super::add_canonical(conn, &entity_type, &name)?;
        super::rebuild(conn)
    })
    .await
}
