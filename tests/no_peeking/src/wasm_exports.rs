//! Read a canister's method surface out of the MODULE, not out of a document.
//!
//! # Why not parse the `.did`
//!
//! Because a `.did` is a claim and a wasm export is a fact, and this project
//! already has a finding for the two disagreeing: FINDING 03 / DEFECTS E-08 is
//! `src/table_canister/table_canister.did` not describing the code it claims to.
//! The whole point of `tests/no_peek.rs` is to call **every** method that exists
//! and prove none of them yields a card; a list that came from a hand-maintained
//! file would let a newly added method escape the test in silence, which is the
//! exact shape of the defect this wave exists to close.
//!
//! `ic_cdk::export_candid!()` emits the interface at BUILD time via
//! `candid-extractor`, so the canister cannot simply be asked either. The module's
//! export section can, and it is the same thing the replica dispatches on.
//!
//! # The format, for anyone checking this by hand
//!
//! A wasm module is `\0asm` + a 4-byte version, then a sequence of sections, each
//! `section_id : u8` then `size : leb128(u32)` then `size` bytes of payload.
//! Section 7 is the export section: `count : leb128`, then `count` entries of
//! `name_len : leb128`, `name : utf8`, `kind : u8`, `index : leb128`. Canister
//! methods are exported as `canister_query <name>`, `canister_update <name>`,
//! `canister_composite_query <name>`.

/// One exported entry point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Method {
    /// The name a caller uses on the wire.
    pub name: String,
    /// `query`, `update` or `composite_query`.
    pub kind: String,
}

impl Method {
    pub fn is_query(&self) -> bool {
        self.kind != "update"
    }
}

fn leb128(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*at)?;
        *at += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// Every `canister_query` / `canister_update` / `canister_composite_query` the
/// module exports, in export order.
///
/// Panics on a malformed module: a harness that cannot read the surface must fail
/// loudly rather than silently test an empty list.
pub fn methods(wasm: &[u8]) -> Vec<Method> {
    assert!(
        wasm.starts_with(b"\0asm"),
        "not a wasm module (is it gzipped?)"
    );
    let mut at = 8usize;
    let mut out = Vec::new();
    while at < wasm.len() {
        let id = wasm[at];
        at += 1;
        let size = leb128(wasm, &mut at).expect("truncated section size") as usize;
        let end = at + size;
        assert!(end <= wasm.len(), "section runs past the end of the module");
        if id == 7 {
            let mut p = at;
            let count = leb128(wasm, &mut p).expect("truncated export count");
            for _ in 0..count {
                let len = leb128(wasm, &mut p).expect("truncated export name length") as usize;
                let name = String::from_utf8_lossy(&wasm[p..p + len]).into_owned();
                p += len;
                p += 1; // kind byte (0 = func)
                let _index = leb128(wasm, &mut p).expect("truncated export index");
                for (prefix, kind) in [
                    ("canister_composite_query ", "composite_query"),
                    ("canister_query ", "query"),
                    ("canister_update ", "update"),
                ] {
                    if let Some(rest) = name.strip_prefix(prefix) {
                        out.push(Method {
                            name: rest.to_string(),
                            kind: kind.to_string(),
                        });
                        break;
                    }
                }
            }
        }
        at = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module with no export section has no methods, and saying so must not
    /// panic — but an empty list from a REAL canister is a harness bug, which is
    /// why every caller asserts a non-empty surface.
    #[test]
    fn a_bare_header_has_no_methods() {
        let wasm = b"\0asm\x01\x00\x00\x00";
        assert!(methods(wasm).is_empty());
    }

    #[test]
    fn an_export_section_is_parsed_by_kind() {
        // one section, id 7, two exports.
        let mut payload: Vec<u8> = vec![2];
        for name in ["canister_query foo", "canister_update bar"] {
            payload.push(name.len() as u8);
            payload.extend_from_slice(name.as_bytes());
            payload.push(0x00); // func
            payload.push(0x00); // index
        }
        let mut wasm: Vec<u8> = b"\0asm\x01\x00\x00\x00".to_vec();
        wasm.push(7);
        wasm.push(payload.len() as u8);
        wasm.extend_from_slice(&payload);

        let got = methods(&wasm);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], Method { name: "foo".into(), kind: "query".into() });
        assert_eq!(got[1], Method { name: "bar".into(), kind: "update".into() });
        assert!(got[0].is_query());
        assert!(!got[1].is_query());
    }
}
