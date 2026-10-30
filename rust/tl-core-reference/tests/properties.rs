//! Property and malformed-input tests for the payload layer (deterministic PRNG, no dependencies).

use std::collections::HashMap;
use tl_core_reference::varint::{self, VarintError};
use tl_core_reference::{decode, encode, try_encode, DecodeError, EncodeError, Payload, Str255, CREATABLE, MAX_PAYLOAD_LEN};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Values biased to varint length boundaries.
    fn u64(&mut self) -> u64 {
        let bits = self.next() % 65;
        if bits == 0 {
            0
        } else {
            self.next() >> (64 - bits)
        }
    }
    fn u32(&mut self) -> u32 {
        self.u64() as u32
    }
    fn u16(&mut self) -> u16 {
        self.u64() as u16
    }
    fn u8(&mut self) -> u8 {
        self.u64() as u8
    }
    fn i32(&mut self) -> i32 {
        self.u64() as i32
    }
    fn i64(&mut self) -> i64 {
        self.u64() as i64
    }
    fn string(&mut self) -> Str255 {
        let n = (self.next() % 40) as usize;
        let bytes: Vec<u8> = (0..n).map(|_| 1 + (self.next() % 255) as u8).collect();
        Str255::new(&bytes).unwrap()
    }
    fn kyc(&mut self) -> Vec<i32> {
        (0..self.next() % 4).map(|_| self.i32()).collect()
    }
    fn amounts(&mut self) -> Vec<u64> {
        (0..self.next() % 4).map(|_| self.u64().max(1)).collect()
    }
}

