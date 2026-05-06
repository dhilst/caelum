// Round 10: Unicode operator syntax (exclusive)
//
// Tests ALL unicode operators explicitly and exclusively:
//   ∈  -- type separator (instead of :)
//   □  -- always
//   ◇  -- eventually
//   ◯  -- next
//   𝒰  -- until
//   ∧  -- and
//   ∨  -- or
//   ¬  -- not
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
// Properties (all using unicode operators only):
//
// 1. unicode_always_nonneg (expected PASS):
//    □ (x >= 0)
//    x is always >= 0 since the domain is 0..2.
//
// 2. unicode_eventually_two (expected PASS):
//    ◇ (x = 2)
//    x reaches 2 on the cycle 0 -> 1 -> 2 -> 0 -> ...
//
// 3. unicode_next_one (expected PASS):
//    ◯ (x = 1)
//    From the initial state x=0, the next state is x=1.
//
// 4. unicode_until (expected PASS):
//    (x < 2) 𝒰 (x = 2)
//    Starting from x=0: x=0 < 2, then x=1 < 2, then x=2 satisfies RHS.
//
// 5. unicode_and (expected PASS):
//    □ (x >= 0 ∧ x <= 2)
//    Always in range [0, 2] -- trivially true for domain 0..2.
//
// 6. unicode_or (expected PASS):
//    □ (x = 0 ∨ x = 1 ∨ x = 2)
//    x is always one of 0, 1, or 2 -- exhaustive for domain 0..2.
//
// 7. unicode_not (expected PASS):
//    □ (¬(x = 3))
//    x never equals 3. Since the domain is 0..2, x = 3 is always false,
//    so ¬(x = 3) is always true.

module round_010

let x ∈ 0..2

init {
  x = 0
}

transition step {
  x' = (x + 1) mod 3
}

// PASS: x is always >= 0 (trivially true for domain 0..2)
property unicode_always_nonneg {
  □ (x >= 0)
}

// PASS: x eventually reaches 2 on the cycle
property unicode_eventually_two {
  ◇ (x = 2)
}

// PASS: from initial state x=0, the next state is x=1
property unicode_next_one {
  ◯ (x = 1)
}

// PASS: x < 2 holds until x = 2
property unicode_until {
  (x < 2) 𝒰 (x = 2)
}

// PASS: always (x >= 0 and x <= 2)
property unicode_and {
  □ (x >= 0 ∧ x <= 2)
}

// PASS: always (x=0 or x=1 or x=2)
property unicode_or {
  □ (x = 0 ∨ x = 1 ∨ x = 2)
}

// PASS: x never equals 3 (domain is 0..2, so x=3 is always false)
property unicode_not {
  □ (¬(x = 3))
}
