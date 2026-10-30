//! Source-derived parity: parses the C++ files in this repository at test time and checks the Rust
//! tables against them, so a change to the C++ constants or builders fails here instead of drifting
//! silently. (The compiled vectors in the other tests pin behaviour; these pin the source text.)

use std::collections::{BTreeMap, BTreeSet};
use tl_core_reference::rules::{ConsensusParams, Feature, Network, Param};
use tl_core_reference::types::TYPE_TABLE;
use tl_core_reference::CREATABLE;

const RULES_CPP: &str = include_str!("../../../src/tradelayer/rules.cpp");
const RULES_H: &str = include_str!("../../../src/tradelayer/rules.h");
const TRADELAYER_H: &str = include_str!("../../../src/tradelayer/tradelayer.h");
const CREATEPAYLOAD_CPP: &str = include_str!("../../../src/tradelayer/createpayload.cpp");
const TX_CPP: &str = include_str!("../../../src/tradelayer/tx.cpp");

fn leading_number(s: &str) -> u64 {
    let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or_else(|_| panic!("no number in {s:?}"))
}

/// Text from the first line containing `start` up to (excluding) the next line that is exactly "}".
fn function_body<'a>(src: &'a str, start: &str) -> &'a str {
    let from = src.find(start).unwrap_or_else(|| panic!("{start} not found"));
    let rest = &src[from..];
    let end = rest.find("\n}\n").map_or(rest.len(), |e| e + 2);
    &rest[..end]
}

#[test]
fn feature_ids_match_rules_h() {
    let mut cpp: BTreeMap<String, u64> = BTreeMap::new();
    for line in RULES_H.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("const uint16_t") {
            let (name, value) = rest.split_once('=').unwrap();
            let name = name.trim();
            if name.starts_with("FEATURE_") {
                cpp.insert(name.to_string(), leading_number(value));
            }
        }
    }
    let rust: BTreeMap<String, u64> = Feature::ALL.iter().map(|f| (f.cpp_name().to_string(), u64::from(f.id()))).collect();
    assert_eq!(rust, cpp);
}

#[test]
fn transaction_type_table_matches_tradelayer_h() {
    let from = TRADELAYER_H.find("enum TransactionType").unwrap();
    let body = &TRADELAYER_H[from..];
    let body = &body[..body.find("};").unwrap()];
    let mut cpp: BTreeMap<String, u64> = BTreeMap::new();
    for line in body.lines().skip(1) {
        let l = line.trim();
        if let Some((name, value)) = l.split_once('=') {
            cpp.insert(name.trim().to_string(), leading_number(value));
        }
    }
    let rust: BTreeMap<String, u64> = TYPE_TABLE.iter().map(|t| (t.cpp_name.to_string(), u64::from(t.id))).collect();
    assert_eq!(rust, cpp);

    // `dispatched` and `creatable` flags against interpret_Transaction / createpayload.cpp
    let dispatch = function_body(TX_CPP, "bool CMPTransaction::interpret_Transaction()");
    for t in TYPE_TABLE {
        let case = format!("case {}:", t.cpp_name);
        assert_eq!(dispatch.contains(&case), t.dispatched, "dispatch flag for {}", t.cpp_name);
    }
    let creatable_ids: BTreeSet<u16> = CREATABLE.iter().map(|(id, _, _)| *id).collect();
    for t in TYPE_TABLE {
        assert_eq!(creatable_ids.contains(&t.id), t.creatable, "creatable flag for {}", t.cpp_name);
    }
}

#[test]
fn feature_to_parameter_maps_match_rules_cpp() {
    fn collect(body: &str, mut extract: impl FnMut(&str) -> Option<String>) -> BTreeMap<String, String> {
        let mut cur: Option<String> = None;
        let mut out = BTreeMap::new();
        for line in body.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("case FEATURE_") {
                cur = Some(format!("FEATURE_{}", rest.trim_end_matches(':').trim()));
            } else if let (Some(c), Some(field)) = (&cur, extract(l)) {
                assert!(out.insert(c.clone(), field).is_none(), "{c} assigned twice");
            }
        }
        out
    }
    let activate = collect(function_body(RULES_CPP, "bool ActivateFeature("), |l| {
        l.strip_prefix("params.")
            .and_then(|r| r.split_once('='))
            .and_then(|(f, v)| (v.trim() == "activationBlock;").then(|| f.trim().to_string()))
    });
    let deactivate = collect(function_body(RULES_CPP, "bool DeactivateFeature("), |l| {
        l.strip_prefix("MutableConsensusParams().")
            .and_then(|r| r.split_once('='))
            .and_then(|(f, v)| (v.trim() == "99999999;").then(|| f.trim().to_string()))
    });
    let read = collect(function_body(RULES_CPP, "bool IsFeatureActivated("), |l| {
        l.strip_prefix("activationBlock = params.").map(|f| f.trim_end_matches(';').trim().to_string())
    });
    assert_eq!(activate.len(), 28);
    assert_eq!(read.len(), 28);
    for f in Feature::ALL {
        let n = f.cpp_name().to_string();
        assert_eq!(read[&n], f.read_param().cpp_name(), "read {n}");
        assert_eq!(activate[&n], f.write_param().cpp_name(), "activate {n}");
        assert_eq!(deactivate[&n], f.write_param().cpp_name(), "deactivate {n}");
    }
}

