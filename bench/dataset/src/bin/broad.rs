//! `broad` — the v0.8 broad-distribution shape emitter.
//!
//! Sweeps the family generators across a seed range, runs every tier-0/2
//! instance through the REAL oracle gate, dedups by canonical MTL program, and
//! emits one JSON record per distinct task shape with a stable `template_key`
//! plus the integer args a per-family idiomatic-Python template needs. Tier-3
//! capability tasks are excluded (they are I/O-capability programs, not
//! static-compressible glyph programs).
//!
//! SPLIT CONVENTION (reproducible, seed-independent, family-balanced): each
//! distinct MTL program is assigned to TRAIN or DEV by the low bit of the first
//! hex nibble of its canonical SHA-256 — even nibble -> train, odd -> dev. This
//! is documented in `bench/design-v0.8/BROAD-DISTRIBUTION.md`. Raw seed parity
//! is rejected because most scan/bitdigit programs are seed-invariant and would
//! all land in a single split; the sha-parity split gives both sides coverage
//! of every family. The seed SWEEP still supplies breadth for the
//! seed-parameterized families (affine/lincomb2/predicate/quotation offsets).
//!
//! CLI: `broad [--seeds N] [--out FILE]`  (defaults: 6 seeds, stdout).

use std::collections::BTreeMap;
use std::io::Write;

use mtl_datagen::canon::canonical_sha;
use mtl_datagen::families::family_groups;
use mtl_datagen::oracle::gate;
use mtl_datagen::{canon::io_hash, TaskInstance};

/// Extract the runs of ASCII digits from `s` as i64 (params live in programs).
fn ints_in(s: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.parse().unwrap());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.parse().unwrap());
    }
    out
}

/// Map (family, canonical program) -> (template_key, args). Returns None for a
/// shape we deliberately do not measure statically (e.g. tier-3 capability).
fn classify(fam: &str, prog: &str) -> Option<(String, Vec<i64>)> {
    let k = |s: &str| Some((s.to_string(), Vec::<i64>::new()));
    match fam {
        "arithmetic" => {
            if prog == "+" {
                return k("binop_add");
            }
            if prog == "-" {
                return k("binop_sub");
            }
            if prog == "*" {
                return k("binop_mul");
            }
            if prog == "/" {
                return k("binop_div");
            }
            if prog == "%" {
                return k("binop_mod");
            }
            if let Some(rest) = prog.strip_prefix(':') {
                // square :*{b}+
                if rest.starts_with('*') {
                    return Some(("square".into(), ints_in(prog)));
                }
            }
            if prog.contains("*~") {
                // lincomb2 {a}*~{b}*+
                return Some(("lincomb2".into(), ints_in(prog)));
            }
            // affine {a}*{b}+
            Some(("affine".into(), ints_in(prog)))
        }
        "predicate" => {
            match prog {
                "0=" => return k("is_zero"),
                "0<" => return k("is_neg"),
                "0~<" => return k("is_pos"),
                "2%0=" => return k("is_even"),
                "=" => return k("eq2"),
                "<" => return k("lt2"),
                _ => {}
            }
            if let Some(kk) = prog.strip_suffix('=') {
                if kk.chars().all(|c| c.is_ascii_digit()) {
                    return Some(("eq_k".into(), ints_in(prog)));
                }
            }
            if let Some(kk) = prog.strip_suffix('<') {
                if kk.chars().all(|c| c.is_ascii_digit()) {
                    return Some(("lt_k".into(), ints_in(prog)));
                }
            }
            None
        }
        "stack-shuffle" => {
            let key = match prog {
                ":" => "dup",
                "_" => "drop2",
                "~" => "swap",
                "^" => "over",
                "~_" => "nip",
                "@" => "rot3",
                "~@" => "rev3",
                "::" => "triple",
                _ => return None,
            };
            Some((format!("shuffle_{key}"), Vec::new()))
        }
        "recursion" => {
            match prog {
                "[1][*]&" => return k("factorial"),
                "[0][+]&" => return k("sum_to"),
                "0 1@[~^+]._" => return k("fib"),
                "[:0=][_][~^%][]|" => return k("gcd"),
                "1~[^*].~_" => return k("power"),
                _ => {}
            }
            if prog.starts_with("0~[") && prog.ends_with("+].") {
                return Some(("times_mul".into(), ints_in(prog)));
            }
            None
        }
        "fold" => {
            let key = match prog {
                "0[+](" => "list_sum",
                "1[*](" => "list_product",
                "0[_1+](" => "list_length",
                ">_~[^^<[~_][_]?](" => "list_max",
                ">_~[^^<[_][~_]?](" => "list_min",
                "[>0=][0][][$]|" => "list_xor",
                "[][~;](" => "list_reverse",
                _ => return None,
            };
            k(key)
        }
        "bitwise" => {
            if prog == "$" {
                return k("xor2");
            }
            None
        }
        "quotation" => {
            if prog == "," {
                return k("cat2");
            }
            if prog.starts_with('[') && prog.ends_with("+]!") {
                return Some(("apply_add_k".into(), ints_in(prog)));
            }
            if prog.starts_with('[') && prog.ends_with("+]'") {
                return Some(("dip_add_k".into(), ints_in(prog)));
            }
            if prog.starts_with('[') && prog.ends_with("];") {
                return Some(("cons_k".into(), ints_in(prog)));
            }
            if prog.starts_with('[') && prog.ends_with("],") {
                return Some(("append_k".into(), ints_in(prog)));
            }
            None
        }
        "bitdigit" => {
            if prog == ":0<[0~-][]?0~[:0=][_][:2/~2%@+~][]|" {
                return k("popcount");
            }
            if prog.contains("@+~") {
                // digit-sum base b: base is the first literal after the guard.
                let ints = ints_in(prog); // [0,0,b,b] -> take b
                let b = ints.into_iter().find(|&x| x >= 2).unwrap_or(10);
                return Some(("digit_sum_base".into(), vec![b]));
            }
            if prog.contains("@*~") {
                let ints = ints_in(prog);
                let b = ints.into_iter().find(|&x| x >= 2).unwrap_or(10);
                return Some(("digit_product_base".into(), vec![b]));
            }
            None
        }
        "scan" => {
            let key = match prog {
                "[>0=][0][][-]|" => "alt_sum",
                "[:>[~_][[]]?>[~_][[]]?>[__0][1]?][_0][:>_>_>__^<@@<*~>_~_][+]|" => "local_maxima",
                "[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|" => {
                    "max_adj_diff"
                }
                "[][^>[_^=][0]?[_][~;]?]([][~;](" => "dedup_adj",
                "[][~>[@:@:>__~[=]'~[~_~1+~;][~[;]'~;1~;]?][[];1~;]?]([][~;](" => "rle_flatten",
                "^~[>0=][_][[+:[^^<[_][~_]?]']'][]|" => "min_running_balance",
                _ => return None,
            };
            k(key)
        }
        _ => None,
    }
}

