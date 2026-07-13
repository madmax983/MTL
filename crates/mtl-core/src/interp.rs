//! MTL executable interpreter (Track B, GREEN phase).
//!
//! This is the cargo-compiled, stable-Rust executable interpreter for the MTL
//! concatenative core. It is the runtime counterpart of the Verus-verified
//! `exec_step`/`run` in [`crate::mtl_core`] (`src/mtl_core.rs`).
//!
//! ## Why a separate module?
//!
//! `src/mtl_core.rs` is a self-contained Verus artifact wrapped in `verus! { .. }`
//! and depending on `vstd`; it is checked by the `verus` binary, **not** compiled
//! by `cargo` (see `lib.rs`). The exec AST types (`Word`/`Value`/`Vm`) and the
//! verified `exec_step`/`run` live there for the P2 refinement proof. This module
//! is a byte-for-byte semantic mirror in plain stable Rust so the interpreter can
//! actually run and be exercised by the cargo test suite (golden / boundary /
//! precedence / differential-oracle). The two must stay in lock-step; the fault
//! check order below mirrors `spec_step`/`spec_step_prim` exactly.
//!
//! ## Totality
//!
//! `exec_step` and `run` are total: every stack access and every type refinement
//! is handled explicitly and yields a `Fault` rather than a panic — there are no
//! `expect`/`unreachable!`/`panic!` sites. (Well-formed VM states never reach the
//! internal type-refinement fault arms; those are exercised by the differential
//! tests and the verified twin.) All arithmetic is checked; div/mod truncate
//! toward zero (Rust semantics); every fault is a value. `run` is fuel-bounded and
//! does not assume termination (MTL is Turing complete).

/// The primitives of MTL: the 17 v0.1 primitives, the 4 v0.2 recursion
/// primitives, and the 2 v0.3 sequence primitives (`fold`, `xor`). Mirrors
/// `SpecPrim` in `mtl_core.rs`.
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
    // v0.2 recursion primitives (design: docs/design/v0.2-recursion-primitives.md).
    /// `( n [I] [C] -- r )` bounded primitive recursion. Total, terminating.
    PrimRec,
    /// `( n [Q] -- ... )` bounded iteration: run Q max(n,0) times. Total.
    Times,
    /// `( [P] [T] [R1] [R2] -- ... )` linear recursion; desugars into `If`. Partial.
    LinRec,
    /// `( [w ...] -- w [...] 1 )` | `( [] -- 0 )` quotation deconstructor. Affine.
    Uncons,
    // v0.3 sequence primitives (design: docs/design/v0.3-sequences.md).
    /// `( [seq] init [C] -- r )` native LEFT fold. Total & terminating (the
    /// sequence spine strictly shrinks each step, like `primrec`'s count).
    Fold,
    /// `( a b -- a^b )` total i64 two's-complement bitwise XOR. No overflow.
    Xor,
}

/// A program word. Mirrors exec `Word` in `mtl_core.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Word {
    PushInt(i64),
    PushQuote(Vec<Word>),
    Prim(Prim),
    /// Host capability / definition name. v0.4 effects: executing a `Call`
    /// dispatches to the host by yielding `Step::Invoke(name)` (spec §8.2) — the
    /// pure core has no host/dictionary machinery and does not distinguish bound
    /// vs unbound names; grant/deny is a host-side decision.
    Call(String),
}

/// A first-class value. Mirrors exec `Value` in `mtl_core.rs`.
///
/// Note: the spec prose (§3) lists `Str`, but the verified ghost model in
/// `mtl_core.rs` carries only `Int | Quote`. This module matches the *model*
/// (the source of truth for the proof), so there is no `Str` variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Quote(Vec<Word>),
}

/// A runtime fault kind. Mirrors `Error` in `mtl_core.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Underflow,
    TypeMismatch,
    Overflow,
    DivByZero,
}

/// The machine state: operand stack + continuation. Mirrors exec `Vm`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Vm {
    pub stack: Vec<Value>,
    pub cont: Vec<Word>,
}

