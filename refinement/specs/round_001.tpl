// Round 1: Single boolean variable with always invariant
// Tests: bool variable, init block, toggle transition, always property

module round_001

let flag: bool

init {
  flag = false
}

// Toggle: false -> true, true -> false
transition toggle {
  (flag = false and flag' = true) or (flag = true and flag' = false)
}

// PASS: flag is always a boolean, so it is always true or false
property always_bool {
  always (flag = true or flag = false)
}

// FAIL: flag starts false and toggles to true, so it is not always false
property always_false {
  always (flag = false)
}
