mod db;
mod cards_database;

use anyhow::Result;
use clap::{Parser, Subcommand};
use db::{create_pool, initialize_database, repository::Repository};
use std::path::PathBuf;
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "poketrack")]
#[command(about = "Pokemon Card Tracker - Track your Pokemon TCG collection", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "poketrack.sqlite")]
    db: PathBuf,

    #[arg(short, long)]
    update_tcgdex: bool,

    #[arg(long)]
    force: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        pokemon: String,
    },
    Remove {
        pokemon: String,
    },
    List {
        #[arg(short, long)]
        dex: i32,
    },
    Missing {
        #[arg(short, long)]
        name: bool,
    },
    Stats {
        #[arg(long)]
        sets: bool,
        #[arg(long)]
        sets_cards: bool,
        #[arg(long, conflicts_with = "en")]
        jp: bool,
        #[arg(long, conflicts_with = "jp")]
        en: bool,
    },
}

fn parse_dex_ids(input: &str) -> Result<Vec<i32>> {
    let mut dex_ids = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let range: Vec<&str> = part.split('-').collect();
            if range.len() != 2 {
                anyhow::bail!("Invalid range: {}", part);
            }
            let start: i32 = range[0]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number: {}", range[0]))?;
            let end: i32 = range[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number: {}", range[1]))?;
            if start > end {
                anyhow::bail!("Invalid range: {}-{} (start > end)", start, end);
            }
            for i in start..=end {
                dex_ids.push(i);
            }
        } else {
            let dex_id: i32 = part
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number: {}", part))?;
            dex_ids.push(dex_id);
        }
    }
    Ok(dex_ids)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let cli = Cli::parse();

    if !cli.db.exists() {
        tracing::info!("Database not found, creating new database at {:?}", cli.db);
    }

    let pool = create_pool(&cli.db).await?;
    initialize_database(&pool).await?;

    let repo = Repository::new(pool);

    if cli.update_tcgdex {
        repo.ensure_finished_column().await?;
        update_tcgdex_cache(&repo, cli.force).await?;
    }

    if let Some(command) = cli.command {
        match command {
            Commands::Add { pokemon } => {
                let dex_ids = parse_dex_ids(&pokemon)?;
                let existing = repo.get_existing_dex_ids(&dex_ids).await?;

                let mut added = 0;
                for dex_id in &dex_ids {
                    if existing.contains(dex_id) {
                        repo.mark_pokemon_collected(*dex_id).await?;
                        added += 1;
                    } else {
                        println!("Pokemon #{} not found in database, skipping", dex_id);
                    }
                }
                if added > 0 {
                    println!("Added {} Pokemon to collection", added);
                }
            }
            Commands::Remove { pokemon } => {
                let dex_ids = parse_dex_ids(&pokemon)?;
                let existing = repo.get_existing_dex_ids(&dex_ids).await?;

                let mut removed = 0;
                for dex_id in &dex_ids {
                    if existing.contains(dex_id) {
                        repo.unmark_pokemon_collected(*dex_id).await?;
                        removed += 1;
                    } else {
                        println!("Pokemon #{} not found in database, skipping", dex_id);
                    }
                }
                if removed > 0 {
                    println!("Removed {} Pokemon from collection", removed);
                }
            }
            Commands::List { dex } => {
                let cards = repo.get_pokemon_sets(dex).await?;
                let translations = repo.get_all_translations().await?;
                if cards.is_empty() {
                    println!("No cards found for Pokemon #{}", dex);
                } else {
                    let en_name = translations.get(&dex);
                    if let Some(name) = en_name {
                        println!("#{} - {}", dex, name);
                    } else {
                        println!("#{}", dex);
                    }
                    for card in cards {
                        let en_suffix = translations
                            .get(&card.dex_id)
                            .map(|n| format!(" [EN: {}]", n))
                            .unwrap_or_default();
                        println!(
                            "  {}: {} ({}) - {}{}",
                            card.set_id, card.set_name, card.local_id, card.rarity, en_suffix
                        );
                    }
                }
            }
            Commands::Missing { name } => {
                let missing = repo.get_missing_pokemon().await?;
                if missing.is_empty() {
                    println!("No missing Pokemon! You have them all!");
                } else if name {
                    let translations = repo.get_all_translations().await?;
                    for dex_id in missing.iter() {
                        let en_name = translations.get(dex_id);
                        if let Some(n) = en_name {
                            println!("#{} - {}", dex_id, n);
                        } else {
                            println!("#{}", dex_id);
                        }
                    }
                } else {
                    println!("Missing Pokemon ({} total):", missing.len());
                    let mut cnt: usize = 1;
                    for dex_id in missing.iter() {
                        print!(" #{} ", dex_id);
                        if cnt % 10 == 0 {
                            println!();
                        }
                        cnt += 1;
                    }
                }
            }
            Commands::Stats { sets, sets_cards, jp, en } => {
                let lang = if jp { Some("ja") } else if en { Some("en") } else { None };
                if sets_cards {
                    let stats = repo.get_set_missing_stats(lang).await?;
                    if stats.is_empty() {
                        println!("No missing Pokemon! You have them all!");
                    } else {
                        println!("Missing Pokemon by Set:");
                        for stat in &stats {
                            println!(
                                "  {}: {} - {} missing",
                                stat.set_id, stat.set_name, stat.missing
                            );
                            let missing_cards =
                                repo.get_set_missing_pokemon_details(&stat.set_id).await?;
                            for chunk in missing_cards.chunks(4) {
                                let card_strs: Vec<String> = chunk
                                    .iter()
                                    .map(|c| match &c.en_name {
                                        Some(name) => format!("#{} {}", c.dex_id, name),
                                        None => format!("#{}", c.dex_id),
                                    })
                                    .collect();
                                println!("    {}", card_strs.join(", "));
                            }
                        }
                    }
                } else if sets {
                    let stats = repo.get_set_missing_stats(lang).await?;
                    if stats.is_empty() {
                        println!("No missing Pokemon! You have them all!");
                    } else {
                        println!("Missing Pokemon by Set:");
                        for stat in stats {
                            println!(
                                "  {}: {} - {} missing",
                                stat.set_id, stat.set_name, stat.missing
                            );
                        }
                    }
                } else {
                    let completion = repo.get_pokedex_completion().await?;
                    let pct = if completion.total > 0 {
                        (completion.collected as f64 / completion.total as f64 * 100.0).round()
                    } else {
                        0.0
                    };
                    println!(
                        "Pokedex: {}/{} Pokemon collected ({:.0}%)",
                        completion.collected, completion.total, pct
                    );
                }
            }
        }
    }

    Ok(())
}

