slint::include_modules!();

use poketrack::cards_database;
use poketrack::db::repository::Repository;
use poketrack::db::{create_pool, initialize_database};
use poketrack::gui::{App, GuiRepository};
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing_subscriber::prelude::*;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let db_path = std::env::args().nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("poketrack.sqlite"));

    let runtime = tokio::runtime::Runtime::new()?;
    let pool_result: anyhow::Result<sqlx::SqlitePool> = runtime.block_on(async {
        let p = create_pool(&db_path).await?;
        initialize_database(&p).await?;
        Ok(p)
    });
    let pool = pool_result?;

    let repo = GuiRepository::new(pool.clone());
    let app = Rc::new(RefCell::new(App::new(repo)));

    let ui = AppWindow::new()?;
    ui.invoke_set_theme();

    refresh_cards(&ui, &app.borrow());
    refresh_sets(&ui, &app.borrow());
    refresh_status(&ui, &app.borrow());
    refresh_progress(&ui, &app.borrow());

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
    ui.on_toggle_collected({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |dex_id| {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            if let Err(e) = app.toggle_card_collection(dex_id) {
                app.set_status(&format!("Error: {e}"));
            }
            refresh_cards(&ui, &app);
            refresh_status(&ui, &app);
            refresh_progress(&ui, &app);
            app.refresh_set_cards();
            refresh_set_cards(&ui, &app);
        }
    });

    ui.on_collect_set({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |set_id| {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            if let Err(e) = app.add_missing_cards_from_set(set_id.as_str()) {
                app.set_status(&format!("Error: {e}"));
            } else {
                app.set_status("Added all missing cards from set");
            }
            refresh_sets(&ui, &app);
            refresh_cards(&ui, &app);
            refresh_status(&ui, &app);
            refresh_progress(&ui, &app);
            app.refresh_set_cards();
            refresh_set_cards(&ui, &app);
        }
    });

    ui.on_select_set({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |set_id| {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            app.select_set(set_id.as_str());
            let name = SharedString::from(app.get_selected_set_name());
            drop(app);
            ui.set_selected_set_name(name);
            let app = app_rc.borrow();
            refresh_set_cards(&ui, &app);
        }
    });

    ui.on_clear_selected_set({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move || {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            app.clear_selected_set();
            drop(app);
            ui.set_selected_set_name(SharedString::from(""));
            let app = app_rc.borrow();
            refresh_set_cards(&ui, &app);
        }
    });

    ui.on_set_filter({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |filter| {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            app.filter_cards(filter.as_str());
            refresh_cards(&ui, &app);
        }
    });

    ui.on_set_sort({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |sort_by, direction| {
            let ui = ui_weak.unwrap();
            let mut app = app_rc.borrow_mut();
            app.sort_cards(sort_by.as_str(), direction.as_str());
            refresh_cards(&ui, &app);
        }
    });

    // Holder keeps the current Timer alive while an update is in progress.
    let timer_holder: Rc<RefCell<Option<Rc<RefCell<Option<slint::Timer>>>>>> =
        Rc::new(RefCell::new(None));

    ui.on_update_database({
        let app_rc = app_rc.clone();
        let pool_clone = pool.clone();
        let timer_holder = timer_holder.clone();
        move || {
            ui_weak.unwrap().set_status_text(SharedString::from("Updating database..."));

            let (tx, rx) = std::sync::mpsc::channel::<String>();
            let pool = pool_clone.clone();
            std::thread::spawn(move || {
                let _ = tx.send(run_database_update(pool, false));
            });

            let timer_cell = Rc::new(RefCell::new(Some(slint::Timer::default())));
            {
                let mut guard = timer_cell.borrow_mut();
                let timer = guard.as_mut().unwrap();
                timer.start(
                    slint::TimerMode::Repeated,
                    std::time::Duration::from_millis(100),
                    {
                        let ui_weak = ui_weak.clone();
                        let app_rc = app_rc.clone();
                        let timer_cell = timer_cell.clone();
                        move || {
                            if let Ok(result_msg) = rx.try_recv() {
                                let ui = ui_weak.unwrap();
                                ui.set_status_text(SharedString::from(result_msg));
                                let mut app = app_rc.borrow_mut();
                                app.reload();
                                drop(app);
                                let app = app_rc.borrow();
                                refresh_cards(&ui, &app);
                                refresh_sets(&ui, &app);
                                refresh_status(&ui, &app);
                                refresh_progress(&ui, &app);
                                drop(app);
                                let mut app = app_rc.borrow_mut();
                                app.refresh_set_cards();
                                drop(app);
                                let app = app_rc.borrow();
                                refresh_set_cards(&ui, &app);
                                if let Some(t) = timer_cell.borrow_mut().as_ref() {
                                    t.stop();
                                }
                            }
                        }
                    },
                );
            }
            *timer_holder.borrow_mut() = Some(timer_cell);
        }
    });

    ui.run()?;
    Ok(())
}

