//! Generate the sealed contamination manifest (`sealed/sealed.manifest.json`).
//!
//! Reads the real sealed task set at `bench/sealed/tasks.json` (the source of
//! truth, authored blind to the MTL glyph/primitive set) and writes the salted
//! `mtl-sealed-manifest/v2` object: one entry per task carrying the salted
//! `content_sha256`, the `canon::io_hash` over the task's I/O vectors, and an
//! empty `canonical_sha256` (reference solutions are withheld until the
//! post-freeze unseal). Fully deterministic — run any time, the output is
//! committed.
//!
//! Usage: `cargo run -p mtl-datagen --bin mkseal`

use std::path::PathBuf;

use mtl_datagen::sealed_spec::{self, SALT};

const GENERATED_FROM: &str = "bench/sealed/tasks.json";

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // bench/dataset/.. == bench ; the sealed task set lives at bench/sealed/.
    let tasks_path = manifest_dir.join("..").join("sealed").join("tasks.json");
    let data = std::fs::read_to_string(&tasks_path)
        .unwrap_or_else(|e| panic!("read sealed tasks {}: {e}", tasks_path.display()));
    let tasks = sealed_spec::parse_tasks(&data);
    let manifest = sealed_spec::manifest_from_tasks(&tasks, SALT, GENERATED_FROM);

    let out_dir = manifest_dir.join("sealed");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("sealed.manifest.json");
    let json = serde_json::to_string_pretty(&manifest).expect("manifest serializes");
    std::fs::write(&out, json + "\n").unwrap();
    eprintln!(
        "wrote {} entries (salt {}) to {}",
        manifest.entries.len(),
        manifest.salt,
        out.display()
    );
}
