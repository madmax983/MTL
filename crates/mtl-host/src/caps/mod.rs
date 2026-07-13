//! The standard capability set for the eight Tier-3 tasks (design §8), plus
//! per-task fixture builders. All names are lexer-safe (lowercase alphanumeric,
//! `[a-z][a-z0-9]*`) so they parse as `Call` words: the mtl-syntax lexer treats
//! `-` as `sub` and `?` as `if`, so the design's `read-line`/`done?` become
//! `readline`/`donep` here.

use mtl_core::interp::Value;

use crate::capability::{Capability, FaultKind, Registry, StackEffect};
use crate::handle::{list_of_handles, HandleTable};
use crate::host::{HostCtx, HostFault, TaskFixture};
use crate::meter::MeterError;

/// Map a meter refusal to the matching host fault.
fn map_meter(e: MeterError) -> HostFault {
    match e {
        MeterError::BudgetExhausted => HostFault::BudgetExhausted,
        MeterError::OutputCapExceeded => HostFault::OutputCapExceeded,
    }
}

/// Peek the top of the stack as an `Int`, or a `ToolError`.
fn top_int(stack: &[Value], who: &str) -> Result<i64, HostFault> {
    match stack.last() {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(HostFault::ToolError(format!("{who}: expected Int on top"))),
    }
}

/// Resolve a handle to an owned string, or a `ToolError`.
fn resolve_owned(handles: &HandleTable, h: i64, who: &str) -> Result<String, HostFault> {
    handles
        .resolve(h)
        .map(|s| s.to_string())
        .ok_or_else(|| HostFault::ToolError(format!("{who}: unknown handle {h}")))
}

