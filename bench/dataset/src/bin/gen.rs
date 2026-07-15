//! `gen` — the dataset generation driver.
//!
//! CLI: `gen --count N --out DIR [--seed S]`
//!
//! Deterministic (seed-driven, no clock). Generates candidates, runs the REAL
//! oracle gate, canonicalizes + dedups, harvests repair traces (~20%), meters
//! coverage, runs the contamination gate, and writes `dataset.jsonl`,
//! `coverage.json`, `contamination_report.json`, `stats.json`. It FAILS
//! (non-zero exit) if the contamination gate finds a collision or the inline
//! re-validation invariant breaks.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::process::exit;

use mtl_core::interp::Fault;
use mtl_host::caps::task_setup;

use mtl_datagen::canon::{canonical_sha, io_hash, sha256_hex};
use mtl_datagen::contamination::{self, Item, SealedEntry};
use mtl_datagen::coverage::{self, Prog};
use mtl_datagen::oracle;
use mtl_datagen::repair::{self, RepairTrace};
use mtl_datagen::sft::{to_jsonl, Check, Kind, Record};
use mtl_datagen::{candidates, families, TaskInstance};

struct Args {
    count: usize,
    out: PathBuf,
    seed: u64,
    floor: u64,
}

fn parse_args() -> Args {
    let mut count = 1200usize;
    let mut out = PathBuf::from("pilot");
    let mut seed = 0u64;
    let mut floor = 3u64;
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--count" => {
                count = argv[i + 1].parse().expect("--count N");
                i += 2;
            }
            "--out" => {
                out = PathBuf::from(&argv[i + 1]);
                i += 2;
            }
            "--seed" => {
                seed = argv[i + 1].parse().expect("--seed S");
                i += 2;
            }
            "--floor" => {
                floor = argv[i + 1].parse().expect("--floor F");
                i += 2;
            }
            other => panic!("unknown arg {other}"),
        }
    }
    Args {
        count,
        out,
        seed,
        floor,
    }
}

/// The io-behavior hash for one instance (tier-3 uses the capability contract).
fn instance_io_hash(inst: &TaskInstance) -> String {
    match &inst.tier3_task {
        Some(task) => {
            let expected = task_setup(task)
                .map(|t| t.expected_output)
                .unwrap_or_default();
            sha256_hex(format!("tier3:{task}:{expected}").as_bytes())
        }
        None => io_hash(&inst.io),
    }
}

/// Build a gen Record from an accepted instance (returns the dedup key too).
fn gen_record(inst: &TaskInstance) -> Option<(Record, String)> {
    let (_, canon_sha) = canonical_sha(&inst.program)?;
    let io_sha = instance_io_hash(inst);
    let dedup_key = format!("{canon_sha}|{io_sha}");
    let instruction = if inst.tier3_task.is_some() {
        format!(
            "{}\nWrite an MTL program using the granted capabilities.",
            inst.description
        )
    } else {
        inst.description.clone()
    };
    let check = match &inst.tier3_task {
        Some(task) => Check {
            task: Some(task.clone()),
            vectors: Vec::new(),
        },
        None => oracle::check_from_io(&inst.io),
    };
    let rec = Record {
        instruction,
        response: mtl_datagen::canon::canonical(&inst.program)?,
        tier: inst.tier,
        family: inst.family.clone(),
        difficulty: inst.difficulty,
        kind: Kind::Gen,
        canonical_sha256: canon_sha,
        io_sha256: io_sha,
        check,
    };
    Some((rec, dedup_key))
}

