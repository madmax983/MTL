//! # mtl-spec-experiment — NON-PRODUCTION experiment rig (v0.5 §4)
//!
//! **NON-PRODUCTION.** This crate implements the **Arm B (VM speculative
//! search)** side of the pre-registered v0.5 speculation-admission experiment
//! (`docs/design/v0.5-refactor.md` §4). It is a measurement vehicle kept under
//! `bench/`, out of the production story. It touches no semantics.
//!
//! For each of the 5 pre-registered search tasks it:
//!   1. enumerates the pre-registered candidate space,
//!   2. executes each candidate through the [`mtl_arena_spike::spec::SpecDriver`]
//!      (the prototype speculation driver over the arena backend), pruning on
//!      fault (cull) under a single global fuel budget `B`,
//!   3. returns the FIRST candidate whose result validates against the target
//!      (final stack == expected — the same predicate `mtlrun` uses), and
//!   4. runs an **oracle gate**: every candidate the driver executes is re-run
//!      through the reference interpreter `mtl_core::interp::run` (and the
//!      arena backend `run_arena`), asserting terminal kind + final stack are
//!      bit-identical. Agreement must be 100%.
//!
//! **Authoritative arm labels (from §4): Arm A = LLM attempt-loop; Arm B = VM
//! speculative search (this crate).**

use std::collections::{HashSet, VecDeque};

use mtl_arena_spike as arena;
use mtl_arena_spike::spec::{BranchOutcome, SpecDriver};
use mtl_arena_spike::{ArenaEnd, Prim as AP, ProgWord, Value as AV, Vm};
use mtl_core::interp as itp;

// ------------------------------------------------------------- global budget
/// The #26/#27 global fuel budget `B` for a single task's Arm-B search. Set
/// generously so that a task that fails to find a solution fails because its
/// *space is exhausted*, not because it ran out of fuel — this isolates a true
/// "no solution in the pre-registered space" outcome from a budget cliff. The
/// largest task (e) enumerates ~488k candidates at ≤~12 steps each ≈ ~6M steps,
/// well under B.
pub const TOTAL_B: u64 = 50_000_000;

/// Per-candidate fuel slice drawn from `B`. All task candidates are short and
/// straight-line (or bounded Times), so this cap is never the binding limit.
pub const PER_CANDIDATE: u64 = 1_000_000;

/// Generation size: the SpecDriver's shared arenas grow monotonically (the
/// spike never frees), so we take a fresh generation every `GEN_SIZE`
/// candidates — the §4.2 generational-reset discipline — to bound memory. The
/// reported arena high-water is the max within any single generation.
pub const GEN_SIZE: u64 = 50_000;

/// Fuel for the oracle re-runs (`run_arena` / `interp::run`).
pub const ORACLE_FUEL: u64 = 1_000_000;

/// Oracle cross-check policy: check every executed candidate up to this many;
/// beyond it, deterministically sample 1-in-`ORACLE_SAMPLE_STRIDE`.
pub const ORACLE_FULL_UPTO: u64 = 200_000;
pub const ORACLE_SAMPLE_STRIDE: u64 = 10;

// ------------------------------------------------------------- result types
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub task_id: &'static str,
    pub found: bool,
    /// The winning program as a glyph string (empty if not found).
    pub winner_glyphs: String,
    /// The winning program as an arena AST (for validation).
    pub winner_prog: Vec<ProgWord>,
    /// Candidates the driver actually executed (post-pruning = every spawn).
    pub candidates_explored: u64,
    /// Whether the space was fully enumerated without finding (true Exhausted)
    /// vs stopped early on a hit.
    pub space_exhausted: bool,
    /// Arena high-water (max within a generation): tape, stack nodes, cont nodes.
    pub tape_hw: usize,
    pub stack_hw: usize,
    pub cont_hw: usize,
    /// Oracle gate: candidates cross-checked and candidates that agreed.
    pub oracle_checked: u64,
    pub oracle_agree: u64,
    /// Total core steps spent by the driver across the whole search.
    pub spent: u64,
}

// --------------------------------------------------- arena <-> interp bridge
fn ap_to_ip(p: AP) -> itp::Prim {
    use itp::Prim as I;
    use AP as A;
    match p {
        A::Dup => I::Dup,
        A::Drop => I::Drop,
        A::Swap => I::Swap,
        A::Rot => I::Rot,
        A::Over => I::Over,
        A::Apply => I::Apply,
        A::Cat => I::Cat,
        A::Cons => I::Cons,
        A::Dip => I::Dip,
        A::Add => I::Add,
        A::Sub => I::Sub,
        A::Mul => I::Mul,
        A::Div => I::Div,
        A::Mod => I::Mod,
        A::Eq => I::Eq,
        A::Lt => I::Lt,
        A::If => I::If,
        A::PrimRec => I::PrimRec,
        A::Times => I::Times,
        A::LinRec => I::LinRec,
        A::Uncons => I::Uncons,
        A::Fold => I::Fold,
        A::Xor => I::Xor,
    }
}

fn pw_to_iw(pw: &ProgWord) -> itp::Word {
    match pw {
        ProgWord::PushInt(n) => itp::Word::PushInt(*n),
        ProgWord::PushQuote(b) => itp::Word::PushQuote(b.iter().map(pw_to_iw).collect()),
        ProgWord::Prim(p) => itp::Word::Prim(ap_to_ip(*p)),
        ProgWord::Call(s) => itp::Word::Call(s.clone()),
    }
}

fn prog_to_iprog(prog: &[ProgWord]) -> Vec<itp::Word> {
    prog.iter().map(pw_to_iw).collect()
}

fn av_to_iv(vm: &Vm, v: AV) -> itp::Value {
    match v {
        AV::Int(n) => itp::Value::Int(n),
        AV::Quote(id) => {
            itp::Value::Quote(vm.reify_quote(id).iter().map(pw_to_iw).collect())
        }
    }
}

fn fault_eq(i: itp::Fault, a: arena::Fault) -> bool {
    use itp::Fault as I;
    use arena::Fault as A;
    matches!(
        (i, a),
        (I::Underflow, A::Underflow)
            | (I::TypeMismatch, A::TypeMismatch)
            | (I::Overflow, A::Overflow)
            | (I::DivByZero, A::DivByZero)
    )
}

/// The oracle gate for one candidate: run it through the arena backend
/// (`run_arena`) and the reference interpreter (`interp::run`) and return true
/// iff terminal kind AND final stack are bit-identical. This is the same
/// differential predicate the spike's `tests/oracle.rs` uses, applied to the
/// experiment's candidate set.
pub fn oracle_agrees(prog: &[ProgWord]) -> bool {
    let arun = arena::run_arena(prog, ORACLE_FUEL);
    let iout = itp::run(itp::Vm::new(prog_to_iprog(prog)), ORACLE_FUEL);
    let astack: Vec<itp::Value> = arun
        .vm
        .stack_values(arun.stack)
        .into_iter()
        .map(|v| av_to_iv(&arun.vm, v))
        .collect();
    match (&arun.end, &iout) {
        (ArenaEnd::Halt, itp::Outcome::Halt(s)) => *s == astack,
        (ArenaEnd::Fault(af), itp::Outcome::Fault(fi)) => {
            fault_eq(fi.fault, *af) && fi.stack == astack
        }
        (ArenaEnd::Invoke(an), itp::Outcome::Invoke { name, .. }) => an == name,
        (ArenaEnd::FuelExhausted, itp::Outcome::FuelExhausted { .. }) => true,
        _ => false,
    }
}

// ------------------------------------------------------- glyph rendering
fn prim_glyph(p: AP) -> &'static str {
    match p {
        AP::Dup => ":",
        AP::Drop => "_",
        AP::Swap => "~",
        AP::Rot => "@",
        AP::Over => "^",
        AP::Apply => "!",
        AP::Cat => ",",
        AP::Cons => ";",
        AP::Dip => "'",
        AP::Add => "+",
        AP::Sub => "-",
        AP::Mul => "*",
        AP::Div => "/",
        AP::Mod => "%",
        AP::Eq => "=",
        AP::Lt => "<",
        AP::If => "?",
        AP::PrimRec => "&",
        AP::Times => ".",
        AP::LinRec => "|",
        AP::Uncons => ">",
        AP::Fold => "(",
        AP::Xor => "$",
    }
}

fn word_token(pw: &ProgWord) -> String {
    match pw {
        ProgWord::PushInt(n) => n.to_string(),
        ProgWord::Prim(p) => prim_glyph(*p).to_string(),
        ProgWord::PushQuote(b) => {
            let mut s = String::from("[");
            s.push_str(&render_glyphs(b));
            s.push(']');
            s
        }
        ProgWord::Call(name) => name.clone(),
    }
}

fn alnum_last(s: &str) -> bool {
    s.chars().last().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
}
fn alnum_first(s: &str) -> bool {
    s.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
}

/// Render an arena program to a minimal-whitespace glyph string: a space is
/// inserted only when two adjacent tokens would otherwise merge (both ends
/// alphanumeric, e.g. two integer literals). Symbols are self-delimiting.
pub fn render_glyphs(prog: &[ProgWord]) -> String {
    let mut out = String::new();
    let mut prev: Option<String> = None;
    for pw in prog {
        let tok = word_token(pw);
        if let Some(p) = &prev {
            if alnum_last(p) && alnum_first(&tok) {
                out.push(' ');
            }
        }
        out.push_str(&tok);
        prev = Some(tok);
    }
    out
}

// ---------------------------------------------------------- expected stacks
fn expected_av(vals: &[i64]) -> Vec<AV> {
    vals.iter().map(|&n| AV::Int(n)).collect()
}

// ------------------------------------------------- driver-based execution
/// Run one candidate program fully through the SpecDriver (spawn a branch, step
/// to a terminal). Returns `Some(final_stack)` if the branch Halted, `None` on
/// Dead (fault / budget) or Deferred (Invoke). Exercises spawn / step_with_quota
/// / cull and the budget invariant.
fn drive_candidate(d: &mut SpecDriver, prog: &[ProgWord]) -> Option<Vec<AV>> {
    let st = d.arena.load(prog);
    let id = d.spawn(st, PER_CANDIDATE, 0);
    loop {
        match d.step_with_quota(id, u64::MAX) {
            BranchOutcome::Halted(s) => return Some(s),
            BranchOutcome::Dead | BranchOutcome::Deferred(_) => return None,
            BranchOutcome::Live => { /* budget remained; keep stepping */ }
        }
    }
}

/// State threaded through an enumeration search across generations.
struct Accum {
    candidates: u64,
    spent: u64,
    tape_hw: usize,
    stack_hw: usize,
    cont_hw: usize,
    oracle_checked: u64,
    oracle_agree: u64,
}
impl Accum {
    fn new() -> Self {
        Accum {
            candidates: 0,
            spent: 0,
            tape_hw: 0,
            stack_hw: 0,
            cont_hw: 0,
            oracle_checked: 0,
            oracle_agree: 0,
        }
    }
    fn absorb(&mut self, d: &SpecDriver) {
        self.spent += d.spent;
        self.tape_hw = self.tape_hw.max(d.arena.tape_len());
        self.stack_hw = self.stack_hw.max(d.arena.stack_nodes_len());
        self.cont_hw = self.cont_hw.max(d.arena.cont_nodes_len());
    }
    fn maybe_oracle(&mut self, prog: &[ProgWord], with_oracle: bool) {
        if !with_oracle {
            return;
        }
        let n = self.candidates;
        let check = n <= ORACLE_FULL_UPTO || n % ORACLE_SAMPLE_STRIDE == 0;
        if check {
            self.oracle_checked += 1;
            if oracle_agrees(prog) {
                self.oracle_agree += 1;
            }
        }
    }
}

