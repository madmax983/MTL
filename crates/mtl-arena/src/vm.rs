//! The arena VM: the three arenas plus the operations that thread a [`VmState`]
//! through them. Forking is a plain `Copy` of the `VmState`; the arenas are
//! shared.
//!
//! Everything here is **total**: there are no `unwrap`/`expect`/`unreachable!`/
//! `panic!` sites and no panicking array index in the step/exec path (matching the
//! reference interpreter's "never panics" contract, `interp.rs` doc §Totality).
//! Every stack access, tape access, refinement, and bounds/overflow check yields a
//! value — either a [`Fault`] to the caller or an internal `Option` that a
//! `debug_assert`-guarded invariant collapses safely.

use crate::arena::{
    ContArena, ContNode, Mark, QuoteArena, StackArena, StackPtr, VmState, EMPTY_STACK, NIL_CONT,
};
use crate::types::{Fault, ProgWord, Prim, QuoteId, Value, Word};
use mtl_core::interp as itp;

/// Internal per-word step result (mirrors `interp::Step` minus `Halt`, which is
/// signalled by [`Vm::next_word`] returning `None`).
pub(crate) enum StepR {
    Next,
    Fault(Fault),
    Invoke(String),
}

/// The arena VM: the three arenas. A [`VmState`] is threaded through explicitly so
/// forking is a 12-byte `Copy`.
#[derive(Clone, Debug)]
pub struct Vm {
    /// The interned quote tape + call-name table. Public for read-only reification.
    pub quotes: QuoteArena,
    pub(crate) stack: StackArena,
    pub(crate) cont: ContArena,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// A fresh VM with empty arenas.
    pub fn new() -> Self {
        Self { quotes: QuoteArena::new(), stack: StackArena::new(), cont: ContArena::new() }
    }

    // ============================================================= allocation
    // All tape growth goes through these fallible helpers so a u32-address
    // overflow becomes a clean `Fault::Overflow` (design §3.4) instead of a
    // silent `as u32` wraparound. In practice a corpus never approaches 4.29e9
    // words, so these `None` arms are defensive, not hot.

    /// Intern `ws` at the end of the tape. `None` iff that would overflow u32.
    #[inline]
    pub(crate) fn try_alloc(&mut self, ws: &[Word]) -> Option<QuoteId> {
        if !self.quotes.tape_fits(ws.len()) {
            return None;
        }
        let start = self.quotes.tape.len() as u32;
        self.quotes.tape.extend_from_slice(ws);
        Some(QuoteId { start, len: ws.len() as u32 })
    }

    /// `cat(a, b)`: append copies of both bodies, returning a fresh contiguous id.
    /// O(|a|+|b|). `None` on u32 overflow.
    #[inline]
    pub(crate) fn try_cat(&mut self, a: QuoteId, b: QuoteId) -> Option<QuoteId> {
        let total = (a.len as usize).checked_add(b.len as usize)?;
        if !self.quotes.tape_fits(total) {
            return None;
        }
        let start = self.quotes.tape.len() as u32;
        self.quotes.tape.extend_from_within(a.start as usize..a.end() as usize);
        self.quotes.tape.extend_from_within(b.start as usize..b.end() as usize);
        Some(QuoteId { start, len: a.len + b.len })
    }

    /// `cons(head, q)`: prepend one word to a copy of `q`'s body. O(|q|). `None`
    /// on u32 overflow.
    #[inline]
    pub(crate) fn try_cons(&mut self, head: Word, q: QuoteId) -> Option<QuoteId> {
        let total = (q.len as usize).checked_add(1)?;
        if !self.quotes.tape_fits(total) {
            return None;
        }
        let start = self.quotes.tape.len() as u32;
        self.quotes.tape.push(head);
        self.quotes.tape.extend_from_within(q.start as usize..q.end() as usize);
        Some(QuoteId { start, len: q.len + 1 })
    }

