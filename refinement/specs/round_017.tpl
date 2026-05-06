// Round 17: Implication operator (->) in properties
// System: mod-4 counter cycling through 0,1,2,3,0,1,...
// Properties exercise -> in various contexts

module round_017

let x: 0..3

init {
  x = 0
}

// Single transition: cycle 0 -> 1 -> 2 -> 3 -> 0
transition cycle {
  x' = (x + 1) mod 4
}

// PASS: when x = 3, next state wraps to 0
property wrap_at_three {
  always (x = 3 -> next (x = 0))
}

// PASS: when x = 0, next state is 1
property step_from_zero {
  always (x = 0 -> next (x = 1))
}

// PASS: tautology — x > 0 implies x >= 1 (always true for integers)
property tautology_gt_ge {
  always (x > 0 -> x >= 1)
}

// when x = 0, the antecedent is true but x = 1 is false
invalid false_consequent {
  always (x = 0 -> x = 1)
}
