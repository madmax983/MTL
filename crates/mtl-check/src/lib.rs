//! # mtl-check — a PROTOTYPE static stack-effect checker for MTL
//!
//! This crate implements **Layer C**: static stack typing over *literal*
//! quotations. It walks a parsed [`mtl_syntax`] program by abstract
//! interpretation and produces one of three [`Verdict`]s:
//!
//! * [`Verdict::Static`] — the program's row-polymorphic stack effect
//!   `∀ρ. ρ++pre -> ρ++post` was fully inferred with no opaque quotes applied
//!   and no provable `Underflow`/`TypeMismatch`.
//! * [`Verdict::Guarded`] — inferred *modulo* runtime guards (an opaque-quote
//!   application, an unprovable Int type-guard, a branch reconcile, or a host
//!   `Call` seam). Each guard is an [`Obligation`].
//! * [`Verdict::Reject`] — the program provably faults (`Underflow` /
//!   `TypeMismatch`) or is outside Layer C (self-application / unbounded
//!   recursion, branch-shape incompatibility, opaque-length uncons).
//!
//! ## Soundness contract
//!
//! Soundness is **relative to the executable reference semantics** in
//! `mtl_core::interp` (`exec_prim`/`exec_step`). The invariant is:
//!
//! > If [`check`] returns [`Verdict::Static`], the program does NOT fault with
//! > `Underflow` or `TypeMismatch` when run on any input stack of shape `pre`.
//!
//! A provably-faulting program must return `Reject` (or, when a runtime guard
//! could in principle avoid the fault, `Guarded`) — **never** `Static`. This is
//! exercised by the fault-corpus soundness smoke test in `tests/`.
//!
//! Every effect rule below was derived by reading `mtl_core::interp::exec_prim`
//! for the EXACT pop order and re-emission of each primitive.

use mtl_syntax::ast::{Prim, Word};
use mtl_syntax::manifest::meta_of;

/// Depth bound for inline application of literal quote bodies (self-app guard).
const MAX_DEPTH: usize = 256;

// ---------------------------------------------------------------------------
// Abstract value lattice
// ---------------------------------------------------------------------------

/// The abstract value lattice (design §"Abstract value lattice").
///
/// `Base(i)` is an internal refinement handle: a cell that was *borrowed* from
/// the polymorphic base still points at `AbsStack::base[i]`, so a later typed
/// use (e.g. an arithmetic operand) can refine that base requirement in place
/// (and, via `Dup`, refine every shared copy at once). `Base` never escapes into
/// a reported [`Effect`]; it is resolved to a concrete [`AbsVal`] first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbsVal {
    /// Definitely an integer.
    Int,
    /// Definitely a quotation whose literal body is KNOWN (constant-folded).
    Lit(Vec<Word>),
    /// Definitely a quotation, body unknown (built from `Any`, or borrowed/host).
    OpaqueQuote,
    /// Kind unknown (host `Call` result, or a join of `Int` vs `Quote`).
    Any,
    /// Internal: a still-unrefined cell borrowed from `AbsStack::base[i]`.
    Base(usize),
}

/// A coarse kind used for type checks and effect display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Quote,
    Any,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Int => "Int",
            Kind::Quote => "Quote",
            Kind::Any => "Any",
        }
    }
}

// ---------------------------------------------------------------------------
// Abstract stack: a symbolic polymorphic base row + known cells.
// ---------------------------------------------------------------------------

/// Abstract stack = a symbolic-base row (`base`) plus known cells (`cells`).
///
/// `cells` are the known cells above an unknown-but-sufficient polymorphic base.
/// Popping when `cells` is empty *borrows* from the base: a fresh `base` slot is
/// created (initially [`AbsVal::Any`]) and the popped cell is [`AbsVal::Base`]
/// pointing at it. `base` is stored in **borrow order** (`base[0]` = the first /
/// topmost cell borrowed). The inferred `Effect.pre` is `base` reversed (so it
/// reads bottom..top like a real input stack); `Effect.post` is the resolved
/// `cells`.
#[derive(Clone, Debug)]
pub struct AbsStack {
    /// Inferred base requirements, in borrow order (`base[0]` = topmost input).
    pub base: Vec<AbsVal>,
    /// Known cells above the base; top of stack is the last element.
    pub cells: Vec<AbsVal>,
    /// Set once an opaque quote is applied / dipped: the region below becomes
    /// fully unknown. A poisoned stack forces later `Reject` if a concrete shape
    /// is required (this is acceptable per the design).
    pub poison: bool,
}

impl AbsStack {
    fn empty() -> Self {
        AbsStack {
            base: Vec::new(),
            cells: Vec::new(),
            poison: false,
        }
    }

    /// Resolve an [`AbsVal`] (following a `Base` handle) to its coarse [`Kind`].
    fn kind_of(&self, v: &AbsVal) -> Kind {
        match v {
            AbsVal::Int => Kind::Int,
            AbsVal::Lit(_) | AbsVal::OpaqueQuote => Kind::Quote,
            AbsVal::Any => Kind::Any,
            AbsVal::Base(i) => match &self.base[*i] {
                AbsVal::Int => Kind::Int,
                AbsVal::Lit(_) | AbsVal::OpaqueQuote => Kind::Quote,
                _ => Kind::Any,
            },
        }
    }

