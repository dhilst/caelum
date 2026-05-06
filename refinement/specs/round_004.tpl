// Round 4: Single integer variable exercising `eventually` (◇)
//
// System: x ranges over 0..3. Init at 0.
// Non-deterministic transitions: x can either increment (mod 4) or stay put.
//
// Property eventually_zero (PASS):
//   eventually (x = 0) — since x starts at 0, the initial state already
//   satisfies x = 0, so the property holds trivially.
//
// Property eventually_three (FAIL):
//   eventually (x = 3) — because x can always choose to stay at its current
//   value, there exists a path (e.g. 0 -> 0 -> 0 -> ...) where x never
//   reaches 3. Under universal path quantification this means the property
//   fails.

module round_004

let x: 0..3

init {
  x = 0
}

// Non-deterministic: increment with wraparound
transition inc {
  x' = (x + 1) mod 4
}

// Non-deterministic: stutter (stay at current value)
transition stutter {
  x' = x
}

// PASS: x starts at 0, so "eventually (x = 0)" is immediately satisfied
property eventually_zero {
  eventually (x = 0)
}

// FAIL: from x = 0 there is a path (stutter forever) that never reaches x = 3
property eventually_three {
  eventually (x = 3)
}
