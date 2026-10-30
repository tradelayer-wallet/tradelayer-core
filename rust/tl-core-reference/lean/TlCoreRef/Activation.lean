namespace TlCoreRef

/-!
# Feature activation rules

Models `tl_core_reference::rules` (`ActivationState::{activate, deactivate}`,
`ConsensusParams::{is_feature_activated, is_transaction_type_allowed}`), i.e. `ActivateFeature`,
`DeactivateFeature`, `IsFeatureActivated` and `IsTransactionTypeAllowed` in
`src/tradelayer/rules.cpp`.

* A *slot* is an index into the 43-entry parameter table (`Param` in `rules.rs`, in the order of
  `CConsensusParams` members dumped by `scripts/rules_vectors.cpp`). `Params` maps a slot to a height.
* `readSlot id` is the slot `IsFeatureActivated` compares against; `writeSlot id` is the slot
  `ActivateFeature` / `DeactivateFeature` assign. They differ for exactly two features, reproducing
  the C++ (feature 12 reads the node-reward height, feature 28 reads the vesting height). The
  Rust test `lean_tables_match_rust` parses the two tables below and compares them to `rules.rs`.
* Heights and the notice window are `Nat`: the C++ uses `int`, so negative heights (a `uint32_t`
  above `INT_MAX` after conversion) are outside the model; the Rust tests and the compiled C++
  vectors cover them (they are refused by the lower window bound).
* Authorization and alert text are not modelled.
-/

/-- Height assigned to a deactivated or unscheduled feature (`99999999`). -/
def never : Nat := 99999999

/-- `TL_VERSION` of 0.2.2.0. -/
def tlVersion : Nat := 20002000

abbrev Params := Nat → Nat

def setSlot (p : Params) (s v : Nat) : Params := fun i => if i = s then v else p i

/-- `(feature id, slot read by IsFeatureActivated)`; unknown ids are absent. -/
def readTable : List (Nat × Nat) :=
  [(1, 15), (2, 17), (3, 19), (4, 20), (5, 18), (6, 24), (7, 25), (8, 10), (9, 11), (10, 23),
   (11, 9), (12, 23), (13, 22), (14, 26), (15, 27), (16, 28), (17, 29), (18, 30), (19, 31),
   (20, 32), (21, 33), (22, 34), (23, 35), (24, 36), (25, 37), (26, 38), (27, 40), (28, 15)]

/-- `(feature id, slot written by ActivateFeature / DeactivateFeature)`. -/
def writeTable : List (Nat × Nat) :=
  [(1, 15), (2, 17), (3, 19), (4, 20), (5, 18), (6, 24), (7, 25), (8, 10), (9, 11), (10, 23),
   (11, 9), (12, 21), (13, 22), (14, 26), (15, 27), (16, 28), (17, 29), (18, 30), (19, 31),
   (20, 32), (21, 33), (22, 34), (23, 35), (24, 36), (25, 37), (26, 38), (27, 40), (28, 16)]

def readSlot (id : Nat) : Option Nat := readTable.lookup id
def writeSlot (id : Nat) : Option Nat := writeTable.lookup id

/-- `IsFeatureActivated`: unknown ids are never active. -/
def isActive (p : Params) (id h : Nat) : Bool :=
  match readSlot id with
  | none => false
  | some s => decide (p s ≤ h)

inductive ActResult where
  | refusedWindow
  | refusedLive
  | accepted (supported : Bool)
  deriving DecidableEq, Repr

/-- `ActivateFeature` (checks in the C++ order: notice window, already live, then the write). -/
def activate (minB maxB : Nat) (p : Params) (id ab minClient txb : Nat) : Params × ActResult :=
  if ab < txb + minB ∨ txb + maxB < ab then (p, .refusedWindow)
  else if isActive p id txb then (p, .refusedLive)
  else match writeSlot id with
    | none => (p, .accepted false)
    | some s => (setSlot p s ab, .accepted (decide (minClient ≤ tlVersion)))

/-- `DeactivateFeature`: only a live feature can be deactivated. -/
def deactivate (p : Params) (id txb : Nat) : Params × Bool :=
  if isActive p id txb then
    match writeSlot id with
    | none => (p, false)
    | some s => (setSlot p s never, true)
  else (p, false)

