//! Feature ids, per-network activation heights and the activation checks of
//! `src/tradelayer/rules.cpp` / `rules.h` (plus the pending-activation bookkeeping of
//! `activation.cpp`).
//!
//! This module reproduces the C++ behaviour, including behaviour that looks unintended (see the
//! README "unresolved questions"): the reference oracle must not silently change consensus.
//! Authorization (`CheckActivationAuthorization`) and alert text are outside the payload layer
//! and are not modelled; alerts are only counted.

/// "Never" sentinel height used throughout `rules.cpp` (`99999999`).
pub const NEVER_BLOCK: i32 = 99_999_999;

/// `TL_VERSION` from `src/tradelayer/version.h` for 0.2.2.0:
/// `100000000000*0 + 10000000*2 + 1000*2 + 0`.
pub const TL_VERSION: u32 = 20_002_000;

macro_rules! params {
    ($(($variant:ident, $cpp:literal)),* $(,)?) => {
        /// A named height/period field of `CConsensusParams`, in the order the differential
        /// harness dumps them.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        #[allow(missing_docs)]
        pub enum Param { $($variant),* }

        impl Param {
            /// All parameters in dump order.
            pub const ALL: &'static [Param] = &[$(Param::$variant),*];
            /// The C++ member name.
            pub fn cpp_name(self) -> &'static str {
                match self { $(Param::$variant => $cpp),* }
            }
            /// Finds a parameter by its C++ member name.
            pub fn from_cpp_name(name: &str) -> Option<Param> {
                match name { $($cpp => Some(Param::$variant),)* _ => None }
            }
        }
    };
}

params! {
    (GenesisBlock, "GENESIS_BLOCK"),
    (MultisigBlock, "MULTISIG_BLOCK"),
    (MinActivationBlocks, "MIN_ACTIVATION_BLOCKS"),
    (MaxActivationBlocks, "MAX_ACTIVATION_BLOCKS"),
    (PubkeyhashBlock, "PUBKEYHASH_BLOCK"),
    (ScripthashBlock, "SCRIPTHASH_BLOCK"),
    (NulldataBlock, "NULLDATA_BLOCK"),
    (MscAlertBlock, "MSC_ALERT_BLOCK"),
    (MscSendBlock, "MSC_SEND_BLOCK"),
    (MscSendManyBlock, "MSC_SEND_MANY_BLOCK"),
    (MscSpBlock, "MSC_SP_BLOCK"),
    (MscManualspBlock, "MSC_MANUALSP_BLOCK"),
    (MscSendAllBlock, "MSC_SEND_ALL_BLOCK"),
    (InflexionBlock, "INFLEXION_BLOCK"),
    (MscVestingCreationBlock, "MSC_VESTING_CREATION_BLOCK"),
    (MscVestingBlock, "MSC_VESTING_BLOCK"),
    (MscPeggedCurrencyBlock, "MSC_PEGGED_CURRENCY_BLOCK"),
    (MscKycBlock, "MSC_KYC_BLOCK"),
    (MscMetadexBlock, "MSC_METADEX_BLOCK"),
    (MscDexsellBlock, "MSC_DEXSELL_BLOCK"),
    (MscDexbuyBlock, "MSC_DEXBUY_BLOCK"),
    (MscContractdexBlock, "MSC_CONTRACTDEX_BLOCK"),
    (MscContractdexOraclesBlock, "MSC_CONTRACTDEX_ORACLES_BLOCK"),
    (MscNodeRewardBlock, "MSC_NODE_REWARD_BLOCK"),
    (MscTradechannelTokensBlock, "MSC_TRADECHANNEL_TOKENS_BLOCK"),
    (MscTradechannelContractsBlock, "MSC_TRADECHANNEL_CONTRACTS_BLOCK"),
    (MscTradechannelOptionsBlock, "MSC_TRADECHANNEL_OPTIONS_BLOCK"),
    (MscDispensevaultsBlock, "MSC_DISPENSERVAULTS_BLOCK"),
    (MscPaymentbatchingBlock, "MSC_PAYMENTBATCHING_BLOCK"),
    (MscMarginlendingBlock, "MSC_MARGINLENDING_BLOCK"),
    (MscInteropCtvBlock, "MSC_INTEROP_CTV_BLOCK"),
    (MscInteropLightningBlock, "MSC_INTEROP_LIGHTNING_BLOCK"),
    (MscInteropRepoBlock, "MSC_INTEROP_REPO_BLOCK"),
    (MscInteropSidechainsBlock, "MSC_INTEROP_SIDECHAINS_BLOCK"),
    (MscInteropCrosschainatomicswapsBlock, "MSC_INTEROP_CROSSCHAINATOMICSWAPS_BLOCK"),
    (MscGraphdefaultswapsBlock, "MSC_GRAPHDEFAULTSWAPS_BLOCK"),
    (MscInterestrateswapsBlock, "MSC_INTERESTRATESWAPS_BLOCK"),
    (MscMinerfeecontractsBlock, "MSC_MINERFEECONTRACTS_BLOCK"),
    (MscMasspaymentBlock, "MSC_MASSPAYMENT_BLOCK"),
    (MscMultisendBlock, "MSC_MULTISEND_BLOCK"),
    (MscHedgedcurrencyBlock, "MSC_HEDGEDCURRENCY_BLOCK"),
    (MscSendDonationBlock, "MSC_SEND_DONATION_BLOCK"),
    (OneYear, "ONE_YEAR"),
}

