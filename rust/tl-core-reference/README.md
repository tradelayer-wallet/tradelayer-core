# tl-core-reference

A std-only Rust crate that reproduces, byte for byte, the **payload layer** and the **activation rules**
of this C++ node, so that the C++ behaviour can serve as a differential oracle for the Rust rebuild.
It is deliberately small: the full Rust consensus port lives in `patrickdugan/TradeLayer-Rust`;
nothing here replaces or re-ports the node (no ledger, DEx, margin, channels, wallet, RPC).

* Baseline: `master` at `2bd5bcad5bf0b06b210dcd8bca0856e1085cddb2`, branch `feat/rust-port`.
* Charter row: "C++ reference/differential fixtures and activation review; consolidate implementation in
  existing Rust core". Charter items tradelayer-core #3/#4 (consensus/replay/activation questions) need
  explicit decisions before any of the behaviour below changes; this crate therefore **reproduces**
  questionable C++ behaviour in its oracle functions and lists every case under "Unresolved questions".

## What is ported

| Rust | C++ source (pinned blob) | Notes |
|---|---|---|
| `varint::{compress, push_compressed}` | `varint.cpp` `CompressInteger` (`cb81b11d`) | exact |
| `varint::read_strict` / `decompress_cpp` | `varint.cpp` `DecompressInteger`, `tx.cpp` `GetNextVarIntBytes` | strict reader + the lenient C++ arithmetic |
| `Payload`, `encode` (50 variants) | `createpayload.cpp` (`23f55353`) | every defined `CreatePayload_*`; field order/width as the C++ arguments |
| `decode`, `try_encode` | `tx.cpp` `interpret_*` (`3d6895d5`) | strict counterpart, see differences below |
| `Str255` | `substr(0,255)` in the builders, `char[256]` fields in `CMPTransaction` | 255-byte bound in the type |
| `types::TYPE_TABLE` | `tradelayer.h` `enum TransactionType` (`b337d7df`) + `interpret_Transaction` dispatch | includes `creatable`/`dispatched` flags |
| `rules::{Feature, Param, ConsensusParams, Network}` | `rules.h` (`6ee9f12a`), `rules.cpp` (`4729979a`) | 28 feature ids, 43 parameters, 3 networks, 54-row `GetRestrictions` |
| `ActivationState::{activate, deactivate, complete_live}` | `ActivateFeature`, `DeactivateFeature`, `activation.cpp` (`992aced4`) | `complete_live` is modelled from source only |

Wire layout of every payload: `varint(version) varint(type) fields`, version FIRST. Integers are little-endian
base-128 varints; strings are raw bytes + one NUL (max 255 bytes); KYC id lists (`int`, sign-extended to 10
bytes when negative) and SendMany amounts have no length prefix and run to the end of the payload.

Not ported (out of the payload layer): `CheckActivationAuthorization`/alert text, `logicMath_*` ledger
effects, `ContractDexCancelPrice` (type 30) and `MetaDExCancelEcosystem` (declared in `createpayload.h`, never
defined), types 2/10/11 (enum only), RPC, persistence.

## Differential vectors

Nothing is hand-derived. Both fixture files are printed by the **unmodified** C++ sources compiled with g++
(`scripts/build-vectors.sh`, run in WSL; no node build, no boost, no Bitcoin Core headers):

* `fixtures/payload_vectors.tsv` (80 rows): `scripts/vectors.cpp` `#include`s `createpayload.cpp` and
  `varint.cpp` verbatim; two empty stub headers (`scripts/shim`) replace unused includes. Covers every payload
  variant plus long strings, negative `int`/`int64_t`, zero amounts and the lenient `DecompressInteger` cases.
