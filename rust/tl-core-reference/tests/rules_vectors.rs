//! Differential test: fixtures/rules_vectors.tsv was printed by the UNMODIFIED src/tradelayer/rules.cpp
//! compiled against logging/alert/chain-params stand-ins (scripts/rules_vectors.cpp, scripts/shim_rules).

use std::collections::BTreeMap;
use tl_core_reference::rules::{ActivationState, ConsensusParams, Feature, Network, NEVER_BLOCK};

fn net(s: &str) -> Network {
    match s {
        "main" => Network::Main,
        "test" => Network::Test,
        "regtest" => Network::Regtest,
        _ => panic!("network {s}"),
    }
}

fn dump_params(p: &ConsensusParams) -> String {
    p.values().iter().map(i32::to_string).collect::<Vec<_>>().join(",")
}

fn dump_state(st: &ActivationState, ret: bool) -> String {
    let pending = st
        .pending
        .iter()
        .map(|p| format!("{}:{}:{}", p.feature_id, p.activation_block, p.min_client_version))
        .collect::<Vec<_>>()
        .join("|");
    format!("ret={};alerts={};pending={};params={}", u8::from(ret), st.alerts_raised, pending, dump_params(&st.params))
}

/// The Rust twin of the scenario block at the end of scripts/rules_vectors.cpp.
#[allow(unused_assignments)]
fn replay_scenarios() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut st = ActivationState::new(Network::Regtest);
    let mut k = 0;
    macro_rules! act {
        ($sc:expr, $id:expr, $blk:expr, $minv:expr, $txb:expr) => {{
            let ret = st.activate($id, $blk, $minv, $txb).as_bool();
            out.insert(format!("scn/{}/{}", $sc, k), dump_state(&st, ret));
            k += 1;
        }};
    }
    macro_rules! deact {
        ($sc:expr, $id:expr, $txb:expr) => {{
            let ret = matches!(st.deactivate($id, $txb), tl_core_reference::rules::DeactivateOutcome::Done);
            out.insert(format!("scn/{}/{}", $sc, k), dump_state(&st, ret));
            k += 1;
        }};
    }
    act!("regtest_fixed", 8, 104, 1, 100);
    act!("regtest_fixed", 8, 1101, 1, 100);
    act!("regtest_fixed", 8, 150, 1, 100);
    act!("regtest_fixed", 8, 400, 1, 250);
    deact!("regtest_fixed", 8, 100);
    deact!("regtest_fixed", 8, 250);
    deact!("regtest_fixed", 8, 250);
    act!("regtest_fixed", 8, 300, 1, 250);
    act!("regtest_fixed", 99, 300, 1, 250);
    act!("regtest_fixed", 0, 300, 1, 250);
    deact!("regtest_fixed", 99, 250);
    act!("regtest_fixed", 14, 300, 20_002_000, 250);
    act!("regtest_fixed", 15, 300, 20_002_001, 250);
    act!("regtest_fixed", 16, 300, 4_294_967_295, 250);

    st = ActivationState::new(Network::Regtest);
    k = 0;
    deact!("regtest_alias", 12, 250);
    act!("regtest_alias", 12, 400, 1, 250);
    deact!("regtest_alias", 28, 150);
    act!("regtest_alias", 28, 600, 1, 150);

    st = ActivationState::new(Network::Main);
    k = 0;
    act!("main_window", 5, 2_487_299, 1, 2_487_300);
    act!("main_window", 5, 2_487_300, 1, 2_487_300);
    act!("main_window", 14, 2_487_300, 1, 2_487_300);
    act!("main_window", 14, 99_999_999 + 2_487_300 + 1, 1, 2_487_300);
    act!("main_window", 14, -5, 1, 2_487_300);
    deact!("main_window", 14, 2_487_300);
    out
}