/// Number of [`Param`]s.
pub const NUM_PARAMS: usize = 43;

/// A TradeLayer network as selected by `ConsensusParams(const std::string&)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    /// `"main"`.
    Main,
    /// `"test"`.
    Test,
    /// `"regtest"`.
    Regtest,
}

impl Network {
    /// C++ `ConsensusParams(network)`: any unrecognised network string FALLS BACK to mainnet.
    pub fn from_id_string(s: &str) -> Network {
        match s {
            "test" => Network::Test,
            "regtest" => Network::Regtest,
            _ => Network::Main,
        }
    }
}

/// The mutable consensus parameter block (`CConsensusParams`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusParams {
    values: [i32; NUM_PARAMS],
}

impl ConsensusParams {
    /// Constructor values of `CMainConsensusParams`, `CTestNetConsensusParams` or
    /// `CRegTestConsensusParams`.
    pub fn new(network: Network) -> Self {
        use Param::*;
        let n = NEVER_BLOCK;
        let table: [(Param, i32); NUM_PARAMS] = match network {
            Network::Main => [
                (GenesisBlock, 2_487_000),
                (MultisigBlock, 0),
                (MinActivationBlocks, 0),
                (MaxActivationBlocks, 99_999_999),
                (PubkeyhashBlock, 0),
                (ScripthashBlock, 0),
                (NulldataBlock, 0),
                (MscAlertBlock, 2_487_300),
                (MscSendBlock, 2_487_300),
                (MscSendManyBlock, 2_487_300),
                (MscSpBlock, 2_487_300),
                (MscManualspBlock, 2_487_300),
                (MscSendAllBlock, 2_487_300),
                (InflexionBlock, 25_000),
                (MscVestingCreationBlock, 2_490_000),
                (MscVestingBlock, 2_490_000),
                (MscPeggedCurrencyBlock, n),
                (MscKycBlock, 2_487_300),
                (MscMetadexBlock, 2_487_300),
                (MscDexsellBlock, 2_487_300),
                (MscDexbuyBlock, 2_487_300),
                (MscContractdexBlock, n),
                (MscContractdexOraclesBlock, 2_487_300),
                (MscNodeRewardBlock, n),
                (MscTradechannelTokensBlock, 2_487_300),
                (MscTradechannelContractsBlock, 2_487_300),
                (MscTradechannelOptionsBlock, n),
                (MscDispensevaultsBlock, n),
                (MscPaymentbatchingBlock, n),
                (MscMarginlendingBlock, n),
                (MscInteropCtvBlock, n),
                (MscInteropLightningBlock, n),
                (MscInteropRepoBlock, n),
                (MscInteropSidechainsBlock, n),
                (MscInteropCrosschainatomicswapsBlock, n),
                (MscGraphdefaultswapsBlock, n),
                (MscInterestrateswapsBlock, n),
                (MscMinerfeecontractsBlock, n),
                (MscMasspaymentBlock, n),
                (MscMultisendBlock, n),
                (MscHedgedcurrencyBlock, n),
                (MscSendDonationBlock, n),
                (OneYear, 210_240),
            ],
            Network::Test => [
                (GenesisBlock, 2_121_695),
                (MultisigBlock, 0),
                (MinActivationBlocks, 0),
                (MaxActivationBlocks, 99_999_999),
                (PubkeyhashBlock, 0),
                (ScripthashBlock, 0),
                (NulldataBlock, 0),
                (MscAlertBlock, 2_121_695),
                (MscSendBlock, 2_121_695),
                (MscSendManyBlock, n),
                (MscSpBlock, 2_121_695),
                (MscManualspBlock, n),
                (MscSendAllBlock, n),
                (InflexionBlock, 25_000),
                (MscVestingCreationBlock, 2_121_695),
                (MscVestingBlock, n),
                (MscPeggedCurrencyBlock, n),
                (MscKycBlock, n),
                (MscMetadexBlock, n),
                (MscDexsellBlock, n),
                (MscDexbuyBlock, n),
                (MscContractdexBlock, n),
                (MscContractdexOraclesBlock, n),
                (MscNodeRewardBlock, n),
                (MscTradechannelTokensBlock, n),
                (MscTradechannelContractsBlock, n),
                (MscTradechannelOptionsBlock, n),
                (MscDispensevaultsBlock, n),
                (MscPaymentbatchingBlock, n),
                (MscMarginlendingBlock, n),
                (MscInteropCtvBlock, n),
                (MscInteropLightningBlock, n),
                (MscInteropRepoBlock, n),
                (MscInteropSidechainsBlock, n),
                (MscInteropCrosschainatomicswapsBlock, n),
                (MscGraphdefaultswapsBlock, n),
                (MscInterestrateswapsBlock, n),
                (MscMinerfeecontractsBlock, n),
                (MscMasspaymentBlock, n),
                (MscMultisendBlock, n),
                (MscHedgedcurrencyBlock, n),
                (MscSendDonationBlock, n),
                (OneYear, 2_650),
            ],
            Network::Regtest => [
                (GenesisBlock, 0),
                (MultisigBlock, 0),
                (MinActivationBlocks, 5),
                (MaxActivationBlocks, 1000),
                (PubkeyhashBlock, 0),
                (ScripthashBlock, 0),
                (NulldataBlock, 0),
                (MscAlertBlock, 0),
                (MscSendBlock, 0),
                (MscSendManyBlock, 0),
                (MscSpBlock, 200),
                (MscManualspBlock, 0),
                (MscSendAllBlock, 0),
                (InflexionBlock, 1000),
                (MscVestingCreationBlock, 100),
                (MscVestingBlock, 100),
                (MscPeggedCurrencyBlock, 200),
                (MscKycBlock, 0),
                (MscMetadexBlock, 0),
                (MscDexsellBlock, 0),
                (MscDexbuyBlock, 0),
                (MscContractdexBlock, 0),
                (MscContractdexOraclesBlock, 0),
                (MscNodeRewardBlock, 200),
                (MscTradechannelTokensBlock, 0),
                (MscTradechannelContractsBlock, 0),
                (MscTradechannelOptionsBlock, n),
                (MscDispensevaultsBlock, n),
                (MscPaymentbatchingBlock, n),
                (MscMarginlendingBlock, n),
                (MscInteropCtvBlock, n),
                (MscInteropLightningBlock, n),
                (MscInteropRepoBlock, n),
                (MscInteropSidechainsBlock, n),
                (MscInteropCrosschainatomicswapsBlock, n),
                (MscGraphdefaultswapsBlock, n),
                (MscInterestrateswapsBlock, n),
                (MscMinerfeecontractsBlock, n),
                (MscMasspaymentBlock, n),
                (MscMultisendBlock, n),
                (MscHedgedcurrencyBlock, n),
                (MscSendDonationBlock, n),
                (OneYear, 930),
            ],
        };
        let mut values = [0i32; NUM_PARAMS];
        for (p, v) in table {
            values[p as usize] = v;
        }
        Self { values }
    }

