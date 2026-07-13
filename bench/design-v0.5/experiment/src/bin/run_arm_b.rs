//! NON-PRODUCTION. Arm-B runner for the v0.5 speculation-admission experiment.
//!
//! Runs all 5 pre-registered search tasks through the VM speculative search
//! (SpecDriver over the arena backend), with the differential oracle gate on
//! every executed candidate. Writes `results/arm_b.json` and
//! `results/arm_b_prompts.json`, and prints a summary table.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use mtl_spec_experiment as ex;

const MANIFEST: &str = env!("CARGO_MANIFEST_DIR");

/// Arm-B one-shot framing "completion": the tiny declaration the LLM emits once
/// per task (alphabet/bounds/start/target). Tokens ≈ this string.
fn framing_completion(id: &str) -> &'static str {
    match id {
        "a" => "space: k in [0,65536); program template: k:*15087- ; target stack: [42]",
        "b" => "start stack: [3 5]; alphabet: + - * : ~ ; program length <= 6; target stack: [64]",
        "c" => "loop x:=a*x+b from x0=1 repeated n times; a in [1,8], b in [-8,8], n in [1,16]; emit as 1 n[a*b+]. ; target x = 1000 (stack [1000])",
        "d" => "start: 1; ops: 3+ (=+3), 2* (=x2), 1- (=-1); at most 12 moves; target stack: [100]",
        "e" => "start stack: [1 2 3 4]; alphabet: ~ @ ^ _ : ; program length <= 8; target stack: [4 3 2 1]",
        "c2" => "loop x:=a*x+b from x0=1 repeated n times; a in [1,8], b in [-8,8], n in [1,16]; emit as 1 n[a*b+]. ; target x = 1093 (stack [1093])",
        "e2" => "start stack: [1 2 3 4]; alphabet: ~ @ ^ _ : ; program length <= 8; target stack: [1 2 3 4 4 4 4]",
        _ => "",
    }
}