    /// Pop one cell, borrowing from the polymorphic base if `cells` is empty.
    fn pop_cell(&mut self) -> AbsVal {
        if let Some(v) = self.cells.pop() {
            v
        } else if self.poison {
            // Below a poison line the base is fully unknown; never record a req.
            AbsVal::Any
        } else {
            let i = self.base.len();
            self.base.push(AbsVal::Any);
            AbsVal::Base(i)
        }
    }

    fn push(&mut self, v: AbsVal) {
        self.cells.push(v);
    }

    /// Total abstract height (base borrows + known cells).
    fn height(&self) -> usize {
        self.base.len() + self.cells.len()
    }
}

// ---------------------------------------------------------------------------
// Effect / Verdict / Obligation
// ---------------------------------------------------------------------------

/// A row-polymorphic stack effect `∀ρ. ρ++pre -> ρ++post` (kinds only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    /// Required input cells, bottom..top.
    pub pre: Vec<Kind>,
    /// Produced output cells, bottom..top.
    pub post: Vec<Kind>,
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pre: Vec<&str> = self.pre.iter().map(|k| k.label()).collect();
        let post: Vec<&str> = self.post.iter().map(|k| k.label()).collect();
        write!(f, "∀ρ. ρ[{}] -> ρ[{}]", pre.join(" "), post.join(" "))
    }
}

/// A runtime guard the checker could not discharge statically (the "seam"s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Obligation {
    /// Short machine tag, e.g. `"type-guard"`, `"opaque-apply"`, `"host-call"`.
    pub kind: String,
    /// Top-level word index the guard was raised at (best-effort for nested).
    pub at_word_index: usize,
    /// Human note for the LLM/reader.
    pub note: String,
}

/// The three judgments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Fully inferred; no opaque quote applied; no provable fault.
    Static(Effect),
    /// Inferred modulo the listed runtime guards.
    Guarded(Effect, Vec<Obligation>),
    /// Provably faults, or outside Layer C.
    Reject {
        reason: String,
        at_word_index: usize,
        expected: String,
        found: String,
    },
}

impl Verdict {
    pub fn is_static(&self) -> bool {
        matches!(self, Verdict::Static(_))
    }
    pub fn is_guarded(&self) -> bool {
        matches!(self, Verdict::Guarded(..))
    }
    pub fn is_reject(&self) -> bool {
        matches!(self, Verdict::Reject { .. })
    }
    /// One-line tag for tables.
    pub fn tag(&self) -> &'static str {
        match self {
            Verdict::Static(_) => "Static",
            Verdict::Guarded(..) => "Guarded",
            Verdict::Reject { .. } => "Reject",
        }
    }
}

/// The internal reject payload, threaded as `Err` through abstract interp.
#[derive(Clone, Debug)]
struct Reject {
    reason: String,
    at_word_index: usize,
    expected: String,
    found: String,
}

// ---------------------------------------------------------------------------
// Host capability table (tier-3 `Call` seam)
// ---------------------------------------------------------------------------

/// A capability's static stack effect: `consumes` cells popped, `produces`
/// (Any) cells pushed. Mirrors `mtl_core::host::CapabilitySig` (name/consumes/
/// produces); the fault contract is host-side and irrelevant to shape.
#[derive(Clone, Debug)]
pub struct CapEffect {
    pub consumes: usize,
    pub produces: usize,
}

