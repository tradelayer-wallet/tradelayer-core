import TlCoreRef.Parser

namespace TlCoreRef

/-!
# Payload encode / decode for a representative subset

Models `tl_core_reference::payload::{encode, decode}` for six payload kinds:

| Lean constructor    | Rust variant          | C++ builder                              |
|---------------------|-----------------------|------------------------------------------|
| `simpleSend`        | `SimpleSend`          | `CreatePayload_SimpleSend` (tx 0)        |
| `sendAll`           | `SendAll`             | `CreatePayload_SendAll` (tx 4)           |
| `sendMany`          | `SendMany`            | `CreatePayload_SendMany` (tx 1)          |
| `activateFeature`   | `ActivateFeature`     | `CreatePayload_ActivateFeature` (65534)  |
| `issuanceFixed`     | `IssuanceFixed`       | `CreatePayload_IssuanceFixed` (tx 50)    |
| `contractDexTrade`  | `ContractDexTrade`    | `CreatePayload_ContractDexTrade` (tx 29) |

Abstractions: KYC ids are `Nat` (the Rust type is `i32`, sign-extended to `u64`; the negative
branch is covered by the Rust tests, not by this model). Bytes are `Nat`. The wire layout (version
first, then type, then fields; strings NUL-terminated; list tails run to the end) is the real one.
-/

inductive Payload where
  | simpleSend (propertyId amount : Nat)
  | sendAll
  | sendMany (propertyId : Nat) (amounts : List Nat)
  | activateFeature (featureId activationBlock minClientVersion : Nat)
  | issuanceFixed (propertyType previousPropertyId : Nat) (name url data : Bytes) (amount : Nat) (kyc : List Nat)
  | contractDexTrade (contractName : Bytes) (amountForSale effectivePrice tradingAction leverage : Nat)
  deriving DecidableEq, Repr

/-- Version first, then type. -/
def hdr (ver ty : Nat) : Bytes := encodeVarint ver ++ encodeVarint ty

def enc : Payload → Bytes
  | .simpleSend p a => hdr 0 0 ++ encodeVarint p ++ encodeVarint a
  | .sendAll => hdr 0 4
  | .sendMany p as => hdr 0 1 ++ encodeVarint p ++ encList as
  | .activateFeature f b m => hdr 1 65534 ++ encodeVarint f ++ encodeVarint b ++ encodeVarint m
  | .issuanceFixed pt pp n u d a k =>
    hdr 0 50 ++ encodeVarint pt ++ encodeVarint pp ++ encStr n ++ encStr u ++ encStr d ++ encodeVarint a ++ encList k
  | .contractDexTrade n a p t l =>
    hdr 0 29 ++ encStr n ++ encodeVarint a ++ encodeVarint p ++ encodeVarint t ++ encodeVarint l

/-- The shape selected by `(version, type)`, mirroring `decode_body` in `payload.rs`. -/
def dispatch (ver ty : Nat) : Bytes → Option Payload :=
  if ver = 0 ∧ ty = 0 then
    bindP (rdU (2 ^ 32)) fun p => bindP readVarint fun a => endP (Payload.simpleSend p a)
  else if ver = 0 ∧ ty = 4 then
    endP Payload.sendAll
  else if ver = 0 ∧ ty = 1 then
    bindP (rdU (2 ^ 32)) fun p => tailP true (Payload.sendMany p)
  else if ver = 1 ∧ ty = 65534 then
    bindP (rdU (2 ^ 16)) fun f => bindP (rdU (2 ^ 32)) fun b => bindP (rdU (2 ^ 32)) fun m =>
      endP (Payload.activateFeature f b m)
  else if ver = 0 ∧ ty = 50 then
    bindP (rdU (2 ^ 16)) fun pt => bindP (rdU (2 ^ 32)) fun pp => bindP readStr255 fun n =>
      bindP readStr255 fun u => bindP readStr255 fun d => bindP readVarint fun a =>
        tailP false (Payload.issuanceFixed pt pp n u d a)
  else if ver = 0 ∧ ty = 29 then
    bindP readStr255 fun n => bindP readVarint fun a => bindP readVarint fun p => bindP (rdU 256) fun t =>
      bindP readVarint fun l => endP (Payload.contractDexTrade n a p t l)
  else fun _ => none

def decode : Bytes → Option Payload :=
  bindP readVarint fun ver => bindP readVarint fun ty => dispatch ver ty