async fn update_tcgdex_cache(repo: &Repository, force: bool) -> Result<()> {
    let languages = vec!["en", "ja"];
    let mode = if force { "force refresh" } else { "incremental" };
    tracing::info!("Starting TCGdex cache update from cards-database ({} mode)...", mode);

    if force {
        repo.clear_cache().await?;
    }

    let db = cards_database::CardsDatabase::new()?;
    let mut total_cards_inserted: u64 = 0;
    let mut total_cards_skipped: u64 = 0;
    let mut total_sets_completed = 0u64;

    for lang in &languages {
        let lang_label = if *lang == "en" { "English (en)" } else { "Japanese (ja)" };
        let series_list = db.load_series(lang)?;
        tracing::info!("Found {} series for {}", series_list.len(), lang_label);

        let mut sets_to_process: Vec<cards_database::SetData> = Vec::new();
        let mut sets_skipped = 0u64;

        for serie in &series_list {
            repo.upsert_series(serie, lang).await?;
            let sets = db.load_sets_for_series(&serie.id, lang)?;
            tracing::info!("Found {} sets for series {}", sets.len(), serie.id);

            for set_data in &sets {
                let should_fetch = if force {
                    true
                } else {
                    let set_info = repo.get_set_info(&set_data.id, lang).await?;
                    let is_finished = set_info.map(|s| s.finished).unwrap_or(false);
                    let db_total = repo.count_set_cards(&set_data.id, lang).await?;

                    if is_finished && db_total == Some(set_data.total_cards) {
                        sets_skipped += 1;
                        false
                    } else {
                        true
                    }
                };

                if should_fetch {
                    sets_to_process.push(set_data.clone());
                }
            }
        }

        tracing::info!("{} sets already complete, skipping", sets_skipped);
        tracing::info!("Processing {} new/updated sets for {}...", sets_to_process.len(), lang_label);

        let mut cards_inserted: u64 = 0;
        let cards_skipped: u64 = 0;
        let mut sets_completed = 0u64;

        for (set_idx, set_data) in sets_to_process.iter().enumerate() {
            let set_name = match *lang {
                "en" => set_data.name_en.as_deref().unwrap_or(""),
                "ja" => set_data.name_ja.as_deref().unwrap_or(""),
                _ => "",
            };
            let series_name = series_list.iter()
                .find(|s| s.id == set_data.serie_id)
                .and_then(|s| match *lang {
                    "en" => s.name_en.as_deref(),
                    "ja" => s.name_ja.as_deref(),
                    _ => None,
                })
                .unwrap_or("");

            tracing::info!(
                "[{}] Processing set {}/{}: {} ({})",
                lang, set_idx + 1, sets_to_process.len(), set_name, series_name
            );

            let cards = db.load_cards(&set_data.id, lang)?;
            tracing::info!("Loading {} cards for set {}...", cards.len(), set_data.id);

            repo.upsert_set_with_cards(set_data, &cards, lang).await?;
            cards_inserted += cards.len() as u64;
            sets_completed += 1;
            tracing::debug!("Set {} marked as finished", set_data.id);
        }

        total_cards_inserted += cards_inserted;
        total_cards_skipped += cards_skipped;
        total_sets_completed += sets_completed;
    }

    tracing::info!("Fetching English translations...");
    fetch_english_translations(repo, force).await?;

    tracing::info!(
        "TCGdex cache update complete! Sets completed: {}, Cards inserted: {}, Cards skipped: {}",
        total_sets_completed, total_cards_inserted, total_cards_skipped
    );
    Ok(())
}

async fn fetch_english_translations(repo: &Repository, force: bool) -> Result<()> {
    if force {
        repo.clear_translations().await?;
    }

    tracing::info!("Fetching English translations from local database...");

    let pokemon_names = repo.get_english_pokemon_names().await?;
    let mut translations_added = 0u64;

    for (dex_id, en_name) in pokemon_names {
        if let Ok(inserted) = repo.upsert_translation(dex_id, &en_name).await {
            if inserted {
                translations_added += 1;
            }
        }
    }

    tracing::info!(
        "English translations complete! Added {} new translations",
        translations_added
    );
    Ok(())
}
