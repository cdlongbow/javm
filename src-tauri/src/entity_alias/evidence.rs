//! 原始证据（`alias_evidence`）读写 + 实体证据反查。
//!
//! 证据是别名簇的唯一真相源：每条 = 某源在某番号给出的某名字。投影由它推导，可重建。

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;

use super::text::normalize_name;

/// 追加一条原始证据（归一化空名跳过）。
pub fn record_evidence(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
    name: &str,
    source: &str,
) -> rusqlite::Result<()> {
    let designation = designation.trim();
    let trimmed = name.trim();
    let norm = normalize_name(trimmed);
    if designation.is_empty() || norm.is_empty() {
        return Ok(());
    }
    conn.execute(
        "INSERT OR IGNORE INTO alias_evidence
            (designation, entity_type, name, name_norm, source)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![designation, entity_type, trimmed, norm, source],
    )?;
    Ok(())
}

/// 取某番号某类型在证据中的去重名字（按 norm 去重，保留一个原名），并剔除被 block 的。
pub(super) fn evidence_names(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
    blocked: &HashSet<String>,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT name, name_norm FROM alias_evidence
         WHERE designation = ?1 AND entity_type = ?2 GROUP BY name_norm",
    )?;
    let rows = stmt
        .query_map(params![designation, entity_type], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, norm)| !blocked.contains(norm))
        .collect())
}

/// 各源在该番号报告的（去 block 后）女优数的最大值，用于单人作判定。
pub(super) fn max_source_count(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
    blocked: &HashSet<String>,
) -> rusqlite::Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT source, name_norm FROM alias_evidence
         WHERE designation = ?1 AND entity_type = ?2",
    )?;
    let rows = stmt
        .query_map(params![designation, entity_type], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut per_source: HashMap<String, HashSet<String>> = HashMap::new();
    for (source, norm) in rows {
        if blocked.contains(&norm) {
            continue;
        }
        per_source.entry(source).or_default().insert(norm);
    }
    Ok(per_source.values().map(|set| set.len()).max().unwrap_or(0))
}

/// 删除某数据源贡献的全部证据（「某网站弄错了」时清洗它的脏数据）。返回删除行数。
/// 调用方应随后 [`super::rebuild`]。
pub fn purge_source(conn: &Connection, source: &str) -> rusqlite::Result<usize> {
    let n = conn.execute(
        "DELETE FROM alias_evidence WHERE source = ?1",
        params![source],
    )?;
    Ok(n)
}

/// 证据中出现过的全部（去重）番号——供重建遍历。
pub(super) fn all_designations(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT designation FROM alias_evidence")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 番号证据明细（供清洗 UI 查看某实体背后的来源，决定要清掉谁）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRow {
    pub designation: String,
    pub name: String,
    pub source: String,
}

/// 列出某实体相关的全部原始证据（按其全部别名名字反查证据）。
pub fn evidence_for_entity(
    conn: &Connection,
    entity_type: &str,
    name: &str,
) -> rusqlite::Result<Vec<EvidenceRow>> {
    let Some(eid) = super::resolve_entity(conn, entity_type, name)? else {
        return Ok(Vec::new());
    };
    // 该实体的全部归一名
    let norms: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT name_norm FROM entity_aliases WHERE entity_type = ?1 AND entity_id = ?2",
        )?;
        let rows = stmt
            .query_map(params![entity_type, eid], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let mut out = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT designation, name, source FROM alias_evidence
         WHERE entity_type = ?1 AND name_norm = ?2",
    )?;
    for norm in norms {
        let rows = stmt
            .query_map(params![entity_type, norm], |row| {
                Ok(EvidenceRow {
                    designation: row.get(0)?,
                    name: row.get(1)?,
                    source: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        out.extend(rows);
    }
    Ok(out)
}
