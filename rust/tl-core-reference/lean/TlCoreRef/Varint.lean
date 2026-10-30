namespace TlCoreRef

/-!
# TradeLayer compressed integers

Models `tl_core_reference::varint` (Rust) and `CompressInteger` / `DecompressInteger`
(`src/tradelayer/varint.cpp`).

* `encodeVarint` is `push_compressed`: least significant 7-bit group first, MSB set on every byte
  except the last. The Rust loop runs at most 10 times for a `u64`; the model makes that explicit
  with a fuel parameter (`encF 10`), which keeps every definition structurally recursive so that
  the concrete test vectors below are checked by `decide`.
* `readVarint` is `read_strict`: it rejects truncated input, non-canonical encodings (a zero final
  group after the first byte) and values that do not fit in 64 bits. The Rust code also rejects
  more than ten bytes with a dedicated error; in the model that case is already covered by the
  canonical and 64-bit checks (an eleven-group canonical value is at least 2^70).

Abstraction: bytes are `Nat` values (all produced values are `< 256`), heights and amounts are
unbounded `Nat`s except where a theorem states the `< 2^64` bound.
-/

/-- Seven-bit group encoder with explicit fuel. -/
def encF : Nat → Nat → List Nat
  | 0, v => [v]
  | f + 1, v => if v < 128 then [v] else (v % 128 + 128) :: encF f (v / 128)

/-- `push_compressed` for a `u64` (at most ten groups). -/
def encodeVarint (v : Nat) : List Nat := encF 10 v

/-- Strict reader core. `n` is the index of the next 7-bit group, `acc` the value so far. -/
def readAux : Nat → Nat → List Nat → Option (Nat × List Nat)
  | _, _, [] => none
  | n, acc, b :: rest =>
    if b < 128 then
      if n > 0 ∧ b = 0 then none else some (acc + b * 128 ^ n, rest)
    else readAux (n + 1) (acc + (b % 128) * 128 ^ n) rest

/-- `read_strict`: value and remaining input, or `none` (truncated / non-canonical / overflow). -/
def readVarint (l : List Nat) : Option (Nat × List Nat) :=
  match readAux 0 0 l with
  | some (v, r) => if v < 2 ^ 64 then some (v, r) else none
  | none => none

/-! ### Concrete vectors (the same bytes the C++ `CompressInteger` printed; see
`fixtures/payload_vectors.tsv` and `tests/payload_vectors.rs`). -/

example : encodeVarint 0 = [0x00] := by decide
example : encodeVarint 127 = [0x7f] := by decide
example : encodeVarint 128 = [0x80, 0x01] := by decide
example : encodeVarint 300 = [0xac, 0x02] := by decide
example : encodeVarint 16384 = [0x80, 0x80, 0x01] := by decide
example : encodeVarint 4294967295 = [0xff, 0xff, 0xff, 0xff, 0x0f] := by decide
example : encodeVarint 18446744073709551615 =
    [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01] := by decide
example : encodeVarint 9223372036854775808 =
    [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01] := by decide
example : readVarint [0xac, 0x02] = some (300, []) := by decide
-- non-canonical and overflowing forms are rejected (the C++ reader accepts them)
example : readVarint [0x80, 0x00] = none := by decide
example : readVarint [0xff, 0x00] = none := by decide
example : readVarint [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02] = none := by decide
example : readVarint [] = none := by decide
example : readVarint [0x80] = none := by decide

/-! ### Round trip -/

