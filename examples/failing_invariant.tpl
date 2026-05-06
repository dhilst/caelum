module examples.failing_invariant

let x ∈ 0..2

init {
  x = 0
}

transition step {
  x' = (x + 1) mod 3
}

property never_two {
  □ (x != 2)
}
