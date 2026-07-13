//! Unit tests for the static checker: per-primitive effects, Static/Guarded/
//! Reject examples, and the uncons cases.

use super::*;

fn v(src: &str) -> Verdict {
    check_str(src).expect("parse ok")
}

fn eff(verdict: &Verdict) -> &Effect {
    match verdict {
        Verdict::Static(e) => e,
        Verdict::Guarded(e, _) => e,
        Verdict::Reject { .. } => panic!("expected non-reject, got {verdict:?}"),
    }
}

// ---- per-primitive effects -------------------------------------------------

#[test]
fn arithmetic_pops_two_pushes_int() {
    // `3*7+`  (affine): on empty, borrows one Int, produces one Int.
    let ver = v("3*7+");
    assert!(ver.is_static(), "{ver:?}");
    let e = eff(&ver);
    assert_eq!(e.pre, vec![Kind::Int]);
    assert_eq!(e.post, vec![Kind::Int]);
}

#[test]
fn is_even_static() {
    let ver = v("2%0="); // ( Int -- Int )
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).pre, vec![Kind::Int]);
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

#[test]
fn dup_drop_swap_over_rot() {
    // dup: ( a -- a a )
    assert_eq!(eff(&v(":")).pre.len(), 1);
    assert_eq!(eff(&v(":")).post.len(), 2);
    // drop: ( a -- )
    assert_eq!(eff(&v("_")).post.len(), 0);
    // swap: ( a b -- b a )
    assert_eq!(eff(&v("~")).pre.len(), 2);
    assert_eq!(eff(&v("~")).post.len(), 2);
    // over: ( a b -- a b a )
    assert_eq!(eff(&v("^")).pre.len(), 2);
    assert_eq!(eff(&v("^")).post.len(), 3);
    // rot: ( a b c -- b c a )
    assert_eq!(eff(&v("@")).pre.len(), 3);
    assert_eq!(eff(&v("@")).post.len(), 3);
}

#[test]
fn rev3_is_identity_shape() {
    // ~@ : ( a b c -- ... ) 3 in, 3 out
    let ver = v("~@");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).pre.len(), 3);
    assert_eq!(eff(&ver).post.len(), 3);
}

#[test]
fn cat_constant_folds() {
    // [1][2], : two literals concat into one literal quote.
    let ver = v("[1][2],");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).post, vec![Kind::Quote]);
}

#[test]
fn apply_inlines_literal() {
    // [1+]! : applies a literal body; net ( Int -- Int ).
    let ver = v("[1+]!");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).pre, vec![Kind::Int]);
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

// ---- Static combinator examples -------------------------------------------

#[test]
fn factorial_primrec_static() {
    // [1][*]&  ( Int -- Int )
    let ver = v("[1][*]&");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).pre, vec![Kind::Int]);
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

