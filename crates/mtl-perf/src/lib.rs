//! Scenario builders and an instrumented driver for the MTL runtime-perf suite.
//!
//! Runtime performance is an explicit NON-goal of MTL (spec 1.2: "The reference
//! interpreter optimizes for provability, not speed."). This crate exists only
//! to *measure* the reference interpreter so the spec's REFACTOR phase (12.3 —
//! continuation-representation tuning) has honest data. The continuation is a
//! `Vec<Word>` used as a queue: the head is popped with `cont.remove(0)` (O(n))
//! every step, and re-emission splices with `prefix ++ cont` (O(n)). These
//! generators exercise both costs at increasing scale.

use mtl_core::interp::{exec_step, Prim, Step, Value, Vm, Word};

/// Fuel ceiling for the largest scenarios; also a runaway guard for the driver.
pub const BIG_FUEL: u64 = 50_000_000;

/// Terminal state of a driven VM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Halt,
    Fault,
    Exhausted,
}

/// Result of driving a VM: exact steps executed and peak `cont` length reached.
#[derive(Clone, Debug)]
pub struct RunStats {
    pub steps: u64,
    pub max_cont: usize,
    pub final_stack: Vec<Value>,
    pub outcome: Ending,
}

/// Drive `vm` up to `fuel` steps, counting steps and tracking peak `cont` length.
/// Mirrors `mtl_core::interp::run` but instrumented for measurement.
pub fn drive(mut vm: Vm, fuel: u64) -> RunStats {
    let mut steps: u64 = 0;
    let mut max_cont = vm.cont.len();
    loop {
        if steps >= fuel {
            return RunStats { steps, max_cont, final_stack: vm.stack, outcome: Ending::Exhausted };
        }
        match exec_step(&mut vm) {
            Step::Next => {
                steps += 1;
                let l = vm.cont.len();
                if l > max_cont {
                    max_cont = l;
                }
            }
            Step::Halt => {
                return RunStats { steps, max_cont, final_stack: vm.stack, outcome: Ending::Halt };
            }
            Step::Fault(_) => {
                return RunStats { steps, max_cont, final_stack: vm.stack, outcome: Ending::Fault };
            }
        }
    }
}

// ------------------------------------------------------------------ helpers
pub fn i(n: i64) -> Value {
    Value::Int(n)
}

/// A flat integer list literal `[e0 e1 ...]` as a single `Value::Quote`.
pub fn int_list(elems: &[i64]) -> Value {
    Value::Quote(elems.iter().map(|&e| Word::PushInt(e)).collect())
}

// ---------------------------------------------------- (a) straight-line dispatch
/// A balanced 4-word unit (`1 1 + _`) that nets zero stack effect, repeated
/// `units` times. Never faults. `4*units` steps of pure dispatch — and the
/// growing `cont.remove(0)` front-pop cost, since the whole program sits in
/// `cont` at once.
pub fn straightline(units: usize) -> Vec<Word> {
    let mut p = Vec::with_capacity(units * 4);
    for _ in 0..units {
        p.push(Word::PushInt(1));
        p.push(Word::PushInt(1));
        p.push(Word::Prim(Prim::Add));
        p.push(Word::Prim(Prim::Drop));
    }
    p
}

/// `0 n [1 +] .` — increment 0 to n via `Times`. The continuation stays ~5
/// words across all n iterations, so this is steady-state dispatch throughput
/// with a near-constant `cont` (no front-pop growth).
pub fn times_count(n: i64) -> (Vec<Value>, Vec<Word>) {
    let body = Word::PushQuote(vec![Word::PushInt(1), Word::Prim(Prim::Add)]);
    let prog = vec![body, Word::Prim(Prim::Times)];
    (vec![Value::Int(0), Value::Int(n)], prog)
}

// ------------------------------------------------ (b) `: !` self-application
/// `[ ^ 0 = [ _ ] [ [1 -] ' : ! ] ? ] : !` on stack `[n]` -> `[0]`.
/// Self-application recursion (dup-the-quote, apply) to depth `n`, doing only a
/// decrement per level — the minimal `: !` splice stressor, no arithmetic growth
/// so it runs to any depth without overflow.
pub fn selfapp_countdown(n: i64) -> (Vec<Value>, Vec<Word>) {
    let else_q = Word::PushQuote(vec![
        Word::PushQuote(vec![Word::PushInt(1), Word::Prim(Prim::Sub)]), // [1-]
        Word::Prim(Prim::Dip),                                          // '
        Word::Prim(Prim::Dup),                                          // :
        Word::Prim(Prim::Apply),                                        // !
    ]);
    let then_q = Word::PushQuote(vec![Word::Prim(Prim::Drop)]); // [_]
    let r = Word::PushQuote(vec![
        Word::Prim(Prim::Over), // ^
        Word::PushInt(0),
        Word::Prim(Prim::Eq), // =
        then_q,
        else_q,
        Word::Prim(Prim::If), // ?
    ]);
    let prog = vec![r, Word::Prim(Prim::Dup), Word::Prim(Prim::Apply)]; // [R] : !
    (vec![Value::Int(n)], prog)
}