    /// Reads a parameter.
    pub fn get(&self, p: Param) -> i32 {
        self.values[p as usize]
    }

    /// Writes a parameter.
    pub fn set(&mut self, p: Param, v: i32) {
        self.values[p as usize] = v;
    }

    /// All values in [`Param::ALL`] order.
    pub fn values(&self) -> &[i32; NUM_PARAMS] {
        &self.values
    }

    /// `GetRestrictions()`: the transaction type/version activation table, in C++ order
    /// (including its duplicate entries), resolved against the current parameter values.
    pub fn restrictions(&self) -> Vec<TransactionRestriction> {
        use Param::*;
        let spec: [(u16, u16, Param); 54] = [
            (65535, 0xFFFF, MscAlertBlock),
            (65534, 0xFFFF, MscAlertBlock),
            (65533, 0xFFFF, MscAlertBlock),
            (0, 0, MscSendBlock),
            (1, 0, MscSendManyBlock),
            (50, 0, MscSpBlock),
            (54, 0, MscManualspBlock),
            (55, 0, MscManualspBlock),
            (56, 0, MscManualspBlock),
            (70, 0, MscManualspBlock),
            (40, 0, MscContractdexBlock),
            (29, 0, MscContractdexBlock),
            (32, 0, MscContractdexBlock),
            (4, 0, MscSendAllBlock),
            (33, 0, MscContractdexBlock),
            (34, 0, MscContractdexBlock),
            (25, 0, MscMetadexBlock),
            (20, 1, MscDexsellBlock),
            (21, 0, MscDexbuyBlock),
            (22, 0, MscDexsellBlock),
            (117, 0, MscDexsellBlock),
            (29, 0, MscContractdexBlock),
            (32, 0, MscContractdexBlock),
            (40, 0, MscContractdexBlock),
            (100, 0, MscContractdexBlock),
            (101, 0, MscPeggedCurrencyBlock),
            (102, 0, MscPeggedCurrencyBlock),
            (33, 0, MscPeggedCurrencyBlock),
            (34, 0, MscContractdexBlock),
            (104, 0, MscContractdexOraclesBlock),
            (105, 0, MscContractdexOraclesBlock),
            (107, 0, MscContractdexOraclesBlock),
            (106, 0, MscContractdexOraclesBlock),
            (108, 0, MscTradechannelTokensBlock),
            (109, 0, MscTradechannelTokensBlock),
            (110, 0, MscTradechannelTokensBlock),
            (112, 0, MscTradechannelTokensBlock),
            (114, 0, MscTradechannelContractsBlock),
            (115, 0, MscKycBlock),
            (116, 0, MscKycBlock),
            (103, 0, MscContractdexOraclesBlock),
            (5, 0, MscVestingBlock),
            (118, 0, MscKycBlock),
            (119, 0, MscKycBlock),
            (31, 0, MscContractdexBlock),
            (113, 0, MscTradechannelTokensBlock),
            (35, 0, MscMetadexBlock),
            (36, 0, MscMetadexBlock),
            (26, 0, MscMetadexBlock),
            (37, 0, MscMetadexBlock),
            (120, 0, MscTradechannelTokensBlock),
            (121, 0, MscNodeRewardBlock),
            (122, 0, MscNodeRewardBlock),
            (123, 0, MscSendDonationBlock),
        ];
        spec.iter()
            .map(|&(tx_type, tx_version, p)| TransactionRestriction {
                tx_type,
                tx_version,
                allow_wildcard: true,
                activation_block: self.get(p),
            })
            .collect()
    }

