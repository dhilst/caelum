// Round 38, Tier 2: Toggle pattern with multiple variables (UNICODE syntax)
//
// System: 3-bit ripple counter using three boolean variables.
//
// This models a binary counter that increments by 1 each step, using
// three booleans b0 (LSB), b1, and b2 (MSB). The "toggle" pattern is
// a cascading carry: b0 flips every step, b1 flips when b0 carries
// (was true), and b2 flips when both b0 and b1 carry (both were true).
//
// Variables:
//   b0 : bool   -- bit 0 (least significant), toggles every step
//   b1 : bool   -- bit 1, toggles when b0 carries (b0 was true)
//   b2 : bool   -- bit 2 (most significant), toggles when b0 and b1 both carry
//
// Init: b0 = false, b1 = false, b2 = false  (counter = 0)
//
// Transitions (three cases, exactly one enabled per state):
//   no_carry:    b0 = false -> b0 flips to true, b1 and b2 unchanged
//   carry_one:   b0 = true, b1 = false -> b0 flips, b1 flips, b2 unchanged
//   carry_two:   b0 = true, b1 = true -> b0 flips, b1 flips, b2 flips
//
// Deterministic trace (all 8 states visited in order):
//   s0: b2=F b1=F b0=F  (0)  -> no_carry   -> s1
//   s1: b2=F b1=F b0=T  (1)  -> carry_one  -> s2
//   s2: b2=F b1=T b0=F  (2)  -> no_carry   -> s3
//   s3: b2=F b1=T b0=T  (3)  -> carry_two  -> s4
//   s4: b2=T b1=F b0=F  (4)  -> no_carry   -> s5
//   s5: b2=T b1=F b0=T  (5)  -> carry_one  -> s6
//   s6: b2=T b1=T b0=F  (6)  -> no_carry   -> s7
//   s7: b2=T b1=T b0=T  (7)  -> carry_two  -> s0 (wraps to 0)
//
// Cycle length: 8 (visits every combination of 3 booleans exactly once).
//
// Key observations:
//   1. b0 alternates every step: F,T,F,T,F,T,F,T,...
//   2. b1 alternates every 2 steps: F,F,T,T,F,F,T,T,...
//   3. b2 alternates every 4 steps: F,F,F,F,T,T,T,T,...
//   4. The system is fully deterministic (exactly one transition enabled per state).
//   5. Every state is visited exactly once per full cycle of 8 steps.
//   6. When b0 is true AND b1 is false, the next step has b1 = true (carry from b0).
//   7. When b0 = true AND b1 = true, carry propagates through to b2.
//
// ALL operators use UNICODE syntax:
//   □ always, ◇ eventually, ◯ next, 𝒰 until
//   ∧ and, ∨ or, ¬ not
//   -> implies, <-> iff

module round_038

let b0: bool
let b1: bool
let b2: bool

init {
  b0 = false ∧ b1 = false ∧ b2 = false
}

// Case 1: b0 is false, no carry propagation. Only b0 flips.
transition no_carry {
  b0 = false
  ∧ b0' = true ∧ b1' = b1 ∧ b2' = b2
}

// Case 2: b0 is true but b1 is false. Carry from b0 flips b1, but no further carry.
transition carry_one {
  b0 = true ∧ b1 = false
  ∧ b0' = false ∧ b1' = true ∧ b2' = b2
}

// Case 3: both b0 and b1 are true. Carry propagates through: all three flip.
transition carry_two {
  b0 = true ∧ b1 = true
  ∧ b0' = false ∧ b1' = false ∧ b2' = ¬ b2
}

// --- PROPERTY 1 (PASS) ---
// b0 toggles every single step without exception.
// At every state, if b0 is true now then b0 is false next, and vice versa.
// This holds because: no_carry sets b0'=true when b0=false,
// carry_one and carry_two set b0'=false when b0=true.
property b0_toggles_every_step {
  □ (b0 = true <-> ◯ (b0 = false))
}

// --- PROPERTY 2 (PASS) ---
// The all-false state (counter = 0) recurs infinitely often.
// The cycle has period 8 and passes through (F,F,F) every 8 steps.
property all_false_recurs {
  □ ◇ (b2 = false ∧ b1 = false ∧ b0 = false)
}

// --- PROPERTY 3 (PASS) ---
// Carry propagation from b0 to b1: when b0 is true and b1 is false,
// the next state has b1 = true (carry_one fires).
// Verified: carry_one is the only transition enabled when b0=T, b1=F,
// and it sets b1' = true.
property carry_into_b1 {
  □ (b0 = true ∧ b1 = false -> ◯ (b1 = true))
}

// --- PROPERTY 4 (PASS) ---
// Full carry propagation: when b0=T and b1=T and b2=F, the next state
// has b2=true (carry_two fires and flips b2 from false to true).
property full_carry_to_b2 {
  □ (b0 = true ∧ b1 = true ∧ b2 = false -> ◯ (b2 = true))
}

// --- PROPERTY 5 (PASS) ---
// b0 is true infinitely often (it's true every other step: s1, s3, s5, s7).
property b0_true_infinitely {
  □ ◇ (b0 = true)
}

// --- PROPERTY 6 (PASS) ---
// Phase relationship: b2 and b0 are never both true while b1 is false
// at the NEXT step after b2 was false.
// More precisely: from state (b2=F, b1=T, b0=T) i.e. counter=3,
// carry_two fires giving (T, F, F) i.e. counter=4.
// So: when b2=false and b1=true and b0=true, next state has b2=true and b1=false and b0=false.
property carry_two_result {
  □ (b2 = false ∧ b1 = true ∧ b0 = true -> ◯ (b2 = true ∧ b1 = false ∧ b0 = false))
}

// --- INVALID 1 (expected FAIL) ---
// "b1 toggles every step" -- FALSE.
// b1 does NOT flip every step; it only flips when b0 carries.
// Counterexample: s2 has b1=T, b0=F. no_carry fires, giving s3 with b1=T (unchanged).
// So b1=true at s2 and b1=true at s3, violating b1=true -> next(b1=false).
invalid b1_toggles_every_step {
  □ (b1 = true <-> ◯ (b1 = false))
}

// --- INVALID 2 (expected FAIL) ---
// "b2 is never true" -- FALSE.
// b2 becomes true at s4 (counter=4) when carry_two fires from s3.
// Trace: s0 -> s1 -> s2 -> s3 -> s4, where s4 has b2=true.
invalid b2_never_true {
  □ (b2 = false)
}

// --- INVALID 3 (expected FAIL) ---
// "Once b2 becomes true, it stays true forever" -- FALSE.
// b2 flips back to false at s7 -> s0: carry_two fires from (T,T,T)
// giving (F,F,F). So b2 goes from true back to false.
invalid b2_stays_true {
  □ (b2 = true -> □ (b2 = true))
}

// --- INVALID 4 (expected FAIL) ---
// "b0 and b1 are always opposite (anti-phase)" -- FALSE.
// At s0, b0=F and b1=F, both false -- not opposite.
// At s3, b0=T and b1=T, both true -- not opposite.
invalid b0_b1_anti_phase {
  □ (b0 = true <-> b1 = false)
}
