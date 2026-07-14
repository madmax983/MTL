//! Cross-mirror conformance police for the MTL primitive manifest.
//!
//! The 23 v0.x primitives are mirrored in SIX places (parser `Prim`, Verus
//! `SpecPrim`, interp `Prim`, `GLYPHS`, and the two `conv` functions). These
//! tests assert every mirror agrees with the checked manifest
//! (`mtl_syntax::manifest`), the single source of truth. Every assertion fails
//! LOUDLY, naming the mirror, the primitive, and expected-vs-actual, so a drift
//! is diagnosable from the panic message alone.
//!
//! ## Generated vs hand-written (issue #46)
//!
//! Four of the six mirrors are now **generated** from the manifest's single
//! canonical table `mtl_syntax::for_each_primitive!` and cannot drift by
//! construction: the parser `Prim` enum + `GLYPHS` (`mtl-syntax`), and both
//! `conv` opcode maps (`mtl-bench-validate`, `mtl-host`). The tests below act as
//! **smoke assertions** over the emitted artifacts (a codegen bug is caught, not
//! assumed away). Two mirrors stay hand-written and comparison-policed: the Verus
//! `SpecPrim` (`verus_specprim_matches_manifest`) and the interp `Prim`
//! (`interp_prim_names_match_manifest`, `interp_prim_exhaustive`). See
//! `mirror_generation_propagates` (test #9) for the propagation backstop rooted
//! directly at the canonical table.

use mtl_syntax::ast::{glyph_to_prim, prim_to_glyph, GLYPHS};
use mtl_syntax::manifest::{meta_of, ALL_PRIMS, PRIMITIVES};

use mtl_core::interp::Prim as IPrim;
use mtl_core::interp::{Outcome, Value, Vm, Word as IWord};

/// Convert a syntax `Prim` to an interp `Prim` via the `bench/validate` mirror.
fn bench_iprim(p: mtl_syntax::Prim) -> IPrim {
    match mtl_bench_validate::conv(&mtl_syntax::Word::Prim(p)) {
        IWord::Prim(ip) => ip,
        other => panic!(
            "bench conv mirror: conv(Prim::{:?}) did not yield Word::Prim, got {:?}",
            p, other
        ),
    }
}

/// Convert a syntax `Prim` to an interp `Prim` via the `mtl-host` mirror.
fn host_iprim(p: mtl_syntax::Prim) -> IPrim {
    match mtl_host::conv(&mtl_syntax::Word::Prim(p)) {
        IWord::Prim(ip) => ip,
        other => panic!(
            "host conv mirror: conv(Prim::{:?}) did not yield Word::Prim, got {:?}",
            p, other
        ),
    }
}

// ============================================================
// 1. manifest_shape
// ============================================================
#[test]
fn manifest_shape() {
    assert_eq!(
        PRIMITIVES.len(),
        23,
        "manifest drift: PRIMITIVES.len() expected 23 got {}",
        PRIMITIVES.len()
    );
    assert_eq!(
        ALL_PRIMS.len(),
        23,
        "manifest drift: ALL_PRIMS.len() expected 23 got {}",
        ALL_PRIMS.len()
    );

    for i in 0..23 {
        assert_eq!(
            PRIMITIVES[i].index as usize, i,
            "manifest drift: PRIMITIVES[{}].index expected {} got {}",
            i, i, PRIMITIVES[i].index
        );
        // `meta_of(ALL_PRIMS[i])` must be the i-th manifest entry. `PRIMITIVES`
        // is a `const`, so its address is not stable across use sites — identity
        // is asserted by the entry's own index (0..=22, unique) plus name.
        let m = meta_of(ALL_PRIMS[i]);
        assert_eq!(
            m.index as usize, i,
            "manifest drift: meta_of(ALL_PRIMS[{}]) (name {:?}) has index {} — expected the entry at index {} (name {:?})",
            i, m.name, m.index, i, PRIMITIVES[i].name
        );
        assert_eq!(
            m.name, PRIMITIVES[i].name,
            "manifest drift: meta_of(ALL_PRIMS[{}]) name {:?} != PRIMITIVES[{}] name {:?}",
            i, m.name, i, PRIMITIVES[i].name
        );
    }

    // All glyphs distinct.
    for i in 0..23 {
        for j in (i + 1)..23 {
            assert_ne!(
                PRIMITIVES[i].glyph, PRIMITIVES[j].glyph,
                "manifest drift: duplicate glyph {:?} shared by {} and {}",
                PRIMITIVES[i].glyph, PRIMITIVES[i].name, PRIMITIVES[j].name
            );
        }
    }
    // All names distinct.
    for i in 0..23 {
        for j in (i + 1)..23 {
            assert_ne!(
                PRIMITIVES[i].name, PRIMITIVES[j].name,
                "manifest drift: duplicate name {:?} at indices {} and {}",
                PRIMITIVES[i].name, i, j
            );
        }
    }
}