/// One random payload of the variant at `idx` in CREATABLE order.
fn random_payload(r: &mut Rng, name: &str) -> Payload {
    use Payload::*;
    match name {
        "SimpleSend" => SimpleSend { property_id: r.u32(), amount: r.u64() },
        "SendMany" => SendMany { property_id: r.u32(), amounts: r.amounts() },
        "SendAll" => SendAll {},
        "SendVestingTokens" => SendVestingTokens { amount: r.u64() },
        "DExSell" => DExSell {
            property_id: r.u32(),
            amount_for_sale: r.u64(),
            amount_desired: r.u64(),
            time_limit: r.u8(),
            min_fee: r.u64(),
            sub_action: r.u8(),
        },
        "DEx" => DEx { property_id: r.u32(), amount: r.u64(), price: r.u64(), time_limit: r.u8(), min_fee: r.u64(), sub_action: r.u8() },
        "DExAccept" => DExAccept { property_id: r.u32(), amount: r.u64() },
        "MetaDExTrade" => {
            MetaDExTrade { property_id_for_sale: r.u32(), amount_for_sale: r.u64(), property_id_desired: r.u32(), amount_desired: r.u64() }
        }
        "MetaDExCancelAll" => MetaDExCancelAll {},
        "ContractDexTrade" => ContractDexTrade {
            contract_name: r.string(),
            amount_for_sale: r.u64(),
            effective_price: r.u64(),
            trading_action: r.u8(),
            leverage: r.u64(),
        },
        "ContractDExCancel" => ContractDExCancel { hash: r.string() },
        "ContractDexCancelAll" => ContractDexCancelAll { contract_id: r.u32() },
        "ContractDexClosePosition" => ContractDexClosePosition { contract_id: r.u32() },
        "ContractDexCancelOrderByTxId" => ContractDexCancelOrderByTxId { block: r.i32(), idx: r.u32() },
        "DExCancel" => DExCancel { hash: r.string() },
        "MetaDExCancelPair" => MetaDExCancelPair { property_id_for_sale: r.u32(), property_id_desired: r.u32() },
        "MetaDExCancelPrice" => MetaDExCancelPrice {
            property_id_for_sale: r.u32(),
            amount_for_sale: r.i64(),
            property_id_desired: r.u32(),
            amount_desired: r.i64(),
        },
        "CreateContract" => CreateContract {
            numerator: r.u32(),
            denominator: r.u32(),
            name: r.string(),
            blocks_until_expiration: r.u32(),
            notional_size: r.u32(),
            collateral_currency: r.u32(),
            margin_requirement: r.u64(),
            inverse: r.u8(),
            kyc: r.kyc(),
        },
        "IssuanceFixed" => IssuanceFixed {
            property_type: r.u16(),
            previous_property_id: r.u32(),
            name: r.string(),
            url: r.string(),
            data: r.string(),
            amount: r.u64(),
            kyc: r.kyc(),
        },
        "IssuanceManaged" => IssuanceManaged {
            property_type: r.u16(),
            previous_property_id: r.u32(),
            name: r.string(),
            url: r.string(),
            data: r.string(),
            kyc: r.kyc(),
        },
        "Grant" => Grant { property_id: r.u32(), amount: r.u64() },
        "Revoke" => Revoke { property_id: r.u32(), amount: r.u64() },
        "ChangeIssuer" => ChangeIssuer { property_id: r.u32() },
        "IssuancePegged" => IssuancePegged {
            property_type: r.u16(),
            previous_property_id: r.u32(),
            name: r.string(),
            property_id: r.u32(),
            contract_id: r.u32(),
            amount: r.u64(),
        },
        "RedemptionPegged" => RedemptionPegged { property_id: r.u32(), contract_id: r.u32(), amount: r.u64() },
        "SendPeggedCurrency" => SendPeggedCurrency { property_id: r.u32(), amount: r.u64() },
        "CreateOracleContract" => CreateOracleContract {
            name: r.string(),
            blocks_until_expiration: r.u32(),
            notional_size: r.u32(),
            collateral_currency: r.u32(),
            margin_requirement: r.u64(),
            inverse: r.u8(),
            kyc: r.kyc(),
        },
        "ChangeOracleAdm" => ChangeOracleAdm { contract_id: r.u32() },
        "SetOracle" => SetOracle { contract_id: r.u32(), high: r.u64(), low: r.u64(), close: r.u64() },
        "OracleBackup" => OracleBackup { contract_id: r.u32() },
        "CloseOracle" => CloseOracle { contract_id: r.u32() },
        "CommitChannel" => CommitChannel { property_id: r.u32(), amount: r.u64() },
        "WithdrawalFromChannel" => WithdrawalFromChannel { property_id: r.u32(), amount: r.u64() },
        "InstantTrade" => InstantTrade {
            property_id: r.u32(),
            amount: r.u64(),
            blockheight_expiry: r.i32(),
            property_desired: r.u32(),
            amount_desired: r.u64(),
        },
        "PnlUpdate" => PnlUpdate { property_id: r.u32(), amount: r.u64(), blockheight_expiry: r.u32() },
        "Transfer" => Transfer { option: r.u8(), property_id: r.u32(), amount: r.u64() },
        "InstantLtcTrade" => InstantLtcTrade { property_id: r.u32(), amount: r.u64(), total_price: r.u64(), blockheight_expiry: r.i32() },
        "ContractInstantTrade" => ContractInstantTrade {
            contract_id: r.u32(),
            amount: r.u64(),
            blockheight_expiry: r.u32(),
            price: r.u64(),
            trading_action: r.u8(),
            leverage: r.u64(),
        },
        "NewIdRegistration" => NewIdRegistration { website: r.string(), name: r.string() },
        "UpdateIdRegistration" => UpdateIdRegistration {},
        "DExPayment" => DExPayment {},
        "Attestation" => Attestation { hash: r.string() },
        "RevokeAttestation" => RevokeAttestation {},
        "CloseChannel" => CloseChannel {},
        "SubmitNodeAddress" => SubmitNodeAddress {},
        "ClaimNodeReward" => ClaimNodeReward {},
        "SendDonation" => SendDonation { property_id: r.u32(), amount: r.u64() },
        "DeactivateFeature" => DeactivateFeature { feature_id: r.u16() },
        "ActivateFeature" => ActivateFeature { feature_id: r.u16(), activation_block: r.u32(), min_client_version: r.u32() },
        "Alert" => Alert { alert_type: r.u16(), alert_expiry: r.u32(), message: r.string() },
        other => panic!("generator is missing variant {other}"),
    }
}

fn corpus(per_variant: usize) -> Vec<Payload> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut out = Vec::new();
    for (_, _, name) in CREATABLE {
        for _ in 0..per_variant {
            out.push(random_payload(&mut r, name));
        }
    }
    out
}

/// Whether the variant ends in a length-less list (so a truncated tail is another valid payload).
fn has_list_tail(p: &Payload) -> bool {
    matches!(
        p,
        Payload::SendMany { .. }
            | Payload::IssuanceFixed { .. }
            | Payload::IssuanceManaged { .. }
            | Payload::CreateContract { .. }
            | Payload::CreateOracleContract { .. }
    )
}

