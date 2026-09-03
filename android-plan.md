# Android APK Packaging Plan (poketrack)

Goal: ship the desktop Rust+Slint GUI as an installable Android APK for a personal phone
(Pixel 9 Pro, arm64-v8a).

## Locked-in decisions
- **Data:** manual copy of a pre-populated `poketrack.sqlite` to the app's Android
  external data directory. No bundled card-data assets in the APK.
- **Code sharing:** extract the shared Slint GUI startup/wiring into a library module so
  desktop (`main.rs`) and Android (`android.rs`) use one code path.
- **"Update Database" button:** hidden on Android (it depends on the desktop-only
  `cards-database-json/` data).
- **Icon:** public-domain Poké Ball icon from Wikimedia Commons, rasterized into the
  standard Android launcher mipmap densities.
- **Architecture:** arm64-v8a only (sufficient for the Pixel 9 Pro).

## Step 1 — Extract shared GUI startup into a lib module
- New `src/gui/ui.rs`:
  - Move `slint::include_modules!()` here (currently `src/main.rs:1`); `pub use` the
    generated `AppWindow`, `CardItem`, `SetItem` so both binaries can use them.
  - Move the initial refresh + all `ui.on_*` callback wiring + DB-update helpers
    (`run_database_update`, `update_cache`) into one public function:
    `run_gui(app: Rc<RefCell<App>>, pool: SqlitePool, opts: UiOptions) -> Result<()>`.
  - `UiOptions` carries a `show_update_button: bool` flag.
  - `AppWindow::new()`, `invoke_set_theme()`, then `ui.run()`.
  - The `fill-height` resize Timer is gated with `#[cfg(not(target_os = "android"))]`.
- `src/gui/mod.rs`: add `pub mod ui;` + re-export `run_gui`, `UiOptions`, `AppWindow`,
  `CardItem`, `SetItem`.
- `src/main.rs`: keep `main()` (DB init, `App`/`GuiRepository`), then call
  `ui::run_gui(app, pool, UiOptions { show_update_button: true })`.
- Verify: desktop `cargo build` + app behaviour unchanged.

## Step 2 — Wire the full GUI in `src/android.rs`
- Keep `slint::android::init(app)`.
- DB path from external storage (Step 3).
- Open/init DB via `create_pool` / `initialize_database`.
- Build `Rc<RefCell<App::new(repo))>` via `GuiRepository`.
- Call `ui::run_gui(app, pool, UiOptions { show_update_button: false })`.
- Remove the `// GUI window / event loop ...` TODO.

## Step 3 — DB on external storage (manual copy)
- Use `app.external_data_path()` (fall back to `internal_data_path()` if `None`).
- Copy location for the user:
  `/sdcard/Android/data/rust.poketrack/files/poketrack.sqlite`
- First launch: opens the file if present, else initializes a fresh empty DB.
- Generate a pre-populated DB on the desktop:
  `cargo run --bin poketrack-cli -- --db <tmp>/poketrack.sqlite --update-tcgdex`
  then move it to the phone path above.
- `initialize_database` is idempotent (`CREATE ... IF NOT EXISTS`); opening an existing
  populated DB won't wipe user data.

## Step 4 — Hide "Update Database" on Android
- Add `show-update-button` property to `ui/appwindow.slint`; wrap the button in
  `if root.show-update-button { ... }`.
- Desktop sets it true; Android sets it false.

## Step 5 — Add the Poké Ball launcher icon
- Source: Wikimedia Commons "Poké Ball icon.svg" (public domain / simple geometry).
  Original: https://upload.wikimedia.org/wikipedia/commons/5/53/Pok%C3%A9_Ball_icon.svg
  (Note: tagged as a Nintendo trademark; fine for a personal app, not for commercial
  distribution.)
- Rasterize with `rsvg-convert` to the 5 launcher densities:
  - `res/mipmap-mdpi/ic_launcher.png` 48x48
  - `res/mipmap-hdpi/ic_launcher.png` 72x72
  - `res/mipmap-xhdpi/ic_launcher.png` 96x96
  - `res/mipmap-xxhdpi/ic_launcher.png` 144x144
  - `res/mipmap-xxxhdpi/ic_launcher.png` 192x192
- `Cargo.toml`:
  ```toml
  [package.metadata.android]
  resources = "res"
  [package.metadata.android.application]
  icon = "@mipmap/ic_launcher"
  ```

## Step 6 — Build & package the APK
Env for each build:
```bash
export JAVA_HOME=/opt/android-studio/jbr
export ANDROID_HOME=/home/marco/dev/Android/Sdk
export ANDROID_NDK_HOME=/home/marco/dev/Android/Sdk/ndk/30.0.15729638
export ANDROID_NDK_ROOT=$ANDROID_NDK_HOME
export ANDROID_JAR=$ANDROID_HOME/platforms/android-36/android.jar
```
Build (cdylib only):
```bash
cargo apk build --release --target aarch64-linux-android --lib
```
Output: `target/android-artifacts/release/apk/poketrack.apk`
- `cargo-apk` auto-generates a debug keystore (`~/.android/debug.keystore`) → APK is
  signed and sideloadable. A release keystore (Play Store) is out of scope.
- Align `[package.metadata.android.sdk] min_sdk_version` to 21 to match Cargo.toml and
  avoid the earlier manifest mismatch.

## Step 7 — Install on the phone
1. Enable "Install unknown apps" on the phone.
2. `adb install -r target/android-artifacts/release/apk/poketrack.apk`
   (or copy the APK via MTP and open it).
3. Optionally drop the pre-populated DB at the Step 3 path.
4. Launch "Poketrack".