// ============================================================
// 2. glyphs_agree_with_manifest
// ============================================================
#[test]
fn glyphs_agree_with_manifest() {
    assert_eq!(
        GLYPHS.len(),
        23,
        "GLYPHS drift: GLYPHS.len() expected 23 got {}",
        GLYPHS.len()
    );

    for meta in PRIMITIVES.iter() {
        // Exactly one (g, p) in GLYPHS whose glyph and Debug-name match this meta.
        let matches: Vec<&(char, mtl_syntax::Prim)> = GLYPHS
            .iter()
            .filter(|(g, p)| *g == meta.glyph && format!("{:?}", p) == meta.name)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "GLYPHS drift: primitive {} expected exactly one GLYPHS entry with glyph {:?}, found {}",
            meta.name,
            meta.glyph,
            matches.len()
        );
        let (g, p) = matches[0];
        assert_eq!(
            prim_to_glyph(*p),
            meta.glyph,
            "GLYPHS drift: primitive {} prim_to_glyph expected glyph {:?} got {:?}",
            meta.name,
            meta.glyph,
            prim_to_glyph(*p)
        );
        assert_eq!(
            glyph_to_prim(meta.glyph),
            Some(*p),
            "GLYPHS drift: primitive {} glyph_to_prim({:?}) expected Some({:?}) got {:?}",
            meta.name,
            meta.glyph,
            p,
            glyph_to_prim(meta.glyph)
        );
        // Silence unused-`g` when the format-branch above optimizes out.
        let _ = g;
    }
}

// ============================================================
// 3. syntax_prim_names_match_manifest
// ============================================================
#[test]
fn syntax_prim_names_match_manifest() {
    for i in 0..23 {
        assert_eq!(
            format!("{:?}", ALL_PRIMS[i]),
            PRIMITIVES[i].name,
            "syntax::Prim drift: index {} expected name {:?} got {:?}",
            i,
            PRIMITIVES[i].name,
            format!("{:?}", ALL_PRIMS[i])
        );
    }
}

// ============================================================
// 4. interp_prim_names_match_manifest
// ============================================================
#[test]
fn interp_prim_names_match_manifest() {
    for i in 0..23 {
        let ip = bench_iprim(ALL_PRIMS[i]);
        assert_eq!(
            format!("{:?}", ip),
            PRIMITIVES[i].name,
            "interp::Prim drift: index {} (syntax {:?}) expected name {:?} got interp {:?}",
            i,
            ALL_PRIMS[i],
            PRIMITIVES[i].name,
            format!("{:?}", ip)
        );
    }
}

// ============================================================
// 5. conv_fns_agree
// ============================================================
#[test]
fn conv_fns_agree() {
    for i in 0..23 {
        let b = bench_iprim(ALL_PRIMS[i]);
        let h = host_iprim(ALL_PRIMS[i]);
        assert_eq!(
            format!("{:?}", b),
            format!("{:?}", h),
            "conv mirror drift: primitive {:?} — bench conv yields {:?} but host conv yields {:?}",
            ALL_PRIMS[i],
            b,
            h
        );
    }
}

// ============================================================
// 6. interp_prim_exhaustive
// ============================================================

/// Compile-time drift guard: if an `interp::Prim` variant is added or removed,
/// this match stops being exhaustive (no wildcard) and the test crate fails to
/// COMPILE.
#[allow(dead_code)]
fn _exhaustive(p: IPrim) {
    use mtl_core::interp::Prim::*;
    match p {
        Dup => (),
        Drop => (),
        Swap => (),
        Rot => (),
        Over => (),
        Apply => (),
        Cat => (),
        Cons => (),
        Dip => (),
        Add => (),
        Sub => (),
        Mul => (),
        Div => (),
        Mod => (),
        Eq => (),
        Lt => (),
        If => (),
        PrimRec => (),
        Times => (),
        LinRec => (),
        Uncons => (),
        Fold => (),
        Xor => (),
    }
}

