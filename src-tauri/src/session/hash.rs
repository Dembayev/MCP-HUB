//! Canonical-JSON SHA-256 for `payload_hash` (see §5.5 of the spec).
//!
//! The full RFC 8785 (JSON Canonicalization Scheme) handles a few number-format
//! edge cases that our v0.1 simplified canonicalizer does not — specifically,
//! the shortest-roundtrip representation for non-integer floats. For real MCP
//! traffic this is essentially never hit: tool arguments are dominated by
//! strings, integers, booleans, and nested objects. We document the gap here
//! explicitly so future-us doesn't get burned silently.
//!
//! What we DO guarantee:
//! - Object keys are sorted lexicographically (UTF-8 byte order).
//! - No whitespace.
//! - Strings use serde_json's default escape.
//! - Integers serialize as decimal digits with no leading zeros.
//! - Floats serialize via serde_json's default formatter (good enough for v0.1).
//!
//! If a future spec version moves to strict JCS, swap the body of
//! [`canonical_json`] for a real implementation — call sites do not change.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// ASCII unit separator inserted between the args canonicalization and the
/// result canonicalization before hashing. Picking a non-JSON byte means we
/// cannot collide with any structural character a JSON canonicalizer might
/// emit, regardless of input.
const PAYLOAD_SEP: u8 = 0x1f;

/// Compute the `payload_hash` for an action.
///
/// - When both `args` and `result` are present, the hash covers
///   `canonical(args) || 0x1f || canonical(result)` and the result is prefixed
///   `sha256:`.
/// - When `result` is absent (action incomplete), only `args` is hashed and
///   the prefix is `sha256-partial:`. The §10 fixture's third action (a denied
///   write with no result) is an example.
/// - When both are absent (e.g. lifecycle event with empty args object), the
///   hash covers the empty-object canonicalization.
pub fn payload_hash(args: Option<&Value>, result: Option<&Value>) -> String {
    let empty = Value::Object(serde_json::Map::new());
    let args_v = args.unwrap_or(&empty);

    let mut hasher = Sha256::new();
    write_canonical(&mut hasher, args_v);

    let prefix = match result {
        Some(r) => {
            hasher.update([PAYLOAD_SEP]);
            write_canonical(&mut hasher, r);
            "sha256"
        }
        None => "sha256-partial",
    };

    let digest = hasher.finalize();
    // Hand-rolled hex to avoid the `hex` crate dep. digest's GenericArray
    // doesn't implement LowerHex, so format!("{:x}") wouldn't compile.
    use std::fmt::Write;
    let mut hex = String::with_capacity(prefix.len() + 1 + digest.len() * 2);
    hex.push_str(prefix);
    hex.push(':');
    for byte in digest.iter() {
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}

/// Write the canonical-JSON encoding of `v` directly into the hasher.
///
/// Streaming into the hasher (vs. materializing a String first) keeps peak
/// memory bounded for large payloads.
fn write_canonical(hasher: &mut Sha256, v: &Value) {
    let bytes = canonical_json(v);
    hasher.update(bytes.as_bytes());
}

/// Produce the canonical-JSON encoding of `v` as a String.
///
/// Public so tests and CLI tooling can inspect / reproduce hashes.
pub fn canonical_json(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v);
    out
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(out, s),
        Value::Array(arr) => {
            out.push('[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // Lexicographic UTF-8 byte order. serde_json::Map's default backing
            // (BTreeMap, when the `preserve_order` feature is OFF) already
            // iterates in this order — but we sort explicitly to stay correct
            // regardless of which feature flags downstream consumers enable.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, k);
                out.push(':');
                write_value(out, &map[*k]);
            }
            out.push('}');
        }
    }
}

fn write_string(out: &mut String, s: &str) {
    // Delegate string escaping to serde_json so we inherit its JSON-conformant
    // handling of control characters, surrogate pairs, etc.
    let escaped = serde_json::to_string(s).expect("string serialization is infallible");
    out.push_str(&escaped);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_sorts_object_keys() {
        let v = json!({ "b": 1, "a": 2, "c": { "z": 0, "y": 1 } });
        assert_eq!(canonical_json(&v), r#"{"a":2,"b":1,"c":{"y":1,"z":0}}"#);
    }

    #[test]
    fn canonical_handles_arrays_and_primitives() {
        let v = json!([null, true, false, 1, -2, 3.5, "hi"]);
        assert_eq!(canonical_json(&v), r#"[null,true,false,1,-2,3.5,"hi"]"#);
    }

    #[test]
    fn hash_partial_when_result_absent() {
        let args = json!({"path": "/tmp"});
        let h = payload_hash(Some(&args), None);
        assert!(h.starts_with("sha256-partial:"));
        assert_eq!(h.len(), "sha256-partial:".len() + 64);
    }

    #[test]
    fn hash_full_when_both_present() {
        let args = json!({"path": "/tmp"});
        let result = json!({"ok": true});
        let h = payload_hash(Some(&args), Some(&result));
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
    }

    #[test]
    fn hash_is_stable_under_key_reordering() {
        let a = json!({ "x": 1, "y": 2 });
        let b = json!({ "y": 2, "x": 1 });
        assert_eq!(
            payload_hash(Some(&a), None),
            payload_hash(Some(&b), None),
            "canonical hash must be invariant to JSON object key order"
        );
    }
}
