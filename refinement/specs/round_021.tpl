// Round 21: Chained `not` operators (not not P)

module round_021

let x: 0..3

init {
  x = 0
}

transition step {
  (x = 0 and x' = 1) or
  (x = 1 and x' = 2) or
  (x = 2 and x' = 3) or
  (x = 3 and x' = 0)
}

// Double negation: not not (x >= 0) should be equivalent to (x >= 0).
// Since x is always in 0..3, this should PASS.
property double_not_nonneg {
  always (not not (x >= 0))
}

// Parenthesized double negation: not (not (x >= 0)) — should PASS.
property paren_double_not_nonneg {
  always (not (not (x >= 0)))
}

// Single not: x visits 0 on every cycle, so always (not (x = 0)) should FAIL.
property always_not_zero {
  always (not (x = 0))
}

// Double-not equivalence via biconditional: not not P <-> P — should PASS.
property double_not_equiv {
  always (not not (x >= 0) <-> x >= 0)
}

// Double-not on equality: not not (x = 0) at the initial state — should PASS.
property init_double_not_eq {
  not not (x = 0)
}
