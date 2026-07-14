//! Differential test: the MECHANIZED judgment (`check_m1`, the Verus-proven
//! milestone-1/2/3 fragment in `crates/mtl-core/src/checker_verus.rs`) vs the
//! EXECUTABLE prototype (`mtl_check::check`), over EVERY corpus program.
//!
//! `check_m1_mirror` below is a faithful, line-by-line Rust port of the spec-level
//! `check_m1`: the equal-only milestone-1 lattice (`AInt | ALit(body)`), the
//! straight-line + `If` + `Times` + `PrimRec` fragment, plus the milestone-3
//! additions — deterministic literal-`Uncons` and the homogeneous-all-`PushInt`
//! literal-seq `Fold`. `LinRec` has no arm (conservatively Rejected), exactly as in
//! the mechanization. A program is MECHANIZED-Static iff `check_m1(∅, p, depth)` is
//! `Some` (the `pre = []` self-contained slice the Verus `T-Static` theorem proves).
//!
//! ## Milestone 4 — row-polymorphic borrows (the acid metric)
//!
//! `mech_static_rp` mechanizes the prototype's lazy-borrow mechanism: it searches for
//! the shortest all-`Int` `pre` such that `check_m1([AInt; n], p, depth)` is `Some`.
//! A program is MECHANIZED-Static-**RowPoly** iff such an `n` exists — the checker
//! then INFERS the non-empty required-input row `pre = [Int; n]` a real
//! input-borrowing program needs. Each accepted program is backed by the Verus
//! `thm_static_rowpoly` theorem (`crates/mtl-core/src/checker_verus.rs` §6.5),
//! instantiated at `pre = [AInt; n]`: for every base ρ and every `n` integer
//! arguments, running from `ρ ++ args` never faults Underflow/TypeMismatch and any
//! halt refines `ρ ++ post`. Before M4 the corpus mechanized-Static count was 0
//! (every corpus program borrows its inputs); `acid_mechanized_rowpoly_static_count`
//! reports the post-M4 number and gates it `> 0`.
//!
//! The theorem we assert is the SOUNDNESS DIRECTION of the proven fragment (both for
//! the `pre = []` slice and the M4 row-poly lift):
//!
//!     mechanized-Static(p)          ⟹  prototype-Static(p)   (for every corpus p)
//!     mechanized-Static-RowPoly(p)  ⟹  prototype-Static(p)   (for every corpus p)
//!
//! i.e. everything the machine-checked judgment blesses `Static`, the executable
//! prototype also blesses `Static`. (The converse does NOT hold — the prototype is
//! deliberately broader: `OpaqueQuote`/`Any` cells, an `Int⊔Quote=Any` join,
//! opaque-seq folds, borrowed-QUOTE inputs — none of which the narrow mechanized
//! fragment claims. In particular the mechanized borrow can only name `Int` required
//! inputs, not borrowed quotes; that gap is the honest Layer-C boundary, not a
//! divergence.)
//!
//! We also assert the prototype's corpus **Static count is unchanged (27)** — the
//! soundness-critical, design-doc figure — and record the mechanized-Static count.

use mtl_syntax::ast::Prim;
use mtl_syntax::{parse, Word};
use std::path::{Path, PathBuf};

// ===========================================================================
// Faithful Rust mirror of the spec-level `check_m1` (checker_verus.rs).
// ===========================================================================

#[derive(Clone, PartialEq, Eq)]
enum AbsVal {
    AInt,
    ALit(Vec<Word>),
}

fn absv_is_int(a: &AbsVal) -> bool {
    matches!(a, AbsVal::AInt)
}

