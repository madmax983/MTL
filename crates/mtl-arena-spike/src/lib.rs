//! # mtl-arena-spike — SPIKE / NON-PRODUCTION
//!
//! **SPIKE / NON-PRODUCTION.** Measurement vehicle for the v0.5 arena-backend
//! design (`docs/design/v0.5-refactor.md`). **Not pinned to the frozen semantics
//! as a proof obligation**; validated only by a differential oracle vs the
//! reference interpreter (`crates/mtl-core` `interp.rs`). The reference
//! interpreter remains the twin / oracle of truth; this crate exists purely to
//! *measure* whether the arena continuation representation kills the O(n²)
//! pathologies documented in `crates/mtl-perf/PERF-BASELINE.md`.
//!
//! ## What this implements (the canonical v0.5 arena design)
//!
//! Frozen semantics; `interp.rs` is the reference twin. All hot types are `Copy`.
//!
//! * [`QuoteArena`] — a single `tape: Vec<Word>` interning every quote body.
//!   [`QuoteId`] is a `{start,len}` slice into the tape. Quote bodies are shared
//!   structurally; sub-slicing a list tail (`{start+1, len-1}`) is O(1).
//! * [`Value`] = `Copy` enum `{ Int(i64), Quote(QuoteId) }`.
//! * [`StackArena`] — persistent, structurally shared cons-list of stack nodes;
//!   index 0 is the empty-stack sentinel. Push allocs a node; pop follows parent.
//! * **Continuation = persistent segment cons-list + local cursor.** This is the
//!   fix for the measured O(n²). Each [`ContNode`] means "run `tape[qstart..qend]`
//!   resuming at offset `off`". Reading the next word is a cursor bump (O(1));
//!   prepending a quote freezes the current head (capturing its resume offset)
//!   and pushes a child segment (≤2 node allocs, O(1), **no tail copy**), with
//!   full structural sharing of quote bodies. [`VmState`] is three `u32`s
//!   (`stack`, `cont`, `cursor`) — 12 bytes, `Copy` → fork is a 12-byte copy.
//!
//! Why this kills each measured pathology:
//! * **flat program** — per-step cursor bump replaces `cont.remove(0)` front-pop
//!   (kills the 414× ns/step degradation).
//! * **PrimRec** — re-emitting the combinator body is a single prepend of a
//!   fresh interned segment (O(1)/level), not a `|C|`-per-level tail copy (kills
//!   the `sum_to` O(n²) / 223 ms case).
//! * **Fold** — the shrinking list "tail" becomes `QuoteId{start+1, len-1}`, a
//!   shared sub-slice (O(1)), not a deep spine clone (kills Fold O(n²)).
//! * **`: !`** — already tail-linear; stays fine.
//!
//! The fault-check order mirrors `interp.rs` exactly: arity (`Underflow`) before
//! type (`TypeMismatch`); `DivByZero` before `Overflow`. Faults are terminal.

// ----------------------------------------------------------------- input AST
// A dependency-free mirror of the exec AST (interp.rs `Prim`/`Word`/`Value`), so
// the spike is self-contained. Programs are supplied as a `ProgWord` tree and
// compiled (interned) into the arena tape by `run_arena` / `Vm::compile`.

/// The primitive set. Mirrors `mtl_core::interp::Prim`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    Dup,
    Drop,
    Swap,
    Rot,
    Over,
    Apply,
    Cat,
    Cons,
    Dip,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Lt,
    If,
    PrimRec,
    Times,
    LinRec,
    Uncons,
    Fold,
    Xor,
}

/// A source program word (tree form). Mirrors `mtl_core::interp::Word`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgWord {
    PushInt(i64),
    PushQuote(Vec<ProgWord>),
    Prim(Prim),
    Call(String),
}

/// A runtime fault kind. Mirrors `mtl_core::interp::Fault`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Underflow,
    TypeMismatch,
    Overflow,
    DivByZero,
}

// -------------------------------------------------------------- arena Word/ids

/// An interned program word living in the [`QuoteArena`] tape. `Copy` and small.
/// Nested quotes are referenced by [`QuoteId`]; call names are interned to a
/// `u32` index into `QuoteArena::calls`.
#[derive(Clone, Copy, Debug)]
pub enum Word {
    PushInt(i64),
    PushQuote(QuoteId),
    Prim(Prim),
    Call(u32),
}

/// A quote body: a contiguous `[start, start+len)` slice of the tape. `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteId {
    pub start: u32,
    pub len: u32,
}

impl QuoteId {
    #[inline]
    fn end(self) -> u32 {
        self.start + self.len
    }
}

/// A first-class value. `Copy`. Mirrors `mtl_core::interp::Value`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Quote(QuoteId),
}

// ------------------------------------------------------------------ QuoteArena

/// Interns all quote bodies in one flat `tape`. Call names are interned to
/// `calls`. `alloc` extends the tape; list tails are O(1) sub-slices.
#[derive(Clone, Debug, Default)]
pub struct QuoteArena {
    pub tape: Vec<Word>,
    calls: Vec<String>,
}

impl QuoteArena {
    fn new() -> Self {
        Self { tape: Vec::new(), calls: Vec::new() }
    }

    /// Extend the tape with `ws`, returning the slice id.
    #[inline]
    fn alloc(&mut self, ws: &[Word]) -> QuoteId {
        let start = self.tape.len() as u32;
        self.tape.extend_from_slice(ws);
        QuoteId { start, len: ws.len() as u32 }
    }

    /// `cat(a,b)`: append copies of both bodies (extend_from_within), returning
    /// the fresh contiguous id. O(|a|+|b|) — a genuine value construction.
    #[inline]
    fn cat(&mut self, a: QuoteId, b: QuoteId) -> QuoteId {
        let start = self.tape.len() as u32;
        self.tape.extend_from_within(a.start as usize..a.end() as usize);
        self.tape.extend_from_within(b.start as usize..b.end() as usize);
        QuoteId { start, len: a.len + b.len }
    }

