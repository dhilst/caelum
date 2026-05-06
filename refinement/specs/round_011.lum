// Round 11: Keyword operator syntax (exclusive)
//
// Tests ALL keyword-form operators (no unicode, no ASCII symbols):
//   :          -- type separator (standard colon, not unicode)
//   always     -- for box/always (□)
//   eventually -- for diamond/eventually (◇)
//   next       -- for circle-next (◯)
//   until      -- for until (𝒰)
//   and        -- for conjunction (∧)
//   or         -- for disjunction (∨)
//   not        -- for negation (¬)
//
// ========================================================================
// System: deterministic counter mod 3
// ========================================================================
//
// Variable x ranges over 0..2.  Init x = 0.
// Single transition: x' = (x + 1) mod 3
//
// Reachable states: {0, 1, 2}
// Edge map:
//   0 -> {1}
//   1 -> {2}
//   2 -> {0}
//
// Unique infinite trace from x=0: 0, 1, 2, 0, 1, 2, ...
//
// Properties (all using keyword operators only):
//
// 1. kw_always_nonneg (expected PASS):
//    always (x >= 0)
//    x is always >= 0 since the domain is 0..2.
//
// 2. kw_eventually_two (expected PASS):
//    eventually (x = 2)
//    x reaches 2 on the cycle 0 -> 1 -> 2 -> 0 -> ...
//
// 3. kw_next_one (expected PASS):
//    next (x = 1)
//    From the initial state x=0, the next state is x=1.
//
// 4. kw_until (expected PASS):
//    (x < 2) until (x = 2)
//    Starting from x=0: x=0 < 2, then x=1 < 2, then x=2 satisfies RHS.
//
// 5. kw_and (expected PASS):
//    always (x >= 0 and x <= 2)
//    Always in range [0, 2] -- trivially true for domain 0..2.
//
// 6. kw_or (expected PASS):
//    always (x = 0 or x = 1 or x = 2)
//    x is always one of 0, 1, or 2 -- exhaustive for domain 0..2.
//
// 7. kw_not (expected PASS):
//    always (not (x = 3))
//    x never equals 3. Since the domain is 0..2, x = 3 is always false,
//    so not (x = 3) is always true.

module round_011

let x: 0..2

init {
  x = 0
}

transition step {
  x' = (x + 1) mod 3
}

// PASS: x is always >= 0 (trivially true for domain 0..2)
property kw_always_nonneg {
  always (x >= 0)
}

// PASS: x eventually reaches 2 on the cycle
property kw_eventually_two {
  eventually (x = 2)
}

// PASS: from initial state x=0, the next state is x=1
property kw_next_one {
  next (x = 1)
}

// PASS: x < 2 holds until x = 2
property kw_until {
  (x < 2) until (x = 2)
}

// PASS: always (x >= 0 and x <= 2)
property kw_and {
  always (x >= 0 and x <= 2)
}

// PASS: always (x=0 or x=1 or x=2)
property kw_or {
  always (x = 0 or x = 1 or x = 2)
}

// PASS: x never equals 3 (domain is 0..2, so x=3 is always false)
property kw_not {
  always (not (x = 3))
}
