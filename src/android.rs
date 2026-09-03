use slint::android::AndroidApp;
use tracing_subscriber::prelude::*;

use crate::gui::{App, GuiRepository, UiOptions, run_gui};

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    // The external data dir is user-accessible (USB/MTP), so a pre-populated
    // database can be copied in there. Fall back to the internal dir.
    let external_dir = app.external_data_path();
    let internal_dir = app.internal_data_path();
    tracing::info!("external_data_path={external_dir:?} internal_data_path={internal_dir:?}");

    // Pick a path: prefer the external dir (user-accessible + writable) so the
    // user-copied database is used; otherwise the internal dir. If an existing
    // nonzero DB file is already present in the internal dir but not the external,
    // use the internal one so a previously created database is not "lost".
    let db_path = {
        let mut picked = external_dir
            .clone()
            .map(|d| d.join("poketrack.sqlite"))
            .or_else(|| internal_dir.clone().map(|d| d.join("poketrack.sqlite")))
            .unwrap_or_else(|| std::path::PathBuf::from("poketrack.sqlite"));
        for dir in external_dir.iter().chain(internal_dir.iter()) {
            let cand = dir.join("poketrack.sqlite");
            if let Ok(meta) = std::fs::metadata(&cand) {
                if meta.len() > 0 {
                    tracing::info!("found existing nonzero db at {} ({} bytes)", cand.display(), meta.len());
                    picked = cand;
                    break;
                }
            }
        }
        if let Some(dir) = external_dir.as_ref().or(internal_dir.as_ref()) {
            use std::fs;
            let _ = fs::create_dir_all(dir);
        }
        picked
    };

    tracing::info!("resolved db_path={}", db_path.display());
    if let Ok(meta) = std::fs::metadata(&db_path) {
        tracing::info!("db exists, size={} bytes", meta.len());
    } else {
        tracing::warn!("db file does not exist yet");
    }

    slint::android::init(app).unwrap();

    // Use a multi-threaded runtime for pool creation, matching the desktop
    // path (main.rs). Creating the SqlitePool inside a current_thread runtime
    // and then querying it from GuiRepository's multi-threaded runtime wedges
    // sqlx's connection handling, hanging on the first query.
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let result: anyhow::Result<sqlx::SqlitePool> = runtime.block_on(async {
        let pool = crate::db::create_pool(&db_path).await?;
        crate::db::initialize_database(&pool).await?;
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cards")
            .fetch_one(&pool)
            .await?;
        tracing::info!("cards in opened database: {}", count.0);
        anyhow::Result::Ok(pool)
    });

    let pool = match result {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to open database: {e}");
            return;
        }
    };

    let repo = GuiRepository::new(pool.clone());
    let t0 = std::time::Instant::now();
    let app = std::rc::Rc::new(std::cell::RefCell::new(App::new(repo)));
    tracing::info!("App::new (initial DB load) took {:?}", t0.elapsed());

    let t1 = std::time::Instant::now();
    if let Err(e) = run_gui(app, pool, UiOptions { show_update_button: false, compact_top_bar: true })
    {
        tracing::error!("GUI error: {e:#}");
    }
    tracing::info!("run_gui returned after {:?}", t1.elapsed());
}