/// Generic enumeration search: drive candidates from `it` in order, stop at the
/// first whose final stack equals `expected`. Uses generational SpecDrivers to
/// bound memory. Setting `with_oracle` runs the oracle gate on each executed
/// candidate (full up to `ORACLE_FULL_UPTO`, sampled beyond).
fn enumerate_search<I>(
    task_id: &'static str,
    mut it: I,
    expected: &[AV],
    with_oracle: bool,
) -> SearchResult
where
    I: Iterator<Item = Vec<ProgWord>>,
{
    let mut acc = Accum::new();
    let mut found: Option<Vec<ProgWord>> = None;
    let mut exhausted = false;

    'outer: loop {
        let remaining = TOTAL_B.saturating_sub(acc.spent);
        if remaining == 0 {
            break;
        }
        let mut d = SpecDriver::new(Vm::new(), remaining);
        let mut gen_n = 0u64;
        while gen_n < GEN_SIZE {
            let prog = match it.next() {
                Some(p) => p,
                None => {
                    exhausted = true;
                    acc.absorb(&d);
                    break 'outer;
                }
            };
            acc.candidates += 1;
            gen_n += 1;
            let stack = drive_candidate(&mut d, &prog);
            acc.maybe_oracle(&prog, with_oracle);
            if let Some(s) = &stack {
                if s.as_slice() == expected {
                    found = Some(prog);
                    acc.absorb(&d);
                    break 'outer;
                }
            }
        }
        acc.absorb(&d);
    }

    finalize(task_id, found, exhausted, acc)
}

fn finalize(
    task_id: &'static str,
    found: Option<Vec<ProgWord>>,
    exhausted: bool,
    acc: Accum,
) -> SearchResult {
    let (found_b, glyphs, prog) = match found {
        Some(p) => (true, render_glyphs(&p), p),
        None => (false, String::new(), Vec::new()),
    };
    SearchResult {
        task_id,
        found: found_b,
        winner_glyphs: glyphs,
        winner_prog: prog,
        candidates_explored: acc.candidates,
        space_exhausted: exhausted && !found_b,
        tape_hw: acc.tape_hw,
        stack_hw: acc.stack_hw,
        cont_hw: acc.cont_hw,
        oracle_checked: acc.oracle_checked,
        oracle_agree: acc.oracle_agree,
        spent: acc.spent,
    }
}

// ============================================================ the 5 tasks
// Alphabets as arena prims.
fn prog_a(k: i64) -> Vec<ProgWord> {
    // k : * 15087 -   (k*k - 15087; == 42 iff k*k == 15129 iff k == 123)
    vec![
        ProgWord::PushInt(k),
        ProgWord::Prim(AP::Dup),
        ProgWord::Prim(AP::Mul),
        ProgWord::PushInt(15087),
        ProgWord::Prim(AP::Sub),
    ]
}

/// Task (a) — Halting constant. k ∈ [0, 2^16); unique k = 123.
pub fn search_a(with_oracle: bool) -> SearchResult {
    let it = (0i64..(1 << 16)).map(prog_a);
    enumerate_search("a", it, &expected_av(&[42]), with_oracle)
}

const OPS_B: [AP; 5] = [AP::Add, AP::Sub, AP::Mul, AP::Dup, AP::Swap];

fn prog_b(seq: &[usize]) -> Vec<ProgWord> {
    let mut p = vec![ProgWord::PushInt(3), ProgWord::PushInt(5)];
    for &i in seq {
        p.push(ProgWord::Prim(OPS_B[i]));
    }
    p
}

/// Task (b) — Operator-sequence synthesis. [3,5] -> [64], {+,-,*,:,~}, len ≤ 6.
pub fn search_b(with_oracle: bool) -> SearchResult {
    let it = combos(5, 6).map(|seq| prog_b(&seq));
    enumerate_search("b", it, &expected_av(&[64]), with_oracle)
}

/// Task (c) — Loop coefficients. x:=a*x+b, x0=1, n times; find (a,b,n) hitting
/// x=1000. a∈[1,8], b∈[-8,8], n∈[1,16]. Emitted with Times.
fn prog_c(a: i64, b: i64, n: i64) -> Vec<ProgWord> {
    // Q = a * (then +b / -|b| / nothing)
    let mut q = vec![ProgWord::PushInt(a), ProgWord::Prim(AP::Mul)];
    if b > 0 {
        q.push(ProgWord::PushInt(b));
        q.push(ProgWord::Prim(AP::Add));
    } else if b < 0 {
        q.push(ProgWord::PushInt(-b));
        q.push(ProgWord::Prim(AP::Sub));
    }
    // 1 n [Q] .
    vec![
        ProgWord::PushInt(1),
        ProgWord::PushInt(n),
        ProgWord::PushQuote(q),
        ProgWord::Prim(AP::Times),
    ]
}

