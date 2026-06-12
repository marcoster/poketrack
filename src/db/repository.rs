use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashSet;

use super::models::{
    CardSetInfo, PokedexCompletion, Set as DbSet, SetMissingCardInfo, SetMissingStats,
};
use crate::cards_database::{CardData, SerieData, SetData};

pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn ensure_finished_column(&self) -> Result<()> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pragma_table_info('sets') WHERE name = 'finished'",
        )
        .fetch_one(&self.pool)
        .await?;

        if result == 0 {
            tracing::info!("Adding 'finished' column to sets table...");
            sqlx::query("ALTER TABLE sets ADD COLUMN finished INTEGER NOT NULL DEFAULT 0")
                .execute(&self.pool)
                .await?;
            tracing::info!("'finished' column added successfully");
        }
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        sqlx::query("BEGIN").execute(&self.pool).await?;
        sqlx::query("DELETE FROM pokemon_index")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM cards").execute(&self.pool).await?;
        sqlx::query("DELETE FROM sets").execute(&self.pool).await?;
        sqlx::query("DELETE FROM series")
            .execute(&self.pool)
            .await?;
        sqlx::query("COMMIT").execute(&self.pool).await?;
        tracing::info!("Cache cleared successfully");
        Ok(())
    }

    pub async fn upsert_series(&self, series: &SerieData, lang: &str) -> Result<()> {
        let id = format!("{}-{}", lang, series.id);
        let name = match lang {
            "en" => series.name_en.as_deref().unwrap_or(""),
            "ja" => series.name_ja.as_deref().unwrap_or(""),
            _ => "",
        };

        sqlx::query(
            r#"
            INSERT INTO series (id, name, logo, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                logo = excluded.logo,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(&series.logo)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_set(&self, set: &SetData, lang: &str) -> Result<()> {
        let id = format!("{}-{}", lang, set.id);
        let serie_id = format!("{}-{}", lang, set.serie_id);
        let name = match lang {
            "en" => set.name_en.as_deref().unwrap_or(""),
            "ja" => set.name_ja.as_deref().unwrap_or(""),
            _ => "",
        };
        let release_date = set.release_date.as_deref().unwrap_or("");

        sqlx::query(
            r#"
            INSERT INTO sets (id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards, finished, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                logo = excluded.logo,
                symbol = excluded.symbol,
                serie_id = excluded.serie_id,
                release_date = excluded.release_date,
                tcg_online = excluded.tcg_online,
                total_cards = excluded.total_cards,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(&None::<String>) // logo
        .bind(&None::<String>) // symbol
        .bind(&serie_id)
        .bind(release_date)
        .bind(&set.tcg_online)
        .bind(set.total_cards)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_set_finished(&self, set_id: &str, lang: &str) -> Result<()> {
        let full_id = format!("{}-{}", lang, set_id);
        sqlx::query("UPDATE sets SET finished = 1 WHERE id = ?")
            .bind(&full_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_set_cards(&self, set_id: &str, lang: &str) -> Result<Option<i32>> {
        let full_id = format!("{}-{}", lang, set_id);
        let result: Option<(i32,)> = sqlx::query_as("SELECT COUNT(*) from cards where set_id == ?")
            .bind(full_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(result.map(|r| r.0))
    }

    pub async fn get_set_info(&self, set_id: &str, lang: &str) -> Result<Option<DbSet>> {
        let full_id = format!("{}-{}", lang, set_id);
        let result = sqlx::query_as::<_, DbSet>(
            "SELECT * FROM sets WHERE id = ?"
        )
        .bind(&full_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn upsert_set_with_cards(
        &self,
        set: &SetData,
        cards: &[CardData],
        lang: &str,
    ) -> Result<()> {
        sqlx::query("BEGIN").execute(&self.pool).await?;

        self.upsert_set(set, lang).await?;

        for card in cards {
            self.upsert_card(card, &set.id, lang).await?;
        }

        self.mark_set_finished(&set.id, lang).await?;

        sqlx::query("COMMIT").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn upsert_card(&self, card: &CardData, set_id: &str, lang: &str) -> Result<()> {
        let id = format!("{}-{}-{}", lang, set_id, card.local_id);
        let full_set_id = format!("{}-{}", lang, set_id);

        let types_json = card.types.as_ref();

        let category = card.category.clone();

        sqlx::query(
            r#"
            INSERT INTO cards (id, set_id, local_id, name, category, hp, types, dex_id, rarity, image, stage, evolves_from, illustrator, description, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                set_id = excluded.set_id,
                local_id = excluded.local_id,
                name = excluded.name,
                category = excluded.category,
                hp = excluded.hp,
                types = excluded.types,
                dex_id = excluded.dex_id,
                rarity = excluded.rarity,
                image = excluded.image,
                stage = excluded.stage,
                evolves_from = excluded.evolves_from,
                illustrator = excluded.illustrator,
                description = excluded.description,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(&full_set_id)
        .bind(&card.local_id)
        .bind(&card.name)
        .bind(&category)
        .bind(card.hp)
        .bind(types_json)
        .bind(card.dex_id)
        .bind(&card.rarity)
        .bind(&card.image)
        .bind(&card.stage)
        .bind(&card.evolves_from)
        .bind(&card.illustrator)
        .bind(&card.description)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO collected_pokemon (dex_id)
            VALUES (?)
            "#,
        )
        .bind(dex_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn unmark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        sqlx::query("DELETE FROM collected_pokemon WHERE dex_id = ?")
            .bind(dex_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_existing_dex_ids(&self, dex_ids: &[i32]) -> Result<HashSet<i32>> {
        let mut result = HashSet::new();
        for chunk in dex_ids.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT DISTINCT dex_id FROM cards WHERE dex_id IN ({})",
                placeholders
            );
            let mut q = sqlx::query_as::<_, (i32,)>(&query);
            for &id in chunk {
                q = q.bind(id);
            }
            let rows = q.fetch_all(&self.pool).await?;
            for row in rows {
                result.insert(row.0);
            }
        }
        Ok(result)
    }

    pub async fn get_pokemon_sets(&self, dex_id: i32) -> Result<Vec<CardSetInfo>> {
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
    }

    pub async fn get_missing_pokemon(&self) -> Result<Vec<i32>> {
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
    }

    pub async fn get_pokedex_completion(&self) -> Result<PokedexCompletion> {
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
    }

    pub async fn get_set_missing_stats(&self, lang: Option<&str>) -> Result<Vec<SetMissingStats>> {
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
    }

    pub async fn get_set_missing_pokemon_details(&self, set_id: &str) -> Result<Vec<SetMissingCardInfo>> {
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
    }

    pub async fn get_english_pokemon_names(&self) -> Result<Vec<(i32, String)>> {
        let names = sqlx::query_as::<_, (i32, String)>(
            r#"
            SELECT c.dex_id, c.name
            FROM cards c
            WHERE c.id LIKE 'en-%' AND c.dex_id IS NOT NULL
            GROUP BY c.dex_id
            ORDER BY c.dex_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(names)
    }

    pub async fn upsert_translation(&self, dex_id: i32, en_name: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO translations (dex_id, en_name)
            VALUES (?, ?)
            ON CONFLICT(dex_id) DO UPDATE SET
                en_name = excluded.en_name
            "#,
        )
        .bind(dex_id)
        .bind(en_name)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_all_translations(&self) -> Result<std::collections::HashMap<i32, String>> {
        let rows = sqlx::query_as::<_, (i32, String)>("SELECT dex_id, en_name FROM translations")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().collect())
    }

    pub async fn clear_translations(&self) -> Result<()> {
        sqlx::query("DELETE FROM translations")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
