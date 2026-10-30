//! `Payload`, `encode` and `decode`: the TradeLayer payload layer.
//!
//! Source of truth: `src/tradelayer/createpayload.cpp` (field order, widths, type and version
//! numbers) and the `interpret_*` functions in `src/tradelayer/tx.cpp` (parse order).
//!
//! Wire layout of every payload: `varint(version) varint(type) fields...`. Note that the version
//! comes FIRST. Integers are [`crate::varint`] values, strings are raw bytes followed by one NUL,
//! and the variable-length tails (KYC ids, SendMany amounts) have no length prefix: they run to
//! the end of the payload.

use crate::text::{Str255, MAX_STR_LEN};
use crate::varint::{self, VarintError};
use std::fmt;

/// Largest payload the node parses: `CMPTransaction::pkt` is `unsigned char[65535]`.
pub const MAX_PAYLOAD_LEN: usize = 65535;

/// Errors from [`decode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input is longer than [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge(usize),
    /// A varint field is malformed (truncated, too long, overflowing or non-canonical).
    Varint {
        /// Field being read.
        field: &'static str,
        /// What was wrong with it.
        err: VarintError,
    },
    /// The type number is outside the 16-bit range or not a type this crate decodes.
    UnknownType(u64),
    /// A type id that exists in the C++ enum but has no payload builder or no parser.
    UnsupportedType(u16),
    /// The version does not match the version the C++ builder writes for this type.
    UnexpectedVersion {
        /// Transaction type.
        ty: u16,
        /// Version found on the wire.
        got: u64,
        /// Version the builder writes.
        expected: u16,
    },
    /// A value does not fit the C++ field it is stored in (C++ would silently truncate).
    FieldOutOfRange(&'static str),
    /// A string has no NUL terminator before the end of the payload.
    UnterminatedString(&'static str),
    /// A string is longer than 255 bytes (C++ truncates it and then mis-advances the cursor).
    StringTooLong {
        /// Field being read.
        field: &'static str,
        /// Length found.
        len: usize,
    },
    /// A SendMany amount list contains zero (C++ silently ends the list there).
    ZeroListAmount,
    /// Bytes remain after the last field of a fixed-shape payload (C++ ignores them).
    TrailingBytes(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DecodeError {}

/// Errors from [`try_encode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The encoded payload would exceed [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge(usize),
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn varint(&mut self, field: &'static str) -> Result<u64, DecodeError> {
        varint::read_strict(self.bytes, &mut self.pos).map_err(|err| DecodeError::Varint { field, err })
    }
    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }
}

/// A payload field: how it is written by the C++ builder and read by the C++ parser.
trait Field: Sized {
    fn put(&self, out: &mut Vec<u8>);
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError>;
}

macro_rules! unsigned_field {
    ($($t:ty),*) => {$(
        impl Field for $t {
            fn put(&self, out: &mut Vec<u8>) {
                varint::push_compressed(out, u64::from(*self));
            }
            fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
                let v = r.varint(name)?;
                <$t>::try_from(v).map_err(|_| DecodeError::FieldOutOfRange(name))
            }
        }
    )*};
}
unsigned_field!(u8, u16, u32, u64);

/// C++ passes `int` through `(uint64_t)`: negative values sign-extend to ten bytes.
impl Field for i32 {
    fn put(&self, out: &mut Vec<u8>) {
        varint::push_compressed(out, i64::from(*self) as u64);
    }
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
        let v = r.varint(name)? as i64;
        i32::try_from(v).map_err(|_| DecodeError::FieldOutOfRange(name))
    }
}

/// C++ passes `int64_t` through `CompressInteger(uint64_t)`: two's complement reinterpretation.
impl Field for i64 {
    fn put(&self, out: &mut Vec<u8>) {
        varint::push_compressed(out, *self as u64);
    }
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
        Ok(r.varint(name)? as i64)
    }
}

impl Field for Str255 {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_bytes());
        out.push(0);
    }
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
        let rest = &r.bytes[r.pos..];
        let Some(len) = rest.iter().position(|&b| b == 0) else {
            return Err(DecodeError::UnterminatedString(name));
        };
        if len > MAX_STR_LEN {
            return Err(DecodeError::StringTooLong { field: name, len });
        }
        let s = Str255::new(&rest[..len]).map_err(|_| DecodeError::StringTooLong { field: name, len })?;
        r.pos += len + 1;
        Ok(s)
    }
}

