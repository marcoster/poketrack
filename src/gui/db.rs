use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tokio::runtime::Runtime;

use crate::db::models::{CardSetInfo, PokedexCompletion, SetMissingCardInfo, SetMissingStats, Set};

pub struct GuiRepository {
    pool: SqlitePool,
    runtime: Runtime,
}

impl GuiRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            runtime: Runtime::new().unwrap(),
        }
    }

    pub fn get_all_cards(&self) -> Result<Vec<(String, String, Option<i32>)>> {
        self.runtime.block_on(async {
            let rows = sqlx::query_as::<_, (String, String, Option<i32>)>(
                "SELECT id, name, dex_id FROM cards \
                 WHERE dex_id IS NOT NULL \
                 GROUP BY dex_id \
                 ORDER BY dex_id",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        })
    }

    pub fn get_all_sets(&self) -> Result<Vec<Set>> {
        self.runtime.block_on(async {
            let sets = sqlx::query_as::<_, Set>(
                "SELECT * FROM sets ORDER BY release_date",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(sets)
        })
    }

    pub fn get_pokemon_sets(&self, dex_id: i32) -> Result<Vec<CardSetInfo>> {
        self.runtime.block_on(async {
            let cards = sqlx::query_as::<_, CardSetInfo>(
                r#"
                SELECT
                    cards.id as card_id,
                    cards.set_id,
                    sets.name as set_name,
                    cards.local_id,
                    cards.rarity,
                    cards.dex_id
                FROM cards
                INNER JOIN sets ON cards.set_id = sets.id
                WHERE cards.dex_id = ?
                ORDER BY sets.release_date
                "#,
            )
            .bind(dex_id)
            .fetch_all(&self.pool)
            .await?;
            Ok(cards)
        })
    }

    pub fn get_missing_pokemon(&self) -> Result<Vec<i32>> {
        self.runtime.block_on(async {
            let existing = sqlx::query_as::<_, (i32,)>(
                r#"
                SELECT DISTINCT c.dex_id
                FROM cards c
                LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
                WHERE cp.dex_id IS NULL AND c.dex_id IS NOT NULL
                ORDER BY c.dex_id
                "#,
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(existing.into_iter().map(|r| r.0).collect())
        })
    }

    pub fn get_pokedex_completion(&self) -> Result<PokedexCompletion> {
        self.runtime.block_on(async {
            let result = sqlx::query_as::<_, (i64, i64)>(
                r#"
                SELECT
                    COUNT(DISTINCT cp.dex_id) as collected,
                    COUNT(DISTINCT c.dex_id) as total
                FROM cards c
                LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
                WHERE c.dex_id IS NOT NULL
                "#,
            )
            .fetch_one(&self.pool)
            .await?;
            Ok(PokedexCompletion {
                collected: result.0,
                total: result.1,
            })
        })
    }

    pub fn get_set_missing_stats(&self, lang: Option<&str>) -> Result<Vec<SetMissingStats>> {
        self.runtime.block_on(async {
            let base_sql = r#"
                SELECT
                    cards.set_id,
                    sets.name as set_name,
                    COUNT(DISTINCT cards.dex_id) as missing
                FROM cards
                LEFT JOIN collected_pokemon cp ON cards.dex_id = cp.dex_id
                INNER JOIN sets ON cards.set_id = sets.id
                WHERE cp.dex_id IS NULL AND cards.dex_id IS NOT NULL
            "#;

            let (sql, like_pattern) = match lang {
                Some(l) => (
                    format!("{base_sql} AND cards.set_id LIKE ? GROUP BY cards.set_id, sets.name ORDER BY missing ASC"),
                    Some(format!("{l}-%")),
                ),
                None => (
                    format!("{base_sql} GROUP BY cards.set_id, sets.name ORDER BY missing ASC"),
                    None,
                ),
            };

            let mut q = sqlx::query_as::<_, SetMissingStats>(&sql);
            if let Some(pattern) = &like_pattern {
                q = q.bind(pattern);
            }
            let stats = q.fetch_all(&self.pool).await?;
            Ok(stats)
        })
    }

    pub fn get_set_missing_pokemon_details(&self, set_id: &str) -> Result<Vec<SetMissingCardInfo>> {
        self.runtime.block_on(async {
            let cards = sqlx::query_as::<_, SetMissingCardInfo>(
                r#"
                SELECT DISTINCT
                    cards.dex_id
                FROM cards
                LEFT JOIN collected_pokemon cp ON cards.dex_id = cp.dex_id
                WHERE cards.set_id = ? AND cp.dex_id IS NULL AND cards.dex_id IS NOT NULL
                ORDER BY cards.dex_id
                "#,
            )
            .bind(set_id)
            .fetch_all(&self.pool)
            .await?;
            Ok(cards)
        })
    }

    pub fn mark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        self.runtime.block_on(async {
            sqlx::query(
                "INSERT OR IGNORE INTO collected_pokemon (dex_id) VALUES (?)",
            )
            .bind(dex_id)
            .execute(&self.pool)
            .await?;
            Ok(())
        })
    }

    pub fn unmark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        self.runtime.block_on(async {
            sqlx::query("DELETE FROM collected_pokemon WHERE dex_id = ?")
                .bind(dex_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        })
    }

    pub fn get_all_translations(&self) -> Result<std::collections::HashMap<i32, String>> {
        self.runtime.block_on(async {
            let rows = sqlx::query_as::<_, (i32, String)>("SELECT dex_id, en_name FROM translations")
                .fetch_all(&self.pool)
                .await?;
            Ok(rows.into_iter().collect())
        })
    }

    pub fn get_set_info(&self, set_id: &str) -> Result<Option<Set>> {
        self.runtime.block_on(async {
            let result = sqlx::query_as::<_, Set>(
                "SELECT * FROM sets WHERE id = ?"
            )
            .bind(set_id)
            .fetch_optional(&self.pool)
            .await?;
            Ok(result)
        })
    }

    pub fn get_cards_for_set(&self, set_id: &str) -> Result<Vec<(String, i32, bool)>> {
        self.runtime.block_on(async {
            // Names as shown in the Cards view: one default entry per dex id.
            // Match by dex id so the set details show the same names.
            let names = sqlx::query_as::<_, (i32, String)>(
                "SELECT dex_id, name FROM cards \
                 WHERE dex_id IS NOT NULL \
                 GROUP BY dex_id \
                 ORDER BY dex_id",
            )
            .fetch_all(&self.pool)
            .await?;
            let name_by_dex: HashMap<i32, String> = names.into_iter().collect();

            let rows = sqlx::query_as::<_, (i32, Option<i32>)>(
                r#"
                SELECT DISTINCT c.dex_id, cp.dex_id
                FROM cards c
                LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
                WHERE c.set_id = ? AND c.dex_id IS NOT NULL
                ORDER BY c.dex_id
                "#,
            )
            .bind(set_id)
            .fetch_all(&self.pool)
            .await?;

            Ok(rows
                .into_iter()
                .map(|(dex, cp)| {
                    let name = name_by_dex.get(&dex).cloned().unwrap_or_default();
                    (name, dex, cp.is_some())
                })
                .collect())
        })
    }

    pub fn mark_set_finished(&self, set_id: &str) -> Result<()> {
        self.runtime.block_on(async {
            sqlx::query("UPDATE sets SET finished = 1 WHERE id = ?")
                .bind(set_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        })
    }
}