/// Look up a capability effect for a `Call(name)`.
///
/// The table is derived from the tier-3 `contract.md` "declared stack effects"
/// (see `bench/tier3/tasks/*/contract.md`). Unknown names fall back to a
/// sensible default heuristic documented at the call site.
pub fn default_cap_table() -> std::collections::HashMap<String, CapEffect> {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    let mut add = |name: &str, c: usize, p: usize| {
        m.insert(name.to_string(), CapEffect { consumes: c, produces: p });
    };
    // Derived verbatim from bench/tier3/tasks/*/contract.md "Capabilities":
    add("readstate", 0, 1); //  ( -- s )
    add("donep", 1, 2); //      ( s -- s 0|1 )
    add("step", 1, 1); //       ( s -- s' )
    add("readline", 0, 1); //   ( -- h )
    add("emit", 1, 0); //       ( h -- )
    add("readlines", 0, 1); //  ( -- [h...] )
    add("linehit", 1, 2); //    ( h -- h 0|1 )
    add("readjson", 0, 1); //   ( -- j )
    add("getname", 1, 1); //    ( j -- v )
    add("transform", 1, 1); //  ( h -- h' )
    add("tryop", 0, 1); //      ( -- r )
    add("okp", 1, 2); //        ( r -- r 0|1 )
    add("readinput", 0, 1); //  ( -- q )
    add("fetch", 1, 1); //      ( q -- doc )
    add("parse", 1, 1); //      ( doc -- v )
    add("readtext", 0, 1); //   ( -- t )
    add("tokenize", 1, 1); //   ( t -- [w...] )
    add("emitint", 1, 0); //    ( n -- )
    m
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Check a parsed program with the default (tier-3) capability table.
pub fn check(program: &[Word]) -> Verdict {
    check_with(program, &default_cap_table())
}

/// Parse `src` with `mtl_syntax::parse` and check it.
pub fn check_str(src: &str) -> Result<Verdict, mtl_syntax::ParseError> {
    let prog = mtl_syntax::parse(src)?;
    Ok(check(&prog))
}

/// Check a program against an explicit capability table.
pub fn check_with(
    program: &[Word],
    caps: &std::collections::HashMap<String, CapEffect>,
) -> Verdict {
    let mut ctx = Ctx {
        obligations: Vec::new(),
        active: Vec::new(),
        depth: 0,
        top_index: 0,
        caps,
    };
    let mut st = AbsStack::empty();
    match ctx.interp(program, &mut st, true) {
        Err(r) => Verdict::Reject {
            reason: r.reason,
            at_word_index: r.at_word_index,
            expected: r.expected,
            found: r.found,
        },
        Ok(()) => {
            let effect = ctx.effect_of(&st);
            if ctx.obligations.is_empty() {
                Verdict::Static(effect)
            } else {
                Verdict::Guarded(effect, ctx.obligations)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The abstract interpreter
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    obligations: Vec<Obligation>,
    /// Bodies currently being inlined via `Apply` (re-entrancy = self-app).
    active: Vec<Vec<Word>>,
    depth: usize,
    /// Index of the current top-level word (for messages/obligations).
    top_index: usize,
    caps: &'a std::collections::HashMap<String, CapEffect>,
}

/// Outcome of a typed operand check.
enum Chk {
    Ok,
    /// Could not prove statically → record a guard (still proceeds).
    Guard,
    /// Provably wrong → reject `TypeMismatch`.
    Bad(Kind),
}

impl<'a> Ctx<'a> {
    fn effect_of(&self, st: &AbsStack) -> Effect {
        let pre: Vec<Kind> = st.base.iter().rev().map(|v| bare_kind(v)).collect();
        let post: Vec<Kind> = st.cells.iter().map(|v| st.kind_of(v)).collect();
        Effect { pre, post }
    }

    fn guard(&mut self, kind: &str, note: impl Into<String>) {
        self.obligations.push(Obligation {
            kind: kind.to_string(),
            at_word_index: self.top_index,
            note: note.into(),
        });
    }

    /// Require a popped cell to be `Int`, refining `Base` handles in place.
    fn require_int(&mut self, st: &mut AbsStack, v: &AbsVal) -> Chk {
        match v {
            AbsVal::Int => Chk::Ok,
            AbsVal::Lit(_) | AbsVal::OpaqueQuote => Chk::Bad(Kind::Quote),
            AbsVal::Any => Chk::Guard,
            AbsVal::Base(i) => match &st.base[*i] {
                AbsVal::Any => {
                    st.base[*i] = AbsVal::Int; // refine input requirement to Int
                    Chk::Ok
                }
                AbsVal::Int => Chk::Ok,
                AbsVal::Lit(_) | AbsVal::OpaqueQuote => Chk::Bad(Kind::Quote),
                _ => Chk::Guard,
            },
        }
    }

    /// Require a popped cell to be a quote. Returns the known literal body if the
    /// cell is a `Lit`, refining `Base` handles to `OpaqueQuote`.
    fn require_quote(&mut self, st: &mut AbsStack, v: &AbsVal) -> (Chk, Option<Vec<Word>>) {
        match v {
            AbsVal::Lit(b) => (Chk::Ok, Some(b.clone())),
            AbsVal::OpaqueQuote => (Chk::Ok, None),
            AbsVal::Int => (Chk::Bad(Kind::Int), None),
            AbsVal::Any => (Chk::Guard, None),
            AbsVal::Base(i) => match &st.base[*i] {
                AbsVal::Any => {
                    st.base[*i] = AbsVal::OpaqueQuote;
                    (Chk::Ok, None)
                }
                AbsVal::OpaqueQuote => (Chk::Ok, None),
                AbsVal::Lit(b) => (Chk::Ok, Some(b.clone())),
                AbsVal::Int => (Chk::Bad(Kind::Int), None),
                _ => (Chk::Guard, None),
            },
        }
    }

    /// Walk `words` left→right over `st`. `top_level` toggles `top_index`
    /// tracking (for message locality).
    fn interp(&mut self, words: &[Word], st: &mut AbsStack, top_level: bool) -> Result<(), Reject> {
        if self.depth > MAX_DEPTH {
            return Err(Reject {
                reason: "self-applicative / unbounded recursion — outside Layer C (use a recursion combinator)".into(),
                at_word_index: self.top_index,
                expected: "bounded inline depth".into(),
                found: format!("inline depth > {MAX_DEPTH}"),
            });
        }
        for (i, w) in words.iter().enumerate() {
            if top_level {
                self.top_index = i;
            }
            self.step(w, st)?;
        }
        Ok(())
    }

    fn step(&mut self, w: &Word, st: &mut AbsStack) -> Result<(), Reject> {
        match w {
            Word::PushInt(_) => {
                st.push(AbsVal::Int);
                Ok(())
            }
            Word::PushQuote(body) => {
                st.push(AbsVal::Lit(body.clone()));
                Ok(())
            }
            Word::Call(chars) => {
                let name: String = chars.iter().collect();
                let eff = self.caps.get(&name).cloned().unwrap_or(
                    // Heuristic fallback (documented): a bare capability whose
                    // contract we couldn't parse is treated as `( -- r )` when
                    // it reads (name starts with "read"), else `( x -- )` emit.
                    if name.starts_with("read") {
                        CapEffect { consumes: 0, produces: 1 }
                    } else {
                        CapEffect { consumes: 1, produces: 0 }
                    },
                );
                for _ in 0..eff.consumes {
                    st.pop_cell();
                }
                for _ in 0..eff.produces {
                    st.push(AbsVal::Any);
                }
                self.guard("host-call", format!("host capability `{name}` (consumes {}, produces {})", eff.consumes, eff.produces));
                Ok(())
            }
            Word::Prim(p) => self.prim(*p, st),
        }
    }

    fn reject_type(&self, p: Prim, expected: &str, found: &str) -> Reject {
        let m = meta_of(p);
        Reject {
            reason: "TypeMismatch".into(),
            at_word_index: self.top_index,
            expected: format!("{expected} for {} ({})", m.glyph, m.name),
            found: found.to_string(),
        }
    }

    fn prim(&mut self, p: Prim, st: &mut AbsStack) -> Result<(), Reject> {
        match p {
            // ---------------- stack shuffling ----------------
            Prim::Dup => {
                let x = st.pop_cell();
                st.push(x.clone());
                st.push(x);
                Ok(())
            }
            Prim::Drop => {
                st.pop_cell();
                Ok(())
            }
            Prim::Swap => {
                let b = st.pop_cell();
                let a = st.pop_cell();
                st.push(b);
                st.push(a);
                Ok(())
            }
            Prim::Rot => {
                // ( a b c -- b c a )
                let c = st.pop_cell();
                let b = st.pop_cell();
                let a = st.pop_cell();
                st.push(b);
                st.push(c);
                st.push(a);
                Ok(())
            }
            Prim::Over => {
                // ( a b -- a b a )
                let b = st.pop_cell();
                let a = st.pop_cell();
                st.push(a.clone());
                st.push(b);
                st.push(a);
                Ok(())
            }
            // ---------------- arithmetic / comparison ----------------
            Prim::Add | Prim::Sub | Prim::Mul | Prim::Div | Prim::Mod | Prim::Xor
            | Prim::Eq | Prim::Lt => {
                let b = st.pop_cell();
                let a = st.pop_cell();
                // top operand
                match self.require_int(st, &b) {
                    Chk::Ok => {}
                    Chk::Guard => self.guard("type-guard", format!("{} operand (top) not provably Int", meta_of(p).glyph)),
                    Chk::Bad(k) => {
                        return Err(self.reject_type(p, "[Int, Int] on top", &format!("[Int, {}]", k.label())))
                    }
                }
                match self.require_int(st, &a) {
                    Chk::Ok => {}
                    Chk::Guard => self.guard("type-guard", format!("{} operand (2nd) not provably Int", meta_of(p).glyph)),
                    Chk::Bad(k) => {
                        return Err(self.reject_type(p, "[Int, Int] on top", &format!("[{}, {}]", k.label(), st.kind_of(&b).label())))
                    }
                }
                st.push(AbsVal::Int);
                Ok(())
            }
            // ---------------- quotation algebra ----------------
            Prim::Apply => {
                let q = st.pop_cell();
                let (chk, body) = self.require_quote(st, &q);
                match chk {
                    Chk::Bad(k) => Err(self.reject_type(p, "[Quote] on top", &format!("[{}]", k.label()))),
                    Chk::Guard => {
                        // Any: might be a quote at runtime; can't know its effect.
                        self.guard("opaque-apply", "apply of a value of unknown kind (Any)");
                        st.poison = true;
                        st.cells.clear();
                        Ok(())
                    }
                    Chk::Ok => match body {
                        Some(b) => self.inline_apply(&b, st),
                        None => {
                            // OpaqueQuote: definitely a quote, effect unknown.
                            self.guard("opaque-apply", "apply of an opaque (non-literal) quote");
                            st.poison = true;
                            st.cells.clear();
                            Ok(())
                        }
                    },
                }
            }
            Prim::Dip => {
                // ( a [q] -- ... a ): run q on the rest, then restore a on top.
                let q = st.pop_cell();
                let a = st.pop_cell();
                let (chk, body) = self.require_quote(st, &q);
                match chk {
                    Chk::Bad(k) => Err(self.reject_type(p, "[a, Quote] on top", &format!("[a, {}]", k.label()))),
                    Chk::Guard => {
                        self.guard("opaque-dip", "dip with a value of unknown kind (Any)");
                        st.poison = true;
                        st.cells.clear();
                        st.push(a);
                        Ok(())
                    }
                    Chk::Ok => match body {
                        Some(b) => {
                            self.inline_apply(&b, st)?;
                            st.push(a);
                            Ok(())
                        }
                        None => {
                            self.guard("opaque-dip", "dip with an opaque quote");
                            st.poison = true;
                            st.cells.clear();
                            st.push(a);
                            Ok(())
                        }
                    },
                }
            }
            Prim::Cat => {
                // ( [a] [b] -- [a b] ) : interp does a.extend(b), a is the lower.
                let b = st.pop_cell();
                let a = st.pop_cell();
                let (cb, bb) = self.require_quote(st, &b);
                if let Chk::Bad(k) = cb {
                    return Err(self.reject_type(p, "[Quote, Quote] on top", &format!("[Quote, {}]", k.label())));
                }
                let (ca, ba) = self.require_quote(st, &a);
                if let Chk::Bad(k) = ca {
                    return Err(self.reject_type(p, "[Quote, Quote] on top", &format!("[{}, Quote]", k.label())));
                }
                match (ba, bb) {
                    (Some(mut abody), Some(bbody)) => {
                        abody.extend(bbody);
                        st.push(AbsVal::Lit(abody)); // constant-fold
                    }
                    _ => {
                        self.guard("opaque-cat", "cat of at least one opaque quote");
                        st.push(AbsVal::OpaqueQuote);
                    }
                }
                Ok(())
            }
            Prim::Cons => {
                // ( v [q] -- [v q] ) : newq = value_to_word(v) :: q
                let q = st.pop_cell();
                let v = st.pop_cell();
                let (cq, qbody) = self.require_quote(st, &q);
                if let Chk::Bad(k) = cq {
                    return Err(self.reject_type(p, "[v, Quote] on top", &format!("[v, {}]", k.label())));
                }
                let vword = match &v {
                    AbsVal::Int => Some(Word::PushInt(0)), // value irrelevant to shape
                    AbsVal::Lit(b) => Some(Word::PushQuote(b.clone())),
                    AbsVal::Base(i) if matches!(st.base[*i], AbsVal::Int) => Some(Word::PushInt(0)),
                    _ => None,
                };
                match (qbody, vword) {
                    (Some(qb), Some(vw)) => {
                        let mut newq = Vec::with_capacity(qb.len() + 1);
                        newq.push(vw);
                        newq.extend(qb);
                        st.push(AbsVal::Lit(newq)); // constant-fold
                    }
                    _ => {
                        self.guard("opaque-cons", "cons with opaque quote or non-literal value");
                        st.push(AbsVal::OpaqueQuote);
                    }
                }
                Ok(())
            }
            Prim::Uncons => self.uncons(st),
            Prim::If => self.if_prim(st),
            Prim::Times => self.times(st),
            Prim::PrimRec => self.primrec(st),
            Prim::Fold => self.fold(st),
            Prim::LinRec => self.linrec(st),
        }
    }

    /// Inline `Apply` of a KNOWN literal body, with the self-app / depth guard.
    fn inline_apply(&mut self, body: &[Word], st: &mut AbsStack) -> Result<(), Reject> {
        if self.active.iter().any(|b| b == body) {
            return Err(Reject {
                reason: "self-applicative / unbounded recursion — outside Layer C (use a recursion combinator)".into(),
                at_word_index: self.top_index,
                expected: "bounded, non-self-applicative program".into(),
                found: "re-entrant application of a quote already being applied".into(),
            });
        }
        self.active.push(body.to_vec());
        self.depth += 1;
        let r = self.interp(body, st, false);
        self.depth -= 1;
        self.active.pop();
        r
    }

    // ----- Uncons -----
    fn uncons(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        let q = st.pop_cell();
        let (chk, body) = self.require_quote(st, &q);
        match chk {
            Chk::Bad(k) => Err(self.reject_type(Prim::Uncons, "[Quote] on top", &format!("[{}]", k.label()))),
            Chk::Ok => match body {
                Some(b) => {
                    if let Some(head) = b.first() {
                        match head {
                            Word::PushInt(_) => {
                                let tail: Vec<Word> = b[1..].to_vec();
                                st.push(AbsVal::Int); // head
                                st.push(AbsVal::Lit(tail)); // tail quote
                                st.push(AbsVal::Int); // 1
                            }
                            Word::PushQuote(inner) => {
                                let tail: Vec<Word> = b[1..].to_vec();
                                st.push(AbsVal::Lit(inner.clone())); // head
                                st.push(AbsVal::Lit(tail));
                                st.push(AbsVal::Int);
                            }
                            Word::Prim(_) | Word::Call(_) => {
                                // Malformed head: interp faults TypeMismatch.
                                return Err(self.reject_type(
                                    Prim::Uncons,
                                    "quote whose head is a value word",
                                    "quote whose head is a bare Prim/Call",
                                ));
                            }
                        }
                    } else {
                        // Empty literal: deterministic ( [] -- 0 ).
                        st.push(AbsVal::Int);
                    }
                    Ok(())
                }
                None => {
                    // Opaque quote: length unknown → branch-dependent sum shape.
                    // No literal to discriminate; per design, reject (the peephole
                    // in `if_prim` handles the uncons+if idiom when it occurs).
                    Err(Reject {
                        reason: "branch-dependent stack shape at uncons; opaque quote length unknown".into(),
                        at_word_index: self.top_index,
                        expected: "a literal quote of known length".into(),
                        found: "an opaque quote (host/borrowed) of unknown length".into(),
                    })
                }
            },
            Chk::Guard => Err(Reject {
                reason: "branch-dependent stack shape at uncons; opaque quote length unknown".into(),
                at_word_index: self.top_index,
                expected: "a literal quote of known length".into(),
                found: "a value of unknown kind (Any)".into(),
            }),
        }
    }

    // ----- If -----
    fn if_prim(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        // ( c [t] [f] -- ... )
        let f = st.pop_cell();
        let t = st.pop_cell();
        let c = st.pop_cell();
        // flag must be Int
        match self.require_int(st, &c) {
            Chk::Ok => {}
            Chk::Guard => self.guard("type-guard", "if flag not provably Int"),
            Chk::Bad(k) => {
                return Err(self.reject_type(Prim::If, "[Int, Quote, Quote] (flag, then, else)", &format!("flag = {}", k.label())))
            }
        }
        let (ct, tbody) = self.require_quote(st, &t);
        if let Chk::Bad(k) = ct {
            return Err(self.reject_type(Prim::If, "then-branch Quote", &format!("{}", k.label())));
        }
        let (cf, fbody) = self.require_quote(st, &f);
        if let Chk::Bad(k) = cf {
            return Err(self.reject_type(Prim::If, "else-branch Quote", &format!("{}", k.label())));
        }
        match (tbody, fbody) {
            (Some(tb), Some(fb)) => {
                // Both literal: compute each branch's effect on a copy.
                let mut st_t = st.clone();
                self.interp(&tb, &mut st_t, false)?;
                let mut st_f = st.clone();
                self.interp(&fb, &mut st_f, false)?;
                // Require equal net height delta (equal final heights, same start).
                if st_t.height() != st_f.height() || st_t.cells.len() != st_f.cells.len() {
                    let dt = st_t.height() as i64 - st.height() as i64;
                    let df = st_f.height() as i64 - st.height() as i64;
                    return Err(Reject {
                        reason: format!(
                            "branch-stack incompatibility: then-branch net Δ={dt}, else-branch net Δ={df}"
                        ),
                        at_word_index: self.top_index,
                        expected: "both branches to have equal net stack height delta".into(),
                        found: format!("then Δ={dt} vs else Δ={df}"),
                    });
                }
                // Join per-cell output types; keep the joined stack.
                let n = st_t.cells.len();
                let mut joined = Vec::with_capacity(n);
                for i in 0..n {
                    let kt = st_t.kind_of(&st_t.cells[i]);
                    let kf = st_f.kind_of(&st_f.cells[i]);
                    joined.push(kind_to_abs(join_kind(kt, kf)));
                }
                // Reconstruct: keep t's base refinements (both started from `st`,
                // so base grew consistently for pops that borrowed identically);
                // widen with f's where they differ.
                st.base = reconcile_base(&st_t.base, &st_f.base);
                st.cells = joined;
                if st.base.len() != st_t.base.len() {
                    self.guard("branch-base", "branches imposed differing input requirements");
                }
                Ok(())
            }
            _ => {
                // At least one opaque branch: outcome shape unknown → guard+poison.
                self.guard("opaque-branch", "if with an opaque branch quote");
                st.poison = true;
                st.cells.clear();
                Ok(())
            }
        }
    }

    // ----- Times -----
    fn times(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        // ( n [Q] -- ... )
        let q = st.pop_cell();
        let (cq, qbody) = self.require_quote(st, &q);
        if let Chk::Bad(k) = cq {
            return Err(self.reject_type(Prim::Times, "[Int, Quote] (n, body)", &format!("body = {}", k.label())));
        }
        let n = st.pop_cell();
        match self.require_int(st, &n) {
            Chk::Ok => {}
            Chk::Guard => self.guard("type-guard", "times count not provably Int"),
            Chk::Bad(k) => {
                return Err(self.reject_type(Prim::Times, "[Int, Quote] (n, body)", &format!("count = {}", k.label())))
            }
        }
        match qbody {
            None => {
                self.guard("opaque-times", "times with an opaque body");
                st.poison = true;
                st.cells.clear();
                Ok(())
            }
            Some(body) => {
                let fresh = self.infer_fresh(&body)?;
                if fresh.poison {
                    self.guard("times-unverifiable", "times body effect not statically verifiable");
                    st.poison = true;
                    st.cells.clear();
                    return Ok(());
                }
                match stable_effect(&fresh) {
                    Some(reqs) => {
                        // Impose the body's per-cell input requirement on the
                        // current stack (net effect is identity when stable).
                        self.apply_identity_reqs(st, &reqs)?;
                        Ok(())
                    }
                    None => {
                        let delta = fresh.cells.len() as i64 - fresh.base.len() as i64;
                        Err(Reject {
                            reason: format!("times body must be stack-neutral for static shape; found Δ={delta}"),
                            at_word_index: self.top_index,
                            expected: "a stack-neutral, type-stable body (Δ=0)".into(),
                            found: format!("body with net Δ={delta} or unstable cell types"),
                        })
                    }
                }
            }
        }
    }

    // ----- PrimRec -----
    fn primrec(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        // ( n [I] [C] -- r )
        let qc = st.pop_cell();
        let (cc, cbody) = self.require_quote(st, &qc);
        if let Chk::Bad(k) = cc {
            return Err(self.reject_type(Prim::PrimRec, "[Int, Quote, Quote] (n,[I],[C])", &format!("[C] = {}", k.label())));
        }
        let qi = st.pop_cell();
        let (ci, ibody) = self.require_quote(st, &qi);
        if let Chk::Bad(k) = ci {
            return Err(self.reject_type(Prim::PrimRec, "[Int, Quote, Quote] (n,[I],[C])", &format!("[I] = {}", k.label())));
        }
        let n = st.pop_cell();
        match self.require_int(st, &n) {
            Chk::Ok => {}
            Chk::Guard => self.guard("type-guard", "primrec count not provably Int"),
            Chk::Bad(k) => {
                return Err(self.reject_type(Prim::PrimRec, "[Int, Quote, Quote] (n,[I],[C])", &format!("count = {}", k.label())))
            }
        }
        let (ibody, cbody) = match (ibody, cbody) {
            (Some(i), Some(c)) => (i, c),
            _ => {
                self.guard("opaque-primrec", "primrec with an opaque I or C");
                st.poison = true;
                st.cells.clear();
                return Ok(());
            }
        };
        // Infer I on a fresh stack: it must be self-contained (no borrow) so the
        // accumulator region is well-defined.
        let fresh_i = self.infer_fresh(&ibody)?;
        if fresh_i.poison || !fresh_i.base.is_empty() {
            self.guard("primrec-init", "primrec [I] borrows/opaque; accumulator width not static");
            st.poison = true;
            st.cells.clear();
            return Ok(());
        }
        let acc: Vec<Kind> = fresh_i.cells.iter().map(|v| fresh_i.kind_of(v)).collect();
        // Verify C maps ( counter:Int, acc -- acc ) height- and type-stably.
        let mut cstk = AbsStack::empty();
        cstk.push(AbsVal::Int); // counter (below acc)
        for k in &acc {
            cstk.push(kind_to_abs(*k));
        }
        self.interp(&cbody, &mut cstk, false)?;
        let out: Vec<Kind> = cstk.cells.iter().map(|v| cstk.kind_of(v)).collect();
        if cstk.poison || !cstk.base.is_empty() || out != acc {
            return Err(Reject {
                reason: "primrec combine [C] must map (counter:Int, acc -- acc) type-stably".into(),
                at_word_index: self.top_index,
                expected: format!("acc region {:?}", acc.iter().map(|k| k.label()).collect::<Vec<_>>()),
                found: format!("combine yields {:?}", out.iter().map(|k| k.label()).collect::<Vec<_>>()),
            });
        }
        // Static: consume n,[I],[C]; push the accumulator region.
        for k in &acc {
            st.push(kind_to_abs(*k));
        }
        Ok(())
    }

    // ----- Fold -----
    fn fold(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        // ( [seq] init [C] -- r )  LEFT fold, C : ( acc elem -- acc )
        let qc = st.pop_cell();
        let (cc, cbody) = self.require_quote(st, &qc);
        if let Chk::Bad(k) = cc {
            return Err(self.reject_type(Prim::Fold, "[Quote, init, Quote] ([seq],init,[C])", &format!("[C] = {}", k.label())));
        }
        let init = st.pop_cell();
        let acc_kind = st.kind_of(&init);
        let seq = st.pop_cell();
        let (cs, seq_body) = self.require_quote(st, &seq);
        if let Chk::Bad(k) = cs {
            return Err(self.reject_type(Prim::Fold, "[seq] must be a Quote", &format!("{}", k.label())));
        }
        let cbody = match cbody {
            Some(c) => c,
            None => {
                self.guard("opaque-fold", "fold with an opaque combine [C]");
                st.poison = true;
                st.cells.clear();
                return Ok(());
            }
        };
        // Verify C : ( acc elem -- acc ) type-stable, elem unknown (Any).
        let mut cstk = AbsStack::empty();
        cstk.push(kind_to_abs(acc_kind)); // acc (below)
        cstk.push(AbsVal::Any); // elem (top), kind unknown across the seq
        // record whether the combine guards on the (Any) element
        let obl_before = self.obligations.len();
        self.interp(&cbody, &mut cstk, false)?;
        let elem_guarded = self.obligations.len() > obl_before;
        let out: Vec<Kind> = cstk.cells.iter().map(|v| cstk.kind_of(v)).collect();
        if cstk.poison || !cstk.base.is_empty() || out.len() != 1 {
            return Err(Reject {
                reason: "fold combine [C] must map (acc elem -- acc) height-stably".into(),
                at_word_index: self.top_index,
                expected: format!("acc [{}]", acc_kind.label()),
                found: format!("combine yields {:?}", out.iter().map(|k| k.label()).collect::<Vec<_>>()),
            });
        }
        let result_kind = out[0];
        // Result is the accumulator. seq length drives runtime iteration.
        match seq_body {
            Some(_) => {
                // Literal seq: fully known length.
                if elem_guarded {
                    // element types unknown even in a literal seq (we model them
                    // as Any), so keep the guard already recorded.
                }
                st.push(kind_to_abs(result_kind));
                Ok(())
            }
            None => {
                self.guard("opaque-fold-seq", "fold over an opaque sequence (runtime length)");
                st.push(kind_to_abs(result_kind));
                Ok(())
            }
        }
    }

    // ----- LinRec -----
    fn linrec(&mut self, st: &mut AbsStack) -> Result<(), Reject> {
        // ( [P] [T] [R1] [R2] -- ... ). Conservative: we do NOT unroll the
        // recursion (depth is runtime-dependent). We require the four operands to
        // be quotes and emit a Guarded obligation; a provably-non-quote operand
        // rejects TypeMismatch. This is sound (prefers Guard over unsound Static).
        let r2 = st.pop_cell();
        let r1 = st.pop_cell();
        let t = st.pop_cell();
        let pq = st.pop_cell();
        for (v, lbl) in [(&r2, "[R2]"), (&r1, "[R1]"), (&t, "[T]"), (&pq, "[P]")] {
            let (chk, _) = self.require_quote(st, v);
            if let Chk::Bad(k) = chk {
                return Err(self.reject_type(Prim::LinRec, "[P] [T] [R1] [R2] all Quotes", &format!("{lbl} = {}", k.label())));
            }
        }
        self.guard("linrec-recursion", "linrec recursion depth is runtime-dependent");
        st.poison = true;
        st.cells.clear();
        Ok(())
    }

    /// Infer a body's effect on a FRESH polymorphic stack (used by combinators).
    fn infer_fresh(&mut self, body: &[Word]) -> Result<AbsStack, Reject> {
        let mut fresh = AbsStack::empty();
        self.interp(body, &mut fresh, false)?;
        Ok(fresh)
    }

    /// Given a stable body's per-cell input requirement (`reqs`, borrow order),
    /// impose it on `st` (pop, type-check, push back unchanged — net identity).
    fn apply_identity_reqs(&mut self, st: &mut AbsStack, reqs: &[Kind]) -> Result<(), Reject> {
        let mut popped = Vec::with_capacity(reqs.len());
        for req in reqs {
            let v = st.pop_cell();
            match req {
                Kind::Int => match self.require_int(st, &v) {
                    Chk::Ok => {}
                    Chk::Guard => self.guard("type-guard", "combinator body requires Int here"),
                    Chk::Bad(k) => {
                        // restore before erroring is unnecessary (we error out)
                        return Err(self.reject_type(Prim::Times, "[Int] where combinator body operates", &format!("{}", k.label())));
                    }
                },
                Kind::Quote => {
                    let (chk, _) = self.require_quote(st, &v);
                    if let Chk::Bad(k) = chk {
                        return Err(self.reject_type(Prim::Times, "[Quote] where combinator body operates", &format!("{}", k.label())));
                    }
                }
                Kind::Any => {}
            }
            popped.push(v);
        }
        // push back in original order (reqs[0] was the topmost popped)
        for v in popped.into_iter().rev() {
            st.push(v);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Coarse kind of a bare (non-`Base`) [`AbsVal`], for `pre` display.
fn bare_kind(v: &AbsVal) -> Kind {
    match v {
        AbsVal::Int => Kind::Int,
        AbsVal::Lit(_) | AbsVal::OpaqueQuote => Kind::Quote,
        _ => Kind::Any,
    }
}

fn kind_to_abs(k: Kind) -> AbsVal {
    match k {
        Kind::Int => AbsVal::Int,
        Kind::Quote => AbsVal::OpaqueQuote,
        Kind::Any => AbsVal::Any,
    }
}

/// Join two kinds (the `⊔` of the lattice): equal → same; Int⊔Quote → Any; any
/// `Any` → `Any`.
fn join_kind(a: Kind, b: Kind) -> Kind {
    if a == b {
        a
    } else {
        Kind::Any
    }
}

/// Reconcile two base requirement rows (from two If branches). Both branches
/// started from the same stack, so the common prefix agrees; the join widens any
/// disagreement to `Any` and keeps the longer row's tail.
fn reconcile_base(a: &[AbsVal], b: &[AbsVal]) -> Vec<AbsVal> {
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        match (a.get(i), b.get(i)) {
            (Some(x), Some(y)) => {
                let kx = bare_kind(x);
                let ky = bare_kind(y);
                out.push(kind_to_abs(join_kind(kx, ky)));
            }
            (Some(x), None) | (None, Some(x)) => out.push(x.clone()),
            (None, None) => unreachable!(),
        }
    }
    out
}

/// If `fresh` (a body inferred on a fresh stack) is stack-neutral AND
/// type-stable, return its per-cell input requirement in borrow order (so the
/// caller can impose it as an identity). Otherwise `None`.
///
/// Stack-neutral: `cells.len() == base.len()`. Type-stable: the output stack
/// equals the input stack positionally. The input stack bottom..top is
/// `base` reversed (base[0] = topmost input); the output stack bottom..top is
/// `cells`. So we require `cells[j].kind == base[len-1-j].kind` for all j, with
/// `Any` acting as a wildcard on the base side (an unrefined polymorphic slot).
fn stable_effect(fresh: &AbsStack) -> Option<Vec<Kind>> {
    let k = fresh.base.len();
    if fresh.cells.len() != k {
        return None;
    }
    for j in 0..k {
        let cell_kind = fresh.kind_of(&fresh.cells[j]);
        let base_kind = bare_kind(&fresh.base[k - 1 - j]);
        let ok = base_kind == cell_kind || base_kind == Kind::Any || cell_kind == Kind::Any;
        if !ok {
            return None;
        }
    }
    // Return per-cell input requirement in borrow order (base[0] = topmost).
    Some(fresh.base.iter().map(|v| bare_kind(v)).collect())
}

#[cfg(test)]
mod tests;