/-! ### The tables -/

theorem unknown_ids_have_no_slot (id : Nat) (h : id = 0 ∨ 28 < id) :
    readSlot id = none ∧ writeSlot id = none := by
  have key : ∀ (t : List (Nat × Nat)), (∀ q ∈ t, 1 ≤ q.1 ∧ q.1 ≤ 28) → t.lookup id = none := by
    intro t ht
    induction t with
    | nil => rfl
    | cons q t ih =>
      have hq := ht q (by simp)
      have hne : ¬ (id == q.1) = true := by
        intro heq
        have := beq_iff_eq.mp heq
        omega
      simp only [List.lookup, hne]
      · exact ih (fun r hr => ht r (by simp [hr]))
  exact ⟨key readTable (by decide), key writeTable (by decide)⟩

theorem known_ids_have_slots : ∀ i : Fin 29, 1 ≤ i.val → (readSlot i.val).isSome ∧ (writeSlot i.val).isSome := by
  decide

/-- Exactly two features are read from a different slot than the one written (feature 12 and 28). -/
theorem aliased_features : ∀ i : Fin 29, 1 ≤ i.val →
    (readSlot i.val = writeSlot i.val ∨ i.val = 12 ∨ i.val = 28) := by
  decide

theorem aliased_features_differ : readSlot 12 ≠ writeSlot 12 ∧ readSlot 28 ≠ writeSlot 28 := by decide

/-! ### Activation predicate -/

/-- A feature is active exactly when the current block has reached the height in its read slot. -/
theorem isActive_iff (p : Params) (id h s : Nat) (hs : readSlot id = some s) :
    isActive p id h = true ↔ p s ≤ h := by
  simp [isActive, hs]

/-- Not active before the activation height. -/
theorem not_active_before (p : Params) (id h s : Nat) (hs : readSlot id = some s) (hlt : h < p s) :
    isActive p id h = false := by
  simp [isActive, hs]
  omega

/-- Unknown feature ids are never active, at any height. -/
theorem unknown_never_active (p : Params) (id h : Nat) (hid : id = 0 ∨ 28 < id) :
    isActive p id h = false := by
  simp [isActive, (unknown_ids_have_no_slot id hid).1]

/-- Once active, a feature stays active at later heights (for fixed parameters). -/
theorem active_mono (p : Params) (id h h' : Nat) (hle : h ≤ h') (ha : isActive p id h = true) :
    isActive p id h' = true := by
  unfold isActive at *
  cases hs : readSlot id with
  | none => simp [hs] at ha
  | some s =>
    simp [hs] at ha ⊢
    omega

/-! ### Activation transaction -/

theorem activate_window (minB maxB : Nat) (p : Params) (id ab m txb : Nat) (sup : Bool)
    (h : (activate minB maxB p id ab m txb).2 = .accepted sup) :
    txb + minB ≤ ab ∧ ab ≤ txb + maxB := by
  unfold activate at h
  by_cases hw : ab < txb + minB ∨ txb + maxB < ab
  · simp [hw] at h
  · omega

theorem activate_not_live (minB maxB : Nat) (p : Params) (id ab m txb : Nat) (sup : Bool)
    (h : (activate minB maxB p id ab m txb).2 = .accepted sup) :
    isActive p id txb = false := by
  unfold activate at h
  by_cases hw : ab < txb + minB ∨ txb + maxB < ab
  · simp [hw] at h
  · by_cases hl : isActive p id txb = true
    · simp [hw, hl] at h
    · simpa using hl

/-- An unknown feature id never changes the parameter table and is never reported as supported. -/
theorem activate_unknown (minB maxB : Nat) (p : Params) (id ab m txb : Nat) (hid : id = 0 ∨ 28 < id) :
    (activate minB maxB p id ab m txb).1 = p ∧ (activate minB maxB p id ab m txb).2 ≠ .accepted true := by
  have hw := (unknown_ids_have_no_slot id hid).2
  have hr := unknown_never_active p id txb hid
  unfold activate
  by_cases h1 : ab < txb + minB ∨ txb + maxB < ab
  · simp [h1]
  · simp [h1, hr, hw]

