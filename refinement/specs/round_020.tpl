// Round 20: Single-value domain edge case (0..0 with self-loop)

module round_020

let x: 0..0

init {
  x = 0
}

transition stay {
  x' = x
}

// x is always 0 in the only reachable state — should PASS
property always_zero {
  always (x = 0)
}

// x is always >= 0 — should PASS
property always_nonneg {
  always (x >= 0)
}

// next state x is still 0 — should PASS
property next_zero {
  next (x = 0)
}

// x = 1 is outside domain 0..0, so x is never 1 — should FAIL
// (or produce a semantic/domain error if the checker rejects out-of-range comparisons)
property eventually_one {
  eventually (x = 1)
}
