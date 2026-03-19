mod api;
mod db;

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
    Missing,
    Stats {
        #[arg(long)]
        sets: bool,
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
            let start: i32 = range[0].parse().map_err(|_| anyhow::anyhow!("Invalid number: {}", range[0]))?;
            let end: i32 = range[1].parse().map_err(|_| anyhow::anyhow!("Invalid number: {}", range[1]))?;
            if start > end {
                anyhow::bail!("Invalid range: {}-{} (start > end)", start, end);
            }
            for i in start..=end {
                dex_ids.push(i);
            }
        } else {
            let dex_id: i32 = part.parse().map_err(|_| anyhow::anyhow!("Invalid number: {}", part))?;
            dex_ids.push(dex_id);
        }
    }
    Ok(dex_ids)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
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
                if cards.is_empty() {
                    println!("No cards found for Pokemon #{}", dex);
                } else {
                    println!("#{}", dex);
                    for card in cards {
                        println!("  {}: {} ({}) - {}", card.set_id, card.set_name, card.local_id, card.rarity);
                    }
                }
            }
            Commands::Missing => {
                let missing = repo.get_missing_pokemon().await?;
                if missing.is_empty() {
                    println!("No missing Pokemon! You have them all!");
                } else {
                    println!("Missing Pokemon ({} total):", missing.len());
                    for dex_id in missing.iter().take(50) {
                        println!("  #{}", dex_id);
                    }
                    if missing.len() > 50 {
                        println!("  ... and {} more", missing.len() - 50);
                    }
                }
            }
            Commands::Stats { sets } => {
                if sets {
                    let stats = repo.get_set_missing_stats().await?;
                    if stats.is_empty() {
                        println!("No missing Pokemon! You have them all!");
                    } else {
                        println!("Missing Pokemon by Set:");
                        for stat in stats {
                            println!("  {}: {} - {} missing", stat.set_id, stat.set_name, stat.missing);
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
    let mode = if force { "force refresh" } else { "incremental" };
    tracing::info!("Starting TCGdex cache update ({} mode)...", mode);

    if force {
        repo.clear_cache().await?;
    }

    let series_list = api::Serie::list("en").await?;
    let total_series = series_list.len();
    tracing::info!("Found {} series", total_series);

    let mut sets_to_process: Vec<(String, String, String)> = Vec::new();
    let mut sets_skipped = 0u64;

    for series_resume in &series_list {
        let series = match api::Serie::get(&series_resume.id, "en").await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch series {}: {}", series_resume.id, e);
                continue;
            }
        };

        if let Err(e) = repo.upsert_series(&series).await {
            tracing::error!("Failed to save series {}: {}", series.id, e);
            continue;
        }

        for set_resume in &series.sets {
            let api_total_cards = {
                match api::Set::get(&set_resume.id, "en").await {
                    Ok(s) => s.card_count.total as i32,
                    Err(e) => {
                        tracing::error!("Failed to fetch set {}: {}", set_resume.id, e);
                        continue;
                    }
                }
            };

            let should_fetch = if force {
                true
            } else {
                let db_total = repo.get_set_total_cards(&set_resume.id).await?;
                let is_finished = repo.is_set_finished(&set_resume.id).await?;
                
                if is_finished && db_total == Some(api_total_cards) {
                    sets_skipped += 1;
                    false
                } else {
                    true
                }
            };

            if should_fetch {
                sets_to_process.push((set_resume.id.clone(), set_resume.name.clone(), series.name.clone()));
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    tracing::info!("{} sets already complete, skipping", sets_skipped);
    tracing::info!("Processing {} new/updated sets...", sets_to_process.len());

    let mut cards_inserted: u64 = 0;
    let mut cards_skipped: u64 = 0;
    let mut sets_completed = 0u64;

    for (set_idx, (set_id, set_name, series_name)) in sets_to_process.iter().enumerate() {
        tracing::info!(
            "Processing set {}/{}: {} ({})",
            set_idx + 1,
            sets_to_process.len(),
            set_name,
            series_name
        );

        let set = match api::Set::get(set_id, "en").await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch set {}: {}", set_id, e);
                continue;
            }
        };

        if let Err(e) = repo.upsert_set(&set).await {
            tracing::error!("Failed to save set {}: {}", set.id, e);
            continue;
        }

        let total_cards = set.cards.len();
        tracing::debug!("Fetching {} cards for set {}...", total_cards, set.id);

        for card_resume in &set.cards {
            match api::CardDetails::fetch(&card_resume.id, "en").await {
                Ok(card) => {
                    if let Err(e) = repo.upsert_card(&card).await {
                        tracing::warn!("Failed to save card {}: {}", card.id, e);
                        cards_skipped += 1;
                    } else {
                        cards_inserted += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch card {}: {}", card_resume.id, e);
                    cards_skipped += 1;
                }
            }
        }

        repo.mark_set_finished(set_id).await?;
        sets_completed += 1;
        tracing::debug!("Set {} marked as finished", set.id);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    tracing::info!(
        "TCGdex cache update complete! Sets completed: {}, Cards inserted: {}, Cards skipped: {}",
        sets_completed,
        cards_inserted,
        cards_skipped
    );
    Ok(())
}
