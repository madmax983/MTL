//! The standard capability set for the Tier-3 tasks (design §8), plus per-task
//! fixture builders. All names are lexer-safe (lowercase alphanumeric,
//! `[a-z][a-z0-9]*`) so they parse as `Call` words: the mtl-syntax lexer treats
//! `-` as `sub` and `?` as `if`, so the design's `read-line`/`done?` become
//! `readline`/`donep` here.
//!
//! [`standard_registry`] grants 22 capabilities (the original 18 plus the v0.4
//! expansion `nextline`/`endp`/`concat`/`select`). [`restricted_registry_and_ctx`]
//! builds a *confined* grant set for the `confined_*` tasks, and
//! [`task_setup`] is the deterministic oracle (fixture + correct registry +
//! expected output) shared by the tests and the `tier3run` validator binary.

use mtl_core::interp::Value;

use crate::capability::{Capability, FaultKind, Registry, StackEffect};
use crate::handle::{handles_from_list, list_of_handles, HandleTable};
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

    // nextline : ( -- h ) — intern lines[read_cursor], advance the cursor, push
    // the handle. If the cursor is already past the end, fault InputClosed and
    // DO NOT advance (the fault-handling tasks rely on this being guardable).
    reg.register(cap("nextline", 0, 1, vec![FaultKind::InputClosed], |ctx, stack| {
        if ctx.read_cursor >= ctx.fixture.lines.len() {
            return Err(HostFault::InputClosed);
        }
        let line = ctx.fixture.lines[ctx.read_cursor].clone();
        ctx.read_cursor += 1;
        let h = ctx.handles.intern(line);
        stack.push(Value::Int(h));
        Ok(())
    }));

    // endp : ( -- 0|1 ) — push 1 iff the read cursor is at/past end-of-input.
    // Never faults, so it can guard `nextline` in a linrec loop.
    reg.register(cap("endp", 0, 1, vec![], |ctx, stack| {
        let done = ctx.read_cursor >= ctx.fixture.lines.len();
        stack.push(Value::Int(if done { 1 } else { 0 }));
        Ok(())
    }));

    // concat : ( h1 h2 -- h ) — for stack `... h1 h2`, resolve both and intern
    // resolve(h1) + resolve(h2), pushing the fresh handle. ToolError if either
    // handle can't resolve.
    reg.register(cap("concat", 2, 1, vec![FaultKind::ToolError], |ctx, stack| {
        let h2 = match stack.pop() {
            Some(Value::Int(n)) => n,
            _ => return Err(HostFault::ToolError("concat: expected Int on top".into())),
        };
        let h1 = match stack.pop() {
            Some(Value::Int(n)) => n,
            _ => return Err(HostFault::ToolError("concat: expected Int below top".into())),
        };
        let s1 = resolve_owned(&ctx.handles, h1, "concat")?;
        let s2 = resolve_owned(&ctx.handles, h2, "concat")?;
        let h = ctx.handles.intern(format!("{s1}{s2}"));
        stack.push(Value::Int(h));
        Ok(())
    }));

    // select : ( [h...] n -- h ) — pop index n and a quote of handles, push the
    // n-th handle (0-indexed). ToolError if the value is not a handle list or n
    // is out of range.
    reg.register(cap("select", 2, 1, vec![FaultKind::ToolError], |_ctx, stack| {
        let n = match stack.pop() {
            Some(Value::Int(n)) => n,
            _ => return Err(HostFault::ToolError("select: expected Int index on top".into())),
        };
        let list = stack
            .pop()
            .ok_or_else(|| HostFault::ToolError("select: missing handle list".into()))?;
        let handles = handles_from_list(&list)
            .ok_or_else(|| HostFault::ToolError("select: not a handle list".into()))?;
        let idx = usize::try_from(n)
            .map_err(|_| HostFault::ToolError("select: negative index".into()))?;
        let h = *handles
            .get(idx)
            .ok_or_else(|| HostFault::ToolError(format!("select: index {idx} out of range")))?;
        stack.push(Value::Int(h));
        Ok(())
    }));

    reg
}

/// Build both the standard registry and a fresh context around `fixture`.
pub fn standard_registry_and_ctx(fixture: TaskFixture) -> (Registry, HostCtx) {
    (standard_registry(), HostCtx::new(fixture))
}