/// KYC id tail: `int` values, each `(uint64_t)` sign-extended, running to the end of the payload.
/// (The C++ parser always consumes at least one varint, reading a phantom zero from the zero-padded
/// packet buffer when the list is empty; the reference decodes an empty tail as an empty list.)
impl Field for Vec<i32> {
    fn put(&self, out: &mut Vec<u8>) {
        for k in self {
            k.put(out);
        }
    }
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
        let mut v = Vec::new();
        while !r.at_end() {
            v.push(i32::get(r, name)?);
        }
        Ok(v)
    }
}

/// SendMany amount tail. Zero is rejected: C++ stops reading at the first zero amount.
impl Field for Vec<u64> {
    fn put(&self, out: &mut Vec<u8>) {
        for a in self {
            varint::push_compressed(out, *a);
        }
    }
    fn get(r: &mut Reader<'_>, name: &'static str) -> Result<Self, DecodeError> {
        let mut v = Vec::new();
        while !r.at_end() {
            let a = r.varint(name)?;
            if a == 0 {
                return Err(DecodeError::ZeroListAmount);
            }
            v.push(a);
        }
        Ok(v)
    }
}

macro_rules! payloads {
    ($(
        $(#[$meta:meta])*
        $name:ident = ($ty:literal, $ver:literal) { $($f:ident : $t:ty),* $(,)? }
    ),* $(,)?) => {
        /// Every transaction payload the C++ `CreatePayload_*` functions produce.
        ///
        /// Field order equals wire order. Field widths are those of the C++ builder arguments.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum Payload {$(
            $(#[$meta])*
            #[allow(missing_docs)]
            $name { $($f: $t),* },
        )*}

        impl Payload {
            /// The transaction type number written after the version.
            pub fn type_id(&self) -> u16 {
                match self { $(Payload::$name { .. } => $ty,)* }
            }
            /// The version number written first.
            pub fn version(&self) -> u16 {
                match self { $(Payload::$name { .. } => $ver,)* }
            }
            /// Variant name.
            pub fn name(&self) -> &'static str {
                match self { $(Payload::$name { .. } => stringify!($name),)* }
            }
            fn put_fields(&self, _out: &mut Vec<u8>) {
                match self {$(
                    Payload::$name { $($f),* } => { $( Field::put($f, _out); )* }
                )*}
            }
        }

        /// `(type, version, variant name)` for every creatable payload.
        pub const CREATABLE: &[(u16, u16, &str)] = &[$(($ty, $ver, stringify!($name))),*];

        fn decode_body(ty: u16, _r: &mut Reader<'_>) -> Result<Payload, DecodeError> {
            match ty {
                $($ty => Ok(Payload::$name { $($f: <$t as Field>::get(_r, stringify!($f))?),* }),)*
                _ => Err(DecodeError::UnsupportedType(ty)),
            }
        }
    };
}

