// Round 16: All comparison operators (=, !=, <, <=, >, >=)
// System: counter cycling through 0..4 via (x + 1) mod 5
// One property per comparison operator, mixing expected PASS and FAIL results

module round_016

let x: 0..4

init {
  x = 0
}

// Single transition: cycle through 0,1,2,3,4,0,1,...
transition cycle {
  x' = (x + 1) mod 5
}

// === Operator: = (equality) ===
// FAIL: x visits all values 0..4, so x = 0 does not hold always
property eq_fail {
  always (x = 0)
}

// === Operator: != (not equal) ===
// FAIL: x visits 3, so x != 3 does not hold always
property neq_fail {
  always (x != 3)
}

// === Operator: < (less than) ===
// FAIL: x can be 4, so x < 4 does not hold always
property lt_fail {
  always (x < 4)
}

// === Operator: <= (less than or equal) ===
// PASS: x ranges over 0..4, so x <= 4 always holds
property le_pass {
  always (x <= 4)
}

// === Operator: > (greater than) ===
// FAIL: x can be 0, so x > 0 does not hold always
property gt_fail {
  always (x > 0)
}

// === Operator: >= (greater than or equal) ===
// PASS: x ranges over 0..4, so x >= 0 always holds
property ge_pass {
  always (x >= 0)
}
