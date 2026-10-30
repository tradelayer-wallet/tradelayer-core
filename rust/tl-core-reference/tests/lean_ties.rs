//! Ties the Lean models to the Rust crate: the constants and byte vectors written in the Lean files
//! must equal what the Rust code (and therefore the C++-derived fixtures) says.

use std::collections::BTreeSet;
use tl_core_reference::rules::{ConsensusParams, Feature, Network};

const LEAN_ACTIVATION: &str = include_str!("../lean/TlCoreRef/Activation.lean");
const LEAN_PAYLOAD: &str = include_str!("../lean/TlCoreRef/Payload.lean");
const PAYLOAD_VECTORS: &str = include_str!("../fixtures/payload_vectors.tsv");

/// The numbers inside the first `[...]` after `marker`, in order.
fn numbers_after(src: &str, marker: &str) -> Vec<u64> {
    let from = src.find(marker).unwrap_or_else(|| panic!("{marker} not found"));
    let rest = &src[from..];
    let open = rest.find(":= [").or_else(|| rest.find(":=\n  [")).expect("list start");
    let rest = &rest[open..];
    let start = rest.find('[').unwrap();
    let end = rest[start..].find(']').unwrap() + start;
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in rest[start + 1..end].chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.parse().unwrap());
            cur.clear();
        }
    }
    out
}

#[test]
fn lean_slot_tables_match_rust() {
    for (marker, read) in [("def readTable", true), ("def writeTable", false)] {
        let nums = numbers_after(LEAN_ACTIVATION, marker);
        assert_eq!(nums.len(), 56, "{marker}");
        let pairs: Vec<(u64, u64)> = nums.chunks(2).map(|c| (c[0], c[1])).collect();
        assert_eq!(pairs.len(), Feature::ALL.len());
        for (id, slot) in pairs {
            let f = Feature::from_id(id as u16).unwrap_or_else(|| panic!("Lean lists unknown feature {id}"));
            let p = if read { f.read_param() } else { f.write_param() };
            assert_eq!(p as usize as u64, slot, "{marker} feature {id}");
        }
    }
}

#[test]
fn lean_regtest_block_matches_rust() {
    let nums = numbers_after(LEAN_ACTIVATION, "def regtestList");
    let rust: Vec<u64> = ConsensusParams::new(Network::Regtest).values().iter().map(|v| *v as u64).collect();
    assert_eq!(nums, rust);
    // never / TL_VERSION constants
    assert!(LEAN_ACTIVATION.contains(&format!("def never : Nat := {}", tl_core_reference::rules::NEVER_BLOCK)));
    assert!(LEAN_ACTIVATION.contains(&format!("def tlVersion : Nat := {}", tl_core_reference::rules::TL_VERSION)));
}

#[test]
fn lean_payload_byte_vectors_are_the_cpp_vectors() {
    // every `[0x.., 0x..]` literal in Payload.lean, as lowercase hex
    let mut lean: BTreeSet<String> = BTreeSet::new();
    let mut rest = LEAN_PAYLOAD;
    while let Some(i) = rest.find("[0x") {
        rest = &rest[i..];
        let end = rest.find(']').unwrap();
        let hex: String = rest[1..end]
            .split(',')
            .map(|t| t.trim().trim_start_matches("0x").to_string())
            .map(|t| if t.len() == 1 { format!("0{t}") } else { t })
            .collect();
        lean.insert(hex);
        rest = &rest[end..];
    }
    let rows: std::collections::BTreeMap<&str, &str> = PAYLOAD_VECTORS.lines().filter_map(|l| l.split_once('\t')).collect();
    for name in [
        "simple_send/basic",
        "send_all/basic",
        "send_many/basic",
        "send_many/zero_amount",
        "activate_feature/basic",
        "issuance_fixed/nokyc",
        "issuance_fixed/basic",
        "contract_dex_trade/basic",
    ] {
        assert!(lean.contains(rows[name]), "Lean has no byte literal equal to C++ vector {name}");
    }
}