impl Vm {
    /// Build a VM that will execute `program` against an empty stack.
    pub fn new(program: Vec<Word>) -> Self {
        Vm {
            stack: Vec::new(),
            cont: program,
        }
    }

    /// Build a VM with an explicit initial stack (bottom .. top).
    pub fn with_stack(stack: Vec<Value>, program: Vec<Word>) -> Self {
        Vm {
            stack,
            cont: program,
        }
    }
}

/// Result of a single small step. Mirrors `StepResult` in `mtl_core.rs`.
///
/// Per-step results carry only the fault *kind* (a bare tag), exactly like the
/// spec's `SpecStep::Fault(Error)` — this keeps the P2 refinement statement a
/// direct tag/state equality. The *machine state at the fault* (stack + remaining
/// continuation) is carried by [`Outcome::Fault`] instead (see below), which is
/// what an LLM writer needs to self-correct.
///
/// v0.4 effects: `Invoke(name)` is the fourth outcome — a `Call(name)` word
/// yields to the host instead of faulting. It carries the capability name;
/// the `Call` word is consumed by `exec_step` and the stack is left unchanged
/// (mirrors `SpecStep::Invoke` in `mtl_core.rs`). Because it carries an owned
/// `String`, `Step` is no longer `Copy`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Halt,
    Fault(Fault),
    Invoke(String),
}

/// A fault together with the machine state captured at the moment of the fault.
///
/// Adversarial-review requirement #2: runtime fault *values* must carry the exact
/// stack contents and the position in the continuation, so a program writer can
/// see precisely where and with what state the machine got stuck.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultInfo {
    pub fault: Fault,
    /// Stack contents at the fault (bottom .. top).
    pub stack: Vec<Value>,
    /// The continuation that was about to execute (head = next word), i.e. the
    /// faulting word is `cont[0]`.
    pub cont: Vec<Word>,
}

/// Terminal outcome of a fuel-bounded [`run`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The continuation emptied; final stack (bottom .. top).
    Halt(Vec<Value>),
    /// A fault occurred; carries the machine state at the fault.
    Fault(FaultInfo),
    /// Fuel ran out before Halt/Fault. Carries the live machine state so
    /// execution could be resumed with more fuel.
    FuelExhausted { stack: Vec<Value>, cont: Vec<Word> },
    /// v0.4 effects: a `Call(name)` word yielded to the host. Carries the
    /// capability name, the stack snapshot at the yield point, and the
    /// continuation (the tail AFTER the consumed `Call`), so a host runner can
    /// service the capability and re-seed a fresh `run` from `cont`.
    Invoke {
        name: String,
        stack: Vec<Value>,
        cont: Vec<Word>,
    },
}

/// `value_to_word`: the reification used by `cons` and `dip`. Mirrors
/// `value_to_word` in `mtl_core.rs`.
#[inline]
fn value_to_word(v: Value) -> Word {
    match v {
        Value::Int(i) => Word::PushInt(i),
        Value::Quote(q) => Word::PushQuote(q),
    }
}

