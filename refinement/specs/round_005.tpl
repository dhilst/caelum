// Round 5: Single integer variable exercising `next` (◯)
//
// System: x ranges over 0..2. Init at x = 0.
// Deterministic transition: x increments modulo 3.
//   x = 0 -> x = 1
//   x = 1 -> x = 2
//   x = 2 -> x = 0
//
// Property next_is_one (PASS):
//   next (x = 1) — from the sole initial state x = 0, the only successor
//   is x = 1, so in every trace position 1 has x = 1.
//
// Property next_is_zero (FAIL):
//   next (x = 0) — from x = 0 the successor is x = 1, not x = 0,
//   so this property is violated. The counterexample should show
//   s0: x = 0, s1: x = 1.

module round_005

let x: 0..2

init {
  x = 0
}

transition inc {
  x' = (x + 1) mod 3
}

// PASS: from x = 0 the only successor is x = 1
property next_is_one {
  next (x = 1)
}

// from x = 0 the successor is x = 1, not x = 0
invalid next_is_zero {
  next (x = 0)
}