/// Extract a string-valued JSON field by a tiny hand-parse (serde-free — the
/// fixture JSON is flat and well-formed).
fn extract_json_string_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let idx = json.find(&key)?;
    let after = &json[idx + key.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let inner = rest.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn cap(
    name: &str,
    in_arity: usize,
    out_arity: usize,
    faults: Vec<FaultKind>,
    run: impl FnMut(&mut HostCtx, &mut Vec<Value>) -> Result<(), HostFault> + 'static,
) -> Capability {
    Capability::new(name, StackEffect::new(in_arity, out_arity), faults, Box::new(run))
}

/// Build the full standard registry: every Tier-3 capability granted.
pub fn standard_registry() -> Registry {
    let mut reg = Registry::new();

    // readline : ( -- h ) — intern the first fixture line, push its handle.
    reg.register(cap("readline", 0, 1, vec![FaultKind::InputClosed], |ctx, stack| {
        let line = ctx
            .fixture
            .lines
            .first()
            .cloned()
            .ok_or(HostFault::InputClosed)?;
        let h = ctx.handles.intern(line);
        stack.push(Value::Int(h));
        Ok(())
    }));

    // emit : ( h -- ) {output} — resolve, charge bytes, write `line + "\n"`.
    reg.register(cap("emit", 1, 0, vec![FaultKind::OutputCapExceeded], |ctx, stack| {
        let h = top_int(stack, "emit")?;
        let s = resolve_owned(&ctx.handles, h, "emit")?;
        let line = format!("{s}\n");
        // Charge BEFORE writing; on refusal nothing is written or popped.
        ctx.meter
            .charge_bytes(line.len() as u64)
            .map_err(map_meter)?;
        ctx.write_output(line.as_bytes());
        stack.pop();
        Ok(())
    }));

    // readlines : ( -- [h...] ) — intern every fixture line, push a Quote.
    reg.register(cap("readlines", 0, 1, vec![], |ctx, stack| {
        let handles: Vec<i64> = ctx
            .fixture
            .lines
            .clone()
            .into_iter()
            .map(|l| ctx.handles.intern(l))
            .collect();
        stack.push(list_of_handles(&handles));
        Ok(())
    }));

    // linehit : ( h -- h 0|1 ) — leave handle, push 1 iff line hits predicate.
    reg.register(cap("linehit", 1, 2, vec![], |ctx, stack| {
        let h = top_int(stack, "linehit")?;
        let s = resolve_owned(&ctx.handles, h, "linehit")?;
        let hit = match ctx.fixture.predicate_char {
            Some(c) => s.starts_with(c),
            None => false,
        };
        stack.push(Value::Int(if hit { 1 } else { 0 }));
        Ok(())
    }));

    // readstate : ( -- s ) — push the fixture's initial state.
    reg.register(cap("readstate", 0, 1, vec![], |ctx, stack| {
        stack.push(Value::Int(ctx.fixture.initial_state));
        Ok(())
    }));

    // donep : ( s -- s 0|1 ) — leave s, push 1 iff s >= done_threshold.
    reg.register(cap("donep", 1, 2, vec![], |ctx, stack| {
        let s = top_int(stack, "donep")?;
        let done = s >= ctx.fixture.done_threshold;
        stack.push(Value::Int(if done { 1 } else { 0 }));
        Ok(())
    }));

    // step : ( s -- s' ) — replace top with s + 1.
    reg.register(cap("step", 1, 1, vec![], |_ctx, stack| {
        let s = top_int(stack, "step")?;
        stack.pop();
        stack.push(Value::Int(s + 1));
        Ok(())
    }));

    // readjson : ( -- j ) — intern the fixture json, push handle.
    reg.register(cap("readjson", 0, 1, vec![], |ctx, stack| {
        let h = ctx.handles.intern(ctx.fixture.json.clone());
        stack.push(Value::Int(h));
        Ok(())
    }));

    // getname : ( j -- v ) — extract the "name" field, intern it, push handle.
    reg.register(cap("getname", 1, 1, vec![FaultKind::ToolError], |ctx, stack| {
        let h = top_int(stack, "getname")?;
        let json = resolve_owned(&ctx.handles, h, "getname")?;
        let name = extract_json_string_field(&json, "name")
            .ok_or_else(|| HostFault::ToolError("getname: no \"name\" field".into()))?;
        stack.pop();
        let nh = ctx.handles.intern(name);
        stack.push(Value::Int(nh));
        Ok(())
    }));

    // readinput : ( -- q ) — intern the fixture query, push handle.
    reg.register(cap("readinput", 0, 1, vec![], |ctx, stack| {
        let h = ctx.handles.intern(ctx.fixture.query.clone());
        stack.push(Value::Int(h));
        Ok(())
    }));

    // fetch : ( q -- doc ) — resolve query, intern "doc:" + query, push handle.
    reg.register(cap("fetch", 1, 1, vec![FaultKind::ToolError], |ctx, stack| {
        let h = top_int(stack, "fetch")?;
        let q = resolve_owned(&ctx.handles, h, "fetch")?;
        stack.pop();
        let dh = ctx.handles.intern(format!("doc:{q}"));
        stack.push(Value::Int(dh));
        Ok(())
    }));

    // parse : ( doc -- v ) — resolve doc, intern "parsed:" + <query part>.
    reg.register(cap("parse", 1, 1, vec![FaultKind::ToolError], |ctx, stack| {
        let h = top_int(stack, "parse")?;
        let doc = resolve_owned(&ctx.handles, h, "parse")?;
        let q = doc.strip_prefix("doc:").unwrap_or(&doc).to_string();
        stack.pop();
        let vh = ctx.handles.intern(format!("parsed:{q}"));
        stack.push(Value::Int(vh));
        Ok(())
    }));

    // readtext : ( -- t ) — intern the fixture text, push handle.
    reg.register(cap("readtext", 0, 1, vec![], |ctx, stack| {
        let h = ctx.handles.intern(ctx.fixture.text.clone());
        stack.push(Value::Int(h));
        Ok(())
    }));

    // tokenize : ( t -- [w...] ) — resolve text, split whitespace, intern each.
    reg.register(cap("tokenize", 1, 1, vec![], |ctx, stack| {
        let h = top_int(stack, "tokenize")?;
        let text = resolve_owned(&ctx.handles, h, "tokenize")?;
        stack.pop();
        let handles: Vec<i64> = text
            .split_whitespace()
            .map(|w| ctx.handles.intern(w.to_string()))
            .collect();
        stack.push(list_of_handles(&handles));
        Ok(())
    }));

    // emitint : ( n -- ) {output} — charge bytes, write `n` decimal + "\n".
    reg.register(cap("emitint", 1, 0, vec![FaultKind::OutputCapExceeded], |ctx, stack| {
        let n = top_int(stack, "emitint")?;
        let line = format!("{n}\n");
        ctx.meter
            .charge_bytes(line.len() as u64)
            .map_err(map_meter)?;
        ctx.write_output(line.as_bytes());
        stack.pop();
        Ok(())
    }));

    // transform : ( h -- h' ) — resolve, uppercase, intern, push new handle.
    reg.register(cap("transform", 1, 1, vec![], |ctx, stack| {
        let h = top_int(stack, "transform")?;
        let s = resolve_owned(&ctx.handles, h, "transform")?;
        stack.pop();
        let th = ctx.handles.intern(s.to_uppercase());
        stack.push(Value::Int(th));
        Ok(())
    }));

    // tryop : ( -- r ) — flaky: bump the call counter, push it as the result.
    reg.register(cap("tryop", 0, 1, vec![FaultKind::ToolError], |ctx, stack| {
        ctx.flaky_calls += 1;
        stack.push(Value::Int(ctx.flaky_calls as i64));
        Ok(())
    }));

    // okp : ( r -- r 0|1 ) — leave r, push 1 once the flaky op has "warmed up".
    reg.register(cap("okp", 1, 2, vec![], |ctx, stack| {
        let _r = top_int(stack, "okp")?;
        let ok = ctx.flaky_calls >= ctx.fixture.flaky_success_at;
        stack.push(Value::Int(if ok { 1 } else { 0 }));
        Ok(())
    }));

    reg
}

/// Build both the standard registry and a fresh context around `fixture`.
pub fn standard_registry_and_ctx(fixture: TaskFixture) -> (Registry, HostCtx) {
    (standard_registry(), HostCtx::new(fixture))
}

// ------------------------------------------------------------------------
// Per-task fixture builders (the host-owned inputs the design §8 tasks use).
// ------------------------------------------------------------------------

pub fn fixture_echo_line() -> TaskFixture {
    TaskFixture {
        lines: vec!["hello world".into()],
        ..Default::default()
    }
}

pub fn fixture_grep_filter() -> TaskFixture {
    TaskFixture {
        lines: vec!["apple".into(), "banana".into(), "apricot".into(), "cherry".into()],
        predicate_char: Some('a'),
        ..Default::default()
    }
}

pub fn fixture_agent_loop() -> TaskFixture {
    TaskFixture {
        initial_state: 0,
        done_threshold: 5,
        ..Default::default()
    }
}

pub fn fixture_json_field() -> TaskFixture {
    TaskFixture {
        json: r#"{"name":"neo","age":30}"#.into(),
        ..Default::default()
    }
}

pub fn fixture_two_tool_pipeline() -> TaskFixture {
    TaskFixture {
        query: "q1".into(),
        ..Default::default()
    }
}

pub fn fixture_retry_on_fault() -> TaskFixture {
    TaskFixture {
        flaky_success_at: 3,
        ..Default::default()
    }
}

pub fn fixture_map_lines_tool() -> TaskFixture {
    TaskFixture {
        lines: vec!["a".into(), "b".into(), "c".into()],
        ..Default::default()
    }
}

pub fn fixture_word_count() -> TaskFixture {
    TaskFixture {
        text: "the quick brown fox".into(),
        ..Default::default()
    }
}