fn main() {
    let args = parse_args();

    // ---- per-family acceptance tracking ----
    let mut pulled: BTreeMap<String, u64> = BTreeMap::new();
    let mut accepted: BTreeMap<String, u64> = BTreeMap::new();

    let mut seen: HashSet<String> = HashSet::new();
    let mut gen_records: Vec<Record> = Vec::new();
    // (source, tier, difficulty) for the coverage meter + repair seeds.
    let mut accepted_progs: Vec<(String, u8, u32, String)> = Vec::new(); // src,tier,diff,family

    // Budget split: reserve a slice of `count` for bottom-up enumeration so all
    // three candidate strategies are represented; mutation adds a small bonus.
    let enum_reserve = (args.count / 5).min(400);
    let template_cap = args.count.saturating_sub(enum_reserve);
    let mut_cap = 40usize;

    // ---- strategy 1: template synthesis, round-robin across families ----
    let groups = families::family_groups(args.seed);
    let mut iters: Vec<std::vec::IntoIter<TaskInstance>> =
        groups.into_iter().map(|g| g.into_iter()).collect();
    let mut active = true;
    while active && gen_records.len() < template_cap {
        active = false;
        for it in iters.iter_mut() {
            if gen_records.len() >= template_cap {
                break;
            }
            if let Some(inst) = it.next() {
                active = true;
                *pulled.entry(inst.family.clone()).or_insert(0) += 1;
                if oracle::gate(&inst).is_accept() {
                    if let Some((rec, key)) = gen_record(&inst) {
                        if seen.insert(key) {
                            *accepted.entry(inst.family.clone()).or_insert(0) += 1;
                            accepted_progs.push((
                                inst.program.clone(),
                                inst.tier,
                                inst.difficulty,
                                inst.family.clone(),
                            ));
                            gen_records.push(rec);
                        }
                    }
                }
            }
        }
    }

    // ---- strategy 2: mutation — acceptance-rate signal + alt-correct programs ----
    let mut mut_total = 0u64;
    let mut mut_accept = 0u64;
    // Re-deriving each seed's io-contract from families is heavy; instead we
    // re-gate its mutations against the contract lookup rebuilt below. Only the
    // source string is needed here.
    let sample: Vec<String> = accepted_progs
        .iter()
        .filter(|(_, tier, _, _)| *tier != 3)
        .take(60)
        .map(|(src, ..)| src.clone())
        .collect();
    // For the acceptance signal we need the seed's contract. Rebuild a lookup
    // from the family groups (regenerate; cheap and deterministic).
    let contract: BTreeMap<String, (Vec<mtl_datagen::IoVector>, u8, u32, String)> = {
        let mut m = BTreeMap::new();
        for g in families::family_groups(args.seed) {
            for inst in g {
                if inst.tier3_task.is_none() {
                    m.entry(inst.program.clone()).or_insert((
                        inst.io.clone(),
                        inst.tier,
                        inst.difficulty,
                        inst.family.clone(),
                    ));
                }
            }
        }
        m
    };
    let mut mut_added = 0usize;
    for src in &sample {
        if let Some((io, tier, diff, _fam)) = contract.get(src) {
            for m in candidates::mutations(src) {
                if m == *src {
                    continue;
                }
                mut_total += 1;
                if oracle::gate_tier02(&m, io).is_accept() {
                    mut_accept += 1;
                    if mut_added >= mut_cap {
                        continue;
                    }
                    if let Some((rec, key)) = gen_record(&TaskInstance {
                        family: "mutation".into(),
                        tier: *tier,
                        difficulty: *diff,
                        description: contract_desc(src),
                        io: io.clone(),
                        program: m.clone(),
                        tier3_task: None,
                    }) {
                        if seen.insert(key) {
                            mut_added += 1;
                            *accepted.entry("mutation".into()).or_insert(0) += 1;
                            *pulled.entry("mutation".into()).or_insert(0) += 1;
                            accepted_progs.push((m.clone(), *tier, *diff, "mutation".into()));
                            gen_records.push(rec);
                        }
                    }
                }
            }
        }
    }

    // ---- strategy 3: bottom-up enumeration — discovered tasks ----
    if gen_records.len() < args.count {
        let need = args.count - gen_records.len();
        let discovered = candidates::enumerate(need * 3);
        for inst in discovered {
            if gen_records.len() >= args.count {
                break;
            }
            *pulled.entry("enumerated".into()).or_insert(0) += 1;
            if oracle::gate(&inst).is_accept() {
                if let Some((rec, key)) = gen_record(&inst) {
                    if seen.insert(key) {
                        *accepted.entry("enumerated".into()).or_insert(0) += 1;
                        accepted_progs.push((
                            inst.program.clone(),
                            inst.tier,
                            inst.difficulty,
                            inst.family.clone(),
                        ));
                        gen_records.push(rec);
                    }
                }
            }
        }
    }

    let gen_count = gen_records.len();

    // ---- repair-trace harvesting (~20% of the final dataset) ----
    // total = gen + repair ; want repair/total ~= 0.20  =>  repair = gen/4.
    let repair_target = (gen_count / 4).max(4);
    let mut repair_records: Vec<Record> = Vec::new();
    let mut repair_seen: HashSet<String> = HashSet::new();
    let mut fault_hist: BTreeMap<String, u64> = BTreeMap::new();
    let kinds = [
        Fault::Underflow,
        Fault::TypeMismatch,
        Fault::DivByZero,
        Fault::Overflow,
    ];

    // 2a. targeted, guaranteed-balanced across the four core kinds.
    let mut k: i64 = 1 + (args.seed as i64 % 17);
    'outer: while repair_records.len() < repair_target {
        let before = repair_records.len();
        for kind in kinds {
            if let Some(tr) = repair::targeted(kind, k) {
                push_repair(&mut repair_records, &mut repair_seen, &mut fault_hist, tr);
            }
            if repair_records.len() >= repair_target {
                break 'outer;
            }
        }
        k += 1;
        if repair_records.len() == before {
            break; // no progress (all dups) — stop
        }
    }

    // 2b. organic: mutate accepted tier-0/2 seeds into real faults, for variety.
    for (src, tier, diff, _fam) in accepted_progs.iter() {
        if repair_records.len() >= repair_target {
            break;
        }
        if *tier == 3 {
            continue;
        }
        if let Some((io, _, _, _)) = contract.get(src) {
            let input = io
                .iter()
                .find_map(|v| match &v.expected {
                    mtl_datagen::Expected::Halt(_) => Some(v.input.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if let Some(tr) = repair::from_mutation(src, &input, io, *tier, *diff, None) {
                push_repair(&mut repair_records, &mut repair_seen, &mut fault_hist, tr);
            }
        }
    }

    // ---- assemble ----
    let mut all_records = gen_records.clone();
    all_records.extend(repair_records.clone());

    // ---- coverage meter (over every accepted response program) ----
    let mut cov_progs: Vec<Prog> = accepted_progs
        .iter()
        .map(|(s, t, d, _)| Prog {
            src: s.as_str(),
            tier: *t,
            difficulty: *d,
        })
        .collect();
    // include repair fixed programs
    let repair_fixed: Vec<(String, u8, u32)> = repair_records
        .iter()
        .map(|r| (r.response.clone(), r.tier, r.difficulty))
        .collect();
    for (s, t, d) in &repair_fixed {
        cov_progs.push(Prog {
            src: s.as_str(),
            tier: *t,
            difficulty: *d,
        });
    }
    let cov = coverage::measure(&cov_progs, args.floor);

    // ---- contamination gate ----
    let sealed = load_sealed();
    let items: Vec<Item> = all_records
        .iter()
        .map(|r| Item {
            canonical_sha256: r.canonical_sha256.clone(),
            io_hash: r.io_sha256.clone(),
        })
        .collect();
    let contam = contamination::gate(&items, &sealed);

    // ---- inline re-validation invariant: every response re-passes the oracle ----
    let mut reval_fail = 0u64;
    for rec in gen_records.iter().chain(repair_records.iter()) {
        if !revalidate(rec) {
            eprintln!("RE-VALIDATION FAILED: {} :: {}", rec.family, rec.response);
            reval_fail += 1;
        }
    }

    // ---- write outputs ----
    std::fs::create_dir_all(&args.out).unwrap();
    std::fs::write(args.out.join("dataset.jsonl"), to_jsonl(&all_records)).unwrap();
    std::fs::write(
        args.out.join("coverage.json"),
        serde_json::to_string_pretty(&cov).unwrap() + "\n",
    )
    .unwrap();
    std::fs::write(
        args.out.join("contamination_report.json"),
        serde_json::to_string_pretty(&contam).unwrap() + "\n",
    )
    .unwrap();

    // ---- stats.json ----
    let stats = build_stats(
        &args,
        &gen_records,
        &repair_records,
        &pulled,
        &accepted,
        mut_total,
        mut_accept,
        &fault_hist,
    );
    std::fs::write(
        args.out.join("stats.json"),
        serde_json::to_string_pretty(&stats).unwrap() + "\n",
    )
    .unwrap();

    eprintln!(
        "gen: {} pairs ({} gen, {} repair) -> {}",
        all_records.len(),
        gen_count,
        repair_records.len(),
        args.out.display()
    );
    eprintln!("coverage holes: {:?}", cov.holes);
    eprintln!(
        "contamination: {} collisions across {} sealed items",
        contam.collisions.len(),
        contam.sealed_items
    );

    // ---- fail-loud invariants ----
    let mut failed = false;
    if !contam.is_clean() {
        eprintln!(
            "FATAL: contamination gate found collisions: {:?}",
            contam.collisions
        );
        failed = true;
    }
    if reval_fail > 0 {
        eprintln!("FATAL: {reval_fail} records failed re-validation");
        failed = true;
    }
    if failed {
        exit(1);
    }
}

fn contract_desc(src: &str) -> String {
    format!("Write an MTL program equivalent to `{src}` (produces the same output on every input).")
}

fn push_repair(
    records: &mut Vec<Record>,
    seen: &mut HashSet<String>,
    hist: &mut BTreeMap<String, u64>,
    tr: RepairTrace,
) {
    let (canon, sha) = match canonical_sha(&tr.fixed) {
        Some(v) => v,
        None => return,
    };
    let key = format!("{}=>{}", tr.broken, canon);
    if !seen.insert(key) {
        return;
    }
    let io_sha = io_hash(&tr.io);
    let instruction = repair::instruction(&tr.broken, &tr.fault_turn);
    records.push(Record {
        instruction,
        response: canon,
        tier: tr.tier,
        family: "repair".into(),
        difficulty: tr.difficulty,
        kind: Kind::Repair,
        canonical_sha256: sha,
        io_sha256: io_sha,
        check: oracle::check_from_io(&tr.io),
    });
    *hist.entry(format!("{:?}", tr.fault_kind)).or_insert(0) += 1;
}

/// Re-run the response program through the REAL oracle using the embedded
/// re-runnable contract, and assert canonical-form stability. This is the same
/// invariant the `tests/revalidation.rs` suite reloads and re-checks.
fn revalidate(rec: &Record) -> bool {
    let canon_ok = match canonical_sha(&rec.response) {
        Some((canon, sha)) => canon == rec.response && sha == rec.canonical_sha256,
        None => false,
    };
    canon_ok && oracle::check_ok(&rec.response, &rec.check)
}

fn load_sealed() -> Vec<SealedEntry> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("sealed")
        .join("sealed.manifest.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => contamination::parse_manifest(&s),
        Err(_) => Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_stats(
    args: &Args,
    gen_records: &[Record],
    repair_records: &[Record],
    pulled: &BTreeMap<String, u64>,
    accepted: &BTreeMap<String, u64>,
    mut_total: u64,
    mut_accept: u64,
    fault_hist: &BTreeMap<String, u64>,
) -> serde_json::Value {
    use serde_json::json;
    let total = gen_records.len() + repair_records.len();

    // per-family acceptance rate
    let mut fam_stats = serde_json::Map::new();
    for (fam, p) in pulled {
        let a = accepted.get(fam).copied().unwrap_or(0);
        fam_stats.insert(
            fam.clone(),
            json!({
                "pulled": p,
                "accepted": a,
                "acceptance_rate": if *p > 0 { a as f64 / *p as f64 } else { 0.0 },
            }),
        );
    }

    // tier breakdown
    let mut tier_counts: BTreeMap<String, u64> = BTreeMap::new();
    for r in gen_records.iter().chain(repair_records.iter()) {
        *tier_counts.entry(r.tier.to_string()).or_insert(0) += 1;
    }

    // char totals + heuristic token estimate (glyph≈1 token, English≈chars/4)
    let mut instr_chars = 0usize;
    let mut resp_chars = 0usize;
    let mut heuristic_tokens = 0f64;
    for r in gen_records.iter().chain(repair_records.iter()) {
        instr_chars += r.instruction.chars().count();
        resp_chars += r.response.chars().count();
        heuristic_tokens +=
            r.instruction.chars().count() as f64 / 4.0 + r.response.chars().count() as f64;
        // MTL glyphs ~1 token each
    }

    json!({
        "seed": args.seed,
        "count_requested": args.count,
        "total_pairs": total,
        "gen_pairs": gen_records.len(),
        "repair_pairs": repair_records.len(),
        "repair_fraction": if total > 0 { repair_records.len() as f64 / total as f64 } else { 0.0 },
        "tier_breakdown": tier_counts,
        "per_family": fam_stats,
        "mutation_acceptance": {
            "total": mut_total,
            "accepted": mut_accept,
            "rate": if mut_total > 0 { mut_accept as f64 / mut_total as f64 } else { 0.0 },
        },
        "repair_fault_kinds": fault_hist,
        "chars": {
            "instruction_total": instr_chars,
            "response_total": resp_chars,
            "grand_total": instr_chars + resp_chars,
        },
        "heuristic_token_estimate": heuristic_tokens as u64,
        "tiktoken_note": "exact o200k/cl100k totals are added by bench/dataset/stats.py",
    })
}