/// Build a CONFINED registry granting only `allowed`, plus a fresh context.
/// Starts from [`standard_registry`] and drops every capability not named in
/// `allowed`, so any `Call` to a removed name faults `NotGranted`.
pub fn restricted_registry_and_ctx(
    fixture: TaskFixture,
    allowed: &[&str],
) -> (Registry, HostCtx) {
    let mut reg = standard_registry();
    reg.retain(allowed);
    (reg, HostCtx::new(fixture))
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

// ---- v0.4 expansion fixtures (8 new tasks) --------------------------------

pub fn fixture_transform_hits() -> TaskFixture {
    TaskFixture {
        lines: vec!["apple".into(), "banana".into(), "apricot".into(), "cherry".into()],
        predicate_char: Some('a'),
        ..Default::default()
    }
}

pub fn fixture_emit_budget() -> TaskFixture {
    TaskFixture {
        lines: vec!["one".into(), "two".into(), "three".into(), "four".into()],
        ..Default::default()
    }
}

pub fn fixture_guarded_read() -> TaskFixture {
    TaskFixture {
        lines: vec!["x".into(), "y".into(), "z".into()],
        ..Default::default()
    }
}

pub fn fixture_concat_lines() -> TaskFixture {
    TaskFixture {
        lines: vec!["foo".into(), "bar".into()],
        ..Default::default()
    }
}

pub fn fixture_select_line() -> TaskFixture {
    TaskFixture {
        lines: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        ..Default::default()
    }
}

pub fn fixture_confined_echo() -> TaskFixture {
    TaskFixture {
        lines: vec!["hello".into()],
        ..Default::default()
    }
}

pub fn fixture_confined_grep() -> TaskFixture {
    TaskFixture {
        lines: vec!["cat".into(), "dog".into(), "car".into(), "fish".into()],
        predicate_char: Some('c'),
        ..Default::default()
    }
}

pub fn fixture_budget_grep() -> TaskFixture {
    TaskFixture {
        lines: vec!["ant".into(), "bee".into(), "art".into(), "cod".into()],
        predicate_char: Some('a'),
        ..Default::default()
    }
}

// ------------------------------------------------------------------------
// The deterministic task oracle — shared by the tests and `tier3run`.
// ------------------------------------------------------------------------

/// The fully-built inputs for one task: the correct grant set (restricted for
/// `confined_*`, budgeted for `emit_budget`/`budget_grep`, standard otherwise),
/// a fresh host context, and the expected stdout the program must produce.
pub struct TaskSetup {
    pub reg: Registry,
    pub ctx: HostCtx,
    pub expected_output: String,
}

/// Build the registry + context + expected output for `task`, or `None` for an
/// unknown task name. This is the single source of truth the `tier3run`
/// validator binary uses as its deterministic oracle.
pub fn task_setup(task: &str) -> Option<TaskSetup> {
    let (reg, ctx, expected_output) = match task {
        "echo_line" => {
            let (r, c) = standard_registry_and_ctx(fixture_echo_line());
            (r, c, "hello world\n".to_string())
        }
        "grep_filter" => {
            let (r, c) = standard_registry_and_ctx(fixture_grep_filter());
            (r, c, "apple\napricot\n".to_string())
        }
        "agent_loop" => {
            // Stack-based task; produces no output.
            let (r, c) = standard_registry_and_ctx(fixture_agent_loop());
            (r, c, String::new())
        }
        "json_field" => {
            let (r, c) = standard_registry_and_ctx(fixture_json_field());
            (r, c, "neo\n".to_string())
        }
        "two_tool_pipeline" => {
            let (r, c) = standard_registry_and_ctx(fixture_two_tool_pipeline());
            (r, c, "parsed:q1\n".to_string())
        }
        "retry_on_fault" => {
            // Stack-based task; produces no output.
            let (r, c) = standard_registry_and_ctx(fixture_retry_on_fault());
            (r, c, String::new())
        }
        "map_lines_tool" => {
            let (r, c) = standard_registry_and_ctx(fixture_map_lines_tool());
            (r, c, "A\nB\nC\n".to_string())
        }
        "word_count" => {
            let (r, c) = standard_registry_and_ctx(fixture_word_count());
            (r, c, "4\n".to_string())
        }
        "transform_hits" => {
            let (r, c) = standard_registry_and_ctx(fixture_transform_hits());
            (r, c, "APPLE\nAPRICOT\n".to_string())
        }
        "emit_budget" => {
            let (r, mut c) = standard_registry_and_ctx(fixture_emit_budget());
            c.meter.set_call_budget("emit", 2);
            (r, c, "one\ntwo\n".to_string())
        }
        "guarded_read" => {
            let (r, c) = standard_registry_and_ctx(fixture_guarded_read());
            (r, c, "x\ny\nz\n".to_string())
        }
        "concat_lines" => {
            let (r, c) = standard_registry_and_ctx(fixture_concat_lines());
            (r, c, "foobar\n".to_string())
        }
        "select_line" => {
            let (r, c) = standard_registry_and_ctx(fixture_select_line());
            (r, c, "c\n".to_string())
        }
        "confined_echo" => {
            let (r, c) = restricted_registry_and_ctx(fixture_confined_echo(), &["readline", "emit"]);
            (r, c, "hello\n".to_string())
        }
        "confined_grep" => {
            let (r, c) = restricted_registry_and_ctx(
                fixture_confined_grep(),
                &["readlines", "linehit", "emit"],
            );
            (r, c, "cat\ncar\n".to_string())
        }
        "budget_grep" => {
            let (r, mut c) = standard_registry_and_ctx(fixture_budget_grep());
            c.meter.set_call_budget("emit", 2);
            (r, c, "ant\nart\n".to_string())
        }
        _ => return None,
    };
    Some(TaskSetup {
        reg,
        ctx,
        expected_output,
    })
}