#[test]
fn consensus_constructors_match_rules_cpp() {
    let sections = [
        (Network::Main, "CMainConsensusParams::CMainConsensusParams()"),
        (Network::Test, "CTestNetConsensusParams::CTestNetConsensusParams()"),
        (Network::Regtest, "CRegTestConsensusParams::CRegTestConsensusParams()"),
    ];
    for (network, sig) in sections {
        let from = RULES_CPP.find(sig).unwrap();
        let rest = &RULES_CPP[from..];
        let end = rest.find("ONE_YEAR").unwrap();
        let mut parsed: BTreeMap<&str, i32> = BTreeMap::new();
        let section = &rest[..end + rest[end..].find('\n').unwrap()];
        for line in section.lines().skip(1) {
            let l = line.trim();
            if l.starts_with("//") || l.starts_with('*') || l.starts_with("/*") {
                continue;
            }
            if let Some((name, value)) = l.split_once('=') {
                let name = name.trim();
                if Param::from_cpp_name(name).is_some() {
                    assert!(parsed.insert(name, leading_number(value) as i32).is_none(), "{name} assigned twice in {sig}");
                }
            }
        }
        let rust = ConsensusParams::new(network);
        assert_eq!(parsed.len(), Param::ALL.len(), "{sig} must assign every parameter");
        for p in Param::ALL {
            assert_eq!(parsed[p.cpp_name()], rust.get(*p), "{:?} {}", network, p.cpp_name());
        }
    }
}

#[test]
fn builders_match_createpayload_cpp() {
    // C++ function suffix -> Rust variant name
    let map: BTreeMap<&str, &str> = BTreeMap::from([
        ("SimpleSend", "SimpleSend"),
        ("SendMany", "SendMany"),
        ("SendVestingTokens", "SendVestingTokens"),
        ("SendAll", "SendAll"),
        ("DExSell", "DExSell"),
        ("DEx", "DEx"),
        ("DExAccept", "DExAccept"),
        ("IssuanceFixed", "IssuanceFixed"),
        ("IssuanceManaged", "IssuanceManaged"),
        ("Grant", "Grant"),
        ("Revoke", "Revoke"),
        ("ChangeIssuer", "ChangeIssuer"),
        ("DeactivateFeature", "DeactivateFeature"),
        ("ActivateFeature", "ActivateFeature"),
        ("TradeLayerAlert", "Alert"),
        ("CreateContract", "CreateContract"),
        ("ContractDexTrade", "ContractDexTrade"),
        ("ContractDexCancelAll", "ContractDexCancelAll"),
        ("ContractDexClosePosition", "ContractDexClosePosition"),
        ("ContractDexCancelOrderByTxId", "ContractDexCancelOrderByTxId"),
        ("IssuancePegged", "IssuancePegged"),
        ("SendPeggedCurrency", "SendPeggedCurrency"),
        ("RedemptionPegged", "RedemptionPegged"),
        ("MetaDExTrade", "MetaDExTrade"),
        ("CreateOracleContract", "CreateOracleContract"),
        ("Change_OracleAdm", "ChangeOracleAdm"),
        ("Set_Oracle", "SetOracle"),
        ("OracleBackup", "OracleBackup"),
        ("Close_Oracle", "CloseOracle"),
        ("Commit_Channel", "CommitChannel"),
        ("Withdrawal_FromChannel", "WithdrawalFromChannel"),
        ("Instant_Trade", "InstantTrade"),
        ("Contract_Instant_Trade", "ContractInstantTrade"),
        ("PNL_Update", "PnlUpdate"),
        ("Transfer", "Transfer"),
        ("Instant_LTC_Trade", "InstantLtcTrade"),
        ("New_Id_Registration", "NewIdRegistration"),
        ("Update_Id_Registration", "UpdateIdRegistration"),
        ("DEx_Payment", "DExPayment"),
        ("Attestation", "Attestation"),
        ("Revoke_Attestation", "RevokeAttestation"),
        ("MetaDExCancelAll", "MetaDExCancelAll"),
        ("ContractDExCancel", "ContractDExCancel"),
        ("DExCancel", "DExCancel"),
        ("MetaDExCancelPair", "MetaDExCancelPair"),
        ("MetaDExCancelPrice", "MetaDExCancelPrice"),
        ("Close_Channel", "CloseChannel"),
        ("SubmitNodeAddress", "SubmitNodeAddress"),
        ("ClaimNodeReward", "ClaimNodeReward"),
        ("SendDonation", "SendDonation"),
    ]);

    // Split the file into function bodies at each definition.
    let mut defs: BTreeMap<String, &str> = BTreeMap::new();
    let marker = "CreatePayload_";
    let mut idx = 0;
    let starts: Vec<usize> = CREATEPAYLOAD_CPP.match_indices("\nstd::vector<unsigned char>").map(|(i, _)| i).collect();
    for (n, &s) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(CREATEPAYLOAD_CPP.len());
        let chunk = &CREATEPAYLOAD_CPP[s..end];
        let at = chunk.find(marker).unwrap() + marker.len();
        let name: String = chunk[at..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        assert!(defs.insert(name, chunk).is_none());
        idx += 1;
    }
    assert_eq!(idx, map.len(), "number of defined CreatePayload_ functions changed");
    assert_eq!(defs.keys().map(String::as_str).collect::<BTreeSet<_>>(), map.keys().copied().collect::<BTreeSet<_>>());

    for (cpp, variant) in &map {
        let chunk = defs[*cpp];
        let num = |key: &str| -> u64 {
            let at = chunk.find(key).unwrap_or_else(|| panic!("{cpp}: no {key}")) + key.len();
            leading_number(&chunk[at..])
        };
        let (ty, ver) = (num("messageType = "), num("messageVer = "));
        let (rty, rver, _) = CREATABLE.iter().find(|(_, _, n)| n == variant).unwrap_or_else(|| panic!("no variant {variant}"));
        assert_eq!((u64::from(*rty), u64::from(*rver)), (ty, ver), "{cpp}");
    }
    assert_eq!(CREATABLE.len(), map.len());
}