payloads! {
    /// Tx 0, `CreatePayload_SimpleSend`.
    SimpleSend = (0, 0) { property_id: u32, amount: u64 },
    /// Tx 1, `CreatePayload_SendMany`.
    SendMany = (1, 0) { property_id: u32, amounts: Vec<u64> },
    /// Tx 4, `CreatePayload_SendAll`.
    SendAll = (4, 0) {},
    /// Tx 5, `CreatePayload_SendVestingTokens`.
    SendVestingTokens = (5, 0) { amount: u64 },
    /// Tx 20, `CreatePayload_DExSell` (version 1).
    DExSell = (20, 1) { property_id: u32, amount_for_sale: u64, amount_desired: u64, time_limit: u8, min_fee: u64, sub_action: u8 },
    /// Tx 21, `CreatePayload_DEx`.
    DEx = (21, 0) { property_id: u32, amount: u64, price: u64, time_limit: u8, min_fee: u64, sub_action: u8 },
    /// Tx 22, `CreatePayload_DExAccept`.
    DExAccept = (22, 0) { property_id: u32, amount: u64 },
    /// Tx 25, `CreatePayload_MetaDExTrade`.
    MetaDExTrade = (25, 0) { property_id_for_sale: u32, amount_for_sale: u64, property_id_desired: u32, amount_desired: u64 },
    /// Tx 26, `CreatePayload_MetaDExCancelAll`.
    MetaDExCancelAll = (26, 0) {},
    /// Tx 29, `CreatePayload_ContractDexTrade`.
    ContractDexTrade = (29, 0) { contract_name: Str255, amount_for_sale: u64, effective_price: u64, trading_action: u8, leverage: u64 },
    /// Tx 31, `CreatePayload_ContractDExCancel`.
    ContractDExCancel = (31, 0) { hash: Str255 },
    /// Tx 32, `CreatePayload_ContractDexCancelAll`.
    ContractDexCancelAll = (32, 0) { contract_id: u32 },
    /// Tx 33, `CreatePayload_ContractDexClosePosition`.
    ContractDexClosePosition = (33, 0) { contract_id: u32 },
    /// Tx 34, `CreatePayload_ContractDexCancelOrderByTxId` (`int block`, `unsigned int idx`).
    ContractDexCancelOrderByTxId = (34, 0) { block: i32, idx: u32 },
    /// Tx 35, `CreatePayload_DExCancel` (type 35 is `MSC_TYPE_METADEX_CANCEL`).
    DExCancel = (35, 0) { hash: Str255 },
    /// Tx 36, `CreatePayload_MetaDExCancelPair`.
    MetaDExCancelPair = (36, 0) { property_id_for_sale: u32, property_id_desired: u32 },
    /// Tx 37, `CreatePayload_MetaDExCancelPrice` (`int64_t` amounts).
    MetaDExCancelPrice = (37, 0) { property_id_for_sale: u32, amount_for_sale: i64, property_id_desired: u32, amount_desired: i64 },
    /// Tx 40, `CreatePayload_CreateContract`.
    CreateContract = (40, 0) { numerator: u32, denominator: u32, name: Str255, blocks_until_expiration: u32, notional_size: u32, collateral_currency: u32, margin_requirement: u64, inverse: u8, kyc: Vec<i32> },
    /// Tx 50, `CreatePayload_IssuanceFixed`.
    IssuanceFixed = (50, 0) { property_type: u16, previous_property_id: u32, name: Str255, url: Str255, data: Str255, amount: u64, kyc: Vec<i32> },
    /// Tx 54, `CreatePayload_IssuanceManaged`.
    IssuanceManaged = (54, 0) { property_type: u16, previous_property_id: u32, name: Str255, url: Str255, data: Str255, kyc: Vec<i32> },
    /// Tx 55, `CreatePayload_Grant`.
    Grant = (55, 0) { property_id: u32, amount: u64 },
    /// Tx 56, `CreatePayload_Revoke`.
    Revoke = (56, 0) { property_id: u32, amount: u64 },
    /// Tx 70, `CreatePayload_ChangeIssuer`.
    ChangeIssuer = (70, 0) { property_id: u32 },
    /// Tx 100, `CreatePayload_IssuancePegged`.
    IssuancePegged = (100, 0) { property_type: u16, previous_property_id: u32, name: Str255, property_id: u32, contract_id: u32, amount: u64 },
    /// Tx 101, `CreatePayload_RedemptionPegged`.
    RedemptionPegged = (101, 0) { property_id: u32, contract_id: u32, amount: u64 },
    /// Tx 102, `CreatePayload_SendPeggedCurrency`.
    SendPeggedCurrency = (102, 0) { property_id: u32, amount: u64 },
    /// Tx 103, `CreatePayload_CreateOracleContract`.
    CreateOracleContract = (103, 0) { name: Str255, blocks_until_expiration: u32, notional_size: u32, collateral_currency: u32, margin_requirement: u64, inverse: u8, kyc: Vec<i32> },
    /// Tx 104, `CreatePayload_Change_OracleAdm`.
    ChangeOracleAdm = (104, 0) { contract_id: u32 },
    /// Tx 105, `CreatePayload_Set_Oracle`.
    SetOracle = (105, 0) { contract_id: u32, high: u64, low: u64, close: u64 },
    /// Tx 106, `CreatePayload_OracleBackup`.
    OracleBackup = (106, 0) { contract_id: u32 },
    /// Tx 107, `CreatePayload_Close_Oracle`.
    CloseOracle = (107, 0) { contract_id: u32 },
    /// Tx 108, `CreatePayload_Commit_Channel`.
    CommitChannel = (108, 0) { property_id: u32, amount: u64 },
    /// Tx 109, `CreatePayload_Withdrawal_FromChannel`.
    WithdrawalFromChannel = (109, 0) { property_id: u32, amount: u64 },
    /// Tx 110, `CreatePayload_Instant_Trade` (`int blockheight_expiry`).
    InstantTrade = (110, 0) { property_id: u32, amount: u64, blockheight_expiry: i32, property_desired: u32, amount_desired: u64 },
    /// Tx 111, `CreatePayload_PNL_Update` (the node has no dispatch case for this type).
    PnlUpdate = (111, 0) { property_id: u32, amount: u64, blockheight_expiry: u32 },
    /// Tx 112, `CreatePayload_Transfer`.
    Transfer = (112, 0) { option: u8, property_id: u32, amount: u64 },
    /// Tx 113, `CreatePayload_Instant_LTC_Trade` (`int blockheight_expiry`).
    InstantLtcTrade = (113, 0) { property_id: u32, amount: u64, total_price: u64, blockheight_expiry: i32 },
    /// Tx 114, `CreatePayload_Contract_Instant_Trade`.
    ContractInstantTrade = (114, 0) { contract_id: u32, amount: u64, blockheight_expiry: u32, price: u64, trading_action: u8, leverage: u64 },
    /// Tx 115, `CreatePayload_New_Id_Registration`.
    NewIdRegistration = (115, 0) { website: Str255, name: Str255 },
    /// Tx 116, `CreatePayload_Update_Id_Registration`.
    UpdateIdRegistration = (116, 0) {},
    /// Tx 117, `CreatePayload_DEx_Payment`.
    DExPayment = (117, 0) {},
    /// Tx 118, `CreatePayload_Attestation`.
    Attestation = (118, 0) { hash: Str255 },
    /// Tx 119, `CreatePayload_Revoke_Attestation`.
    RevokeAttestation = (119, 0) {},
    /// Tx 120, `CreatePayload_Close_Channel`.
    CloseChannel = (120, 0) {},
    /// Tx 121, `CreatePayload_SubmitNodeAddress`.
    SubmitNodeAddress = (121, 0) {},
    /// Tx 122, `CreatePayload_ClaimNodeReward`.
    ClaimNodeReward = (122, 0) {},
    /// Tx 123, `CreatePayload_SendDonation`.
    SendDonation = (123, 0) { property_id: u32, amount: u64 },
    /// Tx 65533, `CreatePayload_DeactivateFeature` (version 1).
    DeactivateFeature = (65533, 1) { feature_id: u16 },
    /// Tx 65534, `CreatePayload_ActivateFeature` (version 1).
    ActivateFeature = (65534, 1) { feature_id: u16, activation_block: u32, min_client_version: u32 },
    /// Tx 65535, `CreatePayload_TradeLayerAlert` (version 65535; the C++ builder does not cut the
    /// message to 255 bytes, the node does when parsing).
    Alert = (65535, 65535) { alert_type: u16, alert_expiry: u32, message: Str255 },
}