    /// Build the `LinRec` "else" quote body `R1 ++ [[P],[T],[R1],[R2],linrec] ++ R2`
    /// as a fresh interned value quote. `None` on u32 overflow.
    #[inline]
    pub(crate) fn try_linrec_else(
        &mut self,
        qp: QuoteId,
        qt: QuoteId,
        qr1: QuoteId,
        qr2: QuoteId,
    ) -> Option<QuoteId> {
        let total = (qr1.len as usize)
            .checked_add(5)?
            .checked_add(qr2.len as usize)?;
        if !self.quotes.tape_fits(total) {
            return None;
        }
        let start = self.quotes.tape.len() as u32;
        self.quotes.tape.extend_from_within(qr1.start as usize..qr1.end() as usize);
        self.quotes.tape.push(Word::PushQuote(qp));
        self.quotes.tape.push(Word::PushQuote(qt));
        self.quotes.tape.push(Word::PushQuote(qr1));
        self.quotes.tape.push(Word::PushQuote(qr2));
        self.quotes.tape.push(Word::Prim(Prim::LinRec));
        self.quotes.tape.extend_from_within(qr2.start as usize..qr2.end() as usize);
        Some(QuoteId { start, len: qr1.len + 5 + qr2.len })
    }

    /// Read a tape word by absolute index (bounds-checked).
    #[inline]
    pub(crate) fn word_at(&self, i: u32) -> Option<Word> {
        self.quotes.word_at(i)
    }

    // ================================================================ compile
    /// Intern a source program tree into the tape, returning its body id. `None`
    /// iff interning would overflow the u32 tape address space.
    pub fn compile(&mut self, prog: &[ProgWord]) -> Option<QuoteId> {
        let mut words = Vec::with_capacity(prog.len());
        for pw in prog {
            let w = match pw {
                ProgWord::PushInt(n) => Word::PushInt(*n),
                ProgWord::PushQuote(body) => Word::PushQuote(self.compile(body)?),
                ProgWord::Prim(p) => Word::Prim(*p),
                ProgWord::Call(name) => Word::Call(self.quotes.intern_call(name)),
            };
            words.push(w);
        }
        self.try_alloc(&words)
    }

    // ==================================================== continuation ops
    /// Read (and consume) the next word, popping exhausted segments. Returns
    /// `None` at NIL (halt). Popping segments costs no fuel — only executed words
    /// are steps, exactly like `interp::run`.
    #[inline]
    pub(crate) fn next_word(&self, st: &mut VmState) -> Option<Word> {
        loop {
            if st.cont == NIL_CONT {
                return None;
            }
            let Some(node) = self.cont.node_at(st.cont) else {
                debug_assert!(false, "cont ptr out of bounds");
                return None;
            };
            let len = node.qend.saturating_sub(node.qstart);
            if st.cursor < len {
                // checked_add: a corrupt {qstart,cursor} could exceed u32; treat
                // as halt rather than panic (invariant: never reached in practice).
                let Some(idx) = node.qstart.checked_add(st.cursor) else {
                    debug_assert!(false, "tape index overflow");
                    return None;
                };
                let Some(w) = self.word_at(idx) else {
                    debug_assert!(false, "tape index out of bounds");
                    return None;
                };
                st.cursor += 1;
                return Some(w);
            }
            // Segment exhausted: POP to parent, resume at its frozen offset.
            st.cont = node.parent;
            if st.cont == NIL_CONT {
                return None;
            }
            let Some(parent) = self.cont.node_at(st.cont) else {
                debug_assert!(false, "cont parent ptr out of bounds");
                return None;
            };
            st.cursor = parent.off;
        }
    }

    /// Prepend quote `q` to the continuation: `cont := q ++ cont`. Freezes the
    /// current head (capturing `cursor` as its resume offset) and pushes a child
    /// segment. ≤2 node allocs, O(1), no tail copy. Empty `q` is a no-op (mirror
    /// of `interp::prepend`'s empty-prefix early return).
    #[inline]
    pub fn prepend(&mut self, st: &mut VmState, q: QuoteId) {
        if q.len == 0 {
            return;
        }
        let child = if st.cont == NIL_CONT {
            self.cont.push(ContNode { qstart: q.start, qend: q.end(), off: 0, parent: NIL_CONT })
        } else {
            let Some(h) = self.cont.node_at(st.cont) else {
                debug_assert!(false, "cont ptr out of bounds in prepend");
                return;
            };
            let frozen = self.cont.push(ContNode {
                qstart: h.qstart,
                qend: h.qend,
                off: st.cursor,
                parent: h.parent,
            });
            self.cont.push(ContNode { qstart: q.start, qend: q.end(), off: 0, parent: frozen })
        };
        st.cont = child;
        st.cursor = 0;
    }

