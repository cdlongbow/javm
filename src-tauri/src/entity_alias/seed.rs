//! 别名种子表
//!
//! 同番号关联是主力（随刮削自动积累），种子表只做**冷启动补充**：导入一批众所周知、
//! 稳定的等价写法（片商日英、标签中日英同义词）。
//!
//! 种子以 `merge` **校正规则**形式落库（与人工强制合并同一机制），因此 [`super::rebuild`]
//! 会自动把它纳入重建，清洗/重建后种子不会丢失。导入按版本号幂等。

use rusqlite::{params, Connection};

use super::{add_force_merge, apply_force_merge_group, ENTITY_STUDIO, ENTITY_TAG};

/// 种子版本：新增/调整种子内容时 +1，触发已安装用户重新导入。
const SEED_VERSION: &str = "1";
const SEED_VERSION_KEY: &str = "entity_alias_seed_version";

/// 片商等价写法（日英多写法 → 同一实体）。仅收录稳定、广为人知者。
const STUDIO_SEED: &[&[&str]] = &[
    &["S1 NO.1 STYLE", "エスワン ナンバーワンスタイル", "エスワン", "S1"],
    &["MOODYZ", "ムーディーズ"],
    &["IdeaPocket", "アイデアポケット", "IDEA POCKET"],
    &["PREMIUM", "プレミアム"],
    &["Attackers", "アタッカーズ"],
    &["SOD Create", "SODクリエイト", "ソフト・オン・デマンド"],
    &["Madonna", "マドンナ"],
    &["WANZ FACTORY", "ワンズファクトリー", "WANZ"],
    &["E-BODY", "イーボディ"],
    &["FALENO", "ファレノ"],
    &["Prestige", "プレステージ"],
    &["kawaii", "kawaii*", "カワイイ"],
];

/// 标签同义词（中日英 → 同一实体）。仅收录语义清晰、等价无歧义者。
const TAG_SEED: &[&[&str]] = &[
    &["中出し", "中出", "内射", "Creampie"],
    &["巨乳", "Big Tits"],
    &["美少女", "Beautiful Girl"],
    &["メイド", "女僕", "女仆", "Maid"],
    &["人妻", "Married Woman"],
    &["単体作品", "单体作品", "Solowork"],
    &["素人", "Amateur"],
    &["フェラ", "口交", "Blowjob"],
];

fn current_seed_version(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        params![SEED_VERSION_KEY],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// 按需导入种子（幂等）。已是最新版本则直接返回。
/// 以 `merge` 规则落库 + 立即应用到投影簇。
pub fn import_seed_if_needed(conn: &Connection) -> rusqlite::Result<()> {
    if current_seed_version(conn)?.as_deref() == Some(SEED_VERSION) {
        return Ok(());
    }

    for cluster in STUDIO_SEED {
        let names: Vec<String> = cluster.iter().map(|s| s.to_string()).collect();
        add_force_merge(conn, ENTITY_STUDIO, &names)?;
        apply_force_merge_group(conn, ENTITY_STUDIO, &names)?;
    }
    for cluster in TAG_SEED {
        let names: Vec<String> = cluster.iter().map(|s| s.to_string()).collect();
        add_force_merge(conn, ENTITY_TAG, &names)?;
        apply_force_merge_group(conn, ENTITY_TAG, &names)?;
    }

    conn.execute(
        "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SEED_VERSION_KEY, SEED_VERSION],
    )?;
    log::info!(
        "[entity_alias] event=seed_imported version={} studios={} tags={}",
        SEED_VERSION,
        STUDIO_SEED.len(),
        TAG_SEED.len()
    );
    Ok(())
}
