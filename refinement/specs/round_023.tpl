// Round 23: Arithmetic in transition guards
//
// System: counter x in 0..4, starting at x = 1.
// Transition "inc_wrap": uses arithmetic comparison as the guard condition.
//   When x + 1 < 4, increment: x' = x + 1
//   When x + 1 >= 4, wrap to zero: x' = 0
//
// Trace from x = 1: 1, 2, 3, 0, 1, 2, 3, 0, ...
//   x=1: 1+1=2 < 4, so x'=2
//   x=2: 2+1=3 < 4, so x'=3
//   x=3: 3+1=4 >= 4, so x'=0
//   x=0: 0+1=1 < 4, so x'=1
//
// Properties:
//   in_range        — always (x >= 0 and x <= 3)       — PASS (x never reaches 4)
//   cycles_through  — always eventually (x = 0)         — PASS (wrap fires every 4 steps)
//   never_four      — always (x < 4)                    — PASS (guard prevents reaching 4)
//   always_one      — always (x = 1)                    — FAIL (x takes values 1,2,3,0)

module round_023

let x: 0..4

init {
  x = 1
}

transition inc_wrap {
  (x + 1 < 4 and x' = x + 1) or (x + 1 >= 4 and x' = 0)
}

// PASS: the wrapping guard ensures x stays in {0, 1, 2, 3}
property in_range {
  always (x >= 0 and x <= 3)
}

// PASS: x cycles 1 -> 2 -> 3 -> 0 -> 1 -> ..., so 0 is always reached
property cycles_through {
  always eventually (x = 0)
}

// PASS: when x = 3, x+1 = 4 >= 4 so x wraps to 0; x never actually becomes 4
property never_four {
  always (x < 4)
}

// FAIL: x visits 1, 2, 3, 0 — not always 1
property always_one {
  always (x = 1)
}