#[test]
fn interp_prim_exhaustive() {
    // Runtime half: mapping all 23 syntax prims through bench conv yields 23
    // distinct interp prims.
    let mapped: Vec<IPrim> = ALL_PRIMS.iter().map(|&p| bench_iprim(p)).collect();
    assert_eq!(
        mapped.len(),
        23,
        "interp::Prim drift: expected 23 mapped prims got {}",
        mapped.len()
    );
    for i in 0..23 {
        for j in (i + 1)..23 {
            assert_ne!(
                mapped[i], mapped[j],
                "interp::Prim drift: syntax {:?} and {:?} both map to interp {:?}",
                ALL_PRIMS[i], ALL_PRIMS[j], mapped[i]
            );
        }
    }
}

// ============================================================
// 7. verus_specprim_matches_manifest
// ============================================================
const SRC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../mtl-core/src/mtl_core.rs"
));

/// Strip `//` line comments and `/* */` block comments from Rust source text.
fn strip_comments(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // line comment: skip to newline
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            // block comment: skip to */
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Extract the ordered variant identifiers from the `enum SpecPrim { ... }` block.
fn specprim_variants(src: &str) -> Vec<String> {
    let start = src
        .find("enum SpecPrim")
        .expect("verus SpecPrim mirror: `enum SpecPrim` not found in mtl_core.rs");
    let after = &src[start..];
    let brace = after
        .find('{')
        .expect("verus SpecPrim mirror: opening `{` not found after `enum SpecPrim`");
    let body_start = start + brace + 1;
    // Walk from body_start counting braces to find the matching close.
    let bytes = src.as_bytes();
    let mut depth = 1;
    let mut i = body_start;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            break;
        }
        i += 1;
    }
    assert_eq!(
        depth, 0,
        "verus SpecPrim mirror: unbalanced braces in `enum SpecPrim` block"
    );
    let inner = &src[body_start..i];
    let cleaned = strip_comments(inner);
    let mut variants = Vec::new();
    for entry in cleaned.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Leading identifier of the entry.
        let ident: String = trimmed
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !ident.is_empty() {
            variants.push(ident);
        }
    }
    variants
}

#[test]
fn verus_specprim_matches_manifest() {
    let variants = specprim_variants(SRC);
    assert_eq!(
        variants.len(),
        23,
        "verus SpecPrim mirror: expected 23 variants got {} ({:?})",
        variants.len(),
        variants
    );
    for i in 0..23 {
        assert_eq!(
            variants[i], PRIMITIVES[i].name,
            "verus SpecPrim mirror: variant at index {} expected {:?} got {:?}",
            i, PRIMITIVES[i].name, variants[i]
        );
    }
}

// ============================================================
// 8. arity_matches_interp_underflow
// ============================================================
#[test]
fn arity_matches_interp_underflow() {
    const FUEL: u64 = 10_000;
    for i in 0..23 {
        let meta = &PRIMITIVES[i];
        let a = meta.arity;
        if a < 1 {
            continue;
        }
        // Lower bound: exactly a-1 ints MUST fault Underflow (interp checks depth
        // before type, so a-1 ints underflow regardless of value shape).
        let below: Vec<Value> = (0..(a - 1)).map(|k| Value::Int(k as i64)).collect();
        let prog = vec![IWord::Prim(bench_iprim(ALL_PRIMS[i]))];
        match run(below, prog.clone(), FUEL) {
            Outcome::Fault(fi) => {
                assert_eq!(
                    fi.fault,
                    mtl_core::interp::Fault::Underflow,
                    "arity drift: primitive {} declared arity {} — with {} ints expected Fault::Underflow got {:?}",
                    meta.name,
                    a,
                    a - 1,
                    fi.fault
                );
            }
            other => panic!(
                "arity drift: primitive {} declared arity {} — with {} ints expected Fault::Underflow but got {:?}",
                meta.name,
                a,
                a - 1,
                other
            ),
        }

        // Upper bound: exactly a ints MUST NOT fault Underflow — this pins the
        // interpreter's underflow threshold to EXACTLY `a` (catches a guard that
        // demands MORE than the declared arity, e.g. Add raised to `len < 3`).
        let at: Vec<Value> = (0..a).map(|k| Value::Int(k as i64)).collect();
        if let Outcome::Fault(fi) = run(at, prog, FUEL) {
            assert_ne!(
                fi.fault,
                mtl_core::interp::Fault::Underflow,
                "arity drift: primitive {} declared arity {} — with {} ints it still faulted Underflow (interp guard demands more than {} operands)",
                meta.name,
                a,
                a,
                a
            );
        }
    }
}

