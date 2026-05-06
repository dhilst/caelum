// Round 24: Multiple properties (mixed pass/fail)
//
// System: counter x in 0..4, cycling mod 5.
// Init: x = 0. Transition: x' = (x + 1) mod 5.
// Deterministic trace: 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, ...
//
// We test 8 properties across different categories to verify the engine
// evaluates each property independently — a failing property should not
// affect the evaluation of others.
//
// Safety (always):
//   1. safe_range         — always (x >= 0 and x <= 4)    — PASS (domain is 0..4)
//   2. safe_never_five    — always (x < 5)                 — PASS (x mod 5 never reaches 5)
//   3. safe_always_even   — always (x mod 2 = 0)           — FAIL (x=1,3 are odd)
//
// Liveness (eventually):
//   4. live_reach_four    — always eventually (x = 4)      — PASS (cycle visits 4)
//   5. live_reach_five    — eventually (x = 5)             — FAIL (x never equals 5)
//
// Temporal (next):
//   6. next_is_one        — next (x = 1)                   — PASS (from x=0, next is 1)
//   7. next_is_three      — next (x = 3)                   — FAIL (from x=0, next is 1 not 3)
//
// Temporal (until):
//   8. until_reach_four   — (x < 4) until (x = 4)         — PASS (0,1,2,3 < 4 then x=4)

module round_024

let x: 0..4

init {
  x = 0
}

transition step {
  x' = (x + 1) mod 5
}

// --- Safety (always) ---

// PASS: x is always in range [0, 4] by construction
property safe_range {
  always (x >= 0 and x <= 4)
}

// PASS: x never reaches 5 since mod 5 keeps it in 0..4
property safe_never_five {
  always (x < 5)
}

// FAIL: x takes odd values 1 and 3, so x mod 2 = 0 is not always true
property safe_always_even {
  always (x mod 2 = 0)
}

// --- Liveness (eventually) ---

// PASS: the cycle 0,1,2,3,4,0,... revisits x=4 every 5 steps
property live_reach_four {
  always eventually (x = 4)
}

// FAIL: x ranges over 0..4 only, so x = 5 is never reachable
property live_reach_five {
  eventually (x = 5)
}

// --- Temporal: next ---

// PASS: from initial state x=0, the successor is x=1
property next_is_one {
  next (x = 1)
}

// FAIL: from initial state x=0, the successor is x=1, not x=3
property next_is_three {
  next (x = 3)
}

// --- Temporal: until ---

// PASS: trace 0,1,2,3,4 — x<4 holds at positions 0..3, then x=4 at position 4
property until_reach_four {
  (x < 4) until (x = 4)
}
