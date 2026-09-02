# Pokemon Card Tracker

Track your Pokemon TCG collection by national dex number. Fetches card data from the TCGdex API and supports both English (EN) and Japanese (JA) language sets.

The real highlight for collectors that like to rip packs is the `stats --sets` command, which lists how many of your missing cards can be found in each set.

## Build

```bash
cargo build --release
```

The executable will be at `target/release/poketrack` (or `target/debug/poketrack` for debug builds).

## Setup

On first run, the database is created automatically. Fetch card data with:

```bash
./poketrack --force --update-tcgdex
```

This fetches all sets and cards from both EN and JA APIs. Use `--force` for a full refresh or run without it for incremental updates.

## Usage

```bash
# Add Pokemon to collection (supports ranges)
./poketrack add 25          # Add Pikachu
./poketrack add 1-20        # Add Bulbasaur through Squirtle
./poketrack add 1,4,7       # Add Bulbasaur, Charmander, Squirtle

# Remove Pokemon from collection
./poketrack remove 25

# List all cards for a Pokemon
./poketrack list --dex 25

# Show Pokedex completion
./poketrack stats

# Show missing Pokemon by set
./poketrack stats --sets

# Show all missing Pokemon
./poketrack missing
```

## GUI Usage

### Linux

```bash
# Build the application
cargo build

# Run the application
cargo run
# or
./target/debug/poketrack
# or for release
./target/release/poketrack
```

### Android

#### Prerequisites

1. Install Android Studio with Android SDK and NDK
2. Install Java JDK (version 11 or later)
3. Set up environment variables:

```bash
export ANDROID_NDK_HOME=/path/to/ndk
export ANDROID_HOME=/path/to/android/sdk
export PATH=$ANDROID_HOME/platform-tools:$PATH
```

4. Install Rust Android targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android
```

5. Install cargo-apk:

```bash
cargo install cargo-apk
```

#### Build and Run

```bash
# Build for ARM64 (most modern devices)
cargo apk build --release --target aarch64-linux-android

# Build for multiple architectures (creates a fat APK)
cargo apk build --release

# Run on device/emulator
cargo apk run --release
# Or install the generated APK manually
adb install -r target/android-artifacts/release/apk/poketrack.apk
```

#### Important Notes

- The Android build requires a JDK (Java 11+) and the `backend-android-activity-06` feature of Slint, which compiles a small Java helper stub.
- Set `JAVA_HOME` (e.g. to an Android Studio bundled JBR) before building.
- Set `ANDROID_JAR` if Slint cannot auto-detect an installed Android platform:

```bash
export JAVA_HOME=/path/to/jdk
export ANDROID_JAR=$ANDROID_HOME/platforms/android-36/android.jar
```

- The Rust library is built as a `cdylib`. To build just the native library (without packaging an APK):

```bash
cargo install cargo-ndk
cargo ndk -t arm64-v8a -o target/android/jniLibs build --release
```

## Database

By default, data is stored in `poketrack.sqlite`. Override with:

```bash
./poketrack --db /path/to/database.sqlite [command]
```