    // ============================================================= stack ops
    /// Pop the top value, returning `(top, rest)`. `None` iff the stack is empty.
    /// Non-mutating: it only reads a node's `parent`, so the caller can decide to
    /// commit (`st.stack = rest`) or leave the stack untouched on a type fault.
    #[inline]
    pub(crate) fn pop1(&self, p: StackPtr) -> Option<(Value, StackPtr)> {
        self.stack.node_at(p).filter(|_| p != EMPTY_STACK).map(|n| (n.value, n.parent))
    }

    /// Pop two values: returns `(second, top, rest)`. `None` on underflow.
    #[inline]
    pub(crate) fn pop2(&self, p: StackPtr) -> Option<(Value, Value, StackPtr)> {
        let (b, p1) = self.pop1(p)?;
        let (a, p2) = self.pop1(p1)?;
        Some((a, b, p2))
    }

    /// Pop three values: returns `(third, second, top, rest)`. `None` on underflow.
    #[inline]
    pub(crate) fn pop3(&self, p: StackPtr) -> Option<(Value, Value, Value, StackPtr)> {
        let (c, p1) = self.pop1(p)?;
        let (b, p2) = self.pop1(p1)?;
        let (a, p3) = self.pop1(p2)?;
        Some((a, b, c, p3))
    }

    /// Pop four values: returns `(fourth, third, second, top, rest)`. `None` on underflow.
    #[inline]
    pub(crate) fn pop4(&self, p: StackPtr) -> Option<(Value, Value, Value, Value, StackPtr)> {
        let (d, p1) = self.pop1(p)?;
        let (c, p2) = self.pop1(p1)?;
        let (b, p3) = self.pop1(p2)?;
        let (a, p4) = self.pop1(p3)?;
        Some((a, b, c, d, p4))
    }

    #[inline]
    pub(crate) fn push(&mut self, ptr: StackPtr, v: Value) -> StackPtr {
        self.stack.push(ptr, v)
    }

    // ============================================================= exec a word
    pub(crate) fn exec_word(&mut self, st: &mut VmState, w: Word) -> StepR {
        match w {
            Word::PushInt(n) => {
                st.stack = self.push(st.stack, Value::Int(n));
                StepR::Next
            }
            Word::PushQuote(id) => {
                st.stack = self.push(st.stack, Value::Quote(id));
                StepR::Next
            }
            Word::Call(idx) => match self.quotes.calls.get(idx as usize) {
                Some(name) => StepR::Invoke(name.clone()),
                None => {
                    debug_assert!(false, "call index out of bounds");
                    StepR::Fault(Fault::TypeMismatch)
                }
            },
            Word::Prim(p) => self.exec_prim(st, p),
        }
    }

    // ================================================================= reify
    // Reification is the generation boundary: it produces OWNED reference types
    // (`mtl_core::interp::*`), the shape the rest of the system consumes. A value
    // must be reified out before its generation is reset (design §3.2 / §11.4).

    /// Reify one arena value to the reference `interp::Value` (recursively
    /// resolving nested quote bodies).
    pub fn reify_value(&self, v: Value) -> itp::Value {
        match v {
            Value::Int(n) => itp::Value::Int(n),
            Value::Quote(id) => itp::Value::Quote(self.itp_quote(id)),
        }
    }

    /// The stack (bottom .. top) as reference `interp::Value`s. This is the winner
    /// / host-crossing / fault-snapshot stack materialization.
    pub fn reify_stack(&self, ptr: StackPtr) -> Vec<itp::Value> {
        self.stack_values(ptr).into_iter().map(|v| self.reify_value(v)).collect()
    }