/-- Field-width and string-bound invariants of the Rust types (`u32`, `u16`, `u8`, `Str255`, ...). -/
def WF : Payload → Prop
  | .simpleSend p a => p < 2 ^ 32 ∧ a < 2 ^ 64
  | .sendAll => True
  | .sendMany p as => p < 2 ^ 32 ∧ ∀ a ∈ as, 0 < a ∧ a < 2 ^ 64
  | .activateFeature f b m => f < 2 ^ 16 ∧ b < 2 ^ 32 ∧ m < 2 ^ 32
  | .issuanceFixed pt pp n u d a k =>
    pt < 2 ^ 16 ∧ pp < 2 ^ 32 ∧ Str255 n ∧ Str255 u ∧ Str255 d ∧ a < 2 ^ 64 ∧ ∀ x ∈ k, x < 2 ^ 64
  | .contractDexTrade n a p t l => Str255 n ∧ a < 2 ^ 64 ∧ p < 2 ^ 64 ∧ t < 2 ^ 8 ∧ l < 2 ^ 64

/-! ### Concrete vectors shared with the Rust tests (bytes printed by the C++ builders) -/

-- simple_send/basic = 00 00 07 80c2d72f
example : enc (.simpleSend 7 100000000) = [0x00, 0x00, 0x07, 0x80, 0xc2, 0xd7, 0x2f] := by decide
example : decode [0x00, 0x00, 0x07, 0x80, 0xc2, 0xd7, 0x2f] = some (.simpleSend 7 100000000) := by decide
-- send_all/basic = 00 04
example : enc .sendAll = [0x00, 0x04] := by decide
-- send_many/basic = 00 01 03 01 02 ac02
example : enc (.sendMany 3 [1, 2, 300]) = [0x00, 0x01, 0x03, 0x01, 0x02, 0xac, 0x02] := by decide
example : decode [0x00, 0x01, 0x03, 0x01, 0x02, 0xac, 0x02] = some (.sendMany 3 [1, 2, 300]) := by decide
-- send_many/zero_amount: the C++ builder emits 00 01 03 05 00 07; the strict decoder refuses it
example : enc (.sendMany 3 [5, 0, 7]) = [0x00, 0x01, 0x03, 0x05, 0x00, 0x07] := by decide
example : decode [0x00, 0x01, 0x03, 0x05, 0x00, 0x07] = none := by decide
-- activate_feature/basic = 01 feff03 08 9003 01
example : enc (.activateFeature 8 400 1) = [0x01, 0xfe, 0xff, 0x03, 0x08, 0x90, 0x03, 0x01] := by decide
example : decode [0x01, 0xfe, 0xff, 0x03, 0x08, 0x90, 0x03, 0x01] = some (.activateFeature 8 400 1) := by decide
-- the same payload with version 0 is refused
example : decode [0x00, 0xfe, 0xff, 0x03, 0x08, 0x90, 0x03, 0x01] = none := by decide
-- issuance_fixed/nokyc = 00 32 01 05 "T"\0 \0 \0 01
example : enc (.issuanceFixed 1 5 [0x54] [] [] 1 []) = [0x00, 0x32, 0x01, 0x05, 0x54, 0x00, 0x00, 0x00, 0x01] := by decide
example : decode [0x00, 0x32, 0x01, 0x05, 0x54, 0x00, 0x00, 0x00, 0x01] = some (.issuanceFixed 1 5 [0x54] [] [] 1 []) := by decide
-- issuance_fixed/basic: name "TOK", url, data "meta", amount 21000000, kyc [0,1,2]
example : enc (.issuanceFixed 2 0 [0x54, 0x4f, 0x4b] [0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f, 0x2f, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x69, 0x6e, 0x76, 0x61, 0x6c, 0x69, 0x64] [0x6d, 0x65, 0x74, 0x61] 21000000 [0, 1, 2]) =
    [0x00, 0x32, 0x02, 0x00, 0x54, 0x4f, 0x4b, 0x00,
     0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f, 0x2f, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x69, 0x6e, 0x76, 0x61, 0x6c, 0x69, 0x64, 0x00,
     0x6d, 0x65, 0x74, 0x61, 0x00, 0xc0, 0xde, 0x81, 0x0a, 0x00, 0x01, 0x02] := by decide
