//! `mtlrun` — a fast dev harness for MTL programs (branch tier2-corpus).
//!
//! Reads an MTL program string from argv (joined) or, if no argv, from stdin,
//! parses it via `mtl-syntax`, converts via `conv_program`, runs it on the
//! `mtl-core` interpreter with FUEL = 100_000, and prints the final outcome.
//!
//! Inputs are provided by PREPENDING literals to the solution, exactly as
//! `tests/corpus.rs` does: e.g. to run factorial on 5, pass the program string
//! `5[1][*]&`. The whole string (input literals + solution) is parsed as one
//! program executed against an empty initial stack.
//!
//! For a `Halt`, the full final stack is printed bottom..top in readable form:
//!   ints as decimal, quotations as `[a b c]` (nested recursively). An empty
//!   stack prints as `<empty>`. Faults and fuel-exhaustion print diagnostic
//!   detail.

use std::io::Read;

use mtl_bench_validate::{conv_program, Engine};
use mtl_core::interp::{run, Outcome, Value, Vm, Word as IWord};
use mtl_syntax::parse;

const FUEL: u64 = 100_000;

/// Render a runtime `Value` in the readable surface form.
fn show_value(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Quote(ws) => show_quote(ws),
    }
}

/// Render a quotation body `[w0 w1 ...]`. Nested quotes recurse. Bare prims /
/// calls (only possible inside a literal quote, never as a produced value) are
/// shown via their glyph / name so output is always faithful.
fn show_quote(ws: &[IWord]) -> String {
    let mut parts = Vec::with_capacity(ws.len());
    for w in ws {
        parts.push(show_word(w));
    }
    format!("[{}]", parts.join(" "))
}

fn show_word(w: &IWord) -> String {
    match w {
        IWord::PushInt(n) => n.to_string(),
        IWord::PushQuote(body) => show_quote(body),
        IWord::Prim(p) => format!("{p:?}"),
        IWord::Call(s) => s.clone(),
    }
}

/// Render a whole stack bottom..top.
fn show_stack(stack: &[Value]) -> String {
    if stack.is_empty() {
        return "<empty>".to_string();
    }
    stack
        .iter()
        .map(show_value)
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    // Engine selection: `--engine=arena|interp` (default arena, the
    // refinement-proved backend). The flag is stripped from argv before the
    // remaining tokens are joined into the program source, so it works whether
    // the program comes from argv or stdin. Output for Halt/Fault is byte-
    // identical across engines — that identity is the whole point.
    let mut engine = Engine::default();
    let mut args: Vec<String> = Vec::new();
    for a in std::env::args().skip(1) {
        if let Some(val) = a.strip_prefix("--engine=") {
            engine = match Engine::parse(val) {
                Ok(e) => e,
                Err(msg) => {
                    eprintln!("mtlrun: {msg}");
                    std::process::exit(2);
                }
            };
        } else if a == "--engine" {
            eprintln!("mtlrun: --engine needs a value, e.g. --engine=arena");
            std::process::exit(2);
        } else {
            args.push(a);
        }
    }
    let src = if args.is_empty() {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .expect("failed to read stdin");
        s
    } else {
        args.join(" ")
    };
    let src = src.trim();

    let prog = match parse(src) {
        Ok(p) => p,
        Err(e) => {
            println!("PARSE ERROR: {e}");
            std::process::exit(2);
        }
    };
    let iprog = conv_program(&prog);

    // Program executed against an empty initial stack; input literals are part
    // of the program string (prepended by the caller), mirroring corpus.rs.
    // Both engines produce the identically-shaped `interp::Outcome`, so the
    // rendering below is a single code path — guaranteeing byte-identical output.
    let outcome = match engine {
        Engine::Arena => {
            let prog = mtl_arena::prog_from_interp(&iprog);
            mtl_arena::run_arena(&prog, FUEL).outcome().into_interp()
        }
        Engine::Interp => run(Vm::new(iprog), FUEL),
    };
    match outcome {
        Outcome::Halt(stack) => {
            println!("HALT: {}", show_stack(&stack));
        }
        Outcome::Fault(info) => {
            println!(
                "FAULT: {:?}\n  stack: {}\n  next:  {}",
                info.fault,
                show_stack(&info.stack),
                show_quote(&info.cont)
            );
        }
        Outcome::FuelExhausted { stack, cont } => {
            println!(
                "FUEL EXHAUSTED (fuel={FUEL})\n  stack: {}\n  cont-len: {}",
                show_stack(&stack),
                cont.len()
            );
        }
        Outcome::Invoke { name, stack, cont } => {
            println!(
                "INVOKE: {}\n  stack: {}\n  cont:  {}",
                name,
                show_stack(&stack),
                show_quote(&cont)
            );
        }
    }
}