/// Execute exactly one small step, mutating `vm` in place.
///
/// TOTAL: no `expect`/`unreachable!`/`panic!` — every access and refinement
/// yields a `Fault`. The fault-check order byte-for-byte mirrors
/// `spec_step` / `spec_step_prim` in `mtl_core.rs`:
///   * arity (`Underflow`) is always checked before type (`TypeMismatch`);
///   * for `div`/`mod`, `DivByZero` is checked before `Overflow`, and both are
///     inside the both-operands-`Int` arm (so a type mismatch outranks them);
///   * for `add`/`sub`/`mul`, `Overflow` is checked after the type match.
///
/// On `Fault`, `vm` is left holding the pre-step machine state (the faulting
/// word is still `vm.cont[0]`), so the caller can snapshot it.
pub fn exec_step(vm: &mut Vm) -> Step {
    if vm.cont.is_empty() {
        return Step::Halt;
    }
    // Peek the head word without consuming it yet, so that on a fault vm is left
    // with the faulting word at cont[0].
    match &vm.cont[0] {
        Word::PushInt(n) => {
            let n = *n;
            vm.cont.remove(0);
            vm.stack.push(Value::Int(n));
            Step::Next
        }
        Word::PushQuote(_) => {
            // Move the quote out of the head word.
            let head = vm.cont.remove(0);
            if let Word::PushQuote(q) = head {
                vm.stack.push(Value::Quote(q));
            }
            Step::Next
        }
        Word::Call(name) => {
            // v0.4 effects: yield to the host. Stack unchanged (Call consumes
            // nothing); consume the Call word so cont becomes `rest`.
            let name = name.clone();
            vm.cont.remove(0);
            Step::Invoke(name)
        }
        Word::Prim(p) => {
            let p = *p;
            exec_prim(vm, p)
        }
    }
}

