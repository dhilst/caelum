module examples.counter

const max = 3

let x ∈ 0..max

init {
  x = 0
}

transition step {
  x' = (x + 1) mod (max + 1)
}

property in_range {
  □ (x >= 0 ∧ x <= max)
}

property returns_to_zero {
  □ ◇ (x = 0)
}