fn run(stack: Vec<Value>, prog: Vec<IWord>, fuel: u64) -> Outcome {
    mtl_core::interp::run(Vm::with_stack(stack, prog), fuel)
}

// ============================================================
// 9. mirror_generation_propagates  (issue #46 codegen backstop)
// ============================================================

/// Build an independent `(index, name, glyph, arity)` reference vector by
/// expanding the canonical table `mtl_syntax::for_each_primitive!` DIRECTLY —
/// i.e. rooted at the single source of truth, not at the derived `PRIMITIVES`
/// table. This is what the propagation test checks every generated mirror
/// against, so it proves each mirror really is a function of the one table.
macro_rules! collect_rows {
    ( $( ($idx:expr, $name:ident, $glyph:literal, $arity:literal, $eff:literal) ),* $(,)? ) => {
        vec![ $( ($idx as usize, stringify!($name), $glyph, $arity as u8) ),* ]
    };
}

fn canonical_rows() -> Vec<(usize, &'static str, char, u8)> {
    mtl_syntax::for_each_primitive!(collect_rows)
}

/// Propagation test (issue #46 acceptance criterion): a single edit to the
/// canonical table `mtl_syntax::for_each_primitive!` must ripple into EVERY
/// generated mirror with no hand-edit to those mirrors. We assert that here by
/// re-deriving the rows straight from the macro and checking that all four
/// generated mirrors — the parser `Prim` enum, `GLYPHS`/`prim_to_glyph`/
/// `glyph_to_prim`, and both `conv` opcode maps — reflect them exactly. Editing
/// a row (rename a prim, swap two glyphs, add a 24th) therefore cannot leave any
/// generated mirror behind; if it could, this test (and the per-mirror smoke
/// tests above) would fail loudly.
#[test]
fn mirror_generation_propagates() {
    let rows = canonical_rows();
    assert_eq!(
        rows.len(),
        23,
        "propagation: canonical table has {} rows, expected 23",
        rows.len()
    );

    for (i, name, glyph, _arity) in rows.iter().copied() {
        // --- GENERATED mirror 1: parser `Prim` enum (Debug name + order) ---
        let sp = ALL_PRIMS[i];
        assert_eq!(
            format!("{:?}", sp),
            name,
            "propagation: syntax::Prim at index {} is {:?} but canonical table says {:?}",
            i,
            format!("{:?}", sp),
            name
        );

        // --- GENERATED mirror 2: GLYPHS / prim_to_glyph / glyph_to_prim ---
        assert_eq!(
            prim_to_glyph(sp),
            glyph,
            "propagation: GLYPHS drift — prim_to_glyph({:?}) = {:?}, canonical table says {:?}",
            sp,
            prim_to_glyph(sp),
            glyph
        );
        assert_eq!(
            glyph_to_prim(glyph),
            Some(sp),
            "propagation: GLYPHS drift — glyph_to_prim({:?}) = {:?}, expected Some({:?})",
            glyph,
            glyph_to_prim(glyph),
            sp
        );

        // --- GENERATED mirror 3: bench/validate conv opcode map ---
        let b = bench_iprim(sp);
        assert_eq!(
            format!("{:?}", b),
            name,
            "propagation: bench conv drift — conv({:?}) = interp {:?}, canonical table says {:?}",
            sp,
            format!("{:?}", b),
            name
        );

        // --- GENERATED mirror 4: mtl-host conv opcode map ---
        let h = host_iprim(sp);
        assert_eq!(
            format!("{:?}", h),
            name,
            "propagation: host conv drift — conv({:?}) = interp {:?}, canonical table says {:?}",
            sp,
            format!("{:?}", h),
            name
        );
    }
}