-- contract_dex_trade/basic = 00 1d "ALL-LTC"\0 64 c0f0f50b 01 0a
example : enc (.contractDexTrade [0x41, 0x4c, 0x4c, 0x2d, 0x4c, 0x54, 0x43] 100 25000000 1 10) =
    [0x00, 0x1d, 0x41, 0x4c, 0x4c, 0x2d, 0x4c, 0x54, 0x43, 0x00, 0x64, 0xc0, 0xf0, 0xf5, 0x0b, 0x01, 0x0a] := by decide
example : decode [0x00, 0x1d, 0x41, 0x4c, 0x4c, 0x2d, 0x4c, 0x54, 0x43, 0x00, 0x64, 0xc0, 0xf0, 0xf5, 0x0b, 0x01, 0x0a] =
    some (.contractDexTrade [0x41, 0x4c, 0x4c, 0x2d, 0x4c, 0x54, 0x43] 100 25000000 1 10) := by decide
-- trailing byte and truncation
example : decode [0x00, 0x04, 0x00] = none := by decide
example : decode [0x00, 0x00, 0x07, 0x80, 0xc2, 0xd7] = none := by decide
-- property id 2^32 does not fit u32
example : decode (hdr 0 0 ++ encodeVarint 4294967296 ++ encodeVarint 1) = none := by decide

/-! ### Dispatch equations -/

theorem dispatch_simpleSend : dispatch 0 0 =
    (bindP (rdU (2 ^ 32)) fun p => bindP readVarint fun a => endP (Payload.simpleSend p a)) := by
  simp [dispatch]

theorem dispatch_sendAll : dispatch 0 4 = endP Payload.sendAll := by simp [dispatch]

theorem dispatch_sendMany : dispatch 0 1 =
    (bindP (rdU (2 ^ 32)) fun p => tailP true (Payload.sendMany p)) := by
  simp [dispatch]

theorem dispatch_activateFeature : dispatch 1 65534 =
    (bindP (rdU (2 ^ 16)) fun f => bindP (rdU (2 ^ 32)) fun b => bindP (rdU (2 ^ 32)) fun m =>
      endP (Payload.activateFeature f b m)) := by
  simp [dispatch]

theorem dispatch_issuanceFixed : dispatch 0 50 =
    (bindP (rdU (2 ^ 16)) fun pt => bindP (rdU (2 ^ 32)) fun pp => bindP readStr255 fun n =>
      bindP readStr255 fun u => bindP readStr255 fun d => bindP readVarint fun a =>
        tailP false (Payload.issuanceFixed pt pp n u d a)) := by
  simp [dispatch]

theorem dispatch_contractDexTrade : dispatch 0 29 =
    (bindP readStr255 fun n => bindP readVarint fun a => bindP readVarint fun p => bindP (rdU 256) fun t =>
      bindP readVarint fun l => endP (Payload.contractDexTrade n a p t l)) := by
  simp [dispatch]

/-- Same as `bindP_seg` for a field that ends the input. -/
theorem bindP_seg_nil {α β : Type} {rd : Bytes → Option (α × Bytes)} {A : Bytes} {x : α}
    (h : Seg rd A x) (f : α → Bytes → Option β) : bindP rd f A = f x [] := by
  simpa using bindP_seg h f []

/-! ### Round trip: `decode (enc p) = some p` -/

