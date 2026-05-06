// Round 19: Unary minus operator — bisecting

module round_019

let x: -2..2

init {
  x = -2
}

transition step {
  (x = -2 and x' = -1) or
  (x = -1 and x' = 0) or
  (x = 0 and x' = 1) or
  (x = 1 and x' = 2) or
  (x = 2 and x' = -2)
}

property in_range {
  always (x >= -2 and x <= 2)
}

property neg_in_range {
  always (-x >= -2 and -x <= 2)
}

property double_neg {
  always (-(-x) = x)
}

property additive_inverse {
  always (x + (-x) = 0)
}

// Test: -(x + 1) when x = 1 gives -2
property neg_compound_expr {
  always (x = 1 -> -(x + 1) = -2)
}
