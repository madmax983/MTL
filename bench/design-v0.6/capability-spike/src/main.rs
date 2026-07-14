//! Capability-spike harness for MTL v0.6 indexed-access option (d):
//! HOST-SIDE SEQUENCES via capabilities (the "strings pattern" for sequences).
//!
//! This binary does NOT modify any production/verified file. It uses the REAL
//! invoke_host machinery — `mtl_core::host::drive` via `mtl_host::drive`, the
//! real `HostShim` servicing seam, the real `Registry`/`Capability` types — and
//! registers three NEW sequence capabilities backed by a host-side store:
//!
//!   * a sequence is an opaque `i64` HANDLE on the MTL stack (`Value::Int`),
//!     exactly like a string handle (`crates/mtl-host/src/handle.rs`). The core
//!     never sees the elements; the host owns the `Vec<i64>`.
//!   * `len`  : ( s -- n )                — O(1) host length (option-a parity: CONSUMING)
//!   * `nth`  : ( s i -- x 1 ) | ( s i -- 0 )  — O(1) host random access, FLAGGED,
//!                                          matching option-(a) `nth` semantics exactly
//!   * `slice`: ( s lo hi -- s' )         — O(1)-amortised host subsequence (fresh handle)
//!
//! Because `nth`/`len` index a host-side `Vec<i64>` directly, access is genuine
//! O(1) — so a bisection program is a TRUE O(log n) binary search, unlike
//! option (a)'s cons-list walk (O(n) per probe → O(n·log n)).
//!
//! Run:  cargo run --release -- <program> <csv-seq> <target>
//! e.g.  cargo run --release -- '^len 1-0~@[...]|' '1,3,5,7,9' 7

use std::cell::RefCell;
use std::rc::Rc;

use mtl_core::interp::Value;
use mtl_host::capability::{Capability, FaultKind, Registry, StackEffect};
use mtl_host::host::{HostCtx, HostFault, TaskFixture};
use mtl_host::conv_program;
use mtl_host::driver::{drive, RunResult};
use mtl_syntax::parse;

/// Host-side sequence store: index == opaque handle. Mirrors the string
/// HandleTable but for `Vec<i64>` (design (d): host-side sequences).
type Store = Rc<RefCell<Vec<Vec<i64>>>>;

fn intern(store: &Store, v: Vec<i64>) -> i64 {
    let mut s = store.borrow_mut();
    s.push(v);
    (s.len() - 1) as i64
}

fn pop_int(stack: &mut Vec<Value>, who: &str) -> Result<i64, HostFault> {
    match stack.pop() {
        Some(Value::Int(n)) => Ok(n),
        _ => Err(HostFault::ToolError(format!("{who}: expected Int on top"))),
    }
}