/// Mirror of `abs_step_prim`: the shuffles + arith/cmp; everything else `None`.
fn abs_step_prim(astk: &[AbsVal], p: Prim) -> Option<Vec<AbsVal>> {
    let n = astk.len();
    match p {
        Prim::Dup => {
            if n < 1 {
                None
            } else {
                let mut v = astk.to_vec();
                v.push(astk[n - 1].clone());
                Some(v)
            }
        }
        Prim::Drop => {
            if n < 1 {
                None
            } else {
                Some(astk[..n - 1].to_vec())
            }
        }
        Prim::Swap => {
            if n < 2 {
                None
            } else {
                let mut v = astk[..n - 2].to_vec();
                v.push(astk[n - 1].clone());
                v.push(astk[n - 2].clone());
                Some(v)
            }
        }
        Prim::Rot => {
            if n < 3 {
                None
            } else {
                let mut v = astk[..n - 3].to_vec();
                v.push(astk[n - 2].clone());
                v.push(astk[n - 1].clone());
                v.push(astk[n - 3].clone());
                Some(v)
            }
        }
        Prim::Over => {
            if n < 2 {
                None
            } else {
                let mut v = astk.to_vec();
                v.push(astk[n - 2].clone());
                Some(v)
            }
        }
        Prim::Add | Prim::Sub | Prim::Mul | Prim::Div | Prim::Mod | Prim::Xor | Prim::Eq
        | Prim::Lt => {
            if n < 2 {
                None
            } else if absv_is_int(&astk[n - 1]) && absv_is_int(&astk[n - 2]) {
                let mut v = astk[..n - 2].to_vec();
                v.push(AbsVal::AInt);
                Some(v)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn all_pushint(q: &[Word]) -> bool {
    q.iter().all(|w| matches!(w, Word::PushInt(_)))
}

fn join_cell(a: &AbsVal, b: &AbsVal) -> Option<AbsVal> {
    match (a, b) {
        (AbsVal::AInt, AbsVal::AInt) => Some(AbsVal::AInt),
        (AbsVal::ALit(x), AbsVal::ALit(y)) if x == y => Some(AbsVal::ALit(x.clone())),
        _ => None,
    }
}

fn joinable(pt: &[AbsVal], pf: &[AbsVal]) -> bool {
    pt.len() == pf.len() && pt.iter().zip(pf.iter()).all(|(a, b)| join_cell(a, b).is_some())
}

fn join_stacks(pt: &[AbsVal], pf: &[AbsVal]) -> Vec<AbsVal> {
    pt.iter().zip(pf.iter()).map(|(a, b)| join_cell(a, b).unwrap()).collect()
}

/// Mirror of `check_m1(astk, p, depth)`.
fn check_m1(astk: &[AbsVal], p: &[Word], depth: usize) -> Option<Vec<AbsVal>> {
    if p.is_empty() {
        return Some(astk.to_vec());
    }
    let w = &p[0];
    let rest = &p[1..];
    match w {
        Word::PushInt(_) => {
            let mut a = astk.to_vec();
            a.push(AbsVal::AInt);
            check_m1(&a, rest, depth)
        }
        Word::PushQuote(q) => {
            let mut a = astk.to_vec();
            a.push(AbsVal::ALit(q.clone()));
            check_m1(&a, rest, depth)
        }
        Word::Prim(Prim::If) => {
            let m = astk.len();
            if m < 3 || depth == 0 {
                return None;
            }
            match (&astk[m - 3], &astk[m - 2], &astk[m - 1]) {
                (AbsVal::AInt, AbsVal::ALit(t), AbsVal::ALit(f)) => {
                    let base = &astk[..m - 3];
                    let pt = check_m1(base, t, depth - 1)?;
                    let pf = check_m1(base, f, depth - 1)?;
                    if joinable(&pt, &pf) {
                        check_m1(&join_stacks(&pt, &pf), rest, depth)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        Word::Prim(Prim::Times) => {
            let m = astk.len();
            if m < 2 || depth == 0 {
                return None;
            }
            match (&astk[m - 2], &astk[m - 1]) {
                (AbsVal::AInt, AbsVal::ALit(q)) => {
                    let base = &astk[..m - 2];
                    if check_m1(base, q, depth - 1).as_deref() == Some(base) {
                        check_m1(base, rest, depth)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        Word::Prim(Prim::PrimRec) => {
            let m = astk.len();
            if m < 3 || depth == 0 {
                return None;
            }
            match (&astk[m - 3], &astk[m - 2], &astk[m - 1]) {
                (AbsVal::AInt, AbsVal::ALit(qi), AbsVal::ALit(qc)) => {
                    let base = &astk[..m - 3];
                    let acc = check_m1(&[], qi, depth - 1)?;
                    let mut cin = vec![AbsVal::AInt];
                    cin.extend(acc.clone());
                    if check_m1(&cin, qc, depth - 1).as_deref() == Some(acc.as_slice()) {
                        let mut b = base.to_vec();
                        b.extend(acc);
                        check_m1(&b, rest, depth)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        // Milestone 3: Fold — homogeneous all-PushInt literal element seq only.
        Word::Prim(Prim::Fold) => {
            let m = astk.len();
            if m < 3 || depth == 0 {
                return None;
            }
            match (&astk[m - 3], &astk[m - 1]) {
                (AbsVal::ALit(qs), AbsVal::ALit(qc)) => {
                    if all_pushint(qs) {
                        let a_init = astk[m - 2].clone();
                        let base = &astk[..m - 3];
                        let cin = vec![a_init.clone(), AbsVal::AInt];
                        if check_m1(&cin, qc, depth - 1).as_deref()
                            == Some(std::slice::from_ref(&a_init))
                        {
                            let mut b = base.to_vec();
                            b.push(a_init);
                            check_m1(&b, rest, depth)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        // Milestone 3: Uncons — deterministic on a literal quote operand.
        Word::Prim(Prim::Uncons) => {
            let m = astk.len();
            if m < 1 {
                return None;
            }
            match &astk[m - 1] {
                AbsVal::ALit(q) => {
                    let base = &astk[..m - 1];
                    if q.is_empty() {
                        let mut b = base.to_vec();
                        b.push(AbsVal::AInt);
                        check_m1(&b, rest, depth)
                    } else {
                        let tail = q[1..].to_vec();
                        match &q[0] {
                            Word::PushInt(_) => {
                                let mut b = base.to_vec();
                                b.push(AbsVal::AInt);
                                b.push(AbsVal::ALit(tail));
                                b.push(AbsVal::AInt);
                                check_m1(&b, rest, depth)
                            }
                            Word::PushQuote(s) => {
                                let mut b = base.to_vec();
                                b.push(AbsVal::ALit(s.clone()));
                                b.push(AbsVal::ALit(tail));
                                b.push(AbsVal::AInt);
                                check_m1(&b, rest, depth)
                            }
                            _ => None,
                        }
                    }
                }
                _ => None,
            }
        }
        Word::Prim(p2) => match abs_step_prim(astk, *p2) {
            Some(a2) => check_m1(&a2, rest, depth),
            None => None,
        },
        Word::Call(_) => None,
    }
}

/// MECHANIZED-Static: `check_m1(∅, p, depth)` accepts (the `pre = []` slice).
fn mech_static(program: &[Word], depth: usize) -> bool {
    check_m1(&[], program, depth).is_some()
}

/// Largest inferred `pre` length the row-poly borrow search will try.
const MAX_PRE: usize = 8;

/// MECHANIZED-Static-RowPoly (milestone 4): the borrow-inference mirror. Searches
/// for the SHORTEST all-`AInt` `pre` such that `check_m1([AInt; n], p, depth)` is
/// `Some`, returning `Some((n, post))`. This mechanizes the prototype's lazy-borrow
/// pop-from-empty mechanism, specialized to the milestone lattice: a borrowed cell
/// consumed by the program is inferred as `Int` (the only borrowable kind the
/// `AInt | ALit` lattice can name — a borrowed *quote* would need its literal body,
/// which is unknowable, so those stay a marked gap). By the bottom-frame lemma
/// (`lemma_check_frame`) the succeeding lengths are upward-closed, so the shortest
/// is the tight inferred `pre`. Each accepted program is backed by the Verus
/// `thm_static_rowpoly` theorem instantiated at `pre = [AInt; n]`.
fn mech_static_rp(program: &[Word], depth: usize) -> Option<(usize, Vec<AbsVal>)> {
    for n in 0..=MAX_PRE {
        let pre = vec![AbsVal::AInt; n];
        if let Some(post) = check_m1(&pre, program, depth) {
            return Some((n, post));
        }
    }
    None
}

/// THE ACID METRIC (milestone 4). Reports the mechanized-Static-**RowPoly** count
/// over the corpus and asserts it is `> 0` — the whole point of M4 (before M4 the
/// self-contained `pre = []` count was 0, since every corpus program borrows its
/// inputs). Also asserts the SOUNDNESS DIRECTION for the row-poly judgment:
///
///     mechanized-Static-RowPoly(p)  ⟹  prototype-Static(p)   (every corpus p)
///
/// i.e. every program the machine-checked row-poly judgment (backed by the Verus
/// `thm_static_rowpoly` theorem, instantiated at the inferred all-`Int` `pre`)
/// blesses `Static`, the executable prototype also blesses `Static`.
#[test]
fn acid_mechanized_rowpoly_static_count() {
    let rows = gather();
    assert!(!rows.is_empty(), "corpus enumeration found no programs");

    let mut count = 0usize;
    let mut names: Vec<String> = Vec::new();
    let mut divergences: Vec<String> = Vec::new();
    for (tier, name, src) in &rows {
        let prog = match parse(src) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Some((n, _post)) = mech_static_rp(&prog, DEPTH) {
            count += 1;
            names.push(format!("[{tier}] {name} (pre.len={n}) `{src}`"));
            if !mtl_check::check(&prog).is_static() {
                divergences.push(format!(
                    "[{tier}] {name} `{src}`: RowPoly-Static but prototype = {}",
                    mtl_check::check(&prog).tag()
                ));
            }
        }
    }

    eprintln!(
        "ACID METRIC: mechanized-Static-RowPoly = {count} / {} corpus programs \
         (all also prototype-Static = {})",
        rows.len(),
        count - divergences.len()
    );
    for nm in &names {
        eprintln!("  RP-Static: {nm}");
    }

    assert!(
        divergences.is_empty(),
        "SOUNDNESS-DIRECTION divergence (mechanized-Static-RowPoly ⊄ prototype-Static):\n{}",
        divergences.join("\n")
    );
    // The acid gate: M4 must make the mechanized judgment cover REAL (input-borrowing)
    // corpus programs. Was 0 before M4.
    assert!(
        count > 0,
        "ACID METRIC FAILED: mechanized-Static-RowPoly count is 0 — M4 did not lift the \
         self-contained slice to row-polymorphic borrows"
    );
}

/// NON-VACUITY probes for the row-poly borrow inference. Each program BORROWS its
/// inputs (so `mech_static` on `pre = []` is `false`), yet `mech_static_rp` infers
/// the right non-empty `pre` length and both the mechanization and the prototype
/// bless it `Static`. Exercises the borrow bookkeeping through arith (`3*7+`,
/// pre.len=1: `x -> x*3+7`), shuffles (`~@` = swap;rot, pre.len=3, kind-agnostic
/// cells conservatively typed Int), and the `PrimRec` combinator (`[1][*]&`,
/// factorial, pre.len=1).
#[test]
fn rowpoly_borrow_probe() {
    // (program, expects mech_static(pre=[]) false, expected inferred pre.len, proto tag)
    let cases = [
        ("3*7+", 1usize, "Static"),   // affine: borrow 1 Int, x -> 3x+7
        ("2%0=", 1usize, "Static"),   // is_even: borrow 1 Int
        ("~@", 3usize, "Static"),     // rev3: swap;rot — borrow 3, kind-agnostic
        ("[1][*]&", 1usize, "Static"), // factorial: primrec, borrow 1 Int count
        ("[0][+]&", 1usize, "Static"), // sum_to: primrec, borrow 1 Int count
    ];
    for (src, exp_pre_len, exp_proto) in cases {
        let prog = parse(src).expect("parse");
        // borrows inputs => the pre=[] slice does NOT accept it.
        assert!(
            !mech_static(&prog, DEPTH),
            "pre=[] slice should reject the input-borrowing `{src}`"
        );
        let (n, _post) = mech_static_rp(&prog, DEPTH)
            .unwrap_or_else(|| panic!("row-poly borrow inference should accept `{src}`"));
        assert_eq!(n, exp_pre_len, "inferred pre.len for `{src}`");
        let proto = mtl_check::check(&prog);
        assert_eq!(proto.tag(), exp_proto, "prototype(`{src}`)");
        assert!(proto.is_static(), "subset: RowPoly-Static(`{src}`) => prototype-Static");
    }
}

const DEPTH: usize = 512;

// ===========================================================================
// Corpus enumeration (mirrors crates/mtl-check/src/main.rs `run_corpus`).
// ===========================================================================

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest)
}

fn read_stripped(path: &Path) -> String {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    raw.strip_suffix('\n').unwrap_or(&raw).to_string()
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

/// Minimal JSON string-field extractor (same shape as main.rs).
fn json_string_field(txt: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let mut idx = txt.find(&pat)? + pat.len();
    let bytes = txt.as_bytes();
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

/// Gather every corpus program as `(tier, name, source)` — the SAME set the
/// prototype's acceptance table is computed over.
fn gather() -> Vec<(String, String, String)> {
    let root = repo_root();
    let mut rows: Vec<(String, String, String)> = Vec::new();

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
            rows.push((tier.to_string(), task, read_stripped(&p)));
        }
    }

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
        rows.push(("tier-3".to_string(), task, read_stripped(&p)));
    }

    let att = root.join("bench/agent-trial/results/attempts");
    if let Ok(rd) = std::fs::read_dir(&att) {
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        files.sort();
        for f in files {
            if f.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let txt = std::fs::read_to_string(&f).unwrap_or_default();
            let program = match json_string_field(&txt, "program") {
                Some(p) => p,
                None => continue,
            };
            let arm = json_string_field(&txt, "arm").unwrap_or_default();
            if !arm.contains("mtl") {
                continue;
            }
            let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            rows.push(("agent-trial".to_string(), name, program));
        }
    }

    rows
}

// ===========================================================================
// The differential assertions.
// ===========================================================================

/// SOUNDNESS DIRECTION: everything the mechanized judgment calls Static, the
/// executable prototype also calls Static — over every corpus program.
#[test]
fn mechanized_static_implies_prototype_static() {
    let rows = gather();
    assert!(!rows.is_empty(), "corpus enumeration found no programs");

    let mut mech_static_count = 0usize;
    let mut mech_and_proto_static = 0usize;
    let mut divergences: Vec<String> = Vec::new();

    for (tier, name, src) in &rows {
        // Parse errors are outside the checker's judgment (both would reject); skip.
        let prog = match parse(src) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let proto = mtl_check::check(&prog);
        let mech = mech_static(&prog, DEPTH);
        if mech {
            mech_static_count += 1;
            if proto.is_static() {
                mech_and_proto_static += 1;
            } else {
                divergences.push(format!(
                    "[{tier}] {name} `{src}`: mechanized-Static but prototype = {}",
                    proto.tag()
                ));
            }
        }
    }

    eprintln!(
        "differential: {} corpus programs; mechanized-Static = {}, all also prototype-Static = {}",
        rows.len(),
        mech_static_count,
        mech_and_proto_static
    );

    assert!(
        divergences.is_empty(),
        "SOUNDNESS-DIRECTION divergence (mechanized-Static ⊄ prototype-Static):\n{}",
        divergences.join("\n")
    );
}

/// The prototype's corpus **Static count MUST NOT change** — it is the
/// soundness-critical, design-doc figure (27). A change here would signal the
/// prototype logic drifted; this test pins it. (Guarded/Reject totals are recorded
/// but not pinned: the tier-3 corpus grew since the v0.6 doc snapshot — 27/39/13
/// over 79 became 27/46/14 over 87 — with the Static count unchanged at 27, which
/// is exactly the invariant that matters for soundness.)
#[test]
fn prototype_static_count_unchanged() {
    let rows = gather();
    let (mut st, mut gu, mut rj) = (0usize, 0usize, 0usize);
    for (_tier, _name, src) in &rows {
        let verdict = match parse(src) {
            Ok(p) => mtl_check::check(&p),
            Err(_) => {
                rj += 1; // parse error is recorded as a Reject in the corpus table
                continue;
            }
        };
        if verdict.is_static() {
            st += 1;
        } else if verdict.is_guarded() {
            gu += 1;
        } else {
            rj += 1;
        }
    }
    eprintln!("prototype corpus verdicts: Static={st} Guarded={gu} Reject={rj} (total {})", st + gu + rj);
    assert_eq!(st, 27, "prototype Static count changed from the pinned 27");
}

/// NON-VACUITY + agreement on SELF-CONTAINED (`pre = []`) probes. Every corpus
/// program is row-polymorphic (borrows its inputs), so `mechanized-Static` is empty
/// on the corpus — the subset assertion above holds but proves nothing about the
/// mirror. These constructed self-contained programs exercise each proven construct
/// (straight-line, `If`, `Times`, `PrimRec`, literal-`Uncons`, homogeneous-Int
/// `Fold`) so the mirror is demonstrably non-vacuous, and confirm the subset
/// direction (mechanized-Static ⟹ prototype-Static) holds on each.
#[test]
fn mechanized_fragment_probe() {
    // (program, expected mechanized-Static, expected prototype tag)
    let cases = [
        ("1 2 3", true, "Static"),        // pure pushes
        ("1 2+", true, "Static"),         // straight-line arith
        ("1[2][3]?", true, "Static"),     // If, both literal branches, equal shape
        ("3[5_].", true, "Static"),       // Times, stack-neutral body
        ("5[0][+]&", true, "Static"),     // PrimRec, [I]=[0], [C]=[+] acc-stable
        ("[1 2]>", true, "Static"),       // Uncons, literal, PushInt head
        ("[]>", true, "Static"),          // Uncons, empty literal
        ("[[9]1]>", true, "Static"),      // Uncons, literal, PushQuote head
        ("[1 2 3]0[_](", true, "Static"), // Fold, homogeneous-Int seq, combine drops elem
        ("[+]>", false, "Reject"),        // Uncons, malformed (Prim) head -> both reject
        ("[:0=][0][][+]|", false, "Guarded"), // LinRec: mech Rejects, prototype Guards
    ];
    let mut mech_accepts = 0usize;
    for (src, exp_mech, exp_proto) in cases {
        let prog = parse(src).expect("parse");
        let mech = mech_static(&prog, DEPTH);
        let proto = mtl_check::check(&prog);
        assert_eq!(mech, exp_mech, "mechanized-Static({src})");
        assert_eq!(proto.tag(), exp_proto, "prototype({src}) = {:?}", proto);
        if mech {
            mech_accepts += 1;
            assert!(
                proto.is_static(),
                "subset: mechanized-Static({src}) but prototype = {}",
                proto.tag()
            );
        }
    }
    assert!(mech_accepts >= 8, "mirror looks vacuous: only {mech_accepts} mech-Static probes");
}

/// KNOWN, PINNED over-precision (investigated, not papered over). The
/// homogeneous-all-`PushInt` literal-seq `Fold` with an Int-consuming combine
/// (`[+]`) is PROVEN `Static` by the mechanization (machine-checked
/// `lemma_fold_case`/`lemma_fold_splice`: no `Underflow`/`TypeMismatch`), yet the
/// executable prototype conservatively GUARDS it — the prototype models the fold
/// element as `Any`, so `+` records a type-guard. This is SOUND over-precision, not
/// a soundness violation (the program never faults). It does NOT occur on the
/// corpus (no corpus fold has a *literal* element sequence — every corpus fold
/// ranges over a borrowed/host list), which is why the corpus subset test holds.
/// Pinned so the mechanization-vs-prototype relationship on this constructed input
/// is explicit. A follow-up could tighten the prototype's literal-homogeneous-seq
/// fold to Static to match the proof.
#[test]
fn mechanized_fold_is_more_precise_than_prototype_off_corpus() {
    let src = "[1 2 3]0[+](";
    let prog = parse(src).expect("parse");
    assert!(mech_static(&prog, DEPTH), "mechanization should prove `{src}` Static");
    assert!(
        mtl_check::check(&prog).is_guarded(),
        "prototype conservatively Guards `{src}`"
    );
}