    /// C++ `IsTransactionTypeAllowed(txBlock, txType, version)`: true iff some restriction row
    /// matches the exact type AND exact version and `txBlock >= activationBlock`.
    pub fn is_transaction_type_allowed(&self, tx_block: i32, tx_type: u16, version: u16) -> bool {
        self.restrictions().iter().any(|e| e.tx_type == tx_type && e.tx_version == version && tx_block >= e.activation_block)
    }

    /// C++ `IsFeatureActivated(featureId, transactionBlock)`. Unknown feature ids are never active.
    pub fn is_feature_activated(&self, feature_id: u16, tx_block: i32) -> bool {
        match Feature::from_id(feature_id) {
            Some(f) => tx_block >= self.get(f.read_param()),
            None => false,
        }
    }
}

/// One row of `GetRestrictions()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionRestriction {
    /// Transaction type.
    pub tx_type: u16,
    /// Required transaction version (exact match).
    pub tx_version: u16,
    /// Whether property id 0 may be used as a wildcard.
    pub allow_wildcard: bool,
    /// Block from which the transaction is enabled.
    pub activation_block: i32,
}

macro_rules! features {
    ($(($variant:ident, $id:literal, $cpp:literal, $display:literal, $read:ident, $write:ident)),* $(,)?) => {
        /// Feature identifiers (`FEATURE_*` in `rules.h`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(missing_docs)]
        pub enum Feature { $($variant),* }

        impl Feature {
            /// All features in id order.
            pub const ALL: &'static [Feature] = &[$(Feature::$variant),*];
            /// Maps a wire feature id to a feature; `None` for ids the C++ switch does not know.
            pub fn from_id(id: u16) -> Option<Feature> {
                match id { $($id => Some(Feature::$variant),)* _ => None }
            }
            /// The numeric id.
            pub fn id(self) -> u16 { match self { $(Feature::$variant => $id),* } }
            /// The `FEATURE_*` constant name in `rules.h`.
            pub fn cpp_name(self) -> &'static str { match self { $(Feature::$variant => $cpp),* } }
            /// `GetFeatureName` text.
            pub fn display_name(self) -> &'static str { match self { $(Feature::$variant => $display),* } }
            /// The parameter `IsFeatureActivated` compares against.
            pub fn read_param(self) -> Param { match self { $(Feature::$variant => Param::$read),* } }
            /// The parameter `ActivateFeature` / `DeactivateFeature` write.
            pub fn write_param(self) -> Param { match self { $(Feature::$variant => Param::$write),* } }
        }
    };
}

