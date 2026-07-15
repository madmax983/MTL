//! Generate the PLACEHOLDER sealed manifest (`sealed/sealed.manifest.json`).
//!
//! The real sealed-eval split is reserved-empty in-repo (recon §4); this emits a
//! small set of *reserved* canonical forms — programs the generators never
//! produce — with their real canonical SHA-256 and io-behavior hashes, so the
//! contamination gate has something concrete to exclude against until issue #53
//! authors the actual sealed tasks. Run once; the output is committed.
//!
//! Usage: `cargo run -p mtl-datagen --bin mkseal`

use mtl_core::interp::{Outcome, Value};

use mtl_datagen::canon::{canonical_sha, io_hash};
use mtl_datagen::contamination::SealedEntry;
use mtl_datagen::oracle::run_on;
use mtl_datagen::{Expected, IoVector};

fn grid1() -> Vec<i64> {
    vec![0, 1, -1, 2, -2, 7, -7, 100, -100, i64::MIN, i64::MAX]
}

fn io_for(src: &str, inputs: Vec<Vec<i64>>) -> Vec<IoVector> {
    inputs
        .into_iter()
        .map(|xs| {
            let stack: Vec<Value> = xs.iter().map(|n| Value::Int(*n)).collect();
            let expected = match run_on(src, &stack) {
                Some(Outcome::Halt(out)) => Expected::Halt(out),
                _ => Expected::Fault,
            };
            IoVector {
                input: stack,
                expected,
            }
        })
        .collect()
}

fn main() {
    // Reserved forms, guaranteed disjoint from every generator's output range.
    let reserved: Vec<(&str, &str, u8, Vec<Vec<i64>>)> = vec![
        (
            "seal_affine_100",
            "100*200+",
            0,
            grid1().into_iter().map(|n| vec![n]).collect(),
        ),
        (
            "seal_quartic",
            ":*:*",
            0,
            grid1().into_iter().map(|n| vec![n]).collect(),
        ),
        (
            "seal_mul321",
            "321*",
            0,
            grid1().into_iter().map(|n| vec![n]).collect(),
        ),
        (
            "seal_affine_55",
            "55*66+",
            0,
            grid1().into_iter().map(|n| vec![n]).collect(),
        ),
        ("seal_cat_lits", "[9][8],", 2, vec![vec![]]),
    ];

    let entries: Vec<SealedEntry> = reserved
        .into_iter()
        .map(|(id, src, tier, inputs)| {
            let (_, sha) = canonical_sha(src).expect("reserved form parses");
            let io = io_for(src, inputs);
            SealedEntry {
                task_id: id.to_string(),
                tier,
                canonical_sha256: sha,
                io_hash: io_hash(&io),
            }
        })
        .collect();

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sealed");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sealed.manifest.json");
    let json = serde_json::to_string_pretty(&entries).unwrap();
    std::fs::write(&path, json + "\n").unwrap();
    eprintln!("wrote {} entries to {}", entries.len(), path.display());
}
