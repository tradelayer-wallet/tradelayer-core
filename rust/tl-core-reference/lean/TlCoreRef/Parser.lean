import TlCoreRef.Varint

namespace TlCoreRef

/-!
# Field readers and the segment calculus

`Bytes` are lists of `Nat`. A reader returns the parsed value and the unread remainder.
`Seg rd A x` says that `A` is the canonical encoding of `x` for reader `rd`: the reader parses
`A ++ rest` to `x` leaving exactly `rest`, and every strict prefix of `A` is rejected.
`bindP` sequences readers (the shape of `Reader`-based decoding in `payload.rs`), and the two
generic lemmas below turn per-field `Seg` facts into whole-payload round-trip and truncation facts.
-/

abbrev Bytes := List Nat

/-- Sequence a reader with a continuation. -/
def bindP {α β : Type} (rd : Bytes → Option (α × Bytes)) (f : α → Bytes → Option β) (l : Bytes) : Option β :=
  match rd l with
  | none => none
  | some (x, r) => f x r

/-- Succeed only if the input is exhausted (the Rust decoder's `TrailingBytes` check). -/
def endP {α : Type} (x : α) (l : Bytes) : Option α := if l = [] then some x else none

def Seg {α : Type} (rd : Bytes → Option (α × Bytes)) (A : Bytes) (x : α) : Prop :=
  (∀ rest, rd (A ++ rest) = some (x, rest)) ∧ (∀ k, k < A.length → rd (A.take k) = none)

theorem bindP_seg {α β : Type} {rd : Bytes → Option (α × Bytes)} {A : Bytes} {x : α}
    (h : Seg rd A x) (f : α → Bytes → Option β) (rest : Bytes) :
    bindP rd f (A ++ rest) = f x rest := by
  simp [bindP, h.1 rest]

theorem take_append_seg (A B : Bytes) (k : Nat) (hk : A.length ≤ k) :
    (A ++ B).take k = A ++ B.take (k - A.length) := by
  rw [List.take_append, List.take_of_length_le hk]

/-- If the head field is a segment and the continuation rejects truncated tails, the sequence
rejects every strict prefix of `A ++ B`. -/
theorem bindP_trunc {α β : Type} {rd : Bytes → Option (α × Bytes)} {A B : Bytes} {x : α}
    (h : Seg rd A x) (f : α → Bytes → Option β)
    (hB : ∀ k, k < B.length → f x (B.take k) = none) :
    ∀ k, k < (A ++ B).length → bindP rd f ((A ++ B).take k) = none := by
  intro k hk
  by_cases hkA : k < A.length
  · rw [List.take_append_of_le_length (Nat.le_of_lt hkA)]
    simp [bindP, h.2 k hkA]
  · have hge : A.length ≤ k := by omega
    rw [take_append_seg A B k hge, bindP_seg h]
    apply hB
    simp at hk
    omega

/-! ### Varint fields -/

theorem seg_varint (v : Nat) (hv : v < 2 ^ 64) : Seg readVarint (encodeVarint v) v :=
  ⟨fun rest => readVarint_encodeVarint v hv rest, fun k hk => readVarint_truncated v k hk⟩

/-- A varint field stored in a narrower C++ type: range-checked (`FieldOutOfRange` in Rust). -/
def rdU (m : Nat) (l : Bytes) : Option (Nat × Bytes) :=
  match readVarint l with
  | some (v, r) => if v < m then some (v, r) else none
  | none => none

theorem seg_u (m v : Nat) (hv : v < m) (hm : m ≤ 2 ^ 64) : Seg (rdU m) (encodeVarint v) v := by
  refine ⟨fun rest => ?_, fun k hk => ?_⟩
  · simp [rdU, readVarint_encodeVarint v (Nat.lt_of_lt_of_le hv hm) rest, hv]
  · simp [rdU, readVarint_truncated v k hk]

/-! ### NUL-terminated strings (at most 255 bytes, no embedded NUL) -/

def encStr (s : Bytes) : Bytes := s ++ [0]

def readStr : Bytes → Option (Bytes × Bytes)
  | [] => none
  | b :: rest =>
    if b = 0 then some ([], rest)
    else match readStr rest with
      | some (s, r) => some (b :: s, r)
      | none => none

def readStr255 (l : Bytes) : Option (Bytes × Bytes) :=
  match readStr l with
  | some (s, r) => if s.length ≤ 255 then some (s, r) else none
  | none => none

/-- `Str255` of the Rust crate: at most 255 bytes, none of them NUL. -/
def Str255 (s : Bytes) : Prop := s.length ≤ 255 ∧ ∀ b ∈ s, b ≠ 0

theorem readStr_append (s rest : Bytes) (h : ∀ b ∈ s, b ≠ 0) :
    readStr (s ++ 0 :: rest) = some (s, rest) := by
  induction s with
  | nil => simp [readStr]
  | cons b s ih =>
    have hb : b ≠ 0 := h b (by simp)
    have hs : ∀ c ∈ s, c ≠ 0 := fun c hc => h c (by simp [hc])
    simp [readStr, hb, ih hs]

theorem readStr_unterminated (s : Bytes) (h : ∀ b ∈ s, b ≠ 0) : readStr s = none := by
  induction s with
  | nil => simp [readStr]
  | cons b s ih =>
    have hb : b ≠ 0 := h b (by simp)
    have hs : ∀ c ∈ s, c ≠ 0 := fun c hc => h c (by simp [hc])
    simp [readStr, hb, ih hs]

theorem seg_str (s : Bytes) (h : Str255 s) : Seg readStr255 (encStr s) s := by
  refine ⟨fun rest => ?_, fun k hk => ?_⟩
  · simp [encStr, readStr255, List.append_assoc, readStr_append s rest h.2, h.1]
  · have hk' : k ≤ s.length := by simp [encStr] at hk; omega
    have hpre : ∀ b ∈ s.take k, b ≠ 0 := fun b hb => h.2 b (List.mem_of_mem_take hb)
    simp [encStr, readStr255, List.take_append_of_le_length hk', readStr_unterminated _ hpre]

/-! ### Length-less tails (KYC ids, SendMany amounts) -/

def encList : List Nat → Bytes
  | [] => []
  | x :: xs => encodeVarint x ++ encList xs


/-- Read varints to the end of the input, with fuel. `rz` models the strict SendMany rule
(a zero amount is an error; the C++ parser silently ends the list there). -/
def readAllF (rz : Bool) : Nat → Bytes → Option (List Nat)
  | _, [] => some []
  | 0, _ :: _ => none
  | f + 1, b :: bs =>
    match readVarint (b :: bs) with
    | none => none
    | some (v, r) =>
      if rz = true ∧ v = 0 then none
      else match readAllF rz f r with
        | some xs => some (v :: xs)
        | none => none

/-- Tail reader: consumes everything (fuel = number of remaining bytes). -/
def tailP {β : Type} (rz : Bool) (f : List Nat → β) (l : Bytes) : Option β :=
  match readAllF rz l.length l with
  | some xs => some (f xs)
  | none => none

theorem encodeVarint_ne_nil (v : Nat) : encodeVarint v ≠ [] := by
  unfold encodeVarint
  unfold encF
  split <;> simp

theorem length_le_encList (xs : List Nat) : xs.length ≤ (encList xs).length := by
  induction xs with
  | nil => simp [encList]
  | cons x xs ih =>
    have : 1 ≤ (encodeVarint x).length := by
      have := encodeVarint_ne_nil x
      cases h : encodeVarint x with
      | nil => exact absurd h this
      | cons _ _ => simp
    simp [encList]
    omega

theorem readAllF_encList (rz : Bool) : ∀ (xs : List Nat) (f : Nat), xs.length ≤ f →
    (∀ x ∈ xs, x < 2 ^ 64 ∧ (rz = true → 0 < x)) → readAllF rz f (encList xs) = some xs := by
  intro xs
  induction xs with
  | nil => intro f _ _; cases f <;> simp [encList, readAllF]
  | cons x xs ih =>
    intro f hf hx
    obtain ⟨f', rfl⟩ : ∃ f', f = f' + 1 := ⟨f - 1, by simp at hf; omega⟩
    have hx0 := hx x (by simp)
    have hrest : ∀ y ∈ xs, y < 2 ^ 64 ∧ (rz = true → 0 < y) := fun y hy => hx y (by simp [hy])
    have hlen : xs.length ≤ f' := by simp at hf; omega
    obtain ⟨b, bs, hbs⟩ : ∃ b bs, encodeVarint x = b :: bs := by
      cases h : encodeVarint x with
      | nil => exact absurd h (encodeVarint_ne_nil x)
      | cons b bs => exact ⟨b, bs, rfl⟩
    have key : readVarint (b :: (bs ++ encList xs)) = some (x, encList xs) := by
      rw [← List.cons_append, ← hbs]
      exact readVarint_encodeVarint x hx0.1 _
    have hpos : ¬ (rz = true ∧ x = 0) := by
      intro ⟨h1, h2⟩
      have := hx0.2 h1
      omega
    simp only [encList, hbs, List.cons_append, readAllF, key, hpos, ↓reduceIte, ih f' hlen hrest]

theorem tailP_encList {β : Type} (rz : Bool) (g : List Nat → β) (xs : List Nat)
    (hx : ∀ x ∈ xs, x < 2 ^ 64 ∧ (rz = true → 0 < x)) : tailP rz g (encList xs) = some (g xs) := by
  simp [tailP, readAllF_encList rz xs _ (length_le_encList xs) hx]

end TlCoreRef