#[test]
fn generator_covers_every_creatable_variant_and_header_is_version_then_type() {
    let c = corpus(1);
    assert_eq!(c.len(), CREATABLE.len());
    for p in &c {
        let bytes = encode(p);
        let mut pos = 0;
        assert_eq!(varint::read_strict(&bytes, &mut pos), Ok(u64::from(p.version())));
        assert_eq!(varint::read_strict(&bytes, &mut pos), Ok(u64::from(p.type_id())));
    }
}

#[test]
fn decode_inverts_encode() {
    for p in corpus(200) {
        assert_eq!(decode(&encode(&p)).as_ref(), Ok(&p), "{}", p.name());
    }
}

#[test]
fn encode_is_injective_on_the_corpus() {
    let mut seen: HashMap<Vec<u8>, Payload> = HashMap::new();
    for p in corpus(200) {
        let bytes = encode(&p);
        if let Some(prev) = seen.insert(bytes, p.clone()) {
            assert_eq!(prev, p, "two payloads share an encoding");
        }
    }
}

#[test]
fn decode_rejects_truncation_inside_the_fixed_part() {
    for p in corpus(60) {
        let bytes = encode(&p);
        for cut in 0..bytes.len() {
            let r = decode(&bytes[..cut]);
            if has_list_tail(&p) {
                // A prefix that ends on a field boundary of the list tail is a shorter, valid list:
                // the wire format has no length prefix (README, unresolved question).
                if let Ok(q) = r {
                    assert_eq!(q.name(), p.name());
                    assert_eq!(encode(&q), bytes[..cut]);
                }
            } else {
                assert!(r.is_err(), "{} accepted a {cut}-byte prefix of a {}-byte payload", p.name(), bytes.len());
            }
        }
    }
}

#[test]
fn accepted_inputs_are_canonical() {
    // decode(b) = Ok(p)  =>  encode(p) = b, on random bytes and on single-byte mutations of valid payloads.
    let mut r = Rng(0xDEAD_BEEF_CAFE_F00D);
    let mut accepted = 0usize;
    for p in corpus(40) {
        let bytes = encode(&p);
        for i in 0..bytes.len() {
            for delta in [1u8, 0x80, 0xff] {
                let mut m = bytes.clone();
                m[i] ^= delta;
                if let Ok(q) = decode(&m) {
                    accepted += 1;
                    assert_eq!(encode(&q), m);
                }
            }
        }
        for _ in 0..20 {
            let n = (r.next() % 24) as usize;
            let noise: Vec<u8> = (0..n).map(|_| r.next() as u8).collect();
            if let Ok(q) = decode(&noise) {
                accepted += 1;
                assert_eq!(encode(&q), noise);
            }
        }
    }
    assert!(accepted > 0);
}

#[test]
fn malformed_inputs_get_typed_errors() {
    let ok = encode(&Payload::SimpleSend { property_id: 1, amount: 2 });
    assert!(matches!(decode(&[]), Err(DecodeError::Varint { field: "version", err: VarintError::Truncated })));
    assert!(matches!(decode(&[0x00]), Err(DecodeError::Varint { field: "type", err: VarintError::Truncated })));
    // type 2 (restricted send) exists in the enum but has no builder or parser
    assert_eq!(decode(&[0x00, 0x02]), Err(DecodeError::UnsupportedType(2)));
    // type 3 is not in the enum at all
    assert_eq!(decode(&[0x00, 0x03]), Err(DecodeError::UnknownType(3)));
    // type above 16 bits
    assert_eq!(decode(&[0x00, 0x80, 0x80, 0x04]), Err(DecodeError::UnknownType(65536)));
    // wrong version: activation is version 1, DExSell version 1, simple send version 0
    assert_eq!(
        decode(&[0x00, 0xfe, 0xff, 0x03, 0x08, 0x90, 0x03, 0x01]),
        Err(DecodeError::UnexpectedVersion { ty: 65534, got: 0, expected: 1 })
    );
    assert_eq!(decode(&[0x01, 0x00, 0x01, 0x02]), Err(DecodeError::UnexpectedVersion { ty: 0, got: 1, expected: 0 }));
    // trailing byte after a fixed-shape payload
    let mut t = ok.clone();
    t.push(0);
    assert_eq!(decode(&t), Err(DecodeError::TrailingBytes(1)));
    // property id does not fit u32 (C++ would truncate to 0)
    let mut big = vec![0x00, 0x00];
    varint::push_compressed(&mut big, 1 << 32);
    varint::push_compressed(&mut big, 5);
    assert_eq!(decode(&big), Err(DecodeError::FieldOutOfRange("property_id")));
    // activation feature id 65537 would alias feature 1 in C++ (uint16_t truncation)
    let mut alias = vec![0x01, 0xfe, 0xff, 0x03];
    varint::push_compressed(&mut alias, 65537);
    varint::push_compressed(&mut alias, 400);
    varint::push_compressed(&mut alias, 1);
    assert_eq!(decode(&alias), Err(DecodeError::FieldOutOfRange("feature_id")));
    // non-canonical varint
    assert!(matches!(decode(&[0x80, 0x00, 0x00]), Err(DecodeError::Varint { err: VarintError::NonCanonical, .. })));
    // i32 field that is not a sign-extended 32-bit value
    let mut b = vec![0x00, 34];
    varint::push_compressed(&mut b, 1 << 40);
    varint::push_compressed(&mut b, 0);
    assert_eq!(decode(&b), Err(DecodeError::FieldOutOfRange("block")));
}

