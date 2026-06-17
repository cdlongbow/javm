//! 跨语言别名与实体规范化
//!
//! 女优/片商/标签常有中日英多种写法。本模块把同一实体的多语言名归并到同一
//! `entity_id`，使「输入任一语言都能定位实体、并展开出源偏好语言(日文)的名字去查」。
//!
//! ## 架构：证据 → 投影（可重建、可清洗）
//! - **`alias_evidence`（append-only 原始证据）**：每条 = 某源在某番号给出的某名字。**唯一真相源**。
//! - **`alias_overrides`（校正规则）**：`merge` 强制归并 / `block` 拉黑名字 / `canonical` 锁定展示名。
//!   种子表也以 `merge` 规则形式存在。实时关联与重建都尊重它，故修正不会被重刮覆盖。
//! - **`entity_aliases` + `designation_entities`（投影/缓存）**：由证据 + 规则**推导**而来，可随时
//!   [`rebuild`] 重算。清洗脏数据 = 删证据/源或加规则 → 重建，**合并因此可逆**。
//!
//! ## 关联策略（保守，避免误并）
//! - **片商**：每部影片唯一片商 → 同番号各源给的片商名永远可安全归并。
//! - **女优**：仅当该番号是**单人作**（各源报告的女优数 ≤ 1）才归并，多人作不归并以免错并合演者。

pub mod commands;
mod seed;

use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::Serialize;

pub use seed::import_seed_if_needed;

/// 实体类型常量
pub const ENTITY_ACTOR: &str = "actor";
pub const ENTITY_STUDIO: &str = "studio";
pub const ENTITY_TAG: &str = "tag";

/// 校正规则 kind
const KIND_MERGE: &str = "merge";
const KIND_BLOCK: &str = "block";
const KIND_CANONICAL: &str = "canonical";

/// 一条别名记录（下发前端/供调用方使用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasRow {
    pub name: String,
    pub lang: String,
    pub is_canonical: bool,
    pub source: Option<String>,
    pub confidence: f64,
}

// ==================== 文本处理 ====================

/// 归一化匹配键：全角→半角、小写、去除所有空白。
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let mapped = match ch {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
            _ => ch,
        };
        if mapped.is_whitespace() {
            continue;
        }
        out.extend(mapped.to_lowercase());
    }
    out
}

/// 粗略语言判断（仅作 `lang` 字段提示，排序/canonical 用 [`script_rank`]）。
pub fn detect_lang(name: &str) -> &'static str {
    let mut has_kana = false;
    let mut has_cjk = false;
    let mut has_ascii_alpha = false;
    for ch in name.chars() {
        let u = ch as u32;
        if (0x3040..=0x30FF).contains(&u) {
            has_kana = true;
        } else if (0x4E00..=0x9FFF).contains(&u) {
            has_cjk = true;
        } else if ch.is_ascii_alphabetic() {
            has_ascii_alpha = true;
        }
    }
    if has_kana {
        "ja"
    } else if has_cjk {
        "zh"
    } else if has_ascii_alpha {
        "en"
    } else {
        "unknown"
    }
}

/// 查询/canonical 偏好按**书写体系**排序（不靠不可靠的 ja/zh 判别）：
/// 含假名→0；含汉字→1；纯 ASCII(罗马音)→2。源偏好的日文/汉字名自然排在前。
fn script_rank(name: &str) -> u8 {
    let mut has_kana = false;
    let mut has_cjk = false;
    for ch in name.chars() {
        let u = ch as u32;
        if (0x3040..=0x30FF).contains(&u) {
            has_kana = true;
        } else if (0x4E00..=0x9FFF).contains(&u) {
            has_cjk = true;
        }
    }
    if has_kana {
        0
    } else if has_cjk {
        1
    } else {
        2
    }
}

// ==================== 投影簇底层原语 ====================

fn entity_id_for_norm(
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

fn no_rows_to_none<T>(e: rusqlite::Error) -> rusqlite::Result<Option<T>> {
    match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    }
}