features! {
    (Vesting, 1, "FEATURE_VESTING", "Vesting Tokens", MscVestingBlock, MscVestingBlock),
    (Kyc, 2, "FEATURE_KYC", "Know Your Customer", MscKycBlock, MscKycBlock),
    (DexSell, 3, "FEATURE_DEX_SELL", "Sell Offer in DEx Token Exchange", MscDexsellBlock, MscDexsellBlock),
    (DexBuy, 4, "FEATURE_DEX_BUY", "Buy Offer in DEx Token Exchange", MscDexbuyBlock, MscDexbuyBlock),
    (Metadex, 5, "FEATURE_METADEX", "Distributed Meta Token Exchange", MscMetadexBlock, MscMetadexBlock),
    (TradechannelsTokens, 6, "FEATURE_TRADECHANNELS_TOKENS", "Trade Channels Token Exchange", MscTradechannelTokensBlock, MscTradechannelTokensBlock),
    (TradechannelsContracts, 7, "FEATURE_TRADECHANNELS_CONTRACTS", "Trade Channels Contracts Exchange", MscTradechannelContractsBlock, MscTradechannelContractsBlock),
    (Fixed, 8, "FEATURE_FIXED", "Create Fixed Tokens", MscSpBlock, MscSpBlock),
    (Managed, 9, "FEATURE_MANAGED", "Create Managed Tokens", MscManualspBlock, MscManualspBlock),
    (NodeReward, 10, "FEATURE_NODE_REWARD", "Node Reward activation", MscNodeRewardBlock, MscNodeRewardBlock),
    (SendMany, 11, "FEATURE_SEND_MANY", "Send to Many", MscSendManyBlock, MscSendManyBlock),
    (Contractdex, 12, "FEATURE_CONTRACTDEX", "Native Contracts Exchange", MscNodeRewardBlock, MscContractdexBlock),
    (ContractdexOracles, 13, "FEATURE_CONTRACTDEX_ORACLES", "Oracle Contracts Exchange", MscContractdexOraclesBlock, MscContractdexOraclesBlock),
    (TradechannelsOptions, 14, "FEATURE_TRADECHANNELS_OPTIONS", "Trade Channels Option Exchange", MscTradechannelOptionsBlock, MscTradechannelOptionsBlock),
    (Dispensevaults, 15, "FEATURE_DISPENSERVAULTS", "DispersenVaults", MscDispensevaultsBlock, MscDispensevaultsBlock),
    (Paymentbatching, 16, "FEATURE_PAYMENTBATCHING", "Payment Batching", MscPaymentbatchingBlock, MscPaymentbatchingBlock),
    (Marginlending, 17, "FEATURE_MARGINLENDING", "Margin Lending", MscMarginlendingBlock, MscMarginlendingBlock),
    (InteropCtv, 18, "FEATURE_INTEROP_CTV", "Interoperability CTV", MscInteropCtvBlock, MscInteropCtvBlock),
    (InteropLightning, 19, "FEATURE_INTEROP_LIGHTNING", "Interoperability Lightning", MscInteropLightningBlock, MscInteropLightningBlock),
    (InteropRepo, 20, "FEATURE_INTEROP_REPO", "Interoperability Repo", MscInteropRepoBlock, MscInteropRepoBlock),
    (InteropSidechains, 21, "FEATURE_INTEROP_SIDECHAINS", "Interoperability Side Chains", MscInteropSidechainsBlock, MscInteropSidechainsBlock),
    (InteropCrosschainatomicswaps, 22, "FEATURE_INTEROP_CROSSCHAINATOMICSWAPS", "Interoperability Cross Chain Atomic Swapss", MscInteropCrosschainatomicswapsBlock, MscInteropCrosschainatomicswapsBlock),
    (Graphdefaultswaps, 23, "FEATURE_GRAPHDEFAULTSWAPS", "Interoperability Graph Default Swaps", MscGraphdefaultswapsBlock, MscGraphdefaultswapsBlock),
    (Interestrateswaps, 24, "FEATURE_INTERESTRATESWAPS", "Interoperability Swaps", MscInterestrateswapsBlock, MscInterestrateswapsBlock),
    (Minerfeecontracts, 25, "FEATURE_MINERFEECONTRACTS", "Interoperability Miner Fee Contracts", MscMinerfeecontractsBlock, MscMinerfeecontractsBlock),
    (Masspayment, 26, "FEATURE_MASSPAYMENT", "Mass Payment", MscMasspaymentBlock, MscMasspaymentBlock),
    (Hedgedcurrency, 27, "FEATURE_HEDGEDCURRENCY", "Hedge Currency", MscHedgedcurrencyBlock, MscHedgedcurrencyBlock),
    (PeggedCurrency, 28, "FEATURE_PEGGED_CURRENCY", "Pegged Currency", MscVestingBlock, MscPeggedCurrencyBlock),
}