theorem readAux_encF (f : Nat) : ∀ (v n acc : Nat) (rest : List Nat),
    v < 128 ^ f → (n = 0 ∨ 1 ≤ v) →
    readAux n acc (encF f v ++ rest) = some (acc + v * 128 ^ n, rest) := by
  induction f with
  | zero =>
    intro v n acc rest hv hn
    have hv0 : v = 0 := by simp at hv; omega
    subst hv0
    have hn0 : n = 0 := by omega
    subst hn0
    simp [encF, readAux]
  | succ f ih =>
    intro v n acc rest hv hn
    by_cases h : v < 128
    · have hmod : v % 128 = v := Nat.mod_eq_of_lt h
      have hcanon : ¬ (n > 0 ∧ v = 0) := by omega
      simp [encF, h, readAux, hcanon]
    · have h128 : ¬ v < 128 := h
      have hge : 128 ≤ v := by omega
      have hq : 1 ≤ v / 128 := by omega
      have hlt : v / 128 < 128 ^ f := by
        have : 128 ^ (f + 1) = 128 ^ f * 128 := Nat.pow_succ _ _
        rw [this] at hv
        omega
      have hb : ¬ (v % 128 + 128 < 128) := by omega
      have step := ih (v / 128) (n + 1) (acc + v % 128 * 128 ^ n) rest hlt (Or.inr hq)
      have hm : (v % 128 + 128) % 128 = v % 128 := by omega
      simp only [encF, h128, hb, hm, List.cons_append, readAux, ↓reduceIte]
      rw [step]
      congr 2
      -- acc + (v%128)*128^n + (v/128)*128^(n+1) = acc + v*128^n
      have hv' : v = v % 128 + 128 * (v / 128) := (Nat.mod_add_div v 128).symm
      have hp : 128 ^ (n + 1) = 128 ^ n * 128 := Nat.pow_succ _ _
      rw [hp]
      generalize 128 ^ n = a
      generalize hq' : v / 128 = q at hv'
      generalize hr : v % 128 = r at hv'
      rw [hv']
      rw [Nat.add_mul, Nat.mul_comm 128 q, Nat.mul_assoc, Nat.mul_comm 128 a]
      omega

theorem two_pow_64_le : 2 ^ 64 ≤ 128 ^ 10 := by decide

/-- `read_strict (push_compressed v ++ rest) = Ok (v, rest)` for every `u64`. -/
theorem readVarint_encodeVarint (v : Nat) (hv : v < 2 ^ 64) (rest : List Nat) :
    readVarint (encodeVarint v ++ rest) = some (v, rest) := by
  have h := readAux_encF 10 v 0 0 rest (Nat.lt_of_lt_of_le hv two_pow_64_le) (Or.inl rfl)
  simp only [encodeVarint, readVarint]
  rw [h]
  simp [hv]

/-- Compressed integers are prefix-free and injective on `u64`. -/
theorem encodeVarint_injective (a b : Nat) (ha : a < 2 ^ 64) (hb : b < 2 ^ 64)
    (h : encodeVarint a = encodeVarint b) : a = b := by
  have h1 := readVarint_encodeVarint a ha []
  have h2 := readVarint_encodeVarint b hb []
  rw [h] at h1
  rw [h1] at h2
  simp at h2
  exact h2

/-- Every strict prefix of an encoding is rejected as truncated. -/
theorem readAux_encF_prefix (f : Nat) : ∀ (v n acc k : Nat),
    k < (encF f v).length → readAux n acc ((encF f v).take k) = none := by
  induction f with
  | zero =>
    intro v n acc k hk
    have : k = 0 := by simp [encF] at hk; omega
    subst this
    simp [readAux]
  | succ f ih =>
    intro v n acc k hk
    by_cases h : v < 128
    · have : k = 0 := by simp [encF, h] at hk; omega
      subst this
      simp [readAux]
    · have h128 : ¬ v < 128 := h
      have hb : ¬ (v % 128 + 128 < 128) := by omega
      cases k with
      | zero => simp [readAux]
      | succ j =>
        have hj : j < (encF f (v / 128)).length := by
          simp [encF, h128] at hk
          omega
        simp only [encF, h128, ↓reduceIte, List.take_succ_cons, readAux, hb]
        exact ih (v / 128) (n + 1) _ j hj

theorem readVarint_truncated (v : Nat) (k : Nat) (hk : k < (encodeVarint v).length) :
    readVarint ((encodeVarint v).take k) = none := by
  have := readAux_encF_prefix 10 v 0 0 k hk
  simp only [encodeVarint, readVarint]
  rw [this]

/-- All produced bytes are real bytes. -/
theorem encF_bytes_lt (f : Nat) : ∀ (v : Nat), v < 128 ^ f → ∀ b ∈ encF f v, b < 256 := by
  induction f with
  | zero =>
    intro v hv b hb
    have : v = 0 := by simp at hv; omega
    subst this
    simp [encF] at hb
    omega
  | succ f ih =>
    intro v hv b hb
    by_cases h : v < 128
    · simp [encF, h] at hb
      omega
    · have hlt : v / 128 < 128 ^ f := by
        have : 128 ^ (f + 1) = 128 ^ f * 128 := Nat.pow_succ _ _
        rw [this] at hv
        omega
      simp [encF, h] at hb
      rcases hb with hb | hb
      · omega
      · exact ih (v / 128) hlt b hb

theorem encodeVarint_bytes_lt (v : Nat) (hv : v < 2 ^ 64) : ∀ b ∈ encodeVarint v, b < 256 :=
  encF_bytes_lt 10 v (Nat.lt_of_lt_of_le hv two_pow_64_le)

/-- A `u64` needs at most ten bytes. -/
theorem encF_length_le (f : Nat) : ∀ (v : Nat), v < 128 ^ f → 1 ≤ f → (encF f v).length ≤ f := by
  induction f with
  | zero => intro v hv h; omega
  | succ f ih =>
    intro v hv _
    by_cases h : v < 128
    · simp [encF, h]
    · have hlt : v / 128 < 128 ^ f := by
        have : 128 ^ (f + 1) = 128 ^ f * 128 := Nat.pow_succ _ _
        rw [this] at hv
        omega
      have hf : 1 ≤ f := by
        cases f with
        | zero => simp at hv; omega
        | succ _ => omega
      have := ih (v / 128) hlt hf
      simp [encF, h]
      omega

theorem encodeVarint_length_le (v : Nat) (hv : v < 2 ^ 64) : (encodeVarint v).length ≤ 10 :=
  encF_length_le 10 v (Nat.lt_of_lt_of_le hv two_pow_64_le) (by decide)

end TlCoreRef