    /// `cons(v, q)`: prepend one word to a copy of `q`'s body. O(|q|).
    #[inline]
    fn cons(&mut self, head: Word, q: QuoteId) -> QuoteId {
        let start = self.tape.len() as u32;
        self.tape.push(head);
        self.tape.extend_from_within(q.start as usize..q.end() as usize);
        QuoteId { start, len: q.len + 1 }
    }

    #[inline]
    fn intern_call(&mut self, name: &str) -> u32 {
        if let Some(i) = self.calls.iter().position(|c| c == name) {
            i as u32
        } else {
            let i = self.calls.len() as u32;
            self.calls.push(name.to_string());
            i
        }
    }
}

// ------------------------------------------------------------------ StackArena

/// Persistent, structurally shared stack. `nodes[0]` is the empty-stack
/// sentinel; a [`StackPtr`] of 0 means "empty".
pub type StackPtr = u32;
const EMPTY_STACK: StackPtr = 0;

#[derive(Clone, Copy, Debug)]
struct StackNode {
    value: Value,
    parent: StackPtr,
}

#[derive(Clone, Debug)]
pub struct StackArena {
    nodes: Vec<StackNode>,
}

impl StackArena {
    fn new() -> Self {
        // index 0 = sentinel (value unused).
        Self { nodes: vec![StackNode { value: Value::Int(0), parent: 0 }] }
    }

    #[inline]
    fn push(&mut self, parent: StackPtr, value: Value) -> StackPtr {
        let idx = self.nodes.len() as u32;
        self.nodes.push(StackNode { value, parent });
        idx
    }
}

// ------------------------------------------------------------------- ContArena

/// A continuation segment: "run `tape[qstart..qend]`, resuming at relative
/// offset `off`". `off` is relative to `qstart` (0 = start of the segment).
/// `nodes[0]` is NIL.
#[derive(Clone, Copy, Debug)]
struct ContNode {
    qstart: u32,
    qend: u32,
    off: u32,
    parent: ContPtr,
}

/// A pointer into the [`ContArena`]; 0 = NIL (empty continuation → halt).
pub type ContPtr = u32;
const NIL_CONT: ContPtr = 0;

#[derive(Clone, Debug)]
pub struct ContArena {
    nodes: Vec<ContNode>,
}

impl ContArena {
    fn new() -> Self {
        Self { nodes: vec![ContNode { qstart: 0, qend: 0, off: 0, parent: 0 }] }
    }

    #[inline]
    fn push(&mut self, n: ContNode) -> ContPtr {
        let idx = self.nodes.len() as u32;
        self.nodes.push(n);
        idx
    }
}

// --------------------------------------------------------------------- VmState

/// The entire mutable machine position: three `u32`s = 12 bytes, `Copy`.
/// **Fork = copy this struct, O(1)** — the whole point of the arena design.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmState {
    pub stack: StackPtr,
    pub cont: ContPtr,
    pub cursor: u32,
}

impl VmState {
    #[inline]
    pub fn initial() -> Self {
        VmState { stack: EMPTY_STACK, cont: NIL_CONT, cursor: 0 }
    }
}

// -------------------------------------------------------------------------- Vm

/// The arena VM: the three arenas. `VmState` is threaded through explicitly so
/// forking is a plain `Copy`.
#[derive(Clone, Debug)]
pub struct Vm {
    pub quotes: QuoteArena,
    stack: StackArena,
    cont: ContArena,
}

/// Internal per-word step result (mirrors interp `Step`).
enum StepR {
    Next,
    Fault(Fault),
    Invoke(String),
}

/// Terminal kind of a driven arena run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArenaEnd {
    Halt,
    Fault(Fault),
    FuelExhausted,
    Invoke(String),
}

/// Full result of [`run_arena`]: the arena (for reifying the final stack), the
/// terminal kind, and the final stack pointer.
#[derive(Clone, Debug)]
pub struct ArenaRun {
    pub vm: Vm,
    pub end: ArenaEnd,
    pub stack: StackPtr,
    /// Executed words (segment pops are free) — same counting as `interp::run`.
    pub steps: u64,
}

impl Vm {
    pub fn new() -> Self {
        Self { quotes: QuoteArena::new(), stack: StackArena::new(), cont: ContArena::new() }
    }

    // --------------------------------------------------------------- compile
    /// Intern a source program tree into the tape, returning its body id.
    pub fn compile(&mut self, prog: &[ProgWord]) -> QuoteId {
        let mut words = Vec::with_capacity(prog.len());
        for pw in prog {
            let w = match pw {
                ProgWord::PushInt(n) => Word::PushInt(*n),
                ProgWord::PushQuote(body) => {
                    let id = self.compile(body);
                    Word::PushQuote(id)
                }
                ProgWord::Prim(p) => Word::Prim(*p),
                ProgWord::Call(name) => Word::Call(self.quotes.intern_call(name)),
            };
            words.push(w);
        }
        self.quotes.alloc(&words)
    }

    // ------------------------------------------------------ continuation ops
    /// Read (and consume) the next word, popping exhausted segments. Returns
    /// `None` at NIL (halt). Popping segments costs no fuel — only executed
    /// words are steps, exactly like `interp::run`.
    #[inline]
    fn next_word(&self, st: &mut VmState) -> Option<Word> {
        loop {
            if st.cont == NIL_CONT {
                return None;
            }
            let node = self.cont.nodes[st.cont as usize];
            let len = node.qend - node.qstart;
            if st.cursor < len {
                let w = self.quotes.tape[(node.qstart + st.cursor) as usize];
                st.cursor += 1;
                return Some(w);
            }
            // Segment exhausted: POP to parent, resume at its frozen offset.
            st.cont = node.parent;
            if st.cont == NIL_CONT {
                return None;
            }
            st.cursor = self.cont.nodes[st.cont as usize].off;
        }
    }