    /// Reify the remaining continuation from `st` (head resumed at `st.cursor`,
    /// each ancestor at its frozen `off`) into an owned flat `Vec<interp::Word>` —
    /// identical in shape to `interp`'s `cont`. The next word to execute is the
    /// head of the result.
    pub fn reify_cont(&self, st: &VmState) -> Vec<itp::Word> {
        let mut out = Vec::new();
        let mut ptr = st.cont;
        // The live head segment resumes at `cursor`; every ancestor resumes at the
        // offset that was frozen into it when it became a parent.
        let mut off = st.cursor;
        while ptr != NIL_CONT {
            let Some(node) = self.cont.node_at(ptr) else {
                debug_assert!(false, "cont ptr out of bounds in reify_cont");
                break;
            };
            let seg_len = node.qend.saturating_sub(node.qstart);
            if off < seg_len {
                let from = node.qstart.saturating_add(off);
                for i in from..node.qend {
                    if let Some(w) = self.word_at(i) {
                        out.push(self.itp_word(w));
                    } else {
                        debug_assert!(false, "tape index out of bounds in reify_cont");
                    }
                }
            }
            ptr = node.parent;
            off = match self.cont.node_at(ptr) {
                Some(parent) => parent.off,
                None => 0,
            };
        }
        out
    }

    /// Construct the reference `interp::FaultInfo` at a terminal fault. `at` MUST
    /// be the pre-step state (the faulting word is `reify_cont(at)[0]`, exactly as
    /// `interp`'s `FaultInfo::cont[0]`); `run_arena` restores that state before
    /// reporting a fault.
    pub fn fault_info(&self, at: &VmState, fault: Fault) -> itp::FaultInfo {
        itp::FaultInfo {
            fault: crate::types::to_itp_fault(fault),
            stack: self.reify_stack(at.stack),
            cont: self.reify_cont(at),
        }
    }

    /// Reify one tape word to a reference `interp::Word` (tree form).
    pub(crate) fn itp_word(&self, w: Word) -> itp::Word {
        match w {
            Word::PushInt(n) => itp::Word::PushInt(n),
            Word::PushQuote(id) => itp::Word::PushQuote(self.itp_quote(id)),
            Word::Prim(p) => itp::Word::Prim(itp_prim(p)),
            Word::Call(idx) => match self.quotes.calls.get(idx as usize) {
                Some(name) => itp::Word::Call(name.clone()),
                None => {
                    debug_assert!(false, "call index out of bounds in reify");
                    itp::Word::Call(String::new())
                }
            },
        }
    }

    /// Reify a quote body to a reference `Vec<interp::Word>`.
    pub(crate) fn itp_quote(&self, id: QuoteId) -> Vec<itp::Word> {
        (id.start..id.end()).filter_map(|i| self.word_at(i)).map(|w| self.itp_word(w)).collect()
    }

    // ------------------------------------------------------- arena-typed reify
    // These keep the arena's own `Value`/`ProgWord` mirrors (used by the
    // differential oracle's conversion helpers). They do NOT cross the reference
    // boundary; `reify_stack`/`reify_cont`/`fault_info` above are for that.

    /// The stack (bottom .. top) as arena `Value`s.
    pub fn stack_values(&self, ptr: StackPtr) -> Vec<Value> {
        let mut out = Vec::new();
        let mut p = ptr;
        while p != EMPTY_STACK {
            let Some(node) = self.stack.node_at(p) else {
                debug_assert!(false, "stack ptr out of bounds in stack_values");
                break;
            };
            out.push(node.value);
            p = node.parent;
        }
        out.reverse();
        out
    }

    /// Reify a tape word back to a source `ProgWord` (arena-typed tree).
    pub fn reify_word(&self, w: Word) -> ProgWord {
        match w {
            Word::PushInt(n) => ProgWord::PushInt(n),
            Word::PushQuote(id) => ProgWord::PushQuote(self.reify_quote(id)),
            Word::Prim(p) => ProgWord::Prim(p),
            Word::Call(idx) => match self.quotes.calls.get(idx as usize) {
                Some(name) => ProgWord::Call(name.clone()),
                None => {
                    debug_assert!(false, "call index out of bounds in reify_word");
                    ProgWord::Call(String::new())
                }
            },
        }
    }

