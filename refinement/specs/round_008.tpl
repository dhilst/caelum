// Round 8: Nested temporal operators -- eventually always P
//
// "eventually always P" means: there exists a future point after which P
// holds forever on all paths.  This is the dual of "always eventually".
//
// ========================================================================
// System A -- absorbing state (stabilization)
// ========================================================================
//
// Variable x ranges over 0..2.  Init x = 0.
// Transitions:
//   go_1:   x = 0 -> x' = 1
//   go_2:   x = 1 -> x' = 2
//   absorb: x = 2 -> x' = 2   (self-loop -- absorbing state)
//
// Reachable states: {0, 1, 2}
// Edge map:
//   0 -> {1}
//   1 -> {2}
//   2 -> {2}
//
// Every trace from x = 0 follows the unique path: 0, 1, 2, 2, 2, ...
//
// Property ea_two_pass (expected PASS):
//   eventually always (x = 2)
//   After reaching x = 2, x stays at 2 forever.  Every trace eventually
//   reaches x = 2 and stays there, so this holds universally.
//
//   Checker reasoning (sat_set):
//     always_set(x=2): base={2}. GFP: start all={0,1,2}.
//       Iter 1: keep s if s in base AND all succs in set.
//         0: not in base -> remove.  1: not in base -> remove.
//         2: in base, succ={2} in set -> keep.  Set={2}. Fixed.
//     eventually_set over {2}: LFP starting from {2}.
//       1: all succs {2} in set -> add.  0: all succs {1} -> not yet.
//       Next iter: 0: all succs {1} in set -> add.  Set={0,1,2}. Fixed.
//     Initial state 0 in sat_set -> PASS.
//
// Property ea_zero_fail (expected FAIL):
//   eventually always (x = 0)
//   Once x leaves 0, it never returns.  The unique trace 0, 1, 2, 2, ...
//   shows x = 0 holds only at position 0 but never stabilizes.
//
//   Checker reasoning (sat_set):
//     always_set(x=0): base={0}. GFP: start all={0,1,2}.
//       Iter 1: 0 in base, succ={1} in all -> keep.
//         1 not in base -> remove.  2 not in base -> remove.  Set={0}.
//       Iter 2: 0 in base, succ={1}, 1 NOT in set -> remove.  Set={}.
//     eventually_set over {}: empty base, no expansion.  Set={}.
//     Initial state 0 NOT in sat_set -> FAIL.

module round_008_absorb

let x: 0..2

init {
  x = 0
}

transition go_1 {
  x = 0 and x' = 1
}

transition go_2 {
  x = 1 and x' = 2
}

transition absorb {
  x = 2 and x' = 2
}

// PASS: x stabilizes at 2
property ea_two_pass {
  eventually always (x = 2)
}

// x never stabilizes at 0 (it leaves 0 and never returns)
invalid ea_zero_fail {
  eventually always (x = 0)
}
