//! 投影簇底层原语：对 `entity_aliases` / `designation_entities` 两张投影表的读写。
//!
//! 这些是别名簇的「数据访问层」，上层 [`super`] 的 apply/rebuild 据此构建投影。

use rusqlite::{params, Connection};

use super::overrides::canonical_norms;
use super::text::{detect_lang, normalize_name, script_rank};

pub(super) fn no_rows_to_none<T>(e: rusqlite::Error) -> rusqlite::Result<Option<T>> {
    match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    }
}

pub(super) fn entity_id_for_norm(
    conn: &Connection,
    entity_type: &str,
    name_norm: &str,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT entity_id FROM entity_aliases WHERE entity_type = ?1 AND name_norm = ?2",
        params![entity_type, name_norm],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(no_rows_to_none)
}

/// 查某番号绑定的实体 id（studio 必有；actor 仅单人作有）。供探索/演员模块按番号定位实体。
pub fn designation_entity(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT entity_id FROM designation_entities WHERE designation = ?1 AND entity_type = ?2",
        params![designation, entity_type],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(no_rows_to_none)
}

/// 新建实体：插入首条别名，entity_id 置为该行自增 id（全局唯一，免并发分配碰撞）。
pub(super) fn create_entity(
    conn: &Connection,
    entity_type: &str,
    name: &str,
    name_norm: &str,
    source: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entity_aliases
            (entity_type, entity_id, name, name_norm, lang, is_canonical, source, confidence)
         VALUES (?1, 0, ?2, ?3, ?4, 1, ?5, 1.0)",
        params![entity_type, name, name_norm, detect_lang(name), source],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE entity_aliases SET entity_id = ?1 WHERE id = ?1",
        params![id],
    )?;
    Ok(id)
}

pub(super) fn insert_alias(
    conn: &Connection,
    entity_type: &str,
    entity_id: i64,
    name: &str,
    name_norm: &str,
    source: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO entity_aliases
            (entity_type, entity_id, name, name_norm, lang, is_canonical, source, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, 1.0)",
        params![entity_type, entity_id, name, name_norm, detect_lang(name), source],
    )?;
    Ok(())
}

pub(super) fn merge_entities(
    conn: &Connection,
    entity_type: &str,
    keep: i64,
    from: i64,
) -> rusqlite::Result<()> {
    if keep == from {
        return Ok(());
    }
    conn.execute(
        "UPDATE entity_aliases SET entity_id = ?1 WHERE entity_type = ?2 AND entity_id = ?3",
        params![keep, entity_type, from],
    )?;
    conn.execute(
        "UPDATE designation_entities SET entity_id = ?1 WHERE entity_type = ?2 AND entity_id = ?3",
        params![keep, entity_type, from],
    )?;
    Ok(())
}

/// 重选 canonical：优先采用 `canonical` 校正规则锁定的名字；否则按 script_rank（假名>汉字>罗马音）。
pub(super) fn refresh_canonical(
    conn: &Connection,
    entity_type: &str,
    entity_id: i64,
) -> rusqlite::Result<()> {
    let pinned = canonical_norms(conn, entity_type)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, name_norm FROM entity_aliases WHERE entity_type = ?1 AND entity_id = ?2",
    )?;
    let rows = stmt
        .query_map(params![entity_type, entity_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // 锁定优先：rank 0 给被 pin 的名字
    let best = rows
        .iter()
        .min_by_key(|(id, name, norm)| {
            let pin_rank = if pinned.contains(norm) { 0 } else { 1 };
            (pin_rank, script_rank(name), *id)
        })
        .map(|(id, _, _)| *id);

    conn.execute(
        "UPDATE entity_aliases SET is_canonical = 0 WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id],
    )?;
    if let Some(best_id) = best {
        conn.execute(
            "UPDATE entity_aliases SET is_canonical = 1 WHERE id = ?1",
            params![best_id],
        )?;
    }
    Ok(())
}

pub(super) fn upsert_designation_entity(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
    entity_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO designation_entities (designation, entity_type, entity_id)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(designation, entity_type) DO UPDATE SET entity_id = excluded.entity_id",
        params![designation, entity_type, entity_id],
    )?;
    Ok(())
}

pub(super) fn remove_designation_entity(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM designation_entities WHERE designation = ?1 AND entity_type = ?2",
        params![designation, entity_type],
    )?;
    Ok(())
}

/// 确保名字至少作为单实体存在（多人作的女优名：可被 resolve，但不归并、不绑番号）。
pub(super) fn ensure_entity(
    conn: &Connection,
    entity_type: &str,
    name: &str,
    name_norm: &str,
    source: &str,
) -> rusqlite::Result<i64> {
    match entity_id_for_norm(conn, entity_type, name_norm)? {
        Some(eid) => Ok(eid),
        None => create_entity(conn, entity_type, name, name_norm, source),
    }
}

/// 把一组名字归并到同一实体，返回 entity_id。空名跳过。
pub(super) fn unify_names(
    conn: &Connection,
    entity_type: &str,
    names: &[&str],
    source: &str,
) -> rusqlite::Result<Option<i64>> {
    let mut target: Option<i64> = None;
    for name in names {
        let trimmed = name.trim();
        let norm = normalize_name(trimmed);
        if norm.is_empty() {
            continue;
        }
        let existing = entity_id_for_norm(conn, entity_type, &norm)?;
        match (target, existing) {
            (None, Some(e)) => target = Some(e),
            (None, None) => target = Some(create_entity(conn, entity_type, trimmed, &norm, source)?),
            (Some(t), None) => insert_alias(conn, entity_type, t, trimmed, &norm, source)?,
            (Some(t), Some(e)) if e == t => {}
            (Some(t), Some(e)) => merge_entities(conn, entity_type, t, e)?,
        }
    }
    if let Some(t) = target {
        refresh_canonical(conn, entity_type, t)?;
    }
    Ok(target)
}