    /// Reify a quote body to an arena-typed `ProgWord` list.
    pub fn reify_quote(&self, id: QuoteId) -> Vec<ProgWord> {
        (id.start..id.end()).filter_map(|i| self.word_at(i)).map(|w| self.reify_word(w)).collect()
    }

    // ================================================= re-intern (host resume)
    // The inverse of reification: take OWNED reference data (an `interp::Value`
    // stack handed back by a host `Resume`) and intern it into the arena so the
    // driver can resume in place. This is the return leg of the host Invoke seam.
    // Cost is O(stack depth) plus O(total quote size) — a full copy IN, the
    // mirror of the copy OUT that `reify_stack` performs (see `crate::host`).

    /// Re-intern an owned reference stack (`bottom .. top`, e.g. a host `Resume`
    /// stack) into a FRESH arena stack segment, returning the new top
    /// [`StackPtr`]. Each `interp::Value` is interned into the arena (quote
    /// bodies recursively appended to the tape, call names de-duplicated). Returns
    /// `None` iff interning a quote body would overflow the u32 tape address space
    /// (design §3.4) — the driver turns that `None` into a clean
    /// [`Fault::Overflow`] rather than panicking.
    pub fn reintern_stack(&mut self, values: &[itp::Value]) -> Option<StackPtr> {
        let mut ptr = EMPTY_STACK;
        for v in values {
            let av = self.intern_itp_value(v)?;
            ptr = self.push(ptr, av);
        }
        Some(ptr)
    }

    /// Intern one owned `interp::Value` into an arena [`Value`]. `None` on u32
    /// tape overflow while interning a quote body.
    pub(crate) fn intern_itp_value(&mut self, v: &itp::Value) -> Option<Value> {
        match v {
            itp::Value::Int(n) => Some(Value::Int(*n)),
            itp::Value::Quote(body) => Some(Value::Quote(self.intern_itp_quote(body)?)),
        }
    }

    /// Intern an owned reference quote body into the tape, returning its
    /// [`QuoteId`]. Nested quotes are interned first (matching [`Vm::compile`]'s
    /// ordering). `None` on u32 tape overflow.
    pub(crate) fn intern_itp_quote(&mut self, body: &[itp::Word]) -> Option<QuoteId> {
        let mut words = Vec::with_capacity(body.len());
        for w in body {
            words.push(self.intern_itp_word(w)?);
        }
        self.try_alloc(&words)
    }

    /// Intern one owned reference word into a tape [`Word`]. `None` on u32 tape
    /// overflow while interning a nested quote body.
    pub(crate) fn intern_itp_word(&mut self, w: &itp::Word) -> Option<Word> {
        Some(match w {
            itp::Word::PushInt(n) => Word::PushInt(*n),
            itp::Word::PushQuote(body) => Word::PushQuote(self.intern_itp_quote(body)?),
            itp::Word::Prim(p) => Word::Prim(from_itp_prim(*p)),
            itp::Word::Call(name) => Word::Call(self.quotes.intern_call(name)),
        })
    }

    // ========================================================== generational
    /// Record the current high-water mark of every arena. Pair with
    /// [`Vm::reset_to`] to reclaim everything allocated after this point. See the
    /// [`crate::arena`] module docs for the reification-before-reset invariant.
    pub fn mark(&self) -> Mark {
        Mark {
            tape: self.quotes.tape.len(),
            calls: self.quotes.calls.len(),
            stack_nodes: self.stack.nodes.len(),
            cont_nodes: self.cont.nodes.len(),
        }
    }

