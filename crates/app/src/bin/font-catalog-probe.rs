//! Read-only discovery probe for the near-term desktop font picker.
#[path = "../font_catalog.rs"]
mod font_catalog;

fn main() {
    let started = std::time::Instant::now();
    let families = font_catalog::installed_families();
    let faces: usize = families.iter().map(|family| family.faces.len()).sum();
    let aliases: usize = families.iter().map(|family| family.aliases.len()).sum();
    let all_searchable = families.iter().all(|family| family.matches(&family.name));
    // Report counts, not the user's installed names, paths, or font data.
    println!(
        "{}",
        serde_json::json!({
            "families": families.len(), "faces": faces, "aliases": aliases,
            "self_search_matches": all_searchable, "scan_ms": started.elapsed().as_millis(),
            "os": std::env::consts::OS,
        })
    );
    if families.is_empty() || !all_searchable {
        std::process::exit(1);
    }
}