/// Encodes a payload exactly as the matching C++ `CreatePayload_*` function does.
pub fn encode(p: &Payload) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    varint::push_compressed(&mut out, u64::from(p.version()));
    varint::push_compressed(&mut out, u64::from(p.type_id()));
    p.put_fields(&mut out);
    out
}

/// Like [`encode`] but refuses payloads the node could not parse (over [`MAX_PAYLOAD_LEN`]).
pub fn try_encode(p: &Payload) -> Result<Vec<u8>, EncodeError> {
    let out = encode(p);
    if out.len() > MAX_PAYLOAD_LEN {
        return Err(EncodeError::PayloadTooLarge(out.len()));
    }
    Ok(out)
}

/// Strictly decodes a payload. See the README for the deliberate differences from the lenient
/// C++ parser (truncation, trailing bytes, non-canonical varints, field overflow, long strings).
pub fn decode(bytes: &[u8]) -> Result<Payload, DecodeError> {
    if bytes.len() > MAX_PAYLOAD_LEN {
        return Err(DecodeError::PayloadTooLarge(bytes.len()));
    }
    let mut r = Reader { bytes, pos: 0 };
    let version = r.varint("version")?;
    let ty = r.varint("type")?;
    let ty16 = u16::try_from(ty).map_err(|_| DecodeError::UnknownType(ty))?;
    let expected = CREATABLE.iter().find(|(id, _, _)| *id == ty16).map(|(_, v, _)| *v).ok_or(if crate::types::is_known_type(ty16) {
        DecodeError::UnsupportedType(ty16)
    } else {
        DecodeError::UnknownType(ty)
    })?;
    if version != u64::from(expected) {
        return Err(DecodeError::UnexpectedVersion { ty: ty16, got: version, expected });
    }
    let p = decode_body(ty16, &mut r)?;
    if !r.at_end() {
        return Err(DecodeError::TrailingBytes(bytes.len() - r.pos));
    }
    Ok(p)
}