theorem decode_encode (p : Payload) (h : WF p) : decode (enc p) = some p := by
  cases p with
  | simpleSend p a =>
    obtain ⟨hp, ha⟩ := h
    simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 0 (by decide)),
      dispatch_simpleSend, bindP_seg (seg_u (2 ^ 32) p hp (by decide)), bindP_seg_nil (seg_varint a ha)]
    simp [endP]
  | sendAll =>
    simp only [decode, enc, hdr, bindP_seg (seg_varint 0 (by decide)), bindP_seg_nil (seg_varint 4 (by decide)),
      dispatch_sendAll]
    simp [endP]
  | sendMany p as =>
    obtain ⟨hp, has⟩ := h
    have hx : ∀ x ∈ as, x < 2 ^ 64 ∧ (true = true → 0 < x) := fun x hx => ⟨(has x hx).2, fun _ => (has x hx).1⟩
    simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 0 (by decide)),
      bindP_seg (seg_varint 1 (by decide)), dispatch_sendMany, bindP_seg (seg_u (2 ^ 32) p hp (by decide))]
    rw [tailP_encList true _ as hx]
  | activateFeature f b m =>
    obtain ⟨hf, hb, hm⟩ := h
    simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 1 (by decide)),
      bindP_seg (seg_varint 65534 (by decide)), dispatch_activateFeature,
      bindP_seg (seg_u (2 ^ 16) f hf (by decide)), bindP_seg (seg_u (2 ^ 32) b hb (by decide)),
      bindP_seg_nil (seg_u (2 ^ 32) m hm (by decide))]
    simp [endP]
  | issuanceFixed pt pp n u d a k =>
    obtain ⟨hpt, hpp, hn, hu, hd, ha, hk⟩ := h
    have hx : ∀ x ∈ k, x < 2 ^ 64 ∧ (false = true → 0 < x) := fun x hx => ⟨hk x hx, fun h => by simp at h⟩
    simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 0 (by decide)),
      bindP_seg (seg_varint 50 (by decide)), dispatch_issuanceFixed,
      bindP_seg (seg_u (2 ^ 16) pt hpt (by decide)), bindP_seg (seg_u (2 ^ 32) pp hpp (by decide)),
      bindP_seg (seg_str n hn), bindP_seg (seg_str u hu), bindP_seg (seg_str d hd),
      bindP_seg (seg_varint a ha)]
    rw [tailP_encList false _ k hx]
  | contractDexTrade n a p t l =>
    obtain ⟨hn, ha, hp, ht, hl⟩ := h
    simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 0 (by decide)),
      bindP_seg (seg_varint 29 (by decide)), dispatch_contractDexTrade, bindP_seg (seg_str n hn),
      bindP_seg (seg_varint a ha), bindP_seg (seg_varint p hp), bindP_seg (seg_u 256 t ht (by decide)),
      bindP_seg_nil (seg_varint l hl)]
    simp [endP]

/-- `encode` is injective on well-formed payloads. -/
theorem enc_injective (p q : Payload) (hp : WF p) (hq : WF q) (h : enc p = enc q) : p = q := by
  have h1 := decode_encode p hp
  have h2 := decode_encode q hq
  rw [h] at h1
  rw [h1] at h2
  exact Option.some.inj h2

/-- Every well-formed payload is accepted by the decoder when given its own bytes. -/
theorem decode_total_on_wf (p : Payload) (h : WF p) : (decode (enc p)).isSome = true := by
  rw [decode_encode p h]; rfl

/-! ### Truncation and trailing bytes -/

/-- SimpleSend: every strict prefix of the encoding is rejected. -/
theorem simpleSend_truncated (p a : Nat) (hp : p < 2 ^ 32) (ha : a < 2 ^ 64) :
    ∀ k, k < (enc (.simpleSend p a)).length → decode ((enc (.simpleSend p a)).take k) = none := by
  intro k hk
  simp only [enc, hdr, List.append_assoc] at hk ⊢
  unfold decode
  refine bindP_trunc (seg_varint 0 (by decide)) _ ?_ k hk
  intro k1 hk1
  refine bindP_trunc (seg_varint 0 (by decide)) _ ?_ k1 hk1
  intro k2 hk2
  simp only [dispatch_simpleSend]
  refine bindP_trunc (seg_u (2 ^ 32) p hp (by decide)) _ ?_ k2 hk2
  intro k3 hk3
  simp [bindP, (seg_varint a ha).2 k3 hk3]

/-- ActivateFeature: every strict prefix of the encoding is rejected. -/
theorem activateFeature_truncated (f b m : Nat) (hf : f < 2 ^ 16) (hb : b < 2 ^ 32) (hm : m < 2 ^ 32) :
    ∀ k, k < (enc (.activateFeature f b m)).length →
      decode ((enc (.activateFeature f b m)).take k) = none := by
  intro k hk
  simp only [enc, hdr, List.append_assoc] at hk ⊢
  unfold decode
  refine bindP_trunc (seg_varint 1 (by decide)) _ ?_ k hk
  intro k1 hk1
  refine bindP_trunc (seg_varint 65534 (by decide)) _ ?_ k1 hk1
  intro k2 hk2
  simp only [dispatch_activateFeature]
  refine bindP_trunc (seg_u (2 ^ 16) f hf (by decide)) _ ?_ k2 hk2
  intro k3 hk3
  refine bindP_trunc (seg_u (2 ^ 32) b hb (by decide)) _ ?_ k3 hk3
  intro k4 hk4
  simp [bindP, (seg_u (2 ^ 32) m hm (by decide)).2 k4 hk4]