/// 新建实体：插入首条别名，entity_id 置为该行自增 id（全局唯一，免并发分配碰撞）。
fn create_entity(
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

fn insert_alias(
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

fn merge_entities(
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
fn refresh_canonical(conn: &Connection, entity_type: &str, entity_id: i64) -> rusqlite::Result<()> {
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

fn upsert_designation_entity(
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

fn remove_designation_entity(
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
fn ensure_entity(
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
fn unify_names(
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

// ==================== 证据 / 校正规则 ====================

/// 追加一条原始证据（best-effort 的归一化空名跳过）。
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

fn blocked_norms(conn: &Connection, entity_type: &str) -> rusqlite::Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT name_norm FROM alias_overrides WHERE entity_type = ?1 AND kind = ?2",
    )?;
    let set = stmt
        .query_map(params![entity_type, KIND_BLOCK], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<_>>>()?;
    Ok(set)
}

fn canonical_norms(conn: &Connection, entity_type: &str) -> rusqlite::Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT name_norm FROM alias_overrides WHERE entity_type = ?1 AND kind = ?2",
    )?;
    let set = stmt
        .query_map(params![entity_type, KIND_CANONICAL], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<HashSet<_>>>()?;
    Ok(set)
}

/// 校正：拉黑一个名字（永不入簇）。返回后需 [`rebuild`] 使其对存量生效。
pub fn add_block(conn: &Connection, entity_type: &str, name: &str) -> rusqlite::Result<()> {
    let norm = normalize_name(name);
    if norm.is_empty() {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM alias_overrides WHERE kind = ?1 AND entity_type = ?2 AND name_norm = ?3",
        params![KIND_BLOCK, entity_type, norm],
    )?;
    conn.execute(
        "INSERT INTO alias_overrides (kind, entity_type, group_key, name, name_norm)
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![KIND_BLOCK, entity_type, name.trim(), norm],
    )?;
    Ok(())
}

/// 校正：锁定某名字为该实体展示名。
pub fn add_canonical(conn: &Connection, entity_type: &str, name: &str) -> rusqlite::Result<()> {
    let norm = normalize_name(name);
    if norm.is_empty() {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM alias_overrides WHERE kind = ?1 AND entity_type = ?2 AND name_norm = ?3",
        params![KIND_CANONICAL, entity_type, norm],
    )?;
    conn.execute(
        "INSERT INTO alias_overrides (kind, entity_type, group_key, name, name_norm)
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![KIND_CANONICAL, entity_type, name.trim(), norm],
    )?;
    Ok(())
}

/// 校正：强制把一组名字归并为同一实体（自动关联没认出的等价名时用）。
pub fn add_force_merge(
    conn: &Connection,
    entity_type: &str,
    names: &[String],
) -> rusqlite::Result<()> {
    let valid: Vec<&String> = names
        .iter()
        .filter(|n| !normalize_name(n).is_empty())
        .collect();
    if valid.len() < 2 {
        return Ok(());
    }
    // group_key 用首名归一化值，稳定且便于去重
    let group_key = format!("manual:{}", normalize_name(valid[0]));
    for name in valid {
        let norm = normalize_name(name);
        conn.execute(
            "INSERT INTO alias_overrides (kind, entity_type, group_key, name, name_norm)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![KIND_MERGE, entity_type, group_key, name.trim(), norm],
        )?;
    }
    Ok(())
}

/// 立即把一组名字归并到投影簇（种子导入用，避免等到下次 rebuild 才生效）。
pub fn apply_force_merge_group(
    conn: &Connection,
    entity_type: &str,
    names: &[String],
) -> rusqlite::Result<()> {
    let blocked = blocked_norms(conn, entity_type)?;
    let refs: Vec<&str> = names
        .iter()
        .filter(|n| !blocked.contains(&normalize_name(n)))
        .map(|n| n.as_str())
        .collect();
    if !refs.is_empty() {
        unify_names(conn, entity_type, &refs, "seed")?;
    }
    Ok(())
}

/// 读取所有 merge 规则组（用于 rebuild）。返回每组的 (name, name_norm) 列表。
fn merge_groups(
    conn: &Connection,
    entity_type: &str,
) -> rusqlite::Result<Vec<Vec<(String, String)>>> {
    let mut stmt = conn.prepare(
        "SELECT group_key, name, name_norm FROM alias_overrides
         WHERE entity_type = ?1 AND kind = ?2 ORDER BY group_key, id",
    )?;
    let rows = stmt
        .query_map(params![entity_type, KIND_MERGE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut groups: Vec<Vec<(String, String)>> = Vec::new();
    let mut cur_key: Option<String> = None;
    for (key, name, norm) in rows {
        if cur_key.as_deref() != Some(key.as_str()) {
            groups.push(Vec::new());
            cur_key = Some(key);
        }
        groups.last_mut().unwrap().push((name, norm));
    }
    Ok(groups)
}

// ==================== 投影构建：实时关联 + 重建 ====================

/// 取某番号某类型在证据中的去重名字（按 norm 去重，保留一个原名），并剔除被 block 的。
fn evidence_names(
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
fn max_source_count(
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

    let mut per_source: std::collections::HashMap<String, HashSet<String>> = Default::default();
    for (source, norm) in rows {
        if blocked.contains(&norm) {
            continue;
        }
        per_source.entry(source).or_default().insert(norm);
    }
    Ok(per_source.values().map(|set| set.len()).max().unwrap_or(0))
}

/// 把某番号的证据投影到簇（实时 + 重建共用）：片商总是归并；女优仅单人作归并。
pub fn apply_designation(conn: &Connection, designation: &str) -> rusqlite::Result<()> {
    let designation = designation.trim();
    if designation.is_empty() {
        return Ok(());
    }
    apply_designation_type(conn, designation, ENTITY_STUDIO)?;
    apply_designation_type(conn, designation, ENTITY_ACTOR)?;
    Ok(())
}

fn apply_designation_type(
    conn: &Connection,
    designation: &str,
    entity_type: &str,
) -> rusqlite::Result<()> {
    let blocked = blocked_norms(conn, entity_type)?;
    let names = evidence_names(conn, designation, entity_type, &blocked)?;
    if names.is_empty() {
        remove_designation_entity(conn, designation, entity_type)?;
        return Ok(());
    }

    // 片商总是归并；女优仅单人作（各源女优数 ≤ 1）才归并
    let should_union = if entity_type == ENTITY_ACTOR {
        max_source_count(conn, designation, entity_type, &blocked)? <= 1
    } else {
        true
    };

    if should_union {
        let refs: Vec<&str> = names.iter().map(|(name, _)| name.as_str()).collect();
        if let Some(eid) = unify_names(conn, entity_type, &refs, "scrape")? {
            upsert_designation_entity(conn, designation, entity_type, eid)?;
        }
    } else {
        // 多人作：每个名字仍作为可检索的单实体存在，但不归并、不绑番号
        for (name, norm) in &names {
            ensure_entity(conn, entity_type, name, norm, "scrape")?;
        }
        remove_designation_entity(conn, designation, entity_type)?;
    }
    Ok(())
}

/// 从证据 + 校正规则**整体重建**所有别名簇（清洗脏数据后调用）。
/// 先清空投影，应用 merge 规则（含种子），再按番号重放全部证据——与实时关联同一套规则，
/// 故重建是权威结果，能抹掉实时增量在边界情形下的过度合并。
pub fn rebuild(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entity_aliases", [])?;
    conn.execute("DELETE FROM designation_entities", [])?;

    for entity_type in [ENTITY_STUDIO, ENTITY_TAG, ENTITY_ACTOR] {
        let blocked = blocked_norms(conn, entity_type)?;
        for group in merge_groups(conn, entity_type)? {
            let refs: Vec<&str> = group
                .iter()
                .filter(|(_, norm)| !blocked.contains(norm))
                .map(|(name, _)| name.as_str())
                .collect();
            if !refs.is_empty() {
                unify_names(conn, entity_type, &refs, "override")?;
            }
        }
    }

    let designations: Vec<String> = {
        let mut stmt = conn.prepare("SELECT DISTINCT designation FROM alias_evidence")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for designation in designations {
        apply_designation(conn, &designation)?;
    }
    Ok(())
}

/// 删除某数据源贡献的全部证据（「某网站弄错了」时清洗它的脏数据）。返回删除行数。
/// 调用方应随后 [`rebuild`]。
pub fn purge_source(conn: &Connection, source: &str) -> rusqlite::Result<usize> {
    let n = conn.execute(
        "DELETE FROM alias_evidence WHERE source = ?1",
        params![source],
    )?;
    Ok(n)
}

// ==================== 读 API ====================

/// 解析名字到实体 id（命中任一别名即定位）。
pub fn resolve_entity(
    conn: &Connection,
    entity_type: &str,
    name: &str,
) -> rusqlite::Result<Option<i64>> {
    let norm = normalize_name(name);
    if norm.is_empty() {
        return Ok(None);
    }
    entity_id_for_norm(conn, entity_type, &norm)
}

/// 展开：返回 `name` 所属实体的全部别名，按查询偏好排序（日文/汉字名优先，canonical 居前）。
pub fn expand(
    conn: &Connection,
    entity_type: &str,
    name: &str,
) -> rusqlite::Result<Vec<AliasRow>> {
    let Some(eid) = resolve_entity(conn, entity_type, name)? else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT name, lang, is_canonical, source, confidence
         FROM entity_aliases WHERE entity_type = ?1 AND entity_id = ?2",
    )?;
    let mut rows = stmt
        .query_map(params![entity_type, eid], |row| {
            Ok(AliasRow {
                name: row.get(0)?,
                lang: row.get(1)?,
                is_canonical: row.get::<_, i64>(2)? != 0,
                source: row.get(3)?,
                confidence: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    rows.sort_by(|a, b| {
        script_rank(&a.name)
            .cmp(&script_rank(&b.name))
            .then(b.is_canonical.cmp(&a.is_canonical))
            .then(a.name.cmp(&b.name))
    });
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
    let Some(eid) = resolve_entity(conn, entity_type, name)? else {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE entity_aliases (
                id INTEGER PRIMARY KEY AUTOINCREMENT, entity_type TEXT NOT NULL,
                entity_id INTEGER NOT NULL, name TEXT NOT NULL, name_norm TEXT NOT NULL,
                lang TEXT NOT NULL DEFAULT 'unknown', is_canonical INTEGER NOT NULL DEFAULT 0,
                source TEXT, confidence REAL NOT NULL DEFAULT 1.0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP, UNIQUE(entity_type, name_norm));
            CREATE TABLE designation_entities (
                designation TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id INTEGER NOT NULL,
                PRIMARY KEY (designation, entity_type));
            CREATE TABLE alias_evidence (
                id INTEGER PRIMARY KEY AUTOINCREMENT, designation TEXT NOT NULL,
                entity_type TEXT NOT NULL, name TEXT NOT NULL, name_norm TEXT NOT NULL,
                source TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(designation, entity_type, name_norm, source));
            CREATE TABLE alias_overrides (
                id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, entity_type TEXT NOT NULL,
                group_key TEXT, name TEXT NOT NULL, name_norm TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP);",
        )
        .unwrap();
        conn
    }

    /// 模拟一次搜索：记录各源证据后应用关联
    fn scrape(conn: &Connection, designation: &str, per_source: &[(&str, &[&str], &[&str])]) {
        for (source, studios, actors) in per_source {
            for s in *studios {
                record_evidence(conn, designation, ENTITY_STUDIO, s, source).unwrap();
            }
            for a in *actors {
                record_evidence(conn, designation, ENTITY_ACTOR, a, source).unwrap();
            }
        }
        apply_designation(conn, designation).unwrap();
    }

    #[test]
    fn single_actress_links_cross_language() {
        let conn = mem();
        scrape(
            &conn,
            "SSIS-001",
            &[
                ("srcJa", &["エスワン"], &["三上悠亜"]),
                ("srcEn", &["S1"], &["Yua Mikami"]),
                ("srcZh", &["S1"], &["三上悠亚"]),
            ],
        );
        let aliases = expand(&conn, ENTITY_ACTOR, "三上悠亚").unwrap();
        assert_eq!(aliases.len(), 3);
        // 可靠保证：汉字名(日文/中文)排在罗马音之前，供 JAV 源查询；两个汉字变体谁先不强求
        // （亜/亚 同为 CJK，无法可靠区分日文汉字与简中，探索会把 query_names 都试一遍）。
        assert_eq!(aliases[2].name, "Yua Mikami");
        assert!(aliases[0].name == "三上悠亜" || aliases[0].name == "三上悠亚");
        // 片商也归并
        assert_eq!(expand(&conn, ENTITY_STUDIO, "S1").unwrap().len(), 2);
    }

    #[test]
    fn multi_actress_not_merged_but_resolvable() {
        let conn = mem();
        scrape(
            &conn,
            "SSNI-888",
            &[
                ("srcJa", &["エスワン"], &["三上悠亜", "葵つかさ"]),
                ("srcEn", &["S1"], &["Yua Mikami", "Tsukasa Aoi"]),
            ],
        );
        // 多人作：女优名各自可检索，但不跨语言归并
        assert_eq!(expand(&conn, ENTITY_ACTOR, "三上悠亜").unwrap().len(), 1);
        assert!(resolve_entity(&conn, ENTITY_ACTOR, "Yua Mikami").unwrap().is_some());
        assert_ne!(
            resolve_entity(&conn, ENTITY_ACTOR, "三上悠亜").unwrap(),
            resolve_entity(&conn, ENTITY_ACTOR, "Yua Mikami").unwrap()
        );
        // 片商不受多人作影响，照常归并
        assert_eq!(expand(&conn, ENTITY_STUDIO, "S1").unwrap().len(), 2);
    }

    #[test]
    fn purge_bad_source_then_rebuild_cleans_wrong_merge() {
        let conn = mem();
        // 两部单人片各自正确
        scrape(&conn, "AAA-1", &[("good", &[], &["三上悠亜"])]);
        scrape(&conn, "BBB-2", &[("good", &[], &["葵つかさ"])]);
        assert_ne!(
            resolve_entity(&conn, ENTITY_ACTOR, "三上悠亜").unwrap(),
            resolve_entity(&conn, ENTITY_ACTOR, "葵つかさ").unwrap()
        );
        // 坏源在 CCC-3 把两人当成同一人的别名（单人作误报）→ 错误合并
        scrape(&conn, "CCC-3", &[("bad", &[], &["三上悠亜"])]);
        scrape(&conn, "CCC-3", &[("bad", &[], &["葵つかさ"])]);
        // 注：bad 在 CCC-3 报了 2 个名 → 实为多人作判定，不会合并；构造真正的误并需单人作误报。
        // 这里改为：bad 在两条不同番号把同一对名字分别绑定，制造跨簇 merge。
        scrape(&conn, "DDD-4", &[("bad", &[], &["三上悠亜"])]);
        scrape(&conn, "DDD-4", &[("bad2", &[], &["葵つかさ"])]);
        // DDD-4 两源各报 1 人 → 单人作 → 误并三上悠亜与葵つかさ
        assert_eq!(
            resolve_entity(&conn, ENTITY_ACTOR, "三上悠亜").unwrap(),
            resolve_entity(&conn, ENTITY_ACTOR, "葵つかさ").unwrap(),
            "构造的误并应已发生"
        );
        // 清洗：删掉坏源证据 → 重建 → 误并解开
        purge_source(&conn, "bad").unwrap();
        purge_source(&conn, "bad2").unwrap();
        rebuild(&conn).unwrap();
        assert_ne!(
            resolve_entity(&conn, ENTITY_ACTOR, "三上悠亜").unwrap(),
            resolve_entity(&conn, ENTITY_ACTOR, "葵つかさ").unwrap(),
            "删坏源 + 重建后应恢复为两个实体"
        );
    }

    #[test]
    fn block_survives_rescrape() {
        let conn = mem();
        scrape(&conn, "AAA-1", &[("s", &["广告垃圾名"], &[])]);
        add_block(&conn, ENTITY_STUDIO, "广告垃圾名").unwrap();
        rebuild(&conn).unwrap();
        assert!(resolve_entity(&conn, ENTITY_STUDIO, "广告垃圾名").unwrap().is_none());
        // 重刮（再次记录同名证据）也不应复活
        scrape(&conn, "AAA-1", &[("s", &["广告垃圾名"], &[])]);
        assert!(resolve_entity(&conn, ENTITY_STUDIO, "广告垃圾名").unwrap().is_none());
    }

    #[test]
    fn force_merge_links_unrelated_writings() {
        let conn = mem();
        scrape(&conn, "AAA-1", &[("s", &["IdeaPocket"], &[])]);
        scrape(&conn, "BBB-2", &[("s", &["アイデアポケット"], &[])]);
        assert_ne!(
            resolve_entity(&conn, ENTITY_STUDIO, "IdeaPocket").unwrap(),
            resolve_entity(&conn, ENTITY_STUDIO, "アイデアポケット").unwrap()
        );
        add_force_merge(
            &conn,
            ENTITY_STUDIO,
            &["IdeaPocket".into(), "アイデアポケット".into()],
        )
        .unwrap();
        rebuild(&conn).unwrap();
        assert_eq!(
            resolve_entity(&conn, ENTITY_STUDIO, "IdeaPocket").unwrap(),
            resolve_entity(&conn, ENTITY_STUDIO, "アイデアポケット").unwrap()
        );
    }

    #[test]
    fn rebuild_corrects_overeager_live_merge() {
        let conn = mem();
        // 第一次只看到 1 个女优（实时误判为单人作 → 绑定）
        scrape(&conn, "SSNI-1", &[("srcA", &[], &["三上悠亜"])]);
        assert!(resolve_entity(&conn, ENTITY_ACTOR, "三上悠亜").unwrap().is_some());
        // 后续证据显示其实是双人作
        record_evidence(&conn, "SSNI-1", ENTITY_ACTOR, "葵つかさ", "srcA").unwrap();
        // 重建：用全部证据判定为多人作 → 不应有番号→女优绑定
        rebuild(&conn).unwrap();
        let bound: Option<i64> = designation_entity(&conn, "SSNI-1", ENTITY_ACTOR).unwrap();
        assert!(bound.is_none(), "重建后多人作不应绑定单一女优实体");
    }

    #[test]
    fn canonical_pin_overrides_script_rank() {
        let conn = mem();
        scrape(&conn, "AAA-1", &[("ja", &["エスワン"], &[]), ("en", &["S1"], &[])]);
        // 默认 canonical 是日文「エスワン」(script 0)
        assert_eq!(expand(&conn, ENTITY_STUDIO, "S1").unwrap()[0].name, "エスワン");
        add_canonical(&conn, ENTITY_STUDIO, "S1").unwrap();
        rebuild(&conn).unwrap();
        let aliases = expand(&conn, ENTITY_STUDIO, "エスワン").unwrap();
        let canonical = aliases.iter().find(|a| a.is_canonical).unwrap();
        assert_eq!(canonical.name, "S1");
    }
}
