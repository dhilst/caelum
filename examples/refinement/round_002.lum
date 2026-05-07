// Round 2: Single integer range variable with always invariant
// Tests: integer range variable (0..3), init block, increment with mod wraparound,
//        always invariant that passes (range check), always invariant that fails (x != 3)

module round_002

let x: 0..3

init {
  x = 0
}

// Increment with wraparound: 0 -> 1 -> 2 -> 3 -> 0 -> ...
transition inc {
  x' = (x + 1) mod 4
}

// PASS: x is always in range [0, 3] because the domain is 0..3
property always_in_range {
  always (x >= 0 and x <= 3)
}

// x starts at 0, increments to 1, 2, 3, then wraps to 0 again.
// Once x reaches 3, this property is violated.
invalid never_three {
  always (x != 3)
}