* `fixtures/rules_vectors.tsv` (12,165 rows): `scripts/rules_vectors.cpp` `#include`s `rules.cpp` verbatim against
  stand-in headers (`scripts/shim_rules`: logging, alert counter, chain-params, `uint256`; the `TransactionType`
  enum and `MP_TX_PKT_V*` constants are cut out of the real `tradelayer.h` at build time). Dumps the three
  parameter blocks, the restriction tables, `IsFeatureActivated` for ids 0..31 x 15 heights x 3 networks,
  `IsTransactionTypeAllowed` for 58 types x 4 versions x 15 heights x 3 networks, and 24 `ActivateFeature` /
  `DeactivateFeature` scenario steps with the full resulting parameter block, pending list and alert count.
* `tests/rules_source.rs` also parses the C++ text at test time (feature ids, enum, read/write parameter maps,
  constructor values, builder `messageType`/`messageVer`) so a source change that the fixtures have not seen
  fails the build.

Regenerate: `wsl -d Ubuntu-24.04 -- bash scripts/build-vectors.sh` (needs g++).

## Compatibility table

Legend: **preserved** = same bytes/behaviour as C++; **repaired defect** = legacy defect not reproduced by the
strict decoder, with regression test; **versioned** = none needed; **unresolved** = needs a maintainer decision.

