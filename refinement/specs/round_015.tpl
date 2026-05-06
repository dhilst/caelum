// Round 15: Division operator (/) exercised in init, transition, and property contexts
// Tests: integer division in init expression, division halving in transition,
//        division in property checks, self-loop at 0 (0/2 = 0)

module round_015

let x: 0..8

// Init: use division to set starting value (16 / 2 = 8)
init {
  x = 16 / 2
}

// Transition: halve x each step via integer division
// When x = 0, 0 / 2 = 0, creating a valid self-loop (no deadlock)
transition halve {
  x' = x / 2
}

// PASS: x starts at 8 and only decreases (8, 4, 2, 1, 0, 0, ...)
// so x is always >= 0
property div_nonneg {
  always (x >= 0)
}

// PASS: x starts at 8 and halving never exceeds 8, so x <= 8 always
property div_upper_bound {
  always (x <= 8)
}

// PASS: x eventually reaches 0 (8 -> 4 -> 2 -> 1 -> 0) and stays there
property eventually_zero {
  eventually (x = 0)
}

// PASS: once x reaches 0, x / 2 = 0 self-loops forever, so always eventually x = 0
property always_eventually_zero {
  always eventually (x = 0)
}

// FAIL: x starts at 8, and 8 / 4 = 2 (not 0), so x / 4 = 0 does not hold always
// At x = 8: 8/4 = 2. At x = 4: 4/4 = 1. At x = 2: 2/4 = 0. At x = 1: 1/4 = 0. At x = 0: 0/4 = 0.
// Fails at x = 8 and x = 4
property always_div4_zero {
  always (x / 4 = 0)
}
