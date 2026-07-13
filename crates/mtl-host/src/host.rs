//! The host-side execution context and result types — the impure state that
//! lives *outside* the verified core (v0.4 design §2.4). `host_state` never
//! enters the core; the only channel is the `Invoke` value in and a
//! [`HostResult`] out.

use std::collections::HashMap;

use mtl_core::interp::Value;

use crate::handle::HandleTable;
use crate::meter::Meter;

/// A host-side fault. Mirrors the design's `HostCode`/`HostFault` set (§3.1,
/// §6). These are the codes a capability may raise; grant/deny of a capability
/// itself is handled by [`crate::core_bridge::HostShim`], which maps an ungranted
/// name onto `HostCode::NotGranted` before any capability runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostFault {
    /// A per-capability call budget was exhausted (meter §6a).
    BudgetExhausted,
    /// An output-byte charge would exceed the total byte cap (meter §6b).
    OutputCapExceeded,
    /// A capability name the host does not know how to service.
    UnknownCapability(String),
    /// A capability's own tool logic failed.
    ToolError(String),
    /// An input source was closed / exhausted.
    InputClosed,
}

/// The result the host runner returns to the core at an `Invoke` boundary
/// (design §2.3). `host_state` stays host-local, so only the resumed value
/// stack crosses back into the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostResult {
    /// Service succeeded; resume the core with this value stack.
    Resume(Vec<Value>),
    /// Service failed; the run ends with this host fault.
    HostFault(HostFault),
}

/// Fixed, host-owned inputs and presets for a task run. Capabilities read their
/// data from here; nothing here is visible to the pure core.
#[derive(Debug, Clone, Default)]
pub struct TaskFixture {
    /// Lines available to `readline` (first) / `readlines` (all).
    pub lines: Vec<String>,
    /// Predicate for `linehit`: a line "hits" iff it starts with this char.
    pub predicate_char: Option<char>,
    /// Seed value for `readstate`.
    pub initial_state: i64,
    /// `donep` reports done once state `>= done_threshold`.
    pub done_threshold: i64,
    /// Raw JSON document for `readjson`.
    pub json: String,
    /// Query string for `readinput`.
    pub query: String,
    /// Free text for `readtext`.
    pub text: String,
    /// Number of `tryop` calls after which the flaky op starts succeeding.
    pub flaky_success_at: u64,
}

/// The whole impure host context threaded through a `drive` run.
#[derive(Debug)]
pub struct HostCtx {
    /// Opaque string handles (design §5).
    pub handles: HandleTable,
    /// Resource meter (design §6).
    pub meter: Meter,
    /// Captured output bytes (everything `emit`/`emitint` write).
    output: Vec<u8>,
    /// Host-owned task inputs/presets.
    pub fixture: TaskFixture,
    /// Running count of `tryop` invocations (models flakiness).
    pub flaky_calls: u64,
    /// Per-capability service counts (for at-most-once / call-count assertions).
    call_log: HashMap<String, u64>,
}

impl HostCtx {
    /// A fresh context around a fixture, with unlimited meter and empty output.
    pub fn new(fixture: TaskFixture) -> Self {
        HostCtx {
            handles: HandleTable::new(),
            meter: Meter::new(),
            output: Vec::new(),
            fixture,
            flaky_calls: 0,
            call_log: HashMap::new(),
        }
    }

    /// Append raw bytes to the output buffer. Callers MUST have already charged
    /// the meter for these bytes (the atomic-charge-before-effect discipline).
    pub fn write_output(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    /// The captured output as a UTF-8 string (lossy if a capability wrote
    /// non-UTF-8, which the standard set never does).
    pub fn output_utf8(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    /// The raw captured output bytes.
    pub fn output_bytes(&self) -> &[u8] {
        &self.output
    }

    /// The captured output split into lines (trailing empty line dropped).
    pub fn output_lines(&self) -> Vec<String> {
        let s = self.output_utf8();
        let mut v: Vec<String> = s.split('\n').map(|x| x.to_string()).collect();
        if let Some(last) = v.last() {
            if last.is_empty() {
                v.pop();
            }
        }
        v
    }

    /// Record that capability `name` was serviced once (driver-side bookkeeping).
    pub fn record_call(&mut self, name: &str) {
        *self.call_log.entry(name.to_string()).or_insert(0) += 1;
    }

    /// How many times capability `name` was serviced this run.
    pub fn calls_to(&self, name: &str) -> u64 {
        self.call_log.get(name).copied().unwrap_or(0)
    }
}
