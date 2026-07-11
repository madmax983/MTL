// P4 (round-trip) Verus proof SKELETON — DOCUMENTATION, NOT VERIFIED.
//
// The CI `verus` job targets ONLY `crates/mtl-core/src/mtl_core.rs`. This file
// is NOT on that path and is NOT part of the cargo build, so nothing here is
// auto-verified. It exists to state the P4 obligation in Verus terms. Do not
// read any claim below as "verified" — the proofs are holes (`admit()`).
//
// Obligation (well-formed domain): for every program `p` whose every PushInt is
// non-negative — the image of the executable parser — printing then parsing
// recovers `p`:
//
//     parse(print(p)) == Ok(p)
//
// Verify (once ported + fleshed out):  verus p4_verus.rs

use vstd::prelude::*;

verus! {

// --- Mirror of the surface AST as a ghost model. ---
pub enum GPrim { Dup, Drop, Swap, Rot, Over, Apply, Cat, Cons, Dip,
                 Add, Sub, Mul, Div, Mod, Eq, Lt, If }

pub enum GWord {
    PushInt(int),
    PushQuote(Seq<GWord>),
    Prim(GPrim),
    Call(Seq<char>),
}

// Well-formedness: the parser's image contains only non-negative ints.
pub open spec fn wf_word(w: GWord) -> bool
    decreases w,
{
    match w {
        GWord::PushInt(n) => n >= 0,
        GWord::PushQuote(q) => wf_words(q),
        _ => true,
    }
}

pub open spec fn wf_words(ws: Seq<GWord>) -> bool
    decreases ws.len(),
{
    if ws.len() == 0 {
        true
    } else {
        wf_word(ws[0]) && wf_words(ws.subrange(1, ws.len() as int))
    }
}

// --- Spec functions for print and parse. ---
// These are declared uninterpreted here; a real port would define them to
// mirror src/print.rs and src/parse.rs exactly.

pub uninterp spec fn spec_print(p: Seq<GWord>) -> Seq<char>;

pub enum ParseOutcome { Ok(Seq<GWord>), Err }

pub uninterp spec fn spec_parse(s: Seq<char>) -> ParseOutcome;

// --- Supporting lemma stubs (PROOF HOLES). ---

// The boundary/separator rule keeps token pieces from re-lexing across joins.
// PROOF HOLE: requires defining spec_print's per-token boundary insertion.
pub proof fn lemma_tokens_separated(p: Seq<GWord>)
    requires wf_words(p),
    ensures true,
{
    admit();  // PROOF HOLE
}

// Printing a well-formed word never emits a leading `-` (unsigned image), so
// `-` always lexes as Sub and never merges into an int literal.
// PROOF HOLE.
pub proof fn lemma_no_negative_literal(w: GWord)
    requires wf_word(w),
    ensures true,
{
    admit();  // PROOF HOLE
}

// --- The P4 theorem. ---
// PROOF HOLE: discharged by induction on `p` using the two lemmas above plus a
// characterization of spec_parse as the left inverse of spec_print on the
// well-formed domain.
pub proof fn p4_roundtrip(p: Seq<GWord>)
    requires wf_words(p),
    ensures spec_parse(spec_print(p)) == ParseOutcome::Ok(p),
{
    admit();  // PROOF HOLE
}

} // verus!

fn main() {}
