slint::include_modules!();

use crate::cards_database;
use crate::db::repository::Repository;
use crate::gui::App;
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

/// Per-platform UI options.
pub struct UiOptions {
    /// Whether to show the "Update Database" button (desktop-only; requires the
    /// cards-database-json/ data that is not bundled on Android).
    pub show_update_button: bool,
    /// Whether to render the compact top bar (only the view buttons). Used on
    /// narrow/mobile screens where the full bar does not fit.
    pub compact_top_bar: bool,
}

/// Builds and runs the Slint GUI against the given app state and database pool.
/// This is the single shared entry point used by both the desktop binary
/// (`main.rs`) and the Android entry point (`android.rs`).
pub fn run_gui(app: Rc<RefCell<App>>, pool: sqlx::SqlitePool, opts: UiOptions) -> anyhow::Result<()> {
    let ui = AppWindow::new()?;
    ui.set_show_update_button(opts.show_update_button);
    ui.set_compact_top_bar(opts.compact_top_bar);
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
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

    let ui_weak = ui.as_weak();
    let app_rc = app.clone();
    ui.on_set_sets_sort({
        let ui_weak = ui_weak.clone();
        let app_rc = app_rc.clone();
        move |sort_by, direction| {
            let ui = ui_weak.unwrap();
            {
                let mut app = app_rc.borrow_mut();
                app.sort_sets(sort_by.as_str(), direction.as_str());
            }
            let app = app_rc.borrow();
            refresh_sets(&ui, &app);
        }
    });

    // Holder keeps the current Timer alive while an update is in progress.
    let timer_holder: Rc<RefCell<Option<Rc<RefCell<Option<slint::Timer>>>>>> =
        Rc::new(RefCell::new(None));

    let ui_weak = ui.as_weak();
    ui.on_update_database({
        let app_rc = app.clone();
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

    // (Resize handling is left to the Slint layout: the content panel and its
    // ListViews use vertical-stretch: 1 so they fill the real window height and
    // stay scrollable. A fill-height workaround was previously driven from here,
    // but slint::Timer does not fire reliably on the Android backend, so manual
    // height seeding is not viable there.)

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
    ui.set_collected_count(state.collected_count as i32);
    ui.set_total_count(state.total_count as i32);
}