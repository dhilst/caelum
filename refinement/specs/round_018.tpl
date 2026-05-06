// Round 18: Biconditional operator (<->) in properties
// System: mod-4 counter cycling through 0,1,2,3,0,1,...
// Properties exercise <-> (iff) in various contexts

module round_018

let x: 0..3

init {
  x = 0
}

// Single transition: cycle 0 -> 1 -> 2 -> 3 -> 0
transition cycle {
  x' = (x + 1) mod 4
}

// PASS: x = 0 iff x < 1 — both true at x=0, both false at x=1,2,3
property iff_eq_zero_lt_one {
  always (x = 0 <-> x < 1)
}

// FAIL: x = 0 iff x = 1 — at x=0 the LHS is true but RHS is false
property iff_eq_zero_eq_one {
  always (x = 0 <-> x = 1)
}

// PASS: x >= 2 iff x > 1 — both true at x=2,3 and both false at x=0,1
property iff_ge_two_gt_one {
  always (x >= 2 <-> x > 1)
}

// PASS: mixing <-> with and/or/not
// (x = 0 or x = 3) <-> (x != 1 and x != 2) — both describe {0,3}
property iff_mixed_connectives {
  always ((x = 0 or x = 3) <-> (x != 1 and x != 2))
}