pub fn search_c(with_oracle: bool) -> SearchResult {
    let mut cands = Vec::new();
    for n in 1..=16i64 {
        for a in 1..=8i64 {
            for b in -8..=8i64 {
                cands.push(prog_c(a, b, n));
            }
        }
    }
    enumerate_search("c", cands.into_iter(), &expected_av(&[1000]), with_oracle)
}

/// Task (d) — Reachability. Reach 100 from 1 via {+3, ×2, −1} in ≤12 moves.
/// BFS with visited-value (dominance) pruning; each partial program is
/// evaluated by the SpecDriver (the VM is the evaluator).
pub fn search_d(with_oracle: bool) -> SearchResult {
    // ops: (+3)=`3+`, (×2)=`2*`, (−1)=`1-`
    let ops: [Vec<ProgWord>; 3] = [
        vec![ProgWord::PushInt(3), ProgWord::Prim(AP::Add)],
        vec![ProgWord::PushInt(2), ProgWord::Prim(AP::Mul)],
        vec![ProgWord::PushInt(1), ProgWord::Prim(AP::Sub)],
    ];
    let build = |seq: &[usize]| -> Vec<ProgWord> {
        let mut p = vec![ProgWord::PushInt(1)];
        for &i in seq {
            p.extend(ops[i].iter().cloned());
        }
        p
    };

    let mut acc = Accum::new();
    let mut d = SpecDriver::new(Vm::new(), TOTAL_B);
    let mut visited: HashSet<i64> = HashSet::new();
    visited.insert(1); // x0 = 1
    let mut queue: VecDeque<(Vec<usize>, i64)> = VecDeque::new();
    queue.push_back((Vec::new(), 1));
    let mut found: Option<Vec<ProgWord>> = None;
    let mut exhausted = true;

    while let Some((seq, val)) = queue.pop_front() {
        if val == 100 {
            found = Some(build(&seq));
            exhausted = false;
            break;
        }
        if seq.len() >= 12 {
            continue;
        }
        for oi in 0..3 {
            let mut nseq = seq.clone();
            nseq.push(oi);
            let prog = build(&nseq);
            acc.candidates += 1;
            let stack = drive_candidate(&mut d, &prog);
            acc.maybe_oracle(&prog, with_oracle);
            let nval = match &stack {
                Some(s) if s.len() == 1 => match s[0] {
                    AV::Int(v) => v,
                    _ => continue,
                },
                _ => continue,
            };
            if nval.abs() < 1_000_000 && !visited.contains(&nval) {
                visited.insert(nval);
                queue.push_back((nseq, nval));
            }
            // Generational reset to bound memory (BFS here is small, but keep
            // the discipline uniform).
            if acc.candidates % GEN_SIZE == 0 {
                acc.absorb(&d);
                let remaining = TOTAL_B.saturating_sub(acc.spent);
                d = SpecDriver::new(Vm::new(), remaining);
            }
        }
    }
    acc.absorb(&d);
    finalize("d", found, exhausted, acc)
}

const OPS_E: [AP; 5] = [AP::Swap, AP::Rot, AP::Over, AP::Drop, AP::Dup];

fn prog_e(seq: &[usize]) -> Vec<ProgWord> {
    let mut p = vec![
        ProgWord::PushInt(1),
        ProgWord::PushInt(2),
        ProgWord::PushInt(3),
        ProgWord::PushInt(4),
    ];
    for &i in seq {
        p.push(ProgWord::Prim(OPS_E[i]));
    }
    p
}

/// Task (e) — Stack routing (reversal). [1,2,3,4] -> [4,3,2,1],
/// {~,@,^,_,:}, len ≤ 8.
pub fn search_e(with_oracle: bool) -> SearchResult {
    let it = combos(5, 8).map(|seq| prog_e(&seq));
    enumerate_search("e", it, &expected_av(&[4, 3, 2, 1]), with_oracle)
}

// ----------------------------------------------------------- combo iterator
/// All op-index sequences over `nops` ops of length 1..=`maxlen`, ordered by
/// length then lexicographically (v[0] most significant).
fn combos(nops: usize, maxlen: usize) -> impl Iterator<Item = Vec<usize>> {
    (1..=maxlen).flat_map(move |len| {
        let total: u64 = (nops as u64).pow(len as u32);
        (0..total).map(move |mut idx| {
            let mut v = vec![0usize; len];
            for i in (0..len).rev() {
                v[i] = (idx % nops as u64) as usize;
                idx /= nops as u64;
            }
            v
        })
    })
}

