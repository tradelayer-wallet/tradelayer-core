//! Differential test: every row of fixtures/payload_vectors.tsv was printed by the unmodified C++
//! `createpayload.cpp` / `varint.cpp` (scripts/build-vectors.sh). The Rust encoder must produce the
//! same bytes and the strict decoder must read them back.

use std::collections::BTreeMap;
use tl_core_reference::varint;
use tl_core_reference::{decode, encode, to_hex, DecodeError, Payload, Str255};

fn s(x: &str) -> Str255 {
    Str255::try_from(x).unwrap()
}

fn rep(c: u8, n: usize) -> Str255 {
    Str255::truncating(&vec![c; n]).unwrap()
}

fn rows() -> BTreeMap<String, String> {
    let text = include_str!("../fixtures/payload_vectors.tsv");
    let mut m = BTreeMap::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (k, v) = line.split_once('\t').expect("tab separated");
        assert!(m.insert(k.to_string(), v.to_string()).is_none(), "duplicate row {k}");
    }
    m
}

/// The Rust twin of the inputs in scripts/vectors.cpp, keyed by the same names.
fn cases() -> Vec<(&'static str, Payload)> {
    use Payload::*;
    let h = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    vec![
        ("simple_send/basic", SimpleSend { property_id: 7, amount: 100_000_000 }),
        ("simple_send/zero", SimpleSend { property_id: 0, amount: 0 }),
        ("simple_send/max", SimpleSend { property_id: u32::MAX, amount: u64::MAX }),
        ("send_many/basic", SendMany { property_id: 3, amounts: vec![1, 2, 300] }),
        ("send_many/empty", SendMany { property_id: 3, amounts: vec![] }),
        ("send_many/zero_amount", SendMany { property_id: 3, amounts: vec![5, 0, 7] }),
        ("send_vesting/basic", SendVestingTokens { amount: 123_456_789 }),
        ("send_all/basic", SendAll {}),
        (
            "issuance_fixed/basic",
            IssuanceFixed {
                property_type: 2,
                previous_property_id: 0,
                name: s("TOK"),
                url: s("https://example.invalid"),
                data: s("meta"),
                amount: 21_000_000,
                kyc: vec![0, 1, 2],
            },
        ),
        (
            "issuance_fixed/nokyc",
            IssuanceFixed { property_type: 1, previous_property_id: 5, name: s("T"), url: s(""), data: s(""), amount: 1, kyc: vec![] },
        ),
        (
            "issuance_fixed/long_strings",
            IssuanceFixed {
                property_type: 2,
                previous_property_id: 0,
                name: rep(b'a', 300),
                url: rep(b'b', 256),
                data: rep(b'c', 255),
                amount: 5,
                kyc: vec![7],
            },
        ),
        (
            "issuance_fixed/negative_kyc",
            IssuanceFixed { property_type: 2, previous_property_id: 0, name: s("N"), url: s("u"), data: s("d"), amount: 5, kyc: vec![-1] },
        ),
        (
            "issuance_managed/basic",
            IssuanceManaged {
                property_type: 2,
                previous_property_id: 0,
                name: s("MAN"),
                url: s("https://example.invalid/m"),
                data: s("managed"),
                kyc: vec![3, 4],
            },
        ),
        ("grant/basic", Grant { property_id: 5, amount: 1000 }),
        ("revoke/basic", Revoke { property_id: 5, amount: 500 }),
        ("change_issuer/basic", ChangeIssuer { property_id: 9 }),
        ("deactivate_feature/basic", DeactivateFeature { feature_id: 8 }),
        ("activate_feature/basic", ActivateFeature { feature_id: 8, activation_block: 400, min_client_version: 1 }),
        ("activate_feature/max", ActivateFeature { feature_id: 28, activation_block: u32::MAX, min_client_version: u32::MAX }),
        ("alert/basic", Alert { alert_type: 1, alert_expiry: 1000, message: s("hello") }),
        ("alert/clear", Alert { alert_type: 65535, alert_expiry: 0, message: s("") }),
        (
            "create_contract/basic",
            CreateContract {
                numerator: 1,
                denominator: 1,
                name: s("ALL-LTC"),
                blocks_until_expiration: 0,
                notional_size: 1,
                collateral_currency: 1,
                margin_requirement: 100_000_000,
                inverse: 1,
                kyc: vec![0, 2],
            },
        ),
        (
            "create_contract/expiring",
            CreateContract {
                numerator: 3,
                denominator: 2,
                name: s("BTC-USD"),
                blocks_until_expiration: 4032,
                notional_size: 10,
                collateral_currency: 4,
                margin_requirement: 250_000,
                inverse: 0,
                kyc: vec![],
            },
        ),
        (
            "contract_dex_trade/basic",
            ContractDexTrade {
                contract_name: s("ALL-LTC"),
                amount_for_sale: 100,
                effective_price: 25_000_000,
                trading_action: 1,
                leverage: 10,
            },
        ),
        ("contract_dex_cancel_all/basic", ContractDexCancelAll { contract_id: 3 }),
        ("contract_dex_cancel_by_txid/basic", ContractDexCancelOrderByTxId { block: 1234, idx: 5 }),
        ("contract_dex_cancel_by_txid/negative_block", ContractDexCancelOrderByTxId { block: -1, idx: 0 }),
        ("contract_dex_close_position/basic", ContractDexClosePosition { contract_id: 3 }),
        (
            "issuance_pegged/basic",
            IssuancePegged { property_type: 8, previous_property_id: 0, name: s("sLTC"), property_id: 5, contract_id: 3, amount: 1000 },
        ),
        ("redemption_pegged/basic", RedemptionPegged { property_id: 5, contract_id: 3, amount: 1000 }),
        ("send_pegged/basic", SendPeggedCurrency { property_id: 5, amount: 1000 }),
        (
            "dex_sell/basic",
            DExSell {
                property_id: 4,
                amount_for_sale: 100_000_000,
                amount_desired: 5_000_000,
                time_limit: 10,
                min_fee: 1000,
                sub_action: 1,
            },
        ),
        ("dex_buy/basic", DEx { property_id: 4, amount: 100_000_000, price: 123_456, time_limit: 10, min_fee: 1000, sub_action: 2 }),
        ("dex_accept/basic", DExAccept { property_id: 4, amount: 777 }),
        (
            "metadex_trade/basic",
            MetaDExTrade { property_id_for_sale: 4, amount_for_sale: 1000, property_id_desired: 5, amount_desired: 2000 },
        ),
        (
            "metadex_cancel_price/basic",
            MetaDExCancelPrice { property_id_for_sale: 4, amount_for_sale: 1000, property_id_desired: 5, amount_desired: 2000 },
        ),
        (
            "metadex_cancel_price/negative",
            MetaDExCancelPrice { property_id_for_sale: 4, amount_for_sale: -1, property_id_desired: 5, amount_desired: 2000 },
        ),
        ("metadex_cancel_pair/basic", MetaDExCancelPair { property_id_for_sale: 4, property_id_desired: 5 }),
        ("metadex_cancel_all/basic", MetaDExCancelAll {}),
        (
            "create_oracle_contract/basic",
            CreateOracleContract {
                name: s("ORACLE1"),
                blocks_until_expiration: 0,
                notional_size: 1,
                collateral_currency: 1,
                margin_requirement: 100_000,
                inverse: 1,
                kyc: vec![1],
            },
        ),
        ("change_oracle_adm/basic", ChangeOracleAdm { contract_id: 3 }),
        ("set_oracle/basic", SetOracle { contract_id: 3, high: 30000, low: 29000, close: 29500 }),
        ("oracle_backup/basic", OracleBackup { contract_id: 3 }),
        ("close_oracle/basic", CloseOracle { contract_id: 3 }),
        ("commit_channel/basic", CommitChannel { property_id: 4, amount: 5000 }),
        ("withdrawal_from_channel/basic", WithdrawalFromChannel { property_id: 4, amount: 5000 }),
        (
            "instant_trade/basic",
            InstantTrade { property_id: 4, amount: 100, blockheight_expiry: 5000, property_desired: 5, amount_desired: 200 },
        ),
        (
            "instant_trade/negative_expiry",
            InstantTrade { property_id: 4, amount: 100, blockheight_expiry: -1, property_desired: 5, amount_desired: 200 },
        ),
        (
            "contract_instant_trade/basic",
            ContractInstantTrade {
                contract_id: 3,
                amount: 10,
                blockheight_expiry: 5000,
                price: 25_000_000,
                trading_action: 1,
                leverage: 5,
            },
        ),
        ("pnl_update/basic", PnlUpdate { property_id: 4, amount: 300, blockheight_expiry: 5000 }),
        ("transfer/basic", Transfer { option: 1, property_id: 4, amount: 250 }),
        ("instant_ltc_trade/basic", InstantLtcTrade { property_id: 4, amount: 100, total_price: 12345, blockheight_expiry: 5000 }),
        ("new_id_registration/basic", NewIdRegistration { website: s("example.invalid"), name: s("Example Co") }),
        ("update_id_registration/basic", UpdateIdRegistration {}),
        ("dex_payment/basic", DExPayment {}),
        ("attestation/basic", Attestation { hash: s(h) }),
        ("contract_dex_cancel/basic", ContractDExCancel { hash: s(h) }),
        ("dex_cancel/basic", DExCancel { hash: s(h) }),
        ("revoke_attestation/basic", RevokeAttestation {}),
        ("close_channel/basic", CloseChannel {}),
        ("submit_node_address/basic", SubmitNodeAddress {}),
        ("claim_node_reward/basic", ClaimNodeReward {}),
        ("send_donation/basic", SendDonation { property_id: 4, amount: 42 }),
    ]
}