// ------------------------------------------------------------- (c) primrec
/// `[0] [+] &` on `[n]` -> sum 0..=n. Primitive recursion of depth `n`.
/// Sum to 10_000 = 50_005_000, well within i64 — no overflow.
pub fn primrec_sumto(n: i64) -> (Vec<Value>, Vec<Word>) {
    let prog = vec![
        Word::PushQuote(vec![Word::PushInt(0)]),
        Word::PushQuote(vec![Word::Prim(Prim::Add)]),
        Word::Prim(Prim::PrimRec),
    ];
    (vec![Value::Int(n)], prog)
}

// -------------------------------------------------------------- (c) linrec
/// `[: 0 =] [_] [1 -] [] |` on `[n]` -> `[]`. Linear recursion of depth `n`
/// with an empty post-recursion step (`R2 = []`).
pub fn linrec_countdown(n: i64) -> (Vec<Value>, Vec<Word>) {
    let p = Word::PushQuote(vec![Word::Prim(Prim::Dup), Word::PushInt(0), Word::Prim(Prim::Eq)]);
    let t = Word::PushQuote(vec![Word::Prim(Prim::Drop)]);
    let r1 = Word::PushQuote(vec![Word::PushInt(1), Word::Prim(Prim::Sub)]);
    let r2 = Word::PushQuote(vec![]);
    let prog = vec![p, t, r1, r2, Word::Prim(Prim::LinRec)];
    (vec![Value::Int(n)], prog)
}

// ------------------------------------------------------------ (c)/(d) fold
/// `0 [+] (` over an `n`-element list `[1,1,...,1]` -> `[n]`.
pub fn fold_sum(n: usize) -> (Vec<Value>, Vec<Word>) {
    let list = int_list(&vec![1i64; n]);
    let prog = vec![
        Word::PushInt(0),
        Word::PushQuote(vec![Word::Prim(Prim::Add)]),
        Word::Prim(Prim::Fold),
    ];
    (vec![list], prog)
}

/// (d) Fold over a list whose ELEMENTS are quotations. `[] [_] (` — empty-quote
/// accumulator, combinator drops each quote element. Stresses spine-walking
/// with quote payloads rather than ints.
pub fn fold_quotes(n: usize) -> (Vec<Value>, Vec<Word>) {
    let elem = Word::PushQuote(vec![Word::PushInt(1)]);
    let list = Value::Quote(vec![elem; n]);
    let prog = vec![
        Word::PushQuote(vec![]),
        Word::PushQuote(vec![Word::Prim(Prim::Drop)]),
        Word::Prim(Prim::Fold),
    ];
    (vec![list], prog)
}

// ----------------------------------------------------------------- parser
/// A large but valid MTL source string of `units` repeated balanced fragments,
/// for parser-throughput benching.
pub fn gen_source(units: usize) -> String {
    let mut s = String::with_capacity(units * 22);
    for _ in 0..units {
        s.push_str("12 34 + [3 4 *]! : _ ");
    }
    s
}

// ----------------------------------------------------------------- corpus
pub mod corpus {
    use super::{i, int_list};
    use mtl_core::interp::Value;
    use std::path::PathBuf;

    /// One corpus solution + all its I/O vectors.
    pub struct CorpusCase {
        pub task: &'static str,
        pub version: &'static str, // "mtl" (v0.1) or "mtl-v0.3"
        pub inputs: Vec<Vec<Value>>,
        pub expected: Vec<Vec<Value>>,
    }