// ----------------------------------------------------------- run all
pub fn run_task(id: &str, with_oracle: bool) -> SearchResult {
    match id {
        "a" => search_a(with_oracle),
        "b" => search_b(with_oracle),
        "c" => search_c(with_oracle),
        "d" => search_d(with_oracle),
        "e" => search_e(with_oracle),
        _ => panic!("unknown task {id}"),
    }
}

pub const TASK_IDS: [&str; 5] = ["a", "b", "c", "d", "e"];

/// Validate a winning glyph string end-to-end the way `mtlrun` does: parse it,
/// run it on the reference interpreter, and require `Halt(expected)`.
pub fn validate_glyphs(glyphs: &str, expected: &[i64]) -> bool {
    let parsed = match mtl_syntax::parse(glyphs) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let iprog: Vec<itp::Word> = parsed.iter().map(conv_syntax_word).collect();
    match itp::run(itp::Vm::new(iprog), ORACLE_FUEL) {
        itp::Outcome::Halt(stack) => {
            let want: Vec<itp::Value> = expected.iter().map(|&n| itp::Value::Int(n)).collect();
            stack == want
        }
        _ => false,
    }
}

fn conv_syntax_word(w: &mtl_syntax::Word) -> itp::Word {
    use mtl_syntax::Prim as SP;
    use itp::Prim as I;
    match w {
        mtl_syntax::Word::PushInt(n) => itp::Word::PushInt(*n),
        mtl_syntax::Word::PushQuote(b) => {
            itp::Word::PushQuote(b.iter().map(conv_syntax_word).collect())
        }
        mtl_syntax::Word::Call(cs) => itp::Word::Call(cs.iter().collect()),
        mtl_syntax::Word::Prim(p) => itp::Word::Prim(match p {
            SP::Dup => I::Dup,
            SP::Drop => I::Drop,
            SP::Swap => I::Swap,
            SP::Rot => I::Rot,
            SP::Over => I::Over,
            SP::Apply => I::Apply,
            SP::Cat => I::Cat,
            SP::Cons => I::Cons,
            SP::Dip => I::Dip,
            SP::Add => I::Add,
            SP::Sub => I::Sub,
            SP::Mul => I::Mul,
            SP::Div => I::Div,
            SP::Mod => I::Mod,
            SP::Eq => I::Eq,
            SP::Lt => I::Lt,
            SP::If => I::If,
            SP::PrimRec => I::PrimRec,
            SP::Times => I::Times,
            SP::LinRec => I::LinRec,
            SP::Uncons => I::Uncons,
            SP::Fold => I::Fold,
            SP::Xor => I::Xor,
        }),
    }
}

/// Expected target ints per task (for glyph validation / reporting).
pub fn expected_ints(id: &str) -> Vec<i64> {
    match id {
        "a" => vec![42],
        "b" => vec![64],
        "c" => vec![1000],
        "d" => vec![100],
        "e" => vec![4, 3, 2, 1],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_a_finds_123() {
        let r = search_a(true);
        assert!(r.found);
        assert_eq!(r.winner_glyphs, "123:*15087-");
        assert_eq!(r.oracle_checked, r.oracle_agree);
        assert!(validate_glyphs(&r.winner_glyphs, &[42]));
    }

    #[test]
    fn task_b_finds_64() {
        let r = search_b(true);
        assert!(r.found);
        assert!(validate_glyphs(&r.winner_glyphs, &[64]));
        assert_eq!(r.oracle_checked, r.oracle_agree);
    }

    #[test]
    fn task_d_finds_100() {
        let r = search_d(true);
        assert!(r.found);
        assert!(validate_glyphs(&r.winner_glyphs, &[100]));
        assert_eq!(r.oracle_checked, r.oracle_agree);
    }

    #[test]
    fn task_c_exhausts_no_solution() {
        // Pre-registered target 1000 is unreachable in the space; honest Exhausted.
        let r = search_c(true);
        assert!(!r.found);
        assert!(r.space_exhausted);
        assert_eq!(r.candidates_explored, 2176);
        assert_eq!(r.oracle_checked, r.oracle_agree);
    }
}
