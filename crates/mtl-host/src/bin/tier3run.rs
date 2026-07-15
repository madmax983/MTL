//! `tier3run` — the deterministic Tier-3 oracle binary.
//!
//! Usage: `tier3run <task_name>` reads an MTL program from STDIN, parses it,
//! builds THAT task's fixture + the correct grant set (restricted for the
//! `confined_*` tasks, `emit`-budgeted for `emit_budget`/`budget_grep`, standard
//! otherwise) + expected output, drives it under a fixed fuel bound, and prints
//! EXACTLY ONE deterministic, greppable verdict line to stdout:
//!
//!   * `PASS`                                     — Done and output matches.
//!   * `FAIL: wrong_output got=<repr> want=<repr>` — Done, output differs.
//!   * `FAIL: NotGranted <name>`                  — reached an ungranted cap.
//!   * `FAIL: BudgetExhausted` / `OutputCapExceeded` / `InputClosed` /
//!     `ToolError` / `Timeout`                    — the respective host fault.
//!   * `FAIL: FAULT:<Kind>`                       — a pure-core fault.
//!   * `FAIL: Cancelled`                          — fuel exhaustion.
//!   * `PARSE ERROR: <detail>`                    — the program did not parse.
//!
//! It ALWAYS exits 0 (the verdict is on stdout) — except an unknown task name,
//! which prints `unknown task` to stderr and exits 1. This binary is the oracle
//! the agent trial's repair loop and grant-violation detection depend on, so its
//! output is intentionally stable and line-oriented.

use std::io::Read;

use mtl_host::driver::{drive_with, Engine, HostCode, RunResult};
use mtl_host::caps::{task_setup, TaskSetup};

const FUEL: u64 = 100_000;

fn main() {
    // Args: `tier3run [--engine=arena|interp] <task_name>` (MTL program on stdin).
    // Default engine is the arena (refinement-proved); `--engine=interp` selects
    // the reference interpreter as the differential anchor. The verdict line is
    // byte-identical across engines.
    let mut engine = Engine::default();
    let mut task: Option<String> = None;
    for a in std::env::args().skip(1) {
        if let Some(val) = a.strip_prefix("--engine=") {
            engine = match Engine::parse(val) {
                Ok(e) => e,
                Err(msg) => {
                    eprintln!("tier3run: {msg}");
                    std::process::exit(1);
                }
            };
        } else if task.is_none() {
            task = Some(a);
        }
    }
    let task = match task {
        Some(t) => t,
        None => {
            eprintln!("usage: tier3run [--engine=arena|interp] <task_name>   (MTL program on stdin)");
            std::process::exit(1);
        }
    };

    // Build the task oracle BEFORE reading stdin so an unknown task fails fast.
    let TaskSetup {
        mut reg,
        mut ctx,
        expected_output,
    } = match task_setup(&task) {
        Some(s) => s,
        None => {
            eprintln!("unknown task");
            std::process::exit(1);
        }
    };

    let mut src = String::new();
    if std::io::stdin().read_to_string(&mut src).is_err() {
        println!("PARSE ERROR: could not read stdin");
        return;
    }
    // Match the tokcount / load_solution policy: strip a single trailing newline.
    let src = src.strip_suffix('\n').unwrap_or(&src);

    let parsed = match mtl_syntax::parse(src) {
        Ok(p) => p,
        Err(e) => {
            println!("PARSE ERROR: {e:?}");
            return;
        }
    };
    let prog = mtl_host::conv_program(&parsed);

    let verdict = match drive_with(engine, prog, vec![], FUEL, &mut reg, &mut ctx) {
        RunResult::Done(_) => {
            let got = ctx.output_utf8();
            if got == expected_output {
                "PASS".to_string()
            } else {
                format!("FAIL: wrong_output got={got:?} want={expected_output:?}")
            }
        }
        RunResult::HostFaulted(code) => match code {
            HostCode::NotGranted => match ctx.last_denied {
                Some(ref name) => format!("FAIL: NotGranted {name}"),
                None => "FAIL: NotGranted".to_string(),
            },
            HostCode::BudgetExhausted => "FAIL: BudgetExhausted".to_string(),
            HostCode::OutputCapExceeded => "FAIL: OutputCapExceeded".to_string(),
            HostCode::InputClosed => "FAIL: InputClosed".to_string(),
            HostCode::ToolError => "FAIL: ToolError".to_string(),
            HostCode::Timeout => "FAIL: Timeout".to_string(),
        },
        RunResult::Faulted(fault) => format!("FAIL: FAULT:{fault:?}"),
        RunResult::Cancelled => "FAIL: Cancelled".to_string(),
    };
    println!("{verdict}");
}