/// Build a registry with the three host-side sequence capabilities.
fn seq_registry(store: &Store) -> Registry {
    let mut reg = Registry::new();

    // len : ( s -- s n ) NON-CONSUMING (leaves handle, pushes length). Mirrors
    // the non-consuming-peek precedent (donep/linehit/okp). Handy for probing.
    let st = store.clone();
    reg.register(Capability::new(
        "len",
        StackEffect::new(1, 2),
        vec![FaultKind::ToolError],
        Box::new(move |_ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let s = match stack.last() {
                Some(Value::Int(n)) => *n,
                _ => return Err(HostFault::ToolError("len: expected Int on top".into())),
            };
            let len = st.borrow().get(s as usize)
                .ok_or_else(|| HostFault::ToolError("len: bad handle".into()))?
                .len() as i64;
            stack.push(Value::Int(len));
            Ok(())
        }),
    ));

    // nth : ( s i -- s x ) NON-CONSUMING of the handle (leaves s, consumes the
    // index, pushes the element). O(1) host random access. OOB -> ToolError.
    let st = store.clone();
    reg.register(Capability::new(
        "nth",
        StackEffect::new(2, 2),
        vec![FaultKind::ToolError],
        Box::new(move |_ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let i = pop_int(stack, "nth")?;
            let s = match stack.last() {
                Some(Value::Int(n)) => *n,
                _ => return Err(HostFault::ToolError("nth: expected handle".into())),
            };
            let store = st.borrow();
            let v = store.get(s as usize)
                .ok_or_else(|| HostFault::ToolError("nth: bad handle".into()))?;
            if i < 0 || (i as usize) >= v.len() {
                return Err(HostFault::ToolError(format!("nth: index {i} out of range")));
            }
            stack.push(Value::Int(v[i as usize]));
            Ok(())
        }),
    ));

    // nthc : ( s i -- x ) CONSUMING both (the `select`/opt-a convention). O(1).
    let st = store.clone();
    reg.register(Capability::new(
        "nthc",
        StackEffect::new(2, 1),
        vec![FaultKind::ToolError],
        Box::new(move |_ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let i = pop_int(stack, "nthc")?;
            let s = pop_int(stack, "nthc")?;
            let store = st.borrow();
            let v = store.get(s as usize)
                .ok_or_else(|| HostFault::ToolError("nthc: bad handle".into()))?;
            if i < 0 || (i as usize) >= v.len() {
                return Err(HostFault::ToolError(format!("nthc: index {i} out of range")));
            }
            stack.push(Value::Int(v[i as usize]));
            Ok(())
        }),
    ));

    // lenc : ( s -- n ) CONSUMING (the opt-a convention).
    let st = store.clone();
    reg.register(Capability::new(
        "lenc",
        StackEffect::new(1, 1),
        vec![FaultKind::ToolError],
        Box::new(move |_ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let s = pop_int(stack, "lenc")?;
            let len = st.borrow().get(s as usize)
                .ok_or_else(|| HostFault::ToolError("lenc: bad handle".into()))?
                .len() as i64;
            stack.push(Value::Int(len));
            Ok(())
        }),
    ));

    // slice : ( s lo hi -- s' ) — fresh host handle for s[lo..hi] (clamped).
    let st = store.clone();
    reg.register(Capability::new(
        "slice",
        StackEffect::new(3, 1),
        vec![FaultKind::ToolError],
        Box::new(move |_ctx: &mut HostCtx, stack: &mut Vec<Value>| {
            let hi = pop_int(stack, "slice")?;
            let lo = pop_int(stack, "slice")?;
            let s = pop_int(stack, "slice")?;
            let sub: Vec<i64> = {
                let store = st.borrow();
                let v = store.get(s as usize)
                    .ok_or_else(|| HostFault::ToolError("slice: bad handle".into()))?;
                let lo = lo.max(0) as usize;
                let hi = (hi.max(0) as usize).min(v.len());
                if lo >= hi { Vec::new() } else { v[lo..hi].to_vec() }
            };
            let h = intern(&st, sub);
            stack.push(Value::Int(h));
            Ok(())
        }),
    ));

    reg
}

/// Render a Value for debug output.
fn show(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Quote(ws) => {
            let inner: Vec<String> = ws.iter().map(|w| match w {
                mtl_core::interp::Word::PushInt(n) => n.to_string(),
                _ => "?".into(),
            }).collect();
            format!("[{}]", inner.join(" "))
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: spike <program> <csv-seq> <target>");
        std::process::exit(2);
    }
    let prog_src = &args[1];
    let seq: Vec<i64> = args[2].split(',').filter(|s| !s.is_empty())
        .map(|s| s.trim().parse().expect("int")).collect();
    let target: i64 = args[3].trim().parse().expect("int");

    let store: Store = Rc::new(RefCell::new(Vec::new()));
    let s_handle = intern(&store, seq.clone());

    let prog = match parse(prog_src) {
        Ok(p) => p,
        Err(e) => { eprintln!("parse error: {e}"); std::process::exit(1); }
    };
    let iprog = conv_program(&prog);

    let mut reg = seq_registry(&store);
    let mut ctx = HostCtx::new(TaskFixture::default());
    // Initial stack: sequence HANDLE then target (bottom..top): [ s t ].
    let init = vec![Value::Int(s_handle), Value::Int(target)];
    let result = drive(iprog, init, 100_000, &mut reg, &mut ctx);

    match result {
        RunResult::Done(stack) => {
            let shown: Vec<String> = stack.iter().map(show).collect();
            println!("HALT seq={seq:?} target={target} -> stack=[{}]", shown.join(" "));
        }
        other => {
            println!("NON-HALT seq={seq:?} target={target} -> {other:?}");
            std::process::exit(1);
        }
    }
}