fn main() {
    let mut seeds = 6u64;
    let mut out_path: Option<String> = None;
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--seeds" => {
                seeds = argv[i + 1].parse().unwrap();
                i += 2;
            }
            "--out" => {
                out_path = Some(argv[i + 1].clone());
                i += 2;
            }
            other => panic!("unknown arg {other}"),
        }
    }

    // distinct-by-canonical-program, first occurrence wins.
    let mut seen: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut skipped: BTreeMap<String, u64> = BTreeMap::new();
    let mut gated = 0u64;
    let mut rejected = 0u64;

    for seed in 0..seeds {
        for group in family_groups(seed) {
            for inst in group {
                let inst: TaskInstance = inst;
                if inst.tier == 3 || inst.tier3_task.is_some() {
                    continue;
                }
                gated += 1;
                if !gate(&inst).is_accept() {
                    rejected += 1;
                    continue;
                }
                let (canon, sha) = match canonical_sha(&inst.program) {
                    Some(v) => v,
                    None => continue,
                };
                if seen.contains_key(&canon) {
                    continue;
                }
                let (tkey, args) = match classify(&inst.family, &canon) {
                    Some(v) => v,
                    None => {
                        *skipped.entry(inst.family.clone()).or_default() += 1;
                        continue;
                    }
                };
                // split by low bit of first hex nibble of the canonical sha.
                let nibble = u8::from_str_radix(&sha[0..1], 16).unwrap();
                let split = if nibble % 2 == 0 { "train" } else { "dev" };
                let rec = serde_json::json!({
                    "family": inst.family,
                    "tier": inst.tier,
                    "difficulty": inst.difficulty,
                    "template_key": tkey,
                    "args": args,
                    "program": canon,
                    "canonical_sha256": sha,
                    "io_sha256": io_hash(&inst.io),
                    "split": split,
                    "description": inst.description,
                });
                seen.insert(canon, rec);
            }
        }
    }

    let recs: Vec<serde_json::Value> = seen.into_values().collect();
    let payload = serde_json::json!({
        "schema": "mtl-broad-shapes/v1",
        "seeds_swept": seeds,
        "gated": gated,
        "rejected": rejected,
        "distinct_shapes": recs.len(),
        "skipped_unclassified": skipped,
        "shapes": recs,
    });
    let text = serde_json::to_string_pretty(&payload).unwrap() + "\n";
    match out_path {
        Some(p) => std::fs::write(&p, text).unwrap(),
        None => std::io::stdout().write_all(text.as_bytes()).unwrap(),
    }
    eprintln!(
        "broad: {} distinct shapes ({} gated, {} rejected) skipped-unclassified={:?}",
        recs.len(),
        gated,
        rejected,
        payload["skipped_unclassified"]
    );
}