| # | Behaviour | C++ location | This crate | Status |
|---|---|---|---|---|
| 1 | Builder bytes, type and version numbers for all 50 payloads | `createpayload.cpp` | identical (80 vectors) | preserved |
| 2 | Activation reads: feature 12 uses `MSC_NODE_REWARD_BLOCK`, feature 28 uses `MSC_VESTING_BLOCK` (writes use the right fields) | `rules.cpp` `IsFeatureActivated` (cases `FEATURE_CONTRACTDEX`, `FEATURE_PEGGED_CURRENCY`) | reproduced; Lean theorem `contractdex_deactivation_is_ineffective` | unresolved (looks like copy/paste) |
| 3 | `ActivateFeature` for an unknown feature id returns true, records a pending activation and an alert, changes no parameter | `rules.cpp:565-582` | `Accepted{supported:false}`; test `documented_cpp_quirks_hold`; Lean `activate_unknown` | unresolved (task asked for "unknown ids rejected"; the C++ does not reject them) |
| 4 | Activation/deactivation payloads carry version 1 but the restriction rows require `0xFFFF`, so `IsTransactionTypeAllowed(type, 1)` is false at every height | `createpayload.cpp:246,264` vs `rules.cpp:48-49`, checked in `logicMath_Activation/Deactivation` | reproduced; test `headers_match_the_consensus_version_table...`; Lean `activation_version1_never_allowed` | unresolved. `test/functional/tl_activation.py` expects activations to work, so either the test is stale or a path not read here sets the version; needs a node run |
| 5 | `ConsensusParams(network)` falls back to MAINNET for any unknown network string | `rules.cpp:333` | `Network::from_id_string` | unresolved |
| 6 | `TL_VERSION` is `static const int` built from `100000000000 * MAJOR`; `TL_VERSION >= minClientVersion` compares `int` with `uint32_t` | `version.h:40`, `rules.cpp:451` | `TL_VERSION: u32 = 20_002_000`, `u32` compare | unresolved (overflows for MAJOR >= 1) |
| 7 | Parsed integers are stored in narrower C++ fields without a range check (`feature_id` is `uint16_t`: id 65537 activates feature 1; `property` `uint32_t`; `trading_action` `uint8_t` ...) | `tx.cpp` `interpret_*` | `FieldOutOfRange` | repaired defect (aliasing); tests `malformed_inputs_get_typed_errors` |
| 8 | `GetNextVarIntBytes` reads `pkt[i]` past `pkt_size` (buffer is zero-padded), so a truncated payload parses with zero fields | `tx.cpp:138-150` | `Truncated` for every strict prefix of a fixed-shape payload | repaired defect; tests `decode_rejects_truncation_inside_the_fixed_part`, Lean `*_truncated` |
| 9 | `DecompressInteger` accepts non-canonical encodings (`80 00` = 0), drops bits above bit 63 and shifts by >= 64 (undefined behaviour) beyond 10 bytes | `varint.cpp:36-42` | `NonCanonical`/`Overflow`/`TooLong`; `decompress_cpp` keeps the arithmetic | repaired defect; vectors `varint_decompress/*` |
| 10 | Strings longer than 255 bytes are cut to 255 on parse, but the cursor advances by the CUT length, so the following fields are read from inside the string | `tx.cpp:469-473` and siblings | `StringTooLong` | repaired defect (field desynchronisation); test `string_fields_are_bounded_and_terminated` |
| 11 | Trailing bytes after a fixed-shape payload are ignored | all `interpret_*` | `TrailingBytes`; Lean `simpleSend_trailing` | repaired defect |
| 12 | Empty KYC list parses as `[0]` (`do { read } while`, phantom zero from zero padding) and the node filters 0 later | `tx.cpp:477-486` and siblings | empty tail = empty list | preserved up to the filter (`aux != 0`) |
| 13 | SendMany: a zero amount silently ends the list (`if (n == 0) break`) | `tx.cpp:389-400` | `ZeroListAmount` (the C++ BUILDER accepts zero amounts) | unresolved; vector `send_many/zero_amount`, test `zero_amount_in_send_many_is_not_wire_faithful` |
| 14 | KYC/SendMany tails have no length prefix, so a payload cut at an element boundary is a valid shorter payload | wire format | reproduced; Lean `sendMany_prefix_is_shorter_list` | unresolved (protocol level) |
| 15 | Negative `int` KYC ids, `int block`, `int blockheight_expiry`, `int64_t` cancel-price amounts become 10-byte varints | `createpayload.cpp:27-31,413,1033` | `i32`/`i64` fields, sign-extension checked on decode | preserved |
| 16 | Alert message is not cut to 255 bytes by the builder; the parser cuts it | `createpayload.cpp:282-302`, `tx.cpp:707-709` | `Str255` rejects > 255 | unresolved |
| 17 | `MSC_TYPE_PNL_UPDATE` (111) has a builder and an `interpret_Update_PNL` but no `case` in `interpret_Transaction` | `tx.cpp:170-318,1850` | `TYPE_TABLE.dispatched = false` | unresolved |
| 18 | `MSC_TYPE_CLOSE_CHANNEL` (120) is parsed by `interpret_SimpleSend` (reads property/amount the builder never writes) | `tx.cpp:307-308` | `CloseChannel {}` | unresolved |
| 19 | Wrong "non-empty" guards: `interpret_CreatePeggedCurrency` tests `vecVersionBytes` for the amount, `interpret_Set_Oracle` tests `vecLowBytes` for close, `interpret_Instant_LTC_Trade` tests `vecPrice` for the block | `tx.cpp:1425,1669,1967` | no effect (readers never return empty) | unresolved (cosmetic) |
| 20 | `interpret_Transfer` collapses `option` to a bool | `tx.cpp:1912` | `option: u8` kept | unresolved |
| 21 | `createpayload.h` declares `CreatePayload_ContractDexCancelPrice`, `CreatePayload_MetaDExCancelEcosystem` and a `uint64_t` overload of `CreatePayload_MetaDExCancelPrice` that are never defined | `createpayload.h:29,34,54` | not ported | unresolved |
| 22 | Window arithmetic `transactionBlock + MAX_ACTIVATION_BLOCKS` and `uint32_t` -> `int` conversion of the activation block | `rules.cpp:434`, `tx.cpp:3209` | done in `i64`; negative blocks fail the lower bound (vector `main_window/4`) | preserved (differs from C++ only where `int` would overflow) |

No consensus byte or activation outcome was changed. Items 7-11 and 13 are in the *decoder only*: the C++
parser is the lenient one and stays authoritative on-chain until a maintainer decides.