#[test]
fn encode_matches_cpp_bytes() {
    let rows = rows();
    for (name, p) in cases() {
        let want = rows.get(name).unwrap_or_else(|| panic!("no C++ vector for {name}"));
        assert_eq!(&to_hex(&encode(&p)), want, "{name}");
    }
}

#[test]
fn decode_reads_cpp_bytes_back() {
    let rows = rows();
    for (name, p) in cases() {
        let bytes = tl_core_reference::from_hex(&rows[name]).unwrap();
        match name {
            // The C++ parser silently ends the list at the first zero amount; the strict decoder
            // refuses instead (documented unresolved question).
            "send_many/zero_amount" => assert_eq!(decode(&bytes), Err(DecodeError::ZeroListAmount)),
            _ => assert_eq!(decode(&bytes), Ok(p), "{name}"),
        }
    }
}

#[test]
fn vector_table_and_case_table_cover_each_other() {
    let rows = rows();
    let mut names: Vec<&str> = cases().into_iter().map(|(n, _)| n).collect();
    names.extend(rows.keys().filter(|k| k.starts_with("varint")).map(String::as_str));
    for k in rows.keys() {
        assert!(names.contains(&k.as_str()), "C++ vector {k} has no Rust twin");
    }
    // Every creatable payload kind is exercised by at least one vector.
    let covered: Vec<&str> = cases().iter().map(|(_, p)| p.name()).collect();
    for (_, _, variant) in tl_core_reference::CREATABLE {
        assert!(covered.contains(variant), "variant {variant} has no differential vector");
    }
}