fn read_statement(id: &str) -> String {
    let p: PathBuf = [MANIFEST, "prompts", &format!("task_{id}_statement.txt")]
        .iter()
        .collect();
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn median3(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[1]
}

/// Measure one task: an oracle-gated pass (winner + oracle agreement +
/// high-water) plus three oracle-free timing runs. Prints the summary row and
/// returns the per-task JSON object string.
fn measure_task(id: &str) -> String {
    // One oracle-gated pass: winner, candidates, oracle agreement, high-water.
    let r = ex::run_task(id, true);

    // Three timing runs WITHOUT the oracle (pure Arm-B search wall-clock).
    let mut times_ms = Vec::new();
    for _ in 0..3 {
        let t0 = Instant::now();
        let _ = ex::run_task(id, false);
        times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let med = median3(times_ms.clone());
    let cps = if med > 0.0 {
        r.candidates_explored as f64 / (med / 1000.0)
    } else {
        f64::INFINITY
    };

    // End-to-end glyph validation (same predicate as mtlrun) for found ones.
    let expected = ex::expected_ints(id);
    let validated = if r.found {
        ex::validate_glyphs(&r.winner_glyphs, &expected)
    } else {
        false
    };

    let found_str = if r.found {
        if validated { "YES" } else { "YES?" }
    } else if r.space_exhausted {
        "EXHAUST"
    } else {
        "BUDGET"
    };
    let winner_disp = if r.found { r.winner_glyphs.clone() } else { "(none)".into() };
    let arena_hw = format!("t{} s{} c{}", r.tape_hw, r.stack_hw, r.cont_hw);
    let oracle = format!("{}/{}", r.oracle_agree, r.oracle_checked);

    println!(
        "{:<4} {:<7} {:<28} {:>12} {:>10.3} {:>10.0} {:>12} {:>14}",
        id, found_str, winner_disp, r.candidates_explored, med, cps, arena_hw, oracle
    );

    format!(
        "    {{\n\
         \x20     \"task\": \"{}\",\n\
         \x20     \"found\": {},\n\
         \x20     \"space_exhausted\": {},\n\
         \x20     \"winning_program\": \"{}\",\n\
         \x20     \"validated_by_mtlrun_predicate\": {},\n\
         \x20     \"candidates_explored\": {},\n\
         \x20     \"vm_wall_ms\": [{:.4}, {:.4}, {:.4}],\n\
         \x20     \"vm_wall_ms_median\": {:.4},\n\
         \x20     \"candidates_per_sec\": {:.1},\n\
         \x20     \"arena_high_water\": {{ \"tape\": {}, \"stack_nodes\": {}, \"cont_nodes\": {} }},\n\
         \x20     \"core_steps_spent\": {},\n\
         \x20     \"oracle_checked\": {},\n\
         \x20     \"oracle_agree\": {}\n\
         \x20   }}",
        id,
        r.found,
        r.space_exhausted,
        json_escape(&winner_disp),
        validated,
        r.candidates_explored,
        times_ms[0],
        times_ms[1],
        times_ms[2],
        med,
        cps,
        r.tape_hw,
        r.stack_hw,
        r.cont_hw,
        r.spent,
        r.oracle_checked,
        r.oracle_agree,
    )
}

fn main() {
    let results_dir: PathBuf = [MANIFEST, "results"].iter().collect();
    fs::create_dir_all(&results_dir).unwrap();

    println!("=== Arm B (VM speculative search) — v0.5 admission experiment ===");
    println!("B (global fuel) per task = {}", ex::TOTAL_B);
    println!();

    // header
    println!(
        "{:<4} {:<7} {:<28} {:>12} {:>10} {:>10} {:>12} {:>14}",
        "task", "found", "winner (glyphs)", "cands", "med_ms", "cand/s", "arena_hw", "oracle"
    );
    println!("{}", "-".repeat(104));

    // ---- Primary pre-registered set (a..e) -> arm_b.json + arm_b_prompts.json.
    let mut json_tasks: Vec<String> = Vec::new();
    let mut prompt_tasks: Vec<String> = Vec::new();
    for id in ex::TASK_IDS {
        json_tasks.push(measure_task(id));
        let statement = read_statement(id);
        prompt_tasks.push(format!(
            "    {{\n\
             \x20     \"task\": \"{}\",\n\
             \x20     \"arm_b_framing_prompt\": \"{}\",\n\
             \x20     \"arm_b_completion\": \"{}\"\n\
             \x20   }}",
            id,
            json_escape(&statement),
            json_escape(framing_completion(id)),
        ));
    }

    let arm_b_json = format!(
        "{{\n  \"arm\": \"B (VM speculative search)\",\n  \"global_fuel_B\": {},\n  \"tasks\": [\n{}\n  ]\n}}\n",
        ex::TOTAL_B,
        json_tasks.join(",\n")
    );
    let prompts_json = format!(
        "{{\n  \"note\": \"Arm-B is one fixed framing prompt + one tiny completion per task. Token cost of Arm B ~= tokens(completion). The framing_prompt equals the shared task statement both arms receive.\",\n  \"tasks\": [\n{}\n  ]\n}}\n",
        prompt_tasks.join(",\n")
    );

    fs::write(results_dir.join("arm_b.json"), arm_b_json).unwrap();
    fs::write(results_dir.join("arm_b_prompts.json"), prompts_json).unwrap();

    // ---- Secondary VALIDITY-FIXED set (c2, e2) -> arm_b_fixed.json.
    // Reachable-target variants of (c) and (e). Kept OUT of the primary
    // pre-registered verdict; reported as a clearly-labelled secondary analysis.
    println!();
    println!("--- VALIDITY-FIXED secondary variants (reachable targets) ---");
    let mut fixed_tasks: Vec<String> = Vec::new();
    for id in ex::FIXED_TASK_IDS {
        fixed_tasks.push(measure_task(id));
    }
    let arm_b_fixed_json = format!(
        "{{\n  \"arm\": \"B (VM speculative search)\",\n  \"analysis\": \"SECONDARY — validity-fixed variants of tasks c and e with reachable targets. The pre-registered tasks c (target 1000) and e (target [4,3,2,1]) are provably infeasible in their spaces and remain in arm_b.json for the primary verdict; c2 (target 1093) and e2 (target [1,2,3,4,4,4,4]) swap in reachable targets on the identical search spaces.\",\n  \"global_fuel_B\": {},\n  \"tasks\": [\n{}\n  ]\n}}\n",
        ex::TOTAL_B,
        fixed_tasks.join(",\n")
    );
    fs::write(results_dir.join("arm_b_fixed.json"), arm_b_fixed_json).unwrap();

    println!();
    println!("wrote {}", results_dir.join("arm_b.json").display());
    println!("wrote {}", results_dir.join("arm_b_prompts.json").display());
    println!("wrote {}", results_dir.join("arm_b_fixed.json").display());
}