/// Why `ActivateFeature` returned false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivateRefusal {
    /// `activationBlock` outside `[txBlock + MIN_ACTIVATION_BLOCKS, txBlock + MAX_ACTIVATION_BLOCKS]`.
    NoticeWindow,
    /// `IsFeatureActivated(featureId, txBlock)` is already true.
    AlreadyLive,
}

/// Result of [`ActivationState::activate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivateOutcome {
    /// C++ returned false; no state changed.
    Refused(ActivateRefusal),
    /// C++ returned true. `supported` is false for unknown feature ids (nothing is scheduled in
    /// the parameter block, but a pending activation and an alert are recorded) and when
    /// `TL_VERSION < minClientVersion`.
    Accepted {
        /// Whether this client supports the feature.
        supported: bool,
    },
}

impl ActivateOutcome {
    /// The C++ boolean result.
    pub fn as_bool(self) -> bool {
        matches!(self, ActivateOutcome::Accepted { .. })
    }
}

/// Result of [`ActivationState::deactivate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeactivateOutcome {
    /// C++ returned false: the feature is unknown or not live at the transaction block.
    Refused,
    /// C++ returned true; the feature's write parameter is now [`NEVER_BLOCK`].
    Done,
}

/// A `FeatureActivation` entry of `vecPendingActivations`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingActivation {
    /// Feature id as received (may be unknown).
    pub feature_id: u16,
    /// Scheduled activation height.
    pub activation_block: i32,
    /// Minimum client version.
    pub min_client_version: u32,
}

/// Result of [`ActivationState::complete_live`] (`CheckLiveActivations`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveCheck {
    /// Feature ids moved from pending to completed, in pending order.
    pub completed: Vec<u16>,
    /// True if C++ would call `DoAbortNode` because `TL_VERSION < minClientVersion`.
    pub shutdown: bool,
}

