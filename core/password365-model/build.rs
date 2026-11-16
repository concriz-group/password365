//! Embeds the repository's `templates/` folder: the shared category fields
//! and one file per country (`templates/regions/<CODE>.json`). A country is
//! added by adding its file — no code changes.

#![allow(clippy::unwrap_used, clippy::expect_used)] // a build script should stop loudly

use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
    let regions_dir = root.join("regions");
    println!("cargo:rerun-if-changed={}", root.display());
    println!("cargo:rerun-if-changed={}", regions_dir.display());

    let mut regions: Vec<_> = fs::read_dir(&regions_dir)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    regions.sort();

    let mut code = String::new();
    let categories = root.join("categories.json");
    println!("cargo:rerun-if-changed={}", categories.display());
    writeln!(
        code,
        "pub(crate) const CATEGORIES: &str = include_str!({:?});",
        categories.display().to_string()
    )
    .unwrap();
    code.push_str("pub(crate) const REGIONS: &[(&str, &str)] = &[\n");
    for path in &regions {
        println!("cargo:rerun-if-changed={}", path.display());
        let code_name = path.file_stem().unwrap().to_string_lossy();
        writeln!(
            code,
            "    ({code_name:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    code.push_str("];\n");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("templates.rs");
    fs::write(out, code).unwrap();
}