## Commands

```
cargo test --locked --offline                                   # 32 tests
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/build-vectors.sh                                   # regenerate fixtures (g++, Linux/WSL)
cd lean && lake build && lake env lean AxiomCheck.lean          # Lean 4.34.1, core only
```

Dependencies: none (std only), so no justification table is needed.

## Lean theorems (`lean/`, core Lean 4.34.1, no Mathlib, no `sorry`/`axiom`)

| Theorem | File | Models |
|---|---|---|
| `readVarint_encodeVarint` (decode (encode v) = v for all u64, rest preserved) | `Varint.lean` | `push_compressed` / `read_strict` |
| `encodeVarint_injective`, `readVarint_truncated`, `encodeVarint_bytes_lt`, `encodeVarint_length_le` (<= 10 bytes) | `Varint.lean` | same |
| `decode_encode` (round trip for SimpleSend, SendAll, SendMany, ActivateFeature, IssuanceFixed, ContractDexTrade under `WF`) | `Payload.lean` | `payload::{encode, decode}` |
| `enc_injective`, `decode_total_on_wf` | `Payload.lean` | same |
| `simpleSend_truncated`, `activateFeature_truncated`, `contractDexTrade_truncated`, `sendMany_head_truncated` (every strict prefix rejected) | `Payload.lean` | `decode` |
| `simpleSend_trailing` (trailing bytes rejected), `sendMany_prefix_is_shorter_list` (documents the length-less tail) | `Payload.lean` | `decode` |
| `isActive_iff`, `not_active_before`, `active_mono`, `unknown_never_active` | `Activation.lean` | `IsFeatureActivated` |
| `activate_window`, `activate_not_live`, `activate_unknown`, `activate_sets_slot`, `activate_then_active` | `Activation.lean` | `ActivateFeature` |
| `deactivate_requires_live`, `deactivate_inactive`, `contractdex_deactivation_is_ineffective` | `Activation.lean` | `DeactivateFeature` and C++ item 2 |
| `unknown_ids_have_no_slot`, `aliased_features`, `aliased_features_differ` | `Activation.lean` | feature tables |
| `allowed_mono`, `allowed_exact_version`, `activation_version1_never_allowed` | `Activation.lean` | `IsTransactionTypeAllowed` and C++ item 4 |

`AxiomCheck.lean` prints the axioms of each (only `propext`, `Quot.sound`, `Classical.choice`).

Tie to Rust: the Lean files contain `example ... := by decide` vectors that are the same bytes as
`fixtures/payload_vectors.tsv` (8 payloads, checked by `tests/lean_ties.rs`) and the same scenario results as
`fixtures/rules_vectors.tsv` (regtest `regtest_fixed`/`regtest_alias`). `tests/lean_ties.rs` also parses
`readTable`, `writeTable`, `regtestList`, `never` and `tlVersion` out of `Activation.lean` and compares them with
`rules.rs`.

Where the model abstracts: bytes and heights are `Nat`; KYC ids are non-negative `Nat` (the negative `i32` branch
is only covered by Rust tests and C++ vectors); the model covers 6 of the 50 variants (the other 44 follow the
same field readers and are covered by the property tests); the decoder is the strict one, not the lenient C++ one;
the 10-byte limit of `read_strict` is subsumed by its canonical and `< 2^64` checks; alerts, authorization and
pending-activation bookkeeping are not modelled; `Params` is a function from slot to height.

## Remaining gaps

* The strict decoder is not byte-compatible with the lenient C++ parser on malformed input by design; there is no
  executable model of `GetNextVarIntBytes`' zero-padding. Replaying real chain payloads through both is future work.
* `complete_live` (`CheckLiveActivations`) and `AddPendingActivation` bookkeeping were compared only by reading
  `activation.cpp`, not by compiling it.
* CI (`.github/workflows/rust-tl-core-reference.yml`) has not run: nothing was pushed.