#[test]
fn rules_match_the_compiled_cpp() {
    let text = include_str!("../fixtures/rules_vectors.tsv");
    let scenarios = replay_scenarios();
    let mut restrictions: BTreeMap<&str, Vec<(usize, String)>> = BTreeMap::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();

    for line in text.lines().filter(|l| !l.is_empty()) {
        let (key, want) = line.split_once('\t').expect("tab separated");
        let parts: Vec<&str> = key.split('/').collect();
        *counts.entry(parts[0]).or_default() += 1;
        match parts[0] {
            "params" => {
                assert_eq!(dump_params(&ConsensusParams::new(net(parts[1]))), want, "{key}");
            }
            "restriction" => {
                restrictions.entry(parts[1]).or_default().push((parts[2].parse().unwrap(), want.to_string()));
            }
            "feature_name" => {
                let id: u16 = parts[1].parse().unwrap();
                let got = Feature::from_id(id).map_or("Unknown feature", Feature::display_name);
                assert_eq!(got, want, "{key}");
            }
            "is_feature_activated" => {
                let id: u16 = parts[2][1..].parse().unwrap();
                let blk: i32 = parts[3][1..].parse().unwrap();
                let got = ConsensusParams::new(net(parts[1])).is_feature_activated(id, blk);
                assert_eq!(u8::from(got).to_string(), want, "{key}");
            }
            "is_tx_allowed" => {
                let t: u16 = parts[2][1..].parse().unwrap();
                let v: u16 = parts[3][1..].parse().unwrap();
                let blk: i32 = parts[4][1..].parse().unwrap();
                let got = ConsensusParams::new(net(parts[1])).is_transaction_type_allowed(blk, t, v);
                assert_eq!(u8::from(got).to_string(), want, "{key}");
            }
            "scn" => {
                assert_eq!(scenarios.get(key).map(String::as_str), Some(want), "{key}");
            }
            other => panic!("unexpected key kind {other}"),
        }
    }

    for (n, rows) in restrictions {
        let rust = ConsensusParams::new(net(n)).restrictions();
        assert_eq!(rust.len(), rows.len(), "restriction count for {n}");
        for (i, want) in rows {
            let r = rust[i];
            let got = format!("{},{},{},{}", r.tx_type, r.tx_version, u8::from(r.allow_wildcard), r.activation_block);
            assert_eq!(got, want, "restriction {n}/{i}");
        }
    }

    // every scenario step in Rust has a C++ twin, and each kind of vector is non-trivially populated
    assert_eq!(counts["scn"], scenarios.len());
    assert_eq!(counts["params"], 3);
    assert_eq!(counts["restriction"], 3 * 54);
    assert_eq!(counts["feature_name"], 3 * 32);
    assert_eq!(counts["is_feature_activated"], 3 * 32 * 15);
    assert!(counts["is_tx_allowed"] > 10_000);
}

#[test]
fn documented_cpp_quirks_hold() {
    // Unknown feature ids are never active, but ActivateFeature still returns true (pending + alert).
    let mut st = ActivationState::new(Network::Regtest);
    let before = st.params.clone();
    let out = st.activate(99, 300, 1, 250);
    assert!(out.as_bool());
    assert_eq!(out, tl_core_reference::rules::ActivateOutcome::Accepted { supported: false });
    assert_eq!(st.params, before, "unknown feature must not touch the parameter block");
    assert_eq!(st.alerts_raised, 1);
    assert!(!st.params.is_feature_activated(99, i32::MAX));

    // CONTRACTDEX (12) is read from the node-reward height; PEGGED (28) from the vesting height.
    assert_ne!(Feature::Contractdex.read_param(), Feature::Contractdex.write_param());
    assert_ne!(Feature::PeggedCurrency.read_param(), Feature::PeggedCurrency.write_param());
    for f in Feature::ALL {
        if !matches!(f, Feature::Contractdex | Feature::PeggedCurrency) {
            assert_eq!(f.read_param(), f.write_param(), "{}", f.cpp_name());
        }
    }

    // Activation/deactivation payloads are built with version 1 but the restriction table lists
    // version 0xFFFF, so IsTransactionTypeAllowed refuses them at every height.
    let reg = ConsensusParams::new(Network::Regtest);
    for ty in [65533u16, 65534] {
        assert!(!reg.is_transaction_type_allowed(i32::MAX, ty, 1));
        assert!(reg.is_transaction_type_allowed(i32::MAX, ty, 0xFFFF));
    }
    // unrecognised network names fall back to mainnet
    assert_eq!(ConsensusParams::new(Network::from_id_string("regression")), ConsensusParams::new(Network::Main));

    // a deactivated feature reads as never-active
    let mut st = ActivationState::new(Network::Regtest);
    assert!(matches!(st.deactivate(8, 250), tl_core_reference::rules::DeactivateOutcome::Done));
    assert_eq!(st.params.get(Feature::Fixed.write_param()), NEVER_BLOCK);
}

#[test]
fn pending_activations_complete_at_their_height() {
    let mut st = ActivationState::new(Network::Regtest);
    assert!(st.activate(14, 300, 1, 250).as_bool());
    assert!(st.activate(15, 500, 20_002_001, 250).as_bool()); // needs a newer client than TL_VERSION
    let early = st.complete_live(299);
    assert!(early.completed.is_empty() && !early.shutdown);
    let at300 = st.complete_live(300);
    assert_eq!(at300.completed, vec![14]);
    assert!(!at300.shutdown);
    let at500 = st.complete_live(500);
    assert_eq!(at500.completed, vec![15]);
    assert!(at500.shutdown);
}