    /// Truncate every arena back to `mark`, reclaiming a whole generation in O(1)
    /// amortized (bump/reset — no per-object free).
    ///
    /// SAFETY-BY-CONSTRUCTION: this invalidates every `QuoteId`/`StackPtr`/
    /// `ContPtr`/call-index at or above `mark`. Any handle allocated within the
    /// generation must have been reified to owned data *before* this call; using a
    /// stale handle afterwards is a use-after-reset bug the caller is responsible
    /// for avoiding. `debug_assert` guards that the mark is not in the future
    /// (i.e. the arenas did not shrink below it).
    pub fn reset_to(&mut self, mark: Mark) {
        debug_assert!(mark.tape <= self.quotes.tape.len(), "mark.tape is in the future");
        debug_assert!(mark.calls <= self.quotes.calls.len(), "mark.calls is in the future");
        debug_assert!(
            mark.stack_nodes <= self.stack.nodes.len(),
            "mark.stack_nodes is in the future"
        );
        debug_assert!(
            mark.cont_nodes <= self.cont.nodes.len(),
            "mark.cont_nodes is in the future"
        );
        self.quotes.tape.truncate(mark.tape);
        self.quotes.calls.truncate(mark.calls);
        self.stack.nodes.truncate(mark.stack_nodes.max(1)); // keep the sentinel
        self.cont.nodes.truncate(mark.cont_nodes.max(1)); // keep the NIL node
    }
}

/// Map a reference `interp::Prim` back to an arena `Prim` (identical variants).
/// The inverse of [`itp_prim`]; used by the host-resume re-intern path.
#[inline]
fn from_itp_prim(p: itp::Prim) -> Prim {
    match p {
        itp::Prim::Dup => Prim::Dup,
        itp::Prim::Drop => Prim::Drop,
        itp::Prim::Swap => Prim::Swap,
        itp::Prim::Rot => Prim::Rot,
        itp::Prim::Over => Prim::Over,
        itp::Prim::Apply => Prim::Apply,
        itp::Prim::Cat => Prim::Cat,
        itp::Prim::Cons => Prim::Cons,
        itp::Prim::Dip => Prim::Dip,
        itp::Prim::Add => Prim::Add,
        itp::Prim::Sub => Prim::Sub,
        itp::Prim::Mul => Prim::Mul,
        itp::Prim::Div => Prim::Div,
        itp::Prim::Mod => Prim::Mod,
        itp::Prim::Eq => Prim::Eq,
        itp::Prim::Lt => Prim::Lt,
        itp::Prim::If => Prim::If,
        itp::Prim::PrimRec => Prim::PrimRec,
        itp::Prim::Times => Prim::Times,
        itp::Prim::LinRec => Prim::LinRec,
        itp::Prim::Uncons => Prim::Uncons,
        itp::Prim::Fold => Prim::Fold,
        itp::Prim::Xor => Prim::Xor,
    }
}

/// Map an arena `Prim` to the reference `interp::Prim` (identical variants).
#[inline]
fn itp_prim(p: Prim) -> itp::Prim {
    match p {
        Prim::Dup => itp::Prim::Dup,
        Prim::Drop => itp::Prim::Drop,
        Prim::Swap => itp::Prim::Swap,
        Prim::Rot => itp::Prim::Rot,
        Prim::Over => itp::Prim::Over,
        Prim::Apply => itp::Prim::Apply,
        Prim::Cat => itp::Prim::Cat,
        Prim::Cons => itp::Prim::Cons,
        Prim::Dip => itp::Prim::Dip,
        Prim::Add => itp::Prim::Add,
        Prim::Sub => itp::Prim::Sub,
        Prim::Mul => itp::Prim::Mul,
        Prim::Div => itp::Prim::Div,
        Prim::Mod => itp::Prim::Mod,
        Prim::Eq => itp::Prim::Eq,
        Prim::Lt => itp::Prim::Lt,
        Prim::If => itp::Prim::If,
        Prim::PrimRec => itp::Prim::PrimRec,
        Prim::Times => itp::Prim::Times,
        Prim::LinRec => itp::Prim::LinRec,
        Prim::Uncons => itp::Prim::Uncons,
        Prim::Fold => itp::Prim::Fold,
        Prim::Xor => itp::Prim::Xor,
    }
}