#[test]
fn varint_vectors() {
    let rows = rows();
    for v in [0u64, 1, 127, 128, 255, 300, 16383, 16384, 4_294_967_295, 4_294_967_296, 1 << 63, u64::MAX] {
        assert_eq!(&to_hex(&varint::compress(v)), &rows[&format!("varint/{v}")]);
    }
    // C++ DecompressInteger on the non-canonical / oversized forms the strict reader rejects.
    let lenient: [(&str, &[u8]); 5] = [
        ("varint_decompress/80_00", &[0x80, 0x00]),
        ("varint_decompress/ff_00", &[0xff, 0x00]),
        ("varint_decompress/ff_ff_7f", &[0xff, 0xff, 0x7f]),
        ("varint_decompress/max10", &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]),
        ("varint_decompress/ten_high", &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f]),
    ];
    for (name, bytes) in lenient {
        let want: u64 = rows[name].parse().unwrap();
        assert_eq!(varint::decompress_cpp(bytes), Some(want), "{name}");
    }
    // The strict reader refuses exactly the forms where C++ and canonical encoding disagree.
    let mut pos = 0;
    assert!(varint::read_strict(&[0x80, 0x00], &mut pos).is_err());
    assert!(varint::read_strict(&[0xff, 0xff, 0x7f], &mut pos).is_ok());
    pos = 0;
    assert!(varint::read_strict(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f], &mut pos).is_err());
}