#[test]
fn sum_to_primrec_static() {
    let ver = v("[0][+]&");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

#[test]
fn fib_times_static() {
    // 0 1@[~^+]._  — a stack-neutral times body → Static ( Int -- Int ).
    let ver = v("0 1@[~^+]._");
    assert!(ver.is_static(), "{ver:?}");
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

#[test]
fn power_times_static() {
    let ver = v("1~[^*].~_");
    assert!(ver.is_static(), "power: {ver:?}");
}

// ---- Reject: self-application ---------------------------------------------

#[test]
fn self_app_rejects() {
    // the v0.1 factorial: open recursion via `:!`.
    let ver = v("1~[^0=[__][[:@*~1-]':!]?]:!");
    match &ver {
        Verdict::Reject { reason, .. } => {
            assert!(reason.contains("self-applicative"), "reason: {reason}");
        }
        other => panic!("expected self-app Reject, got {other:?}"),
    }
}

#[test]
fn gcd_v01_self_app_rejects() {
    let ver = v("[^0=[__][[~^%]':!]?]:!");
    assert!(ver.is_reject(), "{ver:?}");
}

#[test]
fn plain_dup_apply_rejects() {
    // [:!]:!  — the minimal self-applicative idiom.
    let ver = v("[:!]:!");
    assert!(ver.is_reject(), "{ver:?}");
}

// ---- Reject: TypeMismatch --------------------------------------------------

#[test]
fn add_quote_type_mismatch() {
    // [1]+ : add with a quote operand → provable TypeMismatch.
    // (Need a second operand; `[1]2+` puts a quote as the 2nd operand.)
    let ver = v("[1]2+");
    match &ver {
        Verdict::Reject { reason, .. } => assert_eq!(reason, "TypeMismatch"),
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
}

#[test]
fn apply_int_type_mismatch() {
    // 1 2! -> `12!`? No: `12` is one int. Use `2 1!`? apply an int.
    // A single int then apply: `1!` -> pops Int, applies → TypeMismatch.
    let ver = v("1!");
    match &ver {
        Verdict::Reject { reason, .. } => assert_eq!(reason, "TypeMismatch"),
        other => panic!("expected TypeMismatch on apply-int, got {other:?}"),
    }
}

#[test]
fn uncons_int_type_mismatch() {
    // 1> : uncons an int → TypeMismatch.
    let ver = v("1>");
    match &ver {
        Verdict::Reject { reason, .. } => assert_eq!(reason, "TypeMismatch"),
        other => panic!("expected TypeMismatch on uncons-int, got {other:?}"),
    }
}

#[test]
fn uncons_malformed_head_type_mismatch() {
    // [+]> : uncons a quote whose head is a bare prim → TypeMismatch.
    let ver = v("[+]>");
    match &ver {
        Verdict::Reject { reason, found, .. } => {
            assert_eq!(reason, "TypeMismatch");
            assert!(found.contains("bare Prim"), "found: {found}");
        }
        other => panic!("expected malformed-head TypeMismatch, got {other:?}"),
    }
}

// ---- Uncons cases ----------------------------------------------------------

#[test]
fn uncons_nonempty_literal() {
    // [1 2 3]> : head=Int, tail=[2 3], flag=1 → 3 cells on top.
    let ver = v("[1 2 3]>");
    assert!(!ver.is_reject(), "{ver:?}");
    let e = eff(&ver);
    assert_eq!(e.post, vec![Kind::Int, Kind::Quote, Kind::Int]);
}

#[test]
fn uncons_empty_literal() {
    // []> : empty → pushes Int(0), one cell.
    let ver = v("[]>");
    assert!(!ver.is_reject(), "{ver:?}");
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}

#[test]
fn uncons_quote_head() {
    // [[9]4]> : head is a quote literal, tail=[4], flag=1.
    let ver = v("[[9]4]>");
    let e = eff(&ver);
    assert_eq!(e.post, vec![Kind::Quote, Kind::Quote, Kind::Int]);
}

// ---- Branch incompatibility ------------------------------------------------

#[test]
fn branch_height_incompat_rejects() {
    // flag [dup] [drop] ? : then Δ=+1, else Δ=-1 → incompatible.
    // Build: push a value, push flag, then quotes. `1 1[:][_]?`
    let ver = v("1 1[:][_]?");
    match &ver {
        Verdict::Reject { reason, .. } => {
            assert!(reason.contains("branch-stack incompatibility"), "reason: {reason}");
        }
        other => panic!("expected branch incompatibility, got {other:?}"),
    }
}

#[test]
fn branch_compatible_ok() {
    // 1[1+][2+]? : both branches ( Int -- Int ), Δ=0, joinable.
    let ver = v("1[1+][2+]?");
    assert!(!ver.is_reject(), "{ver:?}");
}

// ---- Guarded: host call ----------------------------------------------------

#[test]
fn host_call_guarded() {
    let ver = v("readline emit");
    assert!(ver.is_guarded(), "{ver:?}");
    if let Verdict::Guarded(_, obls) = &ver {
        assert!(obls.iter().any(|o| o.kind == "host-call"));
    }
}

// ---- Fold ------------------------------------------------------------------

#[test]
fn fold_over_opaque_seq_guarded() {
    // 0[+]( : fold + over an input (opaque) list → Guarded, result Int.
    let ver = v("0[+](");
    assert!(ver.is_guarded(), "{ver:?}");
    assert_eq!(eff(&ver).post, vec![Kind::Int]);
}