#[test]
fn string_fields_are_bounded_and_terminated() {
    // attestation hash: unterminated
    assert_eq!(decode(&[0x00, 118, b'a', b'b']), Err(DecodeError::UnterminatedString("hash")));
    // 255 bytes is accepted, 256 is rejected (C++ would truncate to 255 and then mis-advance the cursor)
    let mut s255 = vec![0x00, 118];
    s255.extend(std::iter::repeat_n(b'x', 255));
    s255.push(0);
    assert!(decode(&s255).is_ok());
    let mut s256 = vec![0x00, 118];
    s256.extend(std::iter::repeat_n(b'x', 256));
    s256.push(0);
    assert_eq!(decode(&s256), Err(DecodeError::StringTooLong { field: "hash", len: 256 }));
    // The builder-side cut: truncating() reproduces substr(0, 255), new() refuses.
    assert!(Str255::new(&[b'x'; 256]).is_err());
    assert_eq!(Str255::truncating(&[b'x'; 256]).unwrap().len(), 255);
}

#[test]
fn zero_amount_in_send_many_is_not_wire_faithful() {
    // C++ builds this payload, but its parser stops the amount list at the zero (README).
    let p = Payload::SendMany { property_id: 3, amounts: vec![5, 0, 7] };
    assert_eq!(decode(&encode(&p)), Err(DecodeError::ZeroListAmount));
}

#[test]
fn size_limits() {
    let big = Payload::SendMany { property_id: 1, amounts: vec![u64::MAX; 7000] };
    assert!(matches!(try_encode(&big), Err(EncodeError::PayloadTooLarge(n)) if n > MAX_PAYLOAD_LEN));
    assert_eq!(decode(&vec![0; MAX_PAYLOAD_LEN + 1]), Err(DecodeError::PayloadTooLarge(MAX_PAYLOAD_LEN + 1)));
    let small = Payload::SendMany { property_id: 1, amounts: vec![3; 10] };
    assert_eq!(try_encode(&small), Ok(encode(&small)));
}

#[test]
fn headers_match_the_consensus_version_table_except_for_known_conflicts() {
    // Every builder version except activation/deactivation equals the version IsTransactionTypeAllowed
    // requires. (Activation/deactivation: builder writes 1, restriction table lists 0xFFFF; README.)
    use tl_core_reference::rules::{ConsensusParams, Network};
    let params = ConsensusParams::new(Network::Regtest);
    let table = params.restrictions();
    for (ty, ver, name) in CREATABLE {
        let rows: Vec<_> = table.iter().filter(|r| r.tx_type == *ty).collect();
        let listed = !rows.is_empty();
        let matching = rows.iter().any(|r| r.tx_version == *ver);
        match *ty {
            65533 | 65534 => assert!(listed && !matching, "{name}: documented version conflict disappeared"),
            // types the restriction table does not list at all can never pass IsTransactionTypeAllowed
            111 => assert!(!listed, "{name}"),
            _ => assert!(matching, "{name}: version {ver} not accepted by the restriction table"),
        }
    }
}
