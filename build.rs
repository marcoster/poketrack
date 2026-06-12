use std::path::Path;
use std::process::Command;

fn main() {
    let script = Path::new("scripts/export-json.ts");
    let en_data = Path::new("data/cards-database/data");
    let ja_data = Path::new("data/cards-database/data-asia");

    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", en_data.display());
    println!("cargo:rerun-if-changed={}", ja_data.display());

    if !en_data.exists() && !ja_data.exists() {
        println!("cargo:warning=cards-database submodule not initialized — run `git submodule update --init`");
        return;
    }

    let bun = find_bun();
    if bun.is_none() {
        panic!(
            "bun is required to generate card data. Install it from https://bun.sh"
        );
    }

    let status = Command::new(bun.unwrap())
        .args(["run", script.to_str().unwrap()])
        .status()
        .expect("failed to run export-json.ts");

    if !status.success() {
        panic!("export-json.ts failed with exit code: {:?}", status.code());
    }
}

fn find_bun() -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec!["bun.exe".to_string(), "bun.cmd".to_string()]
    } else {
        vec!["bun".to_string()]
    };
    for name in &candidates {
        if Command::new(name)
            .arg("--version")
            .output()
            .is_ok()
        {
            return Some(name.clone());
        }
    }
    None
}