/// Consume the head prim word `p` and apply it. Assumes `vm.cont[0] == Prim(p)`.
fn exec_prim(vm: &mut Vm, p: Prim) -> Step {
    let n = vm.stack.len();
    match p {
        // ---------------- stack shuffling ----------------
        Prim::Dup => {
            if n < 1 {
                return Step::Fault(Fault::Underflow);
            }
            vm.cont.remove(0);
            let top = vm.stack[n - 1].clone();
            vm.stack.push(top);
            Step::Next
        }
        Prim::Drop => {
            if n < 1 {
                return Step::Fault(Fault::Underflow);
            }
            vm.cont.remove(0);
            vm.stack.pop();
            Step::Next
        }
        Prim::Swap => {
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            vm.cont.remove(0);
            vm.stack.swap(n - 1, n - 2);
            Step::Next
        }
        Prim::Rot => {
            // ( a b c -- b c a )
            if n < 3 {
                return Step::Fault(Fault::Underflow);
            }
            vm.cont.remove(0);
            // rotate the top three left: [.. a b c] -> [.. b c a]
            let a = vm.stack.remove(n - 3);
            vm.stack.push(a);
            Step::Next
        }
        Prim::Over => {
            // ( a b -- a b a )
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            vm.cont.remove(0);
            let a = vm.stack[n - 2].clone();
            vm.stack.push(a);
            Step::Next
        }
        // ---------------- quotation algebra ----------------
        Prim::Apply => {
            if n < 1 {
                return Step::Fault(Fault::Underflow);
            }
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    if let Some(Value::Quote(q)) = vm.stack.pop() {
                        // cont := q ++ rest
                        prepend(&mut vm.cont, q);
                    }
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::Cat => {
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let Some(b) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(mut a) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    a.extend(b);
                    vm.stack.push(Value::Quote(a));
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::Cons => {
            // ( v [q] -- [v q] )
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let Some(q) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(v) = vm.stack.pop() else {
                        return Step::Fault(Fault::Underflow);
                    };
                    let mut newq = Vec::with_capacity(q.len() + 1);
                    newq.push(value_to_word(v));
                    newq.extend(q);
                    vm.stack.push(Value::Quote(newq));
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::Dip => {
            // ( a [q] -- ... a ) : cont := q ++ (Push(a) :: rest)
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let Some(q) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(a) = vm.stack.pop() else {
                        return Step::Fault(Fault::Underflow);
                    };
                    // cont := q ++ [Push(a)] ++ rest
                    vm.cont.insert(0, value_to_word(a));
                    prepend(&mut vm.cont, q);
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        // ---------------- arithmetic (checked, truncating) ----------------
        Prim::Add => exec_arith(vm, |a, b| a.checked_add(b)),
        Prim::Sub => exec_arith(vm, |a, b| a.checked_sub(b)),
        Prim::Mul => exec_arith(vm, |a, b| a.checked_mul(b)),
        Prim::Div => exec_divmod(vm, true),
        Prim::Mod => exec_divmod(vm, false),
        // ---------------- comparison ----------------
        Prim::Eq => exec_cmp(vm, |a, b| a == b),
        Prim::Lt => exec_cmp(vm, |a, b| a < b),
        // ---------------- branch ----------------
        Prim::If => {
            // ( c [t] [f] -- ... )
            if n < 3 {
                return Step::Fault(Fault::Underflow);
            }
            match (&vm.stack[n - 3], &vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let Some(f) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(t) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let c = match vm.stack.pop() {
                        Some(Value::Int(c)) => c,
                        Some(_) => return Step::Fault(Fault::TypeMismatch),
                        None => return Step::Fault(Fault::Underflow),
                    };
                    let branch = if c != 0 { t } else { f };
                    prepend(&mut vm.cont, branch);
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        // ---------------- v0.2 recursion primitives ----------------
        // Byte-for-byte semantic mirror of the ghost `spec_step_prim` arms in
        // `mtl_core.rs`: same fault precedence (arity -> types), same expansions.
        Prim::PrimRec => {
            // ( n [I] [C] -- r )
            if n < 3 {
                return Step::Fault(Fault::Underflow);
            }
            match (&vm.stack[n - 3], &vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let Some(qc) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(qi) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let k = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        Some(_) => return Step::Fault(Fault::TypeMismatch),
                        None => return Step::Fault(Fault::Underflow),
                    };
                    if k <= 0 {
                        // base: discard the count, run I: cont := qi ++ rest
                        prepend(&mut vm.cont, qi);
                    } else {
                        // else: cont := [k, k-1, [qi], [qc], primrec] ++ qc ++ rest.
                        // k>0 => k-1 does not underflow; k<=i64::MAX => no overflow.
                        let mut recur = Vec::with_capacity(qc.len() + 5);
                        recur.push(Word::PushInt(k));
                        recur.push(Word::PushInt(k - 1));
                        recur.push(Word::PushQuote(qi));
                        recur.push(Word::PushQuote(qc.clone()));
                        recur.push(Word::Prim(Prim::PrimRec));
                        recur.extend(qc);
                        prepend(&mut vm.cont, recur);
                    }
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::Times => {
            // ( n [Q] -- ... )
            if n < 2 {
                return Step::Fault(Fault::Underflow);
            }
            match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let Some(q) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let k = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        Some(_) => return Step::Fault(Fault::TypeMismatch),
                        None => return Step::Fault(Fault::Underflow),
                    };
                    if k > 0 {
                        // cont := q ++ [k-1, [q], times] ++ rest
                        let mut recur = q.clone();
                        recur.push(Word::PushInt(k - 1));
                        recur.push(Word::PushQuote(q));
                        recur.push(Word::Prim(Prim::Times));
                        prepend(&mut vm.cont, recur);
                    }
                    // k <= 0: no-op, cont := rest
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::LinRec => {
            // ( [P] [T] [R1] [R2] -- ... ) — desugars into If.
            if n < 4 {
                return Step::Fault(Fault::Underflow);
            }
            match (
                &vm.stack[n - 4],
                &vm.stack[n - 3],
                &vm.stack[n - 2],
                &vm.stack[n - 1],
            ) {
                (Value::Quote(_), Value::Quote(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let Some(qr2) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(qr1) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(qt) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    let Some(qp) = pop_quote(vm) else {
                        return Step::Fault(Fault::TypeMismatch);
                    };
                    // else_q := R1 ++ [[P],[T],[R1],[R2],linrec] ++ R2
                    let mut else_q = qr1.clone();
                    else_q.push(Word::PushQuote(qp.clone()));
                    else_q.push(Word::PushQuote(qt.clone()));
                    else_q.push(Word::PushQuote(qr1));
                    else_q.push(Word::PushQuote(qr2.clone()));
                    else_q.push(Word::Prim(Prim::LinRec));
                    else_q.extend(qr2);
                    // spliced := P ++ [[T], [else_q], If] ++ rest
                    let mut spliced = qp;
                    spliced.push(Word::PushQuote(qt));
                    spliced.push(Word::PushQuote(else_q));
                    spliced.push(Word::Prim(Prim::If));
                    prepend(&mut vm.cont, spliced);
                    Step::Next
                }
                _ => Step::Fault(Fault::TypeMismatch),
            }
        }
        Prim::Uncons => {
            // ( [w ...] -- w [...] 1 ) | ( [] -- 0 )
            if n < 1 {
                return Step::Fault(Fault::Underflow);
            }
            // Inspect without consuming: a non-value head (bare Prim/Call) or a
            // non-Quote operand faults, leaving the machine state untouched.
            match &vm.stack[n - 1] {
                Value::Quote(q) => {
                    if let Some(head) = q.first() {
                        match head {
                            Word::PushInt(_) | Word::PushQuote(_) => {}
                            _ => return Step::Fault(Fault::TypeMismatch),
                        }
                    }
                }
                _ => return Step::Fault(Fault::TypeMismatch),
            }
            vm.cont.remove(0);
            let Some(q) = pop_quote(vm) else {
                return Step::Fault(Fault::TypeMismatch);
            };
            let mut it = q.into_iter();
            match it.next() {
                None => vm.stack.push(Value::Int(0)),
                Some(head) => {
                    let tail: Vec<Word> = it.collect();
                    let head_val = match head {
                        Word::PushInt(k) => Value::Int(k),
                        Word::PushQuote(s) => Value::Quote(s),
                        Word::Prim(_) | Word::Call(_) => {
                            return Step::Fault(Fault::TypeMismatch)
                        }
                    };
                    vm.stack.push(head_val);
                    vm.stack.push(Value::Quote(tail));
                    vm.stack.push(Value::Int(1));
                }
            }
            Step::Next
        }
        // ---------------- v0.3 sequence primitives ----------------
        // Byte-for-byte semantic mirror of the ghost `spec_step_prim` arms in
        // `mtl_core.rs`: same fault precedence, same native re-emission.
        Prim::Fold => {
            // ( [seq] init [C] -- r ) LEFT fold. init seeds the accumulator;
            // C ( acc w -- acc' ) runs once per element, left to right.
            if n < 3 {
                return Step::Fault(Fault::Underflow);
            }
            // seq (n-3) and combine (n-1) must be quotes; init (n-2) is any value.
            // Inspect without consuming: a non-Quote seq/combine, or a non-value
            // seq head (bare Prim/Call), faults, leaving the machine untouched.
            match (&vm.stack[n - 3], &vm.stack[n - 1]) {
                (Value::Quote(qs), Value::Quote(_)) => {
                    if let Some(head) = qs.first() {
                        match head {
                            Word::PushInt(_) | Word::PushQuote(_) => {}
                            _ => return Step::Fault(Fault::TypeMismatch),
                        }
                    }
                }
                _ => return Step::Fault(Fault::TypeMismatch),
            }
            vm.cont.remove(0);
            let Some(qc) = pop_quote(vm) else {
                return Step::Fault(Fault::TypeMismatch);
            };
            let Some(init) = vm.stack.pop() else {
                return Step::Fault(Fault::Underflow);
            };
            let Some(qs) = pop_quote(vm) else {
                return Step::Fault(Fault::TypeMismatch);
            };
            let mut it = qs.into_iter();
            match it.next() {
                // empty list: the result is the seed accumulator.
                None => vm.stack.push(init),
                Some(head) => {
                    let tail: Vec<Word> = it.collect();
                    // cont := [PushQuote(tail), value_to_word(init), head] ++ qc
                    //         ++ [PushQuote(qc), Fold] ++ rest
                    let mut recur = Vec::with_capacity(qc.len() + 5);
                    recur.push(Word::PushQuote(tail));
                    recur.push(value_to_word(init));
                    recur.push(head);
                    recur.extend(qc.clone());
                    recur.push(Word::PushQuote(qc));
                    recur.push(Word::Prim(Prim::Fold));
                    prepend(&mut vm.cont, recur);
                }
            }
            Step::Next
        }
        Prim::Xor => exec_xor(vm),
    }
}

/// Shared implementation of `add`/`sub`/`mul`: arity -> type -> overflow.
fn exec_arith(vm: &mut Vm, op: fn(i64, i64) -> Option<i64>) -> Step {
    let n = vm.stack.len();
    if n < 2 {
        return Step::Fault(Fault::Underflow);
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let (a, b) = (*a, *b);
            match op(a, b) {
                Some(r) => {
                    vm.cont.remove(0);
                    vm.stack.truncate(n - 2);
                    vm.stack.push(Value::Int(r));
                    Step::Next
                }
                None => Step::Fault(Fault::Overflow),
            }
        }
        _ => Step::Fault(Fault::TypeMismatch),
    }
}

/// Shared implementation of `div`/`mod`: arity -> type -> DivByZero -> Overflow.
/// `checked_div`/`checked_rem` return `None` for both `b == 0` and `MIN / -1`,
/// so `b == 0` is checked explicitly first to distinguish the two faults, exactly
/// as `spec_divmod` distinguishes them.
fn exec_divmod(vm: &mut Vm, is_div: bool) -> Step {
    let n = vm.stack.len();
    if n < 2 {
        return Step::Fault(Fault::Underflow);
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let (a, b) = (*a, *b);
            if b == 0 {
                return Step::Fault(Fault::DivByZero);
            }
            let res = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
            match res {
                Some(r) => {
                    vm.cont.remove(0);
                    vm.stack.truncate(n - 2);
                    vm.stack.push(Value::Int(r));
                    Step::Next
                }
                // Only i64::MIN / -1 (and rem) reach here.
                None => Step::Fault(Fault::Overflow),
            }
        }
        _ => Step::Fault(Fault::TypeMismatch),
    }
}

/// Shared implementation of `eq`/`lt`: arity -> type -> result 0/1.
fn exec_cmp(vm: &mut Vm, op: fn(i64, i64) -> bool) -> Step {
    let n = vm.stack.len();
    if n < 2 {
        return Step::Fault(Fault::Underflow);
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let r = if op(*a, *b) { 1 } else { 0 };
            vm.cont.remove(0);
            vm.stack.truncate(n - 2);
            vm.stack.push(Value::Int(r));
            Step::Next
        }
        _ => Step::Fault(Fault::TypeMismatch),
    }
}

/// `xor`: arity -> type -> i64 two's-complement bitwise XOR. TOTAL — the XOR of
/// two in-range i64 values is always in i64 range, so (unlike `add`/`sub`/`mul`)
/// there is no overflow check: it mirrors `exec_cmp`'s shape, not `exec_arith`'s.
fn exec_xor(vm: &mut Vm) -> Step {
    let n = vm.stack.len();
    if n < 2 {
        return Step::Fault(Fault::Underflow);
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let r = a ^ b;
            vm.cont.remove(0);
            vm.stack.truncate(n - 2);
            vm.stack.push(Value::Int(r));
            Step::Next
        }
        _ => Step::Fault(Fault::TypeMismatch),
    }
}

/// Pop a value expected to be a `Quote` (guarded by the caller's match),
/// returning its body. Returns `None` if the popped value is not a `Quote`
/// (or the stack was empty), restoring a wrongly-typed value first so the
/// machine state is unchanged — the caller propagates the appropriate
/// `TypeMismatch`. Never fabricates an empty quotation (no silent repair);
/// `None` is unreachable under the arity+type-guarded callers.
#[inline]
fn pop_quote(vm: &mut Vm) -> Option<Vec<Word>> {
    match vm.stack.pop() {
        Some(Value::Quote(q)) => Some(q),
        Some(v) => {
            vm.stack.push(v);
            None
        }
        None => None,
    }
}

/// `cont := prefix ++ cont`. The O(n) continuation splice (spec §4.2 / §12 open
/// question 3). Correctness-first: we do not optimize the representation here.
#[inline]
fn prepend(cont: &mut Vec<Word>, prefix: Vec<Word>) {
    if prefix.is_empty() {
        return;
    }
    let mut new = prefix;
    new.append(cont); // moves current cont onto the end
    *cont = new;
}

/// Fuel-bounded driver. Steps until `Halt`, `Fault`, or `fuel` steps elapse.
///
/// Termination is NOT assumed (MTL is Turing complete). `fuel` counts small
/// steps; `fuel == 0` returns `FuelExhausted` immediately with the initial state.
pub fn run(mut vm: Vm, fuel: u64) -> Outcome {
    let mut steps: u64 = 0;
    loop {
        if steps >= fuel {
            return Outcome::FuelExhausted {
                stack: vm.stack,
                cont: vm.cont,
            };
        }
        match exec_step(&mut vm) {
            Step::Next => {
                steps += 1;
            }
            Step::Halt => return Outcome::Halt(vm.stack),
            Step::Fault(f) => {
                return Outcome::Fault(FaultInfo {
                    fault: f,
                    stack: vm.stack,
                    cont: vm.cont,
                })
            }
            // v0.4 effects: the core suspended at a Call; hand the snapshot +
            // continuation out so a host runner can service and re-seed.
            Step::Invoke(name) => {
                return Outcome::Invoke {
                    name,
                    stack: vm.stack,
                    cont: vm.cont,
                }
            }
        }
    }
}

// Convenience constructors for building programs as ASTs directly (no parser).
pub mod build {
    use super::{Prim, Word};

    pub fn int(n: i64) -> Word {
        Word::PushInt(n)
    }
    pub fn quote(ws: Vec<Word>) -> Word {
        Word::PushQuote(ws)
    }
    pub fn call(name: &str) -> Word {
        Word::Call(name.to_string())
    }
    pub fn prim(p: Prim) -> Word {
        Word::Prim(p)
    }
    pub fn dup() -> Word {
        Word::Prim(Prim::Dup)
    }
    pub fn drop() -> Word {
        Word::Prim(Prim::Drop)
    }
    pub fn swap() -> Word {
        Word::Prim(Prim::Swap)
    }
    pub fn rot() -> Word {
        Word::Prim(Prim::Rot)
    }
    pub fn over() -> Word {
        Word::Prim(Prim::Over)
    }
    pub fn apply() -> Word {
        Word::Prim(Prim::Apply)
    }
    pub fn cat() -> Word {
        Word::Prim(Prim::Cat)
    }
    pub fn cons() -> Word {
        Word::Prim(Prim::Cons)
    }
    pub fn dip() -> Word {
        Word::Prim(Prim::Dip)
    }
    pub fn add() -> Word {
        Word::Prim(Prim::Add)
    }
    pub fn sub() -> Word {
        Word::Prim(Prim::Sub)
    }
    pub fn mul() -> Word {
        Word::Prim(Prim::Mul)
    }
    pub fn div() -> Word {
        Word::Prim(Prim::Div)
    }
    pub fn modulo() -> Word {
        Word::Prim(Prim::Mod)
    }
    pub fn eq() -> Word {
        Word::Prim(Prim::Eq)
    }
    pub fn lt() -> Word {
        Word::Prim(Prim::Lt)
    }
    pub fn iff() -> Word {
        Word::Prim(Prim::If)
    }
    // v0.2 recursion primitives.
    pub fn primrec() -> Word {
        Word::Prim(Prim::PrimRec)
    }
    pub fn times() -> Word {
        Word::Prim(Prim::Times)
    }
    pub fn linrec() -> Word {
        Word::Prim(Prim::LinRec)
    }
    pub fn uncons() -> Word {
        Word::Prim(Prim::Uncons)
    }
    // v0.3 sequence primitives.
    pub fn fold() -> Word {
        Word::Prim(Prim::Fold)
    }
    pub fn xor() -> Word {
        Word::Prim(Prim::Xor)
    }
}
