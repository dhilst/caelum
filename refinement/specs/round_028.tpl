// Round 28, Tier 2: Two int ranges (UNICODE operator syntax)
//
// System: two int-range variables x ∈ 0..2 and y ∈ 0..2.
//   - x increments each step: x' = (x + 1) mod 3
//   - y follows x with a one-step delay: y' = x
//
// Init: x = 0, y = 0
//
// Trace (deterministic, 3-state cycle):
//   s0: x=0, y=0
//   s1: x=1, y=0
//   s2: x=2, y=1
//   s3: x=0, y=2
//   s4: x=1, y=0
//   s5: x=2, y=1
//   s6: x=0, y=2
//   ...  (cycle of length 3 starting from s1: {(1,0),(2,1),(0,2)})
//
// ALL operators use UNICODE syntax:
//   □ always, ◇ eventually, ◯ next, 𝒰 until
//   ∧ and, ∨ or, ¬ not
//   -> implies, <-> iff

module round_028

let x ∈ 0..2
let y ∈ 0..2

init {
  x = 0 ∧ y = 0
}

transition step {
  x' = (x + 1) mod 3
  ∧ y' = x
}

// PASS: x and y are always within their domain bounds
property both_in_range {
  □ (x >= 0 ∧ x <= 2 ∧ y >= 0 ∧ y <= 2)
}

// PASS: whenever x = 1, y becomes 1 in the next step (consequence of y' = x).
// Trace check: x=1 occurs at s1(x=1,y=0)->s2 has y=1. At s4(x=1,y=0)->s5 has y=1. Correct.
property x1_implies_next_y1 {
  □ ((x = 1) -> ◯ (y = 1))
}

// PASS: the system always eventually reaches x = 0 (cycle visits x=0)
property x_returns_to_zero {
  □ (◇ (x = 0))
}

// PASS: whenever x = 2, eventually y = 2
// From x=2: next state has y=2 (since y' = x = 2)
property x2_implies_eventually_y2 {
  □ ((x = 2) -> ◇ (y = 2))
}

// PASS: x and y are never both equal to 1 simultaneously
// Checking the cycle states: (1,0), (2,1), (0,2) -- none has x=1 ∧ y=1.
// The initial transient s0=(0,0) also doesn't have x=1 ∧ y=1.
property never_both_one {
  □ (¬(x = 1 ∧ y = 1))
}

// PASS: eventually x and y differ (they are equal only at s0 where x=y=0)
// In the cycle they always differ: (1,0), (2,1), (0,2).
property eventually_differ {
  ◇ (¬(x = y))
}

// FAIL (via invalid): x and y are NOT always equal
// They are equal only at the initial state s0=(0,0); in the cycle they always differ.
invalid always_equal {
  □ (x = y)
}

// FAIL (via invalid): it is NOT the case that y is always strictly greater than x
// For example at s1: x=1, y=0, so y < x.
invalid y_always_greater {
  □ (y > x)
}
