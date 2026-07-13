//! `mtl-check` — CLI for the prototype static stack-effect checker.
//!
//! Subcommands:
//!   * (default) / `corpus` — check every corpus program, print an acceptance
//!     table, and dump a per-program verdict list to
//!     `bench/design-v0.6/corpus-verdicts.md`.
//!   * `check <file.mtl>` — check a single solution file and print the verdict.
//!   * `str  <program>`   — check a program passed as a literal argument.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mtl_check::Verdict;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("check") => {
            let path = args.get(2).expect("usage: mtl-check check <file.mtl>");
            let src = read_stripped(path);
            print_one(path, &src);
        }
        Some("str") => {
            let src = args.get(2).expect("usage: mtl-check str <program>");
            print_one("<arg>", src);
        }
        _ => run_corpus(),
    }
}

fn read_stripped(path: impl AsRef<Path>) -> String {
    let raw = std::fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("read {}: {e}", path.as_ref().display()));
    raw.strip_suffix('\n').unwrap_or(&raw).to_string()
}

fn print_one(name: &str, src: &str) {
    match mtl_check::check_str(src) {
        Err(e) => println!("{name}: PARSE ERROR: {e:?}"),
        Ok(verdict) => {
            println!("{name}: {}", verdict.tag());
            match &verdict {
                Verdict::Static(e) => println!("  effect: {e}"),
                Verdict::Guarded(e, obls) => {
                    println!("  effect: {e}");
                    for o in obls {
                        println!("  guard[{}] @{}: {}", o.kind, o.at_word_index, o.note);
                    }
                }
                Verdict::Reject { reason, at_word_index, expected, found } => {
                    println!("  reason: {reason}");
                    println!("  at word {at_word_index}: expected {expected}, found {found}");
                }
            }
        }
    }
}

/// A checked corpus entry.
struct Row {
    tier: String,
    name: String,
    program: String,
    verdict: Verdict,
}

