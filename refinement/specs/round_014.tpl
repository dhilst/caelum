// Round 14: mod operator exercised in init, transition, and property contexts
// Tests: mod in init expression, mod in transition wrapping, mod in property checks

module round_014

let x: 0..5

// Init: use mod to set starting value (6 mod 6 = 0)
init {
  x = 6 mod 6
}

// Transition: counter wraps around via mod
transition step {
  x' = (x + 1) mod 6
}

// PASS: mod result is always non-negative for non-negative values mod positive
property mod_nonneg {
  always (x mod 6 >= 0)
}

// PASS: x mod 3 is always in {0, 1, 2}, so always < 3
property mod_bound {
  always (x mod 3 < 3)
}

// PASS: x mod 2 = 0 means x is even. x visits 0,1,2,3,4,5 so it
// is not always even, but it always eventually becomes even (0,2,4 are even)
property eventually_even {
  always eventually (x mod 2 = 0)
}

// FAIL: x mod 2 = 0 does not hold always, since x visits odd values 1,3,5
property always_even {
  always (x mod 2 = 0)
}