/-- A known feature accepted for activation gets exactly the requested height in its write slot. -/
theorem activate_sets_slot (minB maxB : Nat) (p : Params) (id ab m txb s : Nat) (sup : Bool)
    (hs : writeSlot id = some s)
    (h : (activate minB maxB p id ab m txb).2 = .accepted sup) :
    (activate minB maxB p id ab m txb).1 s = ab ∧
    ∀ i, i ≠ s → (activate minB maxB p id ab m txb).1 i = p i := by
  unfold activate at h ⊢
  by_cases hw : ab < txb + minB ∨ txb + maxB < ab
  · simp [hw] at h
  · by_cases hl : isActive p id txb = true
    · simp [hw, hl] at h
    · simp [hw, hl, hs, setSlot]
      intro i hi
      simp [hi]

/-- For a non-aliased feature, after an accepted activation the feature is active exactly from `ab`. -/
theorem activate_then_active (minB maxB : Nat) (p : Params) (id ab m txb s : Nat) (sup : Bool)
    (hr : readSlot id = some s) (hw : writeSlot id = some s)
    (h : (activate minB maxB p id ab m txb).2 = .accepted sup) (hh : Nat) :
    isActive (activate minB maxB p id ab m txb).1 id hh = true ↔ ab ≤ hh := by
  have := (activate_sets_slot minB maxB p id ab m txb s sup hw h).1
  rw [isActive_iff _ id hh s hr, this]

/-! ### Deactivation -/

theorem deactivate_requires_live (p : Params) (id txb : Nat) (h : (deactivate p id txb).2 = true) :
    isActive p id txb = true := by
  unfold deactivate at h
  by_cases hl : isActive p id txb = true
  · exact hl
  · simp [hl] at h

/-- After a deactivation of a non-aliased feature it is inactive at every height below `never`. -/
theorem deactivate_inactive (p : Params) (id txb s : Nat) (hr : readSlot id = some s) (hw : writeSlot id = some s)
    (h : (deactivate p id txb).2 = true) (hh : Nat) (hlt : hh < never) :
    isActive (deactivate p id txb).1 id hh = false := by
  have hl := deactivate_requires_live p id txb h
  unfold deactivate
  simp [hl, hw]
  rw [Bool.eq_false_iff]
  intro hact
  have := (isActive_iff _ id hh s hr).mp hact
  simp [setSlot] at this
  omega

/-- The C++ quirk, as a theorem: deactivating FEATURE_CONTRACTDEX (12) succeeds whenever it reads as
live, but it clears the contract-dex height while `IsFeatureActivated` keeps reading the node-reward
height, so the feature stays active. -/
theorem contractdex_deactivation_is_ineffective (p : Params) (txb : Nat) (hl : isActive p 12 txb = true) :
    (deactivate p 12 txb).2 = true ∧ isActive (deactivate p 12 txb).1 12 txb = true := by
  have hw : writeSlot 12 = some 21 := by decide
  have hr : readSlot 12 = some 23 := by decide
  have hp : p 23 ≤ txb := (isActive_iff p 12 txb 23 hr).mp hl
  unfold deactivate
  simp [hl, hw]
  rw [isActive_iff _ 12 txb 23 hr]
  simp [setSlot, hp]

/-! ### Transaction restriction table -/

structure Restriction where
  ty : Nat
  ver : Nat
  block : Nat

/-- `IsTransactionTypeAllowed`: some row matches the exact type and version and has been reached. -/
def allowed (rs : List Restriction) (txb ty ver : Nat) : Bool :=
  rs.any fun r => r.ty == ty && r.ver == ver && decide (r.block ≤ txb)

theorem allowed_mono (rs : List Restriction) (ty ver b b' : Nat) (hle : b ≤ b')
    (h : allowed rs b ty ver = true) : allowed rs b' ty ver = true := by
  unfold allowed at *
  rw [List.any_eq_true] at *
  obtain ⟨r, hr, hr'⟩ := h
  refine ⟨r, hr, ?_⟩
  simp at hr' ⊢
  omega

