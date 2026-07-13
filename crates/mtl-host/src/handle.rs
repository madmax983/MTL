//! Opaque string handles — the realization of v0.4 design §5's decision to keep
//! strings **host-side**. The verified core has no `Value::Str`; a string is an
//! opaque `i64` handle on the MTL stack (`Value::Int`), and the mapping from
//! handle → owned `String` lives here, entirely host-side. The core can copy,
//! drop, and shuffle handles with the existing primitives but can never inspect
//! or forge a string's bytes.

use std::collections::HashMap;

use mtl_core::interp::{Value, Word};

/// A host-owned table mapping opaque `i64` handles to owned `String`s.
///
/// Handles are freshly minted sequential integers starting at `1` (so `0` is
/// never a valid handle and can be reserved as a sentinel by callers).
#[derive(Debug, Default, Clone)]
pub struct HandleTable {
    next: i64,
    map: HashMap<i64, String>,
}

impl HandleTable {
    /// A fresh, empty table. The first handle it mints will be `1`.
    pub fn new() -> Self {
        HandleTable {
            next: 1,
            map: HashMap::new(),
        }
    }

    /// Intern a string, returning a fresh opaque handle for it. Interning the
    /// same string twice yields two distinct handles (no dedup — handles are
    /// identities, not values).
    pub fn intern(&mut self, s: impl Into<String>) -> i64 {
        let h = if self.next == 0 { 1 } else { self.next };
        self.next = h + 1;
        self.map.insert(h, s.into());
        h
    }

    /// Resolve a handle back to its string, or `None` if the handle is unknown.
    pub fn resolve(&self, h: i64) -> Option<&str> {
        self.map.get(&h).map(|s| s.as_str())
    }
}

/// Encode a list of handles as an MTL stack value: `[h1 h2 …]` becomes a
/// `Value::Quote` of `PushInt` words. This is how a `( -- [h…] )` capability
/// such as `readlines` hands a list of strings to the pure core.
pub fn list_of_handles(handles: &[i64]) -> Value {
    Value::Quote(handles.iter().map(|&h| Word::PushInt(h)).collect())
}

/// Decode an MTL quote of `PushInt` words back into a list of handles, the
/// inverse of [`list_of_handles`]. Returns `None` if the value is not a quote
/// of plain integer literals.
pub fn handles_from_list(v: &Value) -> Option<Vec<i64>> {
    match v {
        Value::Quote(words) => {
            let mut out = Vec::with_capacity(words.len());
            for w in words {
                match w {
                    Word::PushInt(h) => out.push(*h),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_fresh_and_sequential_from_one() {
        let mut t = HandleTable::new();
        let a = t.intern("alpha");
        let b = t.intern("beta");
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_round_trips_the_interned_string() {
        let mut t = HandleTable::new();
        let h = t.intern("hello world");
        assert_eq!(t.resolve(h), Some("hello world"));
        assert_eq!(t.resolve(999), None);
    }

    #[test]
    fn interning_the_same_string_yields_distinct_handles() {
        let mut t = HandleTable::new();
        let a = t.intern("dup");
        let b = t.intern("dup");
        assert_ne!(a, b);
        assert_eq!(t.resolve(a), t.resolve(b));
    }

    #[test]
    fn list_encode_decode_round_trip() {
        let v = list_of_handles(&[3, 7, 11]);
        assert_eq!(v, Value::Quote(vec![Word::PushInt(3), Word::PushInt(7), Word::PushInt(11)]));
        assert_eq!(handles_from_list(&v), Some(vec![3, 7, 11]));
    }

    #[test]
    fn decode_rejects_non_int_quote() {
        let bad = Value::Quote(vec![Word::PushQuote(vec![])]);
        assert_eq!(handles_from_list(&bad), None);
        assert_eq!(handles_from_list(&Value::Int(5)), None);
    }
}
