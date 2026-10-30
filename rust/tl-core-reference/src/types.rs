//! Transaction type table (`enum TransactionType` in `src/tradelayer/tradelayer.h`) and the
//! dispatch facts from `CMPTransaction::interpret_Transaction` (`src/tradelayer/tx.cpp`).

/// One row of the C++ `TransactionType` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeInfo {
    /// Numeric type id written on the wire.
    pub id: u16,
    /// C++ enumerator name.
    pub cpp_name: &'static str,
    /// Whether some `CreatePayload_*` function builds this type and is defined.
    pub creatable: bool,
    /// Whether `interpret_Transaction` has a `case` for this type.
    pub dispatched: bool,
}

macro_rules! types {
    ($(($id:literal, $name:literal, $creatable:literal, $dispatched:literal)),* $(,)?) => {
        /// Every enumerator of the C++ `TransactionType` enum, in declaration order.
        pub const TYPE_TABLE: &[TypeInfo] = &[$(TypeInfo { id: $id, cpp_name: $name, creatable: $creatable, dispatched: $dispatched }),*];
    };
}

// Facts that differ from the "creatable and dispatched" default are explained in the README:
//  * 2, 10, 11, 30: no builder, no `interpret_*` case.
//  * 111 (PNL update): builder exists, `interpret_Update_PNL` exists, but no `case` dispatches it.
//  * 120 (close channel): dispatched to `interpret_SimpleSend`, which reads property and amount
//    that the builder never writes (they read as zero bytes from the zero-padded packet buffer).
types! {
    (0, "MSC_TYPE_SIMPLE_SEND", true, true),
    (1, "MSC_TYPE_SEND_MANY", true, true),
    (2, "MSC_TYPE_RESTRICTED_SEND", false, false),
    (4, "MSC_TYPE_SEND_ALL", true, true),
    (5, "MSC_TYPE_SEND_VESTING", true, true),
    (10, "MSC_TYPE_SAVINGS_MARK", false, false),
    (11, "MSC_TYPE_SAVINGS_COMPROMISED", false, false),
    (50, "MSC_TYPE_CREATE_PROPERTY_FIXED", true, true),
    (54, "MSC_TYPE_CREATE_PROPERTY_MANUAL", true, true),
    (55, "MSC_TYPE_GRANT_PROPERTY_TOKENS", true, true),
    (56, "MSC_TYPE_REVOKE_PROPERTY_TOKENS", true, true),
    (70, "MSC_TYPE_CHANGE_ISSUER_ADDRESS", true, true),
    (65533, "TL_MESSAGE_TYPE_DEACTIVATION", true, true),
    (65534, "TL_MESSAGE_TYPE_ACTIVATION", true, true),
    (65535, "TL_MESSAGE_TYPE_ALERT", true, true),
    (20, "MSC_TYPE_DEX_SELL_OFFER", true, true),
    (21, "MSC_TYPE_DEX_BUY_OFFER", true, true),
    (22, "MSC_TYPE_ACCEPT_OFFER_BTC", true, true),
    (25, "MSC_TYPE_METADEX_TRADE", true, true),
    (26, "MSC_TYPE_METADEX_CANCEL_ALL", true, true),
    (29, "MSC_TYPE_CONTRACTDEX_TRADE", true, true),
    (30, "MSC_TYPE_CONTRACTDEX_CANCEL_PRICE", false, false),
    (31, "MSC_TYPE_CONTRACTDEX_CANCEL", true, true),
    (32, "MSC_TYPE_CONTRACTDEX_CANCEL_ECOSYSTEM", true, true),
    (33, "MSC_TYPE_CONTRACTDEX_CLOSE_POSITION", true, true),
    (34, "MSC_TYPE_CONTRACTDEX_CANCEL_ORDERS_BY_BLOCK", true, true),
    (35, "MSC_TYPE_METADEX_CANCEL", true, true),
    (36, "MSC_TYPE_METADEX_CANCEL_BY_PAIR", true, true),
    (37, "MSC_TYPE_METADEX_CANCEL_BY_PRICE", true, true),
    (40, "MSC_TYPE_CREATE_CONTRACT", true, true),
    (100, "MSC_TYPE_PEGGED_CURRENCY", true, true),
    (101, "MSC_TYPE_REDEMPTION_PEGGED", true, true),
    (102, "MSC_TYPE_SEND_PEGGED_CURRENCY", true, true),
    (103, "MSC_TYPE_CREATE_ORACLE_CONTRACT", true, true),
    (104, "MSC_TYPE_CHANGE_ORACLE_REF", true, true),
    (105, "MSC_TYPE_SET_ORACLE", true, true),
    (106, "MSC_TYPE_ORACLE_BACKUP", true, true),
    (107, "MSC_TYPE_CLOSE_ORACLE", true, true),
    (108, "MSC_TYPE_COMMIT_CHANNEL", true, true),
    (109, "MSC_TYPE_WITHDRAWAL_FROM_CHANNEL", true, true),
    (110, "MSC_TYPE_INSTANT_TRADE", true, true),
    (111, "MSC_TYPE_PNL_UPDATE", true, false),
    (112, "MSC_TYPE_TRANSFER", true, true),
    (113, "MSC_TYPE_INSTANT_LTC_TRADE", true, true),
    (114, "MSC_TYPE_CONTRACT_INSTANT", true, true),
    (115, "MSC_TYPE_NEW_ID_REGISTRATION", true, true),
    (116, "MSC_TYPE_UPDATE_ID_REGISTRATION", true, true),
    (117, "MSC_TYPE_DEX_PAYMENT", true, true),
    (118, "MSC_TYPE_ATTESTATION", true, true),
    (119, "MSC_TYPE_REVOKE_ATTESTATION", true, true),
    (120, "MSC_TYPE_CLOSE_CHANNEL", true, true),
    (121, "MSC_TYPE_SUBMIT_NODE_ADDRESS", true, true),
    (122, "MSC_TYPE_CLAIM_NODE_REWARD", true, true),
    (123, "MSC_TYPE_SEND_DONATION", true, true),
}

/// Looks a type id up in [`TYPE_TABLE`].
pub fn type_info(id: u16) -> Option<&'static TypeInfo> {
    TYPE_TABLE.iter().find(|t| t.id == id)
}

/// Whether `id` is an enumerator of the C++ `TransactionType` enum.
pub fn is_known_type(id: u16) -> bool {
    type_info(id).is_some()
}