/-- ContractDexTrade (a string field and four integers): every strict prefix is rejected. -/
theorem contractDexTrade_truncated (n : Bytes) (a p t l : Nat) (hn : Str255 n) (ha : a < 2 ^ 64)
    (hp : p < 2 ^ 64) (ht : t < 2 ^ 8) (hl : l < 2 ^ 64) :
    ∀ k, k < (enc (.contractDexTrade n a p t l)).length →
      decode ((enc (.contractDexTrade n a p t l)).take k) = none := by
  intro k hk
  simp only [enc, hdr, List.append_assoc] at hk ⊢
  unfold decode
  refine bindP_trunc (seg_varint 0 (by decide)) _ ?_ k hk
  intro k1 hk1
  refine bindP_trunc (seg_varint 29 (by decide)) _ ?_ k1 hk1
  intro k2 hk2
  simp only [dispatch_contractDexTrade]
  refine bindP_trunc (seg_str n hn) _ ?_ k2 hk2
  intro k3 hk3
  refine bindP_trunc (seg_varint a ha) _ ?_ k3 hk3
  intro k4 hk4
  refine bindP_trunc (seg_varint p hp) _ ?_ k4 hk4
  intro k5 hk5
  refine bindP_trunc (seg_u 256 t ht (by decide)) _ ?_ k5 hk5
  intro k6 hk6
  simp [bindP, (seg_varint l hl).2 k6 hk6]

/-- SendMany: a cut inside the version, type or property-id field is rejected. (Cuts inside the
amount list are NOT rejected, the list has no length prefix: see `sendMany_prefix_is_shorter_list`.) -/
theorem sendMany_head_truncated (p : Nat) (as : List Nat) (hp : p < 2 ^ 32) :
    ∀ k, k < (encodeVarint 0 ++ encodeVarint 1 ++ encodeVarint p).length →
      decode ((enc (.sendMany p as)).take k) = none := by
  intro k hk
  have hlen : k < (encodeVarint 0 ++ (encodeVarint 1 ++ encodeVarint p)).length := by
    simpa [List.append_assoc] using hk
  have hrw : (enc (.sendMany p as)).take k =
      (encodeVarint 0 ++ (encodeVarint 1 ++ encodeVarint p)).take k := by
    have hH : enc (.sendMany p as) =
        (encodeVarint 0 ++ (encodeVarint 1 ++ encodeVarint p)) ++ encList as := by
      simp only [enc, hdr, List.append_assoc]
    rw [hH, List.take_append_of_le_length (Nat.le_of_lt hlen)]
  rw [hrw]
  unfold decode
  refine bindP_trunc (seg_varint 0 (by decide)) _ ?_ k hlen
  intro k1 hk1
  refine bindP_trunc (seg_varint 1 (by decide)) _ ?_ k1 hk1
  intro k2 hk2
  simp only [dispatch_sendMany]
  simp [bindP, (seg_u (2 ^ 32) p hp (by decide)).2 k2 hk2]

/-- The trade-off of the length-less list tail: dropping the last amount's bytes yields a valid,
shorter SendMany. -/
theorem sendMany_prefix_is_shorter_list (p : Nat) (as : List Nat) (x : Nat) (hp : p < 2 ^ 32)
    (has : ∀ a ∈ as, 0 < a ∧ a < 2 ^ 64) :
    decode ((enc (.sendMany p (as ++ [x]))).take (enc (.sendMany p as)).length) = some (.sendMany p as) := by
  have hcat : ∀ ys : List Nat, encList (ys ++ [x]) = encList ys ++ encodeVarint x := by
    intro ys
    induction ys with
    | nil => simp [encList]
    | cons y ys ih => simp [encList, ih, List.append_assoc]
  have : enc (.sendMany p (as ++ [x])) = enc (.sendMany p as) ++ encodeVarint x := by
    simp only [enc, hcat, List.append_assoc]
  rw [this, List.take_left]
  exact decode_encode _ ⟨hp, has⟩

/-- Trailing bytes after a fixed-shape payload are rejected. -/
theorem simpleSend_trailing (p a : Nat) (hp : p < 2 ^ 32) (ha : a < 2 ^ 64) (b : Nat) (rest : Bytes) :
    decode (enc (.simpleSend p a) ++ b :: rest) = none := by
  simp only [decode, enc, hdr, List.append_assoc, bindP_seg (seg_varint 0 (by decide)),
    dispatch_simpleSend, bindP_seg (seg_u (2 ^ 32) p hp (by decide)), bindP_seg (seg_varint a ha), endP]
  simp

end TlCoreRef