    pub fn corpus_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("bench")
            .join("corpus")
    }

    fn ql(xs: &[i64]) -> Value {
        int_list(xs)
    }

    /// T_v0 — the frozen v0.1 gate set (path `<task>/mtl/solution.mtl`).
    pub fn tv0_cases() -> Vec<CorpusCase> {
        vec![
            CorpusCase {
                task: "affine",
                version: "mtl",
                inputs: vec![vec![i(0)], vec![i(1)], vec![i(5)], vec![i(2)]],
                expected: vec![vec![i(7)], vec![i(10)], vec![i(22)], vec![i(13)]],
            },
            CorpusCase {
                task: "rev3",
                version: "mtl",
                inputs: vec![vec![i(1), i(2), i(3)], vec![i(7), i(8), i(9)]],
                expected: vec![vec![i(3), i(2), i(1)], vec![i(9), i(8), i(7)]],
            },
            CorpusCase {
                task: "is_even",
                version: "mtl",
                inputs: vec![vec![i(0)], vec![i(1)], vec![i(2)], vec![i(3)], vec![i(4)]],
                expected: vec![vec![i(1)], vec![i(0)], vec![i(1)], vec![i(0)], vec![i(1)]],
            },
            CorpusCase {
                task: "factorial",
                version: "mtl",
                inputs: vec![vec![i(0)], vec![i(1)], vec![i(2)], vec![i(3)], vec![i(5)], vec![i(6)]],
                expected: vec![vec![i(1)], vec![i(1)], vec![i(2)], vec![i(6)], vec![i(120)], vec![i(720)]],
            },
            CorpusCase {
                task: "gcd",
                version: "mtl",
                inputs: vec![
                    vec![i(12), i(8)],
                    vec![i(48), i(36)],
                    vec![i(17), i(5)],
                    vec![i(0), i(5)],
                    vec![i(5), i(0)],
                    vec![i(10), i(10)],
                ],
                expected: vec![vec![i(4)], vec![i(12)], vec![i(1)], vec![i(5)], vec![i(5)], vec![i(10)]],
            },
        ]
    }

    /// Tier-2 v0.3 — fold/xor sequence set (path `<task>/mtl-v0.3/solution.mtl`).
    pub fn tier2_v03_cases() -> Vec<CorpusCase> {
        vec![
            CorpusCase {
                task: "sum_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[1, 2, 3])], vec![ql(&[5])], vec![ql(&[10, 20, 30, 40])], vec![ql(&[])], vec![ql(&[0])]],
                expected: vec![vec![i(6)], vec![i(5)], vec![i(100)], vec![i(0)], vec![i(0)]],
            },
            CorpusCase {
                task: "length_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[1, 2, 3])], vec![ql(&[])], vec![ql(&[7, 7, 7, 7, 7])]],
                expected: vec![vec![i(3)], vec![i(0)], vec![i(5)]],
            },
            CorpusCase {
                task: "product_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[1, 2, 3, 4])], vec![ql(&[])], vec![ql(&[5])], vec![ql(&[2, 3, 0, 4])]],
                expected: vec![vec![i(24)], vec![i(1)], vec![i(5)], vec![i(0)]],
            },
            CorpusCase {
                task: "max_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[3, 1, 2])], vec![ql(&[5])], vec![ql(&[1, 9, 4, 9, 2])], vec![ql(&[10, 20, 5])]],
                expected: vec![vec![i(3)], vec![i(5)], vec![i(9)], vec![i(20)]],
            },
            CorpusCase {
                task: "min_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[3, 1, 2])], vec![ql(&[5])], vec![ql(&[9, 4, 9, 2])]],
                expected: vec![vec![i(1)], vec![i(5)], vec![i(2)]],
            },
            CorpusCase {
                task: "reverse_list",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[1, 2, 3])], vec![ql(&[])], vec![ql(&[7])], vec![ql(&[1, 2, 3, 4])]],
                expected: vec![vec![ql(&[3, 2, 1])], vec![ql(&[])], vec![ql(&[7])], vec![ql(&[4, 3, 2, 1])]],
            },
            CorpusCase {
                task: "contains",
                version: "mtl-v0.3",
                inputs: vec![
                    vec![ql(&[1, 2, 3]), i(2)],
                    vec![ql(&[1, 2, 3]), i(5)],
                    vec![ql(&[]), i(5)],
                    vec![ql(&[7]), i(7)],
                    vec![ql(&[4, 4, 4]), i(4)],
                ],
                expected: vec![vec![i(1)], vec![i(0)], vec![i(0)], vec![i(1)], vec![i(1)]],
            },
            CorpusCase {
                task: "count_occurrences",
                version: "mtl-v0.3",
                inputs: vec![
                    vec![ql(&[1, 2, 2, 3]), i(2)],
                    vec![ql(&[1, 2, 3]), i(5)],
                    vec![ql(&[]), i(5)],
                    vec![ql(&[4, 4, 4]), i(4)],
                    vec![ql(&[5, 5, 5, 5]), i(5)],
                ],
                expected: vec![vec![i(2)], vec![i(0)], vec![i(0)], vec![i(3)], vec![i(4)]],
            },
            CorpusCase {
                task: "single_number",
                version: "mtl-v0.3",
                inputs: vec![vec![ql(&[4, 1, 2, 1, 2])], vec![ql(&[2, 2, 7])], vec![ql(&[99])]],
                expected: vec![vec![i(4)], vec![i(7)], vec![i(99)]],
            },
            CorpusCase {
                task: "palindrome_number",
                version: "mtl-v0.3",
                inputs: vec![vec![i(121)], vec![i(123)], vec![i(7)], vec![i(1221)], vec![i(10)], vec![i(0)]],
                expected: vec![vec![i(1)], vec![i(0)], vec![i(1)], vec![i(1)], vec![i(0)], vec![i(1)]],
            },
            CorpusCase {
                task: "climbing_stairs",
                version: "mtl-v0.3",
                inputs: vec![vec![i(0)], vec![i(1)], vec![i(2)], vec![i(3)], vec![i(4)], vec![i(5)]],
                expected: vec![vec![i(1)], vec![i(1)], vec![i(2)], vec![i(3)], vec![i(5)], vec![i(8)]],
            },
        ]
    }
}