theorem allowed_exact_version (rs : List Restriction) (txb ty ver : Nat) (h : allowed rs txb ty ver = true) :
    ∃ r ∈ rs, r.ty = ty ∧ r.ver = ver ∧ r.block ≤ txb := by
  unfold allowed at h
  rw [List.any_eq_true] at h
  obtain ⟨r, hr, hr'⟩ := h
  simp at hr'
  exact ⟨r, hr, hr'.1.1, hr'.1.2, hr'.2⟩

/-- The three rows for alert / activation / deactivation (`GetRestrictions`): version `0xFFFF` only. -/
def alertRows (alertBlock : Nat) : List Restriction :=
  [⟨65535, 65535, alertBlock⟩, ⟨65534, 65535, alertBlock⟩, ⟨65533, 65535, alertBlock⟩]

/-- Activation / deactivation payloads are built with version 1 (`createpayload.cpp`), but the
restriction rows require version 65535: at no height is such a transaction type-allowed. -/
theorem activation_version1_never_allowed (alertBlock txb : Nat) :
    allowed (alertRows alertBlock) txb 65534 1 = false ∧ allowed (alertRows alertBlock) txb 65533 1 = false := by
  simp [allowed, alertRows]

/-! ### Concrete vectors (regtest, the C++-printed scenario `regtest_fixed`) -/

/-- Regtest parameter block, slot order of `Param::ALL`. -/
def regtestList : List Nat :=
  [0, 0, 5, 1000, 0, 0, 0, 0, 0, 0, 200, 0, 0, 1000, 100, 100, 200, 0, 0, 0, 0, 0, 0, 200, 0, 0,
   99999999, 99999999, 99999999, 99999999, 99999999, 99999999, 99999999, 99999999, 99999999,
   99999999, 99999999, 99999999, 99999999, 99999999, 99999999, 99999999, 930]

def regtest : Params := fun s => regtestList.getD s 0

-- scn/regtest_fixed/0: activation block 104 < tx 100 + MIN 5 -> refused
example : (activate 5 1000 regtest 8 104 1 100).2 = .refusedWindow := by decide
-- scn/regtest_fixed/1: 1101 > 100 + 1000 -> refused
example : (activate 5 1000 regtest 8 1101 1 100).2 = .refusedWindow := by decide
-- scn/regtest_fixed/2: feature 8 (MSC_SP_BLOCK, slot 10) is scheduled at 200, not live at 100: accepted, reschedules to 150
example : (activate 5 1000 regtest 8 150 1 100).2 = .accepted true := by decide
example : (activate 5 1000 regtest 8 150 1 100).1 10 = 150 := by decide
-- scn/regtest_fixed/3: at tx 250 the rescheduled feature is live -> refused
example : (activate 5 1000 (activate 5 1000 regtest 8 150 1 100).1 8 400 1 250).2 = .refusedLive := by decide
-- scn/regtest_fixed/5: deactivation at 250 succeeds and writes 99999999
example : (deactivate (activate 5 1000 regtest 8 150 1 100).1 8 250).2 = true := by decide
example : (deactivate (activate 5 1000 regtest 8 150 1 100).1 8 250).1 10 = never := by decide
-- scn/regtest_fixed/8,9: unknown ids are accepted but unsupported, parameter table untouched
example : (activate 5 1000 regtest 99 300 1 250).2 = .accepted false := by decide
example : (activate 5 1000 regtest 0 300 1 250).2 = .accepted false := by decide
-- scn/regtest_fixed/11-13: feature 14 at TL_VERSION is supported, one above is not
example : (activate 5 1000 regtest 14 300 20002000 250).2 = .accepted true := by decide
example : (activate 5 1000 regtest 14 300 20002001 250).2 = .accepted false := by decide
-- scn/regtest_alias/0: deactivating feature 12 at 250 returns true yet feature 12 stays active
example : (deactivate regtest 12 250).2 = true := by decide
example : isActive (deactivate regtest 12 250).1 12 250 = true := by decide
-- is_feature_activated/regtest/f28/b99 (vesting slot 100) and /b100
example : isActive regtest 28 99 = false := by decide
example : isActive regtest 28 100 = true := by decide

end TlCoreRef
