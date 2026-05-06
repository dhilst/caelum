// Round 26, Tier 2: Two bool variables (KEYWORD syntax)
//
// System: two booleans a and b with deterministic toggling.
//   a toggles every step: a' = not a
//   b follows a with a one-step delay: b' = a
//
// Init: a = true, b = false
//
// Trace (deterministic 2-state cycle):
//   s0: a=true,  b=false
//   s1: a=false, b=true
//   s2: a=true,  b=false
//   s3: a=false, b=true
//   ...
//
// Properties using KEYWORD syntax, all referencing BOTH variables:
//
// 1. one_always_true (PASS):
//    always (a = true or b = true)
//    At every state, exactly one of a and b is true.
//
// 2. a_implies_next_b (PASS):
//    always (a = true -> next (b = true))
//    When a is true now, b becomes true next step (b' = a).
//
// 3. a_and_b_opposite (PASS):
//    always (a = true <-> not b = true)
//    a and b are always complementary.
//
// 4. cycle_revisits (PASS):
//    always eventually (a = true and b = false)
//    The state (a=true, b=false) recurs every 2 steps.
//
// 5. both_true (FAIL via invalid):
//    always (a = true and b = true)
//    a and b are never simultaneously true — this must fail.
//
// 6. a_stays_true (FAIL via invalid):
//    always (a = true -> next (a = true))
//    a toggles, so when a is true the next value is false.

module round_026

let a: bool
let b: bool

init {
  a = true and b = false
}

transition step {
  a' = not a and b' = a
}

// PASS: at every reachable state, at least one of a or b is true
property one_always_true {
  always (a = true or b = true)
}

// PASS: whenever a is true now, b is true at the next step
property a_implies_next_b {
  always (a = true -> next (b = true))
}

// PASS: a and b are always complementary
property a_and_b_opposite {
  always (a = true <-> not b = true)
}

// PASS: the state (a=true, b=false) recurs in every cycle
property cycle_revisits {
  always eventually (a = true and b = false)
}

// a and b are never simultaneously true, so this must FAIL
invalid both_true {
  always (a = true and b = true)
}

// a toggles each step, so a=true -> next a=true is false
invalid a_stays_true {
  always (a = true -> next (a = true))
}