/// Consensus parameters plus the pending-activation list and an alert counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationState {
    /// The parameter block being mutated.
    pub params: ConsensusParams,
    /// `vecPendingActivations`.
    pub pending: Vec<PendingActivation>,
    /// Number of `AddAlert` calls made by activate/deactivate.
    pub alerts_raised: u32,
}

impl ActivationState {
    /// Fresh state for a network.
    pub fn new(network: Network) -> Self {
        Self { params: ConsensusParams::new(network), pending: Vec::new(), alerts_raised: 0 }
    }

    /// C++ `ActivateFeature(featureId, activationBlock, minClientVersion, transactionBlock)`.
    ///
    /// `activation_block` is the `int` the node passes after converting the payload's `uint32_t`
    /// (callers modelling a payload use `activation_block as i32`). Window arithmetic is done in
    /// 64 bits (C++ would overflow `int` only for heights near 2^31).
    pub fn activate(&mut self, feature_id: u16, activation_block: i32, min_client_version: u32, tx_block: i32) -> ActivateOutcome {
        let min = i64::from(self.params.get(Param::MinActivationBlocks));
        let max = i64::from(self.params.get(Param::MaxActivationBlocks));
        let ab = i64::from(activation_block);
        let tb = i64::from(tx_block);
        if ab < tb + min || ab > tb + max {
            return ActivateOutcome::Refused(ActivateRefusal::NoticeWindow);
        }
        if self.params.is_feature_activated(feature_id, tx_block) {
            return ActivateOutcome::Refused(ActivateRefusal::AlreadyLive);
        }
        let mut supported = TL_VERSION >= min_client_version;
        match Feature::from_id(feature_id) {
            Some(f) => self.params.set(f.write_param(), activation_block),
            None => supported = false,
        }
        self.pending.retain(|p| p.feature_id != feature_id);
        self.pending.push(PendingActivation { feature_id, activation_block, min_client_version });
        if !supported {
            self.alerts_raised += 1;
        }
        ActivateOutcome::Accepted { supported }
    }

    /// C++ `DeactivateFeature(featureId, transactionBlock)`.
    pub fn deactivate(&mut self, feature_id: u16, tx_block: i32) -> DeactivateOutcome {
        if !self.params.is_feature_activated(feature_id, tx_block) {
            return DeactivateOutcome::Refused;
        }
        let Some(f) = Feature::from_id(feature_id) else {
            return DeactivateOutcome::Refused;
        };
        self.params.set(f.write_param(), NEVER_BLOCK);
        self.alerts_raised += 1;
        DeactivateOutcome::Done
    }

    /// C++ `CheckLiveActivations(blockHeight)` (`activation.cpp`): completes every pending
    /// activation whose height has been reached and reports whether the node would shut down.
    /// Modelled from source only; not exercised by the compiled differential harness.
    pub fn complete_live(&mut self, block_height: i32) -> LiveCheck {
        let mut completed = Vec::new();
        let mut shutdown = false;
        let snapshot = self.pending.clone();
        for p in snapshot {
            if p.activation_block > block_height {
                continue;
            }
            if TL_VERSION < p.min_client_version {
                shutdown = true;
            }
            self.pending.retain(|q| q.feature_id != p.feature_id);
            completed.push(p.feature_id);
        }
        LiveCheck { completed, shutdown }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_table_is_complete() {
        assert_eq!(Param::ALL.len(), NUM_PARAMS);
        for (i, p) in Param::ALL.iter().enumerate() {
            assert_eq!(*p as usize, i);
            assert_eq!(Param::from_cpp_name(p.cpp_name()), Some(*p));
        }
    }

    #[test]
    fn feature_ids_are_contiguous() {
        for (i, f) in Feature::ALL.iter().enumerate() {
            assert_eq!(usize::from(f.id()), i + 1);
            assert_eq!(Feature::from_id(f.id()), Some(*f));
        }
        assert_eq!(Feature::from_id(0), None);
        assert_eq!(Feature::from_id(29), None);
    }

    #[test]
    fn network_fallback_is_mainnet() {
        assert_eq!(Network::from_id_string("signet"), Network::Main);
        assert_eq!(Network::from_id_string(""), Network::Main);
        assert_eq!(Network::from_id_string("regtest"), Network::Regtest);
    }
}