fn run_corpus() {
    let root = repo_root();
    let mut rows: Vec<Row> = Vec::new();

    // v0.1 / v0.2 / v0.3 corpus solutions.
    for (tier, sub) in [("v0.1", "mtl"), ("v0.2", "mtl-v0.2"), ("v0.3", "mtl-v0.3")] {
        let mut paths = glob_solutions(&root.join("bench/corpus"), sub);
        paths.sort();
        for p in paths {
            let task = p
                .parent()
                .and_then(|d| d.parent())
                .and_then(|d| d.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            push_row(&mut rows, tier, &task, &read_stripped(&p));
        }
    }

    // tier-3 tasks (host Call words).
    let mut t3: Vec<PathBuf> = std::fs::read_dir(root.join("bench/tier3/tasks"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path().join("solution.mtl"))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default();
    t3.sort();
    for p in t3 {
        let task = p
            .parent()
            .and_then(|d| d.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        push_row(&mut rows, "tier-3", &task, &read_stripped(&p));
    }

    // agent-trial programs (mtl arm only).
    let att = root.join("bench/agent-trial/results/attempts");
    if let Ok(rd) = std::fs::read_dir(&att) {
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        files.sort();
        for f in files {
            if f.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let txt = std::fs::read_to_string(&f).unwrap_or_default();
            if let Some((program, arm)) = extract_agent_program(&txt) {
                // Only MTL-arm attempts carry an MTL program; skip python arms.
                if !arm.contains("mtl") {
                    continue;
                }
                let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                push_row(&mut rows, "agent-trial", &name, &program);
            }
        }
    }

    // Build the acceptance table.
    let mut counts: BTreeMap<String, [usize; 3]> = BTreeMap::new(); // [Static, Guarded, Reject]
    let order = ["v0.1", "v0.2", "v0.3", "tier-3", "agent-trial"];
    for r in &rows {
        let e = counts.entry(r.tier.clone()).or_default();
        match &r.verdict {
            Verdict::Static(_) => e[0] += 1,
            Verdict::Guarded(..) => e[1] += 1,
            Verdict::Reject { .. } => e[2] += 1,
        }
    }

    let mut table = String::new();
    writeln!(table, "| Tier | Static | Guarded | Reject | Total |").unwrap();
    writeln!(table, "|------|-------:|--------:|-------:|------:|").unwrap();
    let mut totals = [0usize; 3];
    for tier in order {
        if let Some(c) = counts.get(tier) {
            let t = c[0] + c[1] + c[2];
            writeln!(table, "| {tier} | {} | {} | {} | {} |", c[0], c[1], c[2], t).unwrap();
            for i in 0..3 {
                totals[i] += c[i];
            }
        }
    }
    let grand = totals[0] + totals[1] + totals[2];
    writeln!(
        table,
        "| **overall** | **{}** | **{}** | **{}** | **{}** |",
        totals[0], totals[1], totals[2], grand
    )
    .unwrap();

    print!("{table}");

    // Per-program verdict list + write file.
    let mut doc = String::new();
    writeln!(doc, "# MTL static checker — corpus verdicts\n").unwrap();
    writeln!(doc, "Generated by `mtl-check` (crate `crates/mtl-check`).\n").unwrap();
    writeln!(doc, "## Acceptance table\n").unwrap();
    doc.push_str(&table);
    writeln!(doc, "\n## Per-program verdicts\n").unwrap();
    writeln!(doc, "| Tier | Program | Verdict | Effect / Reason |").unwrap();
    writeln!(doc, "|------|---------|---------|-----------------|").unwrap();
    for tier in order {
        for r in rows.iter().filter(|r| r.tier == tier) {
            let (vtag, detail) = match &r.verdict {
                Verdict::Static(e) => ("Static".to_string(), format!("`{e}`")),
                Verdict::Guarded(e, obls) => {
                    let kinds: Vec<&str> = obls.iter().map(|o| o.kind.as_str()).collect();
                    ("Guarded".to_string(), format!("`{e}` — guards: {}", kinds.join(", ")))
                }
                Verdict::Reject { reason, at_word_index, .. } => {
                    ("Reject".to_string(), format!("@{at_word_index}: {reason}"))
                }
            };
            let prog = r.program.replace('|', "\\|");
            writeln!(doc, "| {} | {} `{}` | {} | {} |", r.tier, r.name, prog, vtag, detail).unwrap();
        }
    }

    // A few LLM-repair example messages captured verbatim.
    writeln!(doc, "\n## Example LLM-repair messages\n").unwrap();
    let examples = [
        ("[1]2+", "arithmetic type mismatch"),
        ("1!", "apply a non-quote"),
        ("[+]>", "uncons a malformed-head quote"),
        ("1 1[:][_]?", "branch-stack incompatibility"),
    ];
    writeln!(doc, "```").unwrap();
    for (src, label) in examples {
        if let Ok(Verdict::Reject { reason, at_word_index, expected, found }) = mtl_check::check_str(src) {
            writeln!(
                doc,
                "{src:<24} ({label})\n  at word {at_word_index}: expected {expected}; found {found} — {reason}\n"
            )
            .unwrap();
        }
    }
    writeln!(doc, "```").unwrap();

    let out_dir = root.join("bench/design-v0.6");
    std::fs::create_dir_all(&out_dir).ok();
    let out = out_dir.join("corpus-verdicts.md");
    std::fs::write(&out, doc).unwrap();
    println!("\nWrote {}", out.display());
    println!("Total programs checked: {grand}");
}

fn push_row(rows: &mut Vec<Row>, tier: &str, name: &str, src: &str) {
    match mtl_check::check_str(src) {
        Ok(verdict) => rows.push(Row {
            tier: tier.to_string(),
            name: name.to_string(),
            program: src.to_string(),
            verdict,
        }),
        Err(e) => {
            // A parse error is outside the checker's scope; record as a Reject.
            rows.push(Row {
                tier: tier.to_string(),
                name: name.to_string(),
                program: src.to_string(),
                verdict: Verdict::Reject {
                    reason: format!("parse error: {e:?}"),
                    at_word_index: 0,
                    expected: "a well-formed MTL program".into(),
                    found: "a parse error".into(),
                },
            });
        }
    }
}

/// Minimal JSON field extractor for the two fields we need (`program`, `arm`).
/// Avoids a serde dependency; the attempt files are flat objects with string
/// values, so a targeted scan is sufficient and robust for this corpus.
fn extract_agent_program(txt: &str) -> Option<(String, String)> {
    let program = json_string_field(txt, "program")?;
    let arm = json_string_field(txt, "arm").unwrap_or_default();
    Some((program, arm))
}

fn json_string_field(txt: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let mut idx = txt.find(&pat)? + pat.len();
    let bytes = txt.as_bytes();
    // skip whitespace and ':'
    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b':' {
        return None;
    }
    idx += 1;
    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b'"' {
        return None;
    }
    idx += 1;
    let mut out = String::new();
    let mut chars = txt[idx..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

fn glob_solutions(corpus: &Path, sub: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(corpus) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path().join(sub).join("solution.mtl");
            if p.exists() {
                out.push(p);
            }
        }
    }
    out
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mtl-check
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest)
}