fn run_database_update(pool: sqlx::SqlitePool, force: bool) -> String {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => return format!("Error creating runtime: {e}"),
    };
    runtime.block_on(async {
        let repo = Repository::new(pool);
        if let Err(e) = repo.ensure_finished_column().await {
            return format!("Error: {e}");
        }
        match update_cache(&repo, force).await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {e:#}"),
        }
    })
}

async fn update_cache(repo: &Repository, force: bool) -> anyhow::Result<String> {
    let languages = vec!["en", "ja"];
    let db = cards_database::CardsDatabase::new()?;
    let mut total_inserted: u64 = 0;
    let mut total_sets = 0u64;

    for lang in &languages {
        let series_list = db.load_series(lang)?;
        let mut sets_to_process = Vec::new();
        let mut sets_skipped = 0u64;

        for serie in &series_list {
            repo.upsert_series(serie, lang).await?;
            let sets = db.load_sets_for_series(&serie.id, lang)?;
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

        for set_data in &sets_to_process {
            let cards = db.load_cards(&set_data.id, lang)?;
            repo.upsert_set_with_cards(set_data, &cards, lang).await?;
            total_inserted += cards.len() as u64;
            total_sets += 1;
        }

        tracing::info!(
            "Language {lang}: {} sets processed, {} sets skipped",
            sets_to_process.len(),
            sets_skipped
        );
    }

    if force {
        repo.clear_translations().await?;
    }
    let names = repo.get_english_pokemon_names().await?;
    for (dex_id, en_name) in names {
        repo.upsert_translation(dex_id, &en_name).await?;
    }

    Ok(format!("Update complete! Sets: {total_sets}, Cards: {total_inserted}"))
}

fn refresh_cards(ui: &AppWindow, app: &App) {
    let cards = app.get_filtered_cards();
    let rc: Vec<CardItem> = cards.iter().map(|c| CardItem {
        dex_id: c.dex_id,
        name: SharedString::from(c.name.as_str()),
        collected: c.is_collected,
    }).collect();
    ui.set_cards_model(ModelRc::new(VecModel::from(rc)));
}

fn refresh_sets(ui: &AppWindow, app: &App) {
    let sets = app.get_sets();
    let rc: Vec<SetItem> = sets.iter().map(|s| SetItem {
        set_id: SharedString::from(s.id.as_str()),
        name: SharedString::from(s.name.as_str()),
        language: SharedString::from(s.language.as_str()),
        release_date: SharedString::from(s.release_date.as_str()),
        missing_count: s.missing_count as i32,
    }).collect();
    ui.set_sets_model(ModelRc::new(VecModel::from(rc)));
}

fn refresh_set_cards(ui: &AppWindow, app: &App) {
    let cards = app.get_selected_set_cards();
    let rc: Vec<CardItem> = cards.iter().map(|c| CardItem {
        dex_id: c.dex_id,
        name: SharedString::from(c.name.as_str()),
        collected: c.is_collected,
    }).collect();
    ui.set_set_cards_model(ModelRc::new(VecModel::from(rc)));
}

fn refresh_status(ui: &AppWindow, app: &App) {
    ui.set_status_text(SharedString::from(app.get_status()));
}

fn refresh_progress(ui: &AppWindow, app: &App) {
    let state = app.get_state();
    let pct = if state.total_count > 0 {
        state.collected_count as f32 / state.total_count as f32
    } else {
        0.0
    };
    ui.set_progress(pct);
}