    /// Prepend quote `q` to the continuation: `cont := q ++ cont`. Freezes the
    /// current head (capturing `cursor` as its resume offset) and pushes a child
    /// segment. ≤2 node allocs, O(1), no tail copy. Empty `q` is a no-op (mirror
    /// of interp `prepend`'s empty-prefix early return).
    #[inline]
    fn prepend(&mut self, st: &mut VmState, q: QuoteId) {
        if q.len == 0 {
            return;
        }
        let child = if st.cont == NIL_CONT {
            self.cont.push(ContNode { qstart: q.start, qend: q.end(), off: 0, parent: NIL_CONT })
        } else {
            let h = self.cont.nodes[st.cont as usize];
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

    // --------------------------------------------------------------- stack ops
    /// `k`-th value from the top (0 = top). `None` if fewer than `k+1` present.
    #[inline]
    fn peek(&self, ptr: StackPtr, k: usize) -> Option<Value> {
        let mut p = ptr;
        for _ in 0..k {
            if p == 0 {
                return None;
            }
            p = self.stack.nodes[p as usize].parent;
        }
        if p == 0 {
            None
        } else {
            Some(self.stack.nodes[p as usize].value)
        }
    }

    /// True iff at least `k` values are on the stack.
    #[inline]
    fn has(&self, ptr: StackPtr, k: usize) -> bool {
        k == 0 || self.peek(ptr, k - 1).is_some()
    }

    #[inline]
    fn pop(&self, ptr: StackPtr) -> (Value, StackPtr) {
        let node = self.stack.nodes[ptr as usize];
        (node.value, node.parent)
    }

    #[inline]
    fn push(&mut self, ptr: StackPtr, v: Value) -> StackPtr {
        self.stack.push(ptr, v)
    }

    // ---------------------------------------------------------------- reify
    /// Reify a tape word back to a source `ProgWord` (resolving nested quotes
    /// and call names). Test-only oracle helper.
    pub fn reify_word(&self, w: Word) -> ProgWord {
        match w {
            Word::PushInt(n) => ProgWord::PushInt(n),
            Word::PushQuote(id) => ProgWord::PushQuote(self.reify_quote(id)),
            Word::Prim(p) => ProgWord::Prim(p),
            Word::Call(idx) => ProgWord::Call(self.quotes.calls[idx as usize].clone()),
        }
    }

    /// Reify a quote body to a `ProgWord` list.
    pub fn reify_quote(&self, id: QuoteId) -> Vec<ProgWord> {
        (id.start..id.end())
            .map(|i| self.reify_word(self.quotes.tape[i as usize]))
            .collect()
    }

    /// The final stack (bottom .. top) as `Value`s. Test-only oracle helper.
    pub fn stack_values(&self, ptr: StackPtr) -> Vec<Value> {
        let mut out = Vec::new();
        let mut p = ptr;
        while p != 0 {
            let node = self.stack.nodes[p as usize];
            out.push(node.value);
            p = node.parent;
        }
        out.reverse();
        out
    }

    // ------------------------------------------------------------ exec a word
    #[inline]
    fn value_to_word(v: Value) -> Word {
        match v {
            Value::Int(i) => Word::PushInt(i),
            Value::Quote(id) => Word::PushQuote(id),
        }
    }

    fn exec_word(&mut self, st: &mut VmState, w: Word) -> StepR {
        match w {
            Word::PushInt(n) => {
                st.stack = self.push(st.stack, Value::Int(n));
                StepR::Next
            }
            Word::PushQuote(id) => {
                st.stack = self.push(st.stack, Value::Quote(id));
                StepR::Next
            }
            Word::Call(idx) => StepR::Invoke(self.quotes.calls[idx as usize].clone()),
            Word::Prim(p) => self.exec_prim(st, p),
        }
    }

    fn exec_prim(&mut self, st: &mut VmState, p: Prim) -> StepR {
        match p {
            // ------------------------------------------ stack shuffling
            Prim::Dup => {
                if !self.has(st.stack, 1) {
                    return StepR::Fault(Fault::Underflow);
                }
                let top = self.peek(st.stack, 0).unwrap();
                st.stack = self.push(st.stack, top);
                StepR::Next
            }
            Prim::Drop => {
                if !self.has(st.stack, 1) {
                    return StepR::Fault(Fault::Underflow);
                }
                let (_, p0) = self.pop(st.stack);
                st.stack = p0;
                StepR::Next
            }
            Prim::Swap => {
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                let (b, p1) = self.pop(st.stack);
                let (a, p2) = self.pop(p1);
                let s = self.push(p2, b);
                st.stack = self.push(s, a);
                StepR::Next
            }
            Prim::Rot => {
                // ( a b c -- b c a )
                if !self.has(st.stack, 3) {
                    return StepR::Fault(Fault::Underflow);
                }
                let (c, p1) = self.pop(st.stack);
                let (b, p2) = self.pop(p1);
                let (a, p3) = self.pop(p2);
                let s = self.push(p3, b);
                let s = self.push(s, c);
                st.stack = self.push(s, a);
                StepR::Next
            }
            Prim::Over => {
                // ( a b -- a b a )
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                let a = self.peek(st.stack, 1).unwrap();
                st.stack = self.push(st.stack, a);
                StepR::Next
            }
            // ------------------------------------------ quotation algebra
            Prim::Apply => {
                if !self.has(st.stack, 1) {
                    return StepR::Fault(Fault::Underflow);
                }
                match self.peek(st.stack, 0).unwrap() {
                    Value::Quote(q) => {
                        let (_, p0) = self.pop(st.stack);
                        st.stack = p0;
                        self.prepend(st, q);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cat => {
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
                    (Value::Quote(a), Value::Quote(b)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        let id = self.quotes.cat(a, b);
                        st.stack = self.push(p2, Value::Quote(id));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Cons => {
                // ( v [q] -- [v q] )
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                match self.peek(st.stack, 0).unwrap() {
                    Value::Quote(q) => {
                        let (_, p1) = self.pop(st.stack);
                        let (v, p2) = self.pop(p1);
                        let id = self.quotes.cons(Self::value_to_word(v), q);
                        st.stack = self.push(p2, Value::Quote(id));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Dip => {
                // ( a [q] -- ... a ) : cont := q ++ [Push(a)] ++ rest
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                match self.peek(st.stack, 0).unwrap() {
                    Value::Quote(q) => {
                        let (_, p1) = self.pop(st.stack);
                        let (a, p2) = self.pop(p1);
                        st.stack = p2;
                        let seg = self.quotes.alloc(&[Self::value_to_word(a)]);
                        self.prepend(st, seg);
                        self.prepend(st, q);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ arithmetic
            Prim::Add => self.arith(st, |a, b| a.checked_add(b)),
            Prim::Sub => self.arith(st, |a, b| a.checked_sub(b)),
            Prim::Mul => self.arith(st, |a, b| a.checked_mul(b)),
            Prim::Div => self.divmod(st, true),
            Prim::Mod => self.divmod(st, false),
            // ------------------------------------------ comparison / xor
            Prim::Eq => self.cmp(st, |a, b| a == b),
            Prim::Lt => self.cmp(st, |a, b| a < b),
            Prim::Xor => {
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
                    (Value::Int(a), Value::Int(b)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        st.stack = self.push(p2, Value::Int(a ^ b));
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ branch
            Prim::If => {
                if !self.has(st.stack, 3) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (
                    self.peek(st.stack, 2).unwrap(),
                    self.peek(st.stack, 1).unwrap(),
                    self.peek(st.stack, 0).unwrap(),
                ) {
                    (Value::Int(c), Value::Quote(t), Value::Quote(f)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        let (_, p3) = self.pop(p2);
                        st.stack = p3;
                        let branch = if c != 0 { t } else { f };
                        self.prepend(st, branch);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            // ------------------------------------------ v0.2 recursion
            Prim::PrimRec => {
                // ( n [I] [C] -- r )
                if !self.has(st.stack, 3) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (
                    self.peek(st.stack, 2).unwrap(),
                    self.peek(st.stack, 1).unwrap(),
                    self.peek(st.stack, 0).unwrap(),
                ) {
                    (Value::Int(k), Value::Quote(qi), Value::Quote(qc)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        let (_, p3) = self.pop(p2);
                        st.stack = p3;
                        if k <= 0 {
                            self.prepend(st, qi);
                        } else {
                            // cont := [k, k-1, [qi], [qc], primrec] ++ qc ++ rest
                            // Prepend qc by reference (no |C| copy), then the
                            // tiny setup segment — O(1)/level.
                            let setup = self.quotes.alloc(&[
                                Word::PushInt(k),
                                Word::PushInt(k - 1),
                                Word::PushQuote(qi),
                                Word::PushQuote(qc),
                                Word::Prim(Prim::PrimRec),
                            ]);
                            self.prepend(st, qc);
                            self.prepend(st, setup);
                        }
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Times => {
                // ( n [Q] -- ... )
                if !self.has(st.stack, 2) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
                    (Value::Int(k), Value::Quote(q)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        st.stack = p2;
                        if k > 0 {
                            // cont := q ++ [k-1, [q], times] ++ rest
                            let setup = self.quotes.alloc(&[
                                Word::PushInt(k - 1),
                                Word::PushQuote(q),
                                Word::Prim(Prim::Times),
                            ]);
                            self.prepend(st, setup);
                            self.prepend(st, q);
                        }
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::LinRec => {
                // ( [P] [T] [R1] [R2] -- ... ) — desugars into If.
                if !self.has(st.stack, 4) {
                    return StepR::Fault(Fault::Underflow);
                }
                match (
                    self.peek(st.stack, 3).unwrap(),
                    self.peek(st.stack, 2).unwrap(),
                    self.peek(st.stack, 1).unwrap(),
                    self.peek(st.stack, 0).unwrap(),
                ) {
                    (Value::Quote(qp), Value::Quote(qt), Value::Quote(qr1), Value::Quote(qr2)) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        let (_, p3) = self.pop(p2);
                        let (_, p4) = self.pop(p3);
                        st.stack = p4;
                        // else_q := R1 ++ [[P],[T],[R1],[R2],linrec] ++ R2
                        // Materialised as a value quote (referenced inside If).
                        let start = self.quotes.tape.len() as u32;
                        self.quotes
                            .tape
                            .extend_from_within(qr1.start as usize..qr1.end() as usize);
                        self.quotes.tape.push(Word::PushQuote(qp));
                        self.quotes.tape.push(Word::PushQuote(qt));
                        self.quotes.tape.push(Word::PushQuote(qr1));
                        self.quotes.tape.push(Word::PushQuote(qr2));
                        self.quotes.tape.push(Word::Prim(Prim::LinRec));
                        self.quotes
                            .tape
                            .extend_from_within(qr2.start as usize..qr2.end() as usize);
                        let else_q =
                            QuoteId { start, len: qr1.len + 5 + qr2.len };
                        // spliced := P ++ [[T], [else_q], If] ++ rest
                        let seg = self.quotes.alloc(&[
                            Word::PushQuote(qt),
                            Word::PushQuote(else_q),
                            Word::Prim(Prim::If),
                        ]);
                        self.prepend(st, seg);
                        self.prepend(st, qp);
                        StepR::Next
                    }
                    _ => StepR::Fault(Fault::TypeMismatch),
                }
            }
            Prim::Uncons => {
                // ( [w ...] -- w [...] 1 ) | ( [] -- 0 )
                if !self.has(st.stack, 1) {
                    return StepR::Fault(Fault::Underflow);
                }
                let q = match self.peek(st.stack, 0).unwrap() {
                    Value::Quote(q) => q,
                    _ => return StepR::Fault(Fault::TypeMismatch),
                };
                // Inspect head without consuming: bare Prim/Call head faults.
                if q.len > 0 {
                    match self.quotes.tape[q.start as usize] {
                        Word::PushInt(_) | Word::PushQuote(_) => {}
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    }
                }
                let (_, p0) = self.pop(st.stack);
                st.stack = p0;
                if q.len == 0 {
                    st.stack = self.push(st.stack, Value::Int(0));
                } else {
                    let head_val = match self.quotes.tape[q.start as usize] {
                        Word::PushInt(k) => Value::Int(k),
                        Word::PushQuote(id) => Value::Quote(id),
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    };
                    let tail = QuoteId { start: q.start + 1, len: q.len - 1 };
                    st.stack = self.push(st.stack, head_val);
                    st.stack = self.push(st.stack, Value::Quote(tail));
                    st.stack = self.push(st.stack, Value::Int(1));
                }
                StepR::Next
            }
            // ------------------------------------------ v0.3 sequence
            Prim::Fold => {
                // ( [seq] init [C] -- r ) LEFT fold.
                if !self.has(st.stack, 3) {
                    return StepR::Fault(Fault::Underflow);
                }
                let seq = self.peek(st.stack, 2).unwrap();
                let combine = self.peek(st.stack, 0).unwrap();
                let (qs, qc) = match (seq, combine) {
                    (Value::Quote(qs), Value::Quote(qc)) => (qs, qc),
                    _ => return StepR::Fault(Fault::TypeMismatch),
                };
                // Inspect seq head without consuming.
                if qs.len > 0 {
                    match self.quotes.tape[qs.start as usize] {
                        Word::PushInt(_) | Word::PushQuote(_) => {}
                        _ => return StepR::Fault(Fault::TypeMismatch),
                    }
                }
                let (_, p1) = self.pop(st.stack);
                let (init, p2) = self.pop(p1);
                let (_, p3) = self.pop(p2);
                st.stack = p3;
                if qs.len == 0 {
                    st.stack = self.push(st.stack, init);
                } else {
                    // cont := [PushQuote(tail), init_word, head] ++ qc
                    //         ++ [PushQuote(qc), Fold] ++ rest
                    let head = self.quotes.tape[qs.start as usize];
                    let tail = QuoteId { start: qs.start + 1, len: qs.len - 1 };
                    let seg_c =
                        self.quotes.alloc(&[Word::PushQuote(qc), Word::Prim(Prim::Fold)]);
                    let seg_a = self.quotes.alloc(&[
                        Word::PushQuote(tail),
                        Self::value_to_word(init),
                        head,
                    ]);
                    self.prepend(st, seg_c);
                    self.prepend(st, qc);
                    self.prepend(st, seg_a);
                }
                StepR::Next
            }
        }
    }

    #[inline]
    fn arith(&mut self, st: &mut VmState, op: fn(i64, i64) -> Option<i64>) -> StepR {
        if !self.has(st.stack, 2) {
            return StepR::Fault(Fault::Underflow);
        }
        match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
            (Value::Int(a), Value::Int(b)) => match op(a, b) {
                Some(r) => {
                    let (_, p1) = self.pop(st.stack);
                    let (_, p2) = self.pop(p1);
                    st.stack = self.push(p2, Value::Int(r));
                    StepR::Next
                }
                None => StepR::Fault(Fault::Overflow),
            },
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }

    #[inline]
    fn divmod(&mut self, st: &mut VmState, is_div: bool) -> StepR {
        if !self.has(st.stack, 2) {
            return StepR::Fault(Fault::Underflow);
        }
        match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
            (Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return StepR::Fault(Fault::DivByZero);
                }
                let res = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
                match res {
                    Some(r) => {
                        let (_, p1) = self.pop(st.stack);
                        let (_, p2) = self.pop(p1);
                        st.stack = self.push(p2, Value::Int(r));
                        StepR::Next
                    }
                    None => StepR::Fault(Fault::Overflow),
                }
            }
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }

    #[inline]
    fn cmp(&mut self, st: &mut VmState, op: fn(i64, i64) -> bool) -> StepR {
        if !self.has(st.stack, 2) {
            return StepR::Fault(Fault::Underflow);
        }
        match (self.peek(st.stack, 1).unwrap(), self.peek(st.stack, 0).unwrap()) {
            (Value::Int(a), Value::Int(b)) => {
                let r = if op(a, b) { 1 } else { 0 };
                let (_, p1) = self.pop(st.stack);
                let (_, p2) = self.pop(p1);
                st.stack = self.push(p2, Value::Int(r));
                StepR::Next
            }
            _ => StepR::Fault(Fault::TypeMismatch),
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuel-bounded arena driver. Mirrors `interp::run`: `fuel` counts executed
/// words (segment pops are free), and every fault is terminal. Returns the arena
/// (for stack reification), the terminal kind, and the final stack pointer.
pub fn run_arena(prog: &[ProgWord], fuel: u64) -> ArenaRun {
    let mut vm = Vm::new();
    let pid = vm.compile(prog);
    let mut st = VmState::initial();
    vm.prepend(&mut st, pid);
    let mut steps: u64 = 0;
    loop {
        if steps >= fuel {
            return ArenaRun { vm, end: ArenaEnd::FuelExhausted, stack: st.stack, steps };
        }
        let w = match vm.next_word(&mut st) {
            Some(w) => w,
            None => return ArenaRun { vm, end: ArenaEnd::Halt, stack: st.stack, steps },
        };
        match vm.exec_word(&mut st, w) {
            StepR::Next => steps += 1,
            StepR::Fault(f) => {
                return ArenaRun { vm, end: ArenaEnd::Fault(f), stack: st.stack, steps }
            }
            StepR::Invoke(name) => {
                return ArenaRun { vm, end: ArenaEnd::Invoke(name), stack: st.stack, steps }
            }
        }
    }
}

// -------------------------------------------------------- fork-cost helper
/// Build a depth-`d` persistent arena stack (each level a distinct Int) and
/// return the `Vm` plus the initial `VmState` positioned on top of it. Used by
/// the fork-cost microbenchmark to show that forking is a 12-byte `Copy`
/// independent of stack depth.
pub fn build_stack(depth: usize) -> (Vm, VmState) {
    let mut vm = Vm::new();
    let mut st = VmState::initial();
    for i in 0..depth {
        st.stack = vm.push(st.stack, Value::Int(i as i64));
    }
    // A representative live continuation: intern a small quote and load it.
    let q = vm.compile(&[ProgWord::PushInt(1), ProgWord::Prim(Prim::Add)]);
    vm.prepend(&mut st, q);
    (vm, st)
}

// ============================================================================
// v0.5 SPECULATION-ADMISSION EXPERIMENT — SPIKE / NON-PRODUCTION PROTOTYPE
// ============================================================================
//
// **NON-PRODUCTION PROTOTYPE.** Everything below this line is an *additive*
// prototype built for the v0.5 speculation-admission experiment
// (`docs/design/v0.5-refactor.md` §4). It touches NO semantics: it only wraps
// the existing private `next_word`/`exec_word`/`prepend` step machinery in a
// single-step public entry (`Vm::step`) and layers a host-side speculation
// driver (`mod spec`) *over* cloned `VmState`s, exactly as §4.2 sketches. The
// core computes the same frozen semantics; the driver is untrusted scheduling
// code (the "multiverse lives in untrusted scheduling"), and the differential
// oracle (`tests/oracle.rs`) remains the source of truth. Not proved; validated
// only differentially against `mtl_core::interp`.

/// Public single-step outcome. NON-PRODUCTION prototype (see module banner).
/// Wraps the private per-word [`StepR`] plus the "continuation empty" halt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    /// A word executed; the machine is still live. Consumes one fuel unit.
    Next,
    /// The continuation emptied — terminal Done. Consumes no fuel (mirrors
    /// `run_arena`, where only executed words count as steps).
    Halt,
    /// A fault fired; terminal. Carries the fault kind.
    Fault(Fault),
    /// A `Call(name)` yielded to the host; terminal for a pure run. Carries the
    /// capability name. Speculation buffers/defers these (see [`spec`]).
    Invoke(String),
}

impl Vm {
    /// **NON-PRODUCTION.** Single-step public entry: read the next word (popping
    /// exhausted segments, which is free) and execute it. This is the minimal
    /// `pub` seam the [`spec`] driver steps branches through — it wraps the
    /// private `next_word` + `exec_word` and introduces no new behaviour.
    #[inline]
    pub fn step(&mut self, st: &mut VmState) -> StepOutcome {
        match self.next_word(st) {
            None => StepOutcome::Halt,
            Some(w) => match self.exec_word(st, w) {
                StepR::Next => StepOutcome::Next,
                StepR::Fault(f) => StepOutcome::Fault(f),
                StepR::Invoke(name) => StepOutcome::Invoke(name),
            },
        }
    }

    /// **NON-PRODUCTION.** Compile `prog` into this Vm's tape and return a fresh
    /// `VmState` positioned to run it (program prepended onto an empty machine).
    /// This is how the experiment turns each candidate program into a runnable
    /// branch. Interning is append-only, so many candidates share one tape.
    pub fn load(&mut self, prog: &[ProgWord]) -> VmState {
        let pid = self.compile(prog);
        let mut st = VmState::initial();
        self.prepend(&mut st, pid);
        st
    }

    /// **NON-PRODUCTION.** Arena high-water accessor: interned tape length.
    pub fn tape_len(&self) -> usize {
        self.quotes.tape.len()
    }
    /// **NON-PRODUCTION.** Arena high-water accessor: stack-node count.
    pub fn stack_nodes_len(&self) -> usize {
        self.stack.nodes.len()
    }
    /// **NON-PRODUCTION.** Arena high-water accessor: continuation-node count.
    pub fn cont_nodes_len(&self) -> usize {
        self.cont.nodes.len()
    }
    /// **NON-PRODUCTION.** Number of values on the stack reachable from `ptr`.
    pub fn stack_depth(&self, ptr: StackPtr) -> usize {
        let mut n = 0usize;
        let mut p = ptr;
        while p != 0 {
            n += 1;
            p = self.stack.nodes[p as usize].parent;
        }
        n
    }
}

/// **NON-PRODUCTION PROTOTYPE** — host-layer speculation driver over cloned
/// `VmState`s, implementing the §4.2/§4.3 design of `v0.5-refactor.md`.
///
/// The driver is untrusted scheduling code (TCB, exactly like the `Host`): it
/// owns a *frontier* of branches, each a plain deterministic core state plus a
/// fuel slice, and it decides which to step and which to keep. It can never
/// make a branch report a result the core would not — a buggy driver is a
/// completeness/quality defect, never a soundness one.
///
/// **The load-bearing invariant** (total metering, tied to #26/#27):
/// ```text
///     Σ_{b ∈ live} budget(b)  +  spent  ≤  total_b      (always)
/// ```
/// `B` is *split*, never multiplied: N branches can never collectively out-run
/// a single sequential `drive(B)`. The invariant is asserted at every step
/// boundary. Reclaimed budget (from culled/halted branches) returns to the pool
/// implicitly — it re-appears as spawn headroom (`available`) without ever
/// raising the ceiling.
pub mod spec {
    use super::{StepOutcome, Value, Vm, VmState};

    /// Stable identifier for a branch within one search.
    pub type BranchId = usize;

    /// A live speculative branch: a plain core state + its fuel slice.
    /// The `vm` field is 12 bytes, `Copy`; spawning copies it, O(1).
    #[derive(Clone, Debug)]
    pub struct Branch {
        pub id: BranchId,
        pub vm: VmState,
        pub budget: u64,
        pub score: i64,
    }

    /// The result of advancing a branch by a quota of steps.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum BranchOutcome {
        /// Stepped, still running (quota/budget exhausted before terminal).
        Live,
        /// Reached Done; result stack reified out of the arena.
        Halted(Vec<Value>),
        /// Faulted (or budget hit 0 mid-flight) — cull it.
        Dead,
        /// Yielded a capability Invoke; buffered/deferred (dry-run mode).
        Deferred(String),
    }

    /// The speculation driver. Owns the shared arenas (`arena`), a `frontier`
    /// of live branches, and the global fuel accounting.
    #[derive(Debug)]
    pub struct SpecDriver {
        /// Shared arenas for this search generation. All branches' `VmState`s
        /// index into these; structure is shared, only divergent suffixes
        /// allocate.
        pub arena: Vm,
        /// All live branches.
        pub frontier: Vec<Branch>,
        /// Fuel already consumed across the whole search.
        pub spent: u64,
        /// The #26/#27 global budget `B` — never exceeded.
        pub total_b: u64,
        /// Finished branches' reified results, drained by [`collect_halts`].
        halted: Vec<(BranchId, Vec<Value>)>,
        /// Deferred (buffered) Invokes from speculative branches. For these
        /// tasks nothing Invokes, so this is a record-and-drop stub for losers.
        deferred: Vec<(BranchId, String)>,
        next_id: BranchId,
    }

    impl SpecDriver {
        /// Build a driver over `arena` with global budget `total_b`.
        pub fn new(arena: Vm, total_b: u64) -> Self {
            SpecDriver {
                arena,
                frontier: Vec::new(),
                spent: 0,
                total_b,
                halted: Vec::new(),
                deferred: Vec::new(),
                next_id: 0,
            }
        }

        /// Sum of live branches' remaining budgets.
        #[inline]
        fn live_budget(&self) -> u64 {
            self.frontier.iter().map(|b| b.budget).sum()
        }

        /// Budget headroom still assignable without breaching the invariant:
        /// `total_b − spent − Σ_live budget`. Reclaimed budget from culled
        /// branches reappears here automatically.
        #[inline]
        pub fn available(&self) -> u64 {
            self.total_b - self.spent - self.live_budget()
        }

        /// The invariant `Σ_live budget + spent ≤ total_b`.
        #[inline]
        pub fn invariant_holds(&self) -> bool {
            self.live_budget() + self.spent <= self.total_b
        }

        /// **O(1): copy 12 bytes.** Spawn a branch at position `parent`, drawing
        /// a fuel slice (clamped to `available()` so the invariant can never be
        /// breached) and a heuristic `score`. Returns the new branch id.
        ///
        /// In §4.2 `spawn` partitions a *parent's* remaining budget; here the
        /// experiment spawns candidate roots, so the slice is drawn from the
        /// shared pool via `available()`. Either way the ceiling is `B`.
        pub fn spawn(&mut self, parent: VmState, budget: u64, score: i64) -> BranchId {
            let budget = budget.min(self.available());
            let id = self.next_id;
            self.next_id += 1;
            self.frontier.push(Branch { id, vm: parent, budget, score });
            debug_assert!(self.invariant_holds(), "budget invariant breached on spawn");
            id
        }

        #[inline]
        fn index_of(&self, id: BranchId) -> Option<usize> {
            self.frontier.iter().position(|b| b.id == id)
        }

        /// Advance branch `id` by up to `k` core steps, drawing from its own
        /// fuel slice. Each executed word decrements the branch's budget and
        /// increments the shared `spent` 1:1 — so `Σ_live budget + spent` is
        /// invariant across a `Next` step. Terminal outcomes remove the branch
        /// from the frontier (Halted → recorded; Dead/Deferred → dropped).
        pub fn step_with_quota(&mut self, id: BranchId, k: u64) -> BranchOutcome {
            let idx = match self.index_of(id) {
                Some(i) => i,
                None => return BranchOutcome::Dead,
            };
            let quota = k.min(self.frontier[idx].budget);
            let mut outcome = BranchOutcome::Live;
            for _ in 0..quota {
                let mut vm_state = self.frontier[idx].vm;
                match self.arena.step(&mut vm_state) {
                    StepOutcome::Next => {
                        self.frontier[idx].vm = vm_state;
                        self.frontier[idx].budget -= 1;
                        self.spent += 1;
                    }
                    StepOutcome::Halt => {
                        self.frontier[idx].vm = vm_state;
                        let stack = self.arena.stack_values(vm_state.stack);
                        let b = self.frontier.remove(idx);
                        self.halted.push((b.id, stack.clone()));
                        outcome = BranchOutcome::Halted(stack);
                        break;
                    }
                    StepOutcome::Fault(f) => {
                        let _ = f;
                        self.frontier.remove(idx);
                        outcome = BranchOutcome::Dead;
                        break;
                    }
                    StepOutcome::Invoke(name) => {
                        // Dry-run: buffer/defer, never commit to the real host.
                        let b = &self.frontier[idx];
                        self.deferred.push((b.id, name.clone()));
                        self.frontier.remove(idx);
                        outcome = BranchOutcome::Deferred(name);
                        break;
                    }
                }
            }
            // If the branch ran its slice to 0 without terminating it is stuck
            // (no fuel to make progress) — treat as Dead so callers cull it.
            if matches!(outcome, BranchOutcome::Live) && self.frontier[idx].budget == 0 {
                self.frontier.remove(idx);
                outcome = BranchOutcome::Dead;
            }
            assert!(self.invariant_holds(), "budget invariant breached after step");
            outcome
        }

        /// Cull branch `id`: drop its `VmState`; its unspent slice returns to the
        /// pool (reappears as `available()`), never raising the ceiling.
        pub fn cull(&mut self, id: BranchId) {
            if let Some(i) = self.index_of(id) {
                self.frontier.remove(i);
            }
            debug_assert!(self.invariant_holds(), "budget invariant breached on cull");
        }

        /// Drain and return the reified results of branches that have Halted.
        pub fn collect_halts(&mut self) -> Vec<(BranchId, Vec<Value>)> {
            std::mem::take(&mut self.halted)
        }

        /// Drain buffered (deferred) Invokes — losers' effects, dropped.
        pub fn collect_deferred(&mut self) -> Vec<(BranchId, String)> {
            std::mem::take(&mut self.deferred)
        }
    }

    /// Convenience for the equality anchor / experiment: fully drive one branch
    /// (compile `prog`, spawn it with the whole budget, step to a terminal) and
    /// return `(terminal, final_stack)`. Uses only the public driver API.
    pub fn drive_single(arena: Vm, prog: &[super::ProgWord], budget: u64)
        -> (BranchTerminal, Vec<Value>)
    {
        let mut d = SpecDriver::new(arena, budget);
        let st = d.arena.load(prog);
        let id = d.spawn(st, budget, 0);
        loop {
            match d.step_with_quota(id, u64::MAX) {
                BranchOutcome::Halted(s) => return (BranchTerminal::Halt, s),
                BranchOutcome::Dead => {
                    // Distinguish fault from budget-exhaustion by re-deriving:
                    // if any budget was left unspent the branch had faulted.
                    return (BranchTerminal::DeadOrExhausted, Vec::new());
                }
                BranchOutcome::Deferred(name) => {
                    return (BranchTerminal::Invoke(name), Vec::new())
                }
                BranchOutcome::Live => { /* keep stepping (quota was u64::MAX) */ }
            }
        }
    }

    /// Terminal classification returned by [`drive_single`].
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum BranchTerminal {
        Halt,
        DeadOrExhausted,
        Invoke(String),
    }

    // ------------------------------------------------------------- unit tests
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{run_arena, ArenaEnd, ProgWord, Prim, Value, Vm};

        /// (i) The budget invariant `Σ_live budget + spent ≤ total_b` holds
        /// across a spawn / step / cull sequence, including reclamation.
        #[test]
        fn budget_invariant_holds() {
            let mut d = SpecDriver::new(Vm::new(), 1000);
            assert!(d.invariant_holds());
            // A short terminating program: 2 3 + (2 steps to Halt).
            let prog = [ProgWord::PushInt(2), ProgWord::PushInt(3), ProgWord::Prim(Prim::Add)];

            // Spawn three branches, each asking for more than a third.
            let st = d.arena.load(&prog);
            let a = d.spawn(st, 500, 0);
            let st = d.arena.load(&prog);
            let b = d.spawn(st, 500, 0);
            let st = d.arena.load(&prog);
            let c = d.spawn(st, 500, 0); // clamped: only 0 left (500+500 used)
            assert!(d.invariant_holds(), "after 3 spawns");
            assert!(d.available() <= d.total_b);

            // Step a by one: budget moves from live to spent, sum unchanged.
            let before = d.live_budget() + d.spent;
            let _ = d.step_with_quota(a, 1);
            assert!(d.invariant_holds(), "after stepping a");
            assert_eq!(before, d.live_budget() + d.spent, "step conserves Σ+spent");

            // Cull b: its unspent slice returns to the pool (available rises).
            let avail_before = d.available();
            d.cull(b);
            assert!(d.invariant_holds(), "after cull b");
            assert!(d.available() >= avail_before, "cull reclaims budget");

            // Drive c and a to completion; invariant must survive throughout.
            for id in [a, c] {
                loop {
                    match d.step_with_quota(id, u64::MAX) {
                        BranchOutcome::Live => {}
                        _ => break,
                    }
                    assert!(d.invariant_holds());
                }
            }
            assert!(d.invariant_holds(), "final");
            assert!(d.spent <= d.total_b, "never overspent B");
        }

        /// (ii) The "one branch == drive(B)" equality anchor: a single branch
        /// stepped to Halt yields the SAME final stack as `run_arena(prog, B)`.
        #[test]
        fn one_branch_equals_run_arena() {
            // A mix exercising push / arith / dup / a Times loop.
            let progs: Vec<Vec<ProgWord>> = vec![
                vec![ProgWord::PushInt(2), ProgWord::PushInt(3), ProgWord::Prim(Prim::Add)],
                vec![
                    ProgWord::PushInt(3),
                    ProgWord::PushInt(5),
                    ProgWord::Prim(Prim::Add),
                    ProgWord::Prim(Prim::Dup),
                    ProgWord::Prim(Prim::Mul),
                ],
                // 1 4 [2 *] .  -> double 1 four times = 16
                vec![
                    ProgWord::PushInt(1),
                    ProgWord::PushInt(4),
                    ProgWord::PushQuote(vec![ProgWord::PushInt(2), ProgWord::Prim(Prim::Mul)]),
                    ProgWord::Prim(Prim::Times),
                ],
            ];
            const B: u64 = 1_000_000;
            for prog in &progs {
                let run = run_arena(prog, B);
                let want: Vec<Value> = run.vm.stack_values(run.stack);
                assert_eq!(run.end, ArenaEnd::Halt, "sanity: prog halts");

                let (term, got) = drive_single(Vm::new(), prog, B);
                assert_eq!(term, BranchTerminal::Halt, "driver halts too");
                assert_eq!(got, want, "one-branch drive == run_arena final stack");
            }
        }
    }
}
