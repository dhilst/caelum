// Round 7: Nested temporal operators — always eventually P
//
// System: x ranges over 0..2.  Init at x = 0.
// Non-deterministic transitions:
//   advance:        x' = (x + 1) mod 3     (deterministic cycle 0 -> 1 -> 2 -> 0)
//   stutter_at_one: x = 1 and x' = 1       (x can stay at 1 indefinitely)
//
// Reachable states: {0, 1, 2}
// Successor map:
//   0 -> {1}
//   1 -> {1, 2}   (advance gives 2, stutter gives 1)
//   2 -> {0}
//
// Traces include:
//   0, 1, 2, 0, 1, 2, ...          (pure cycle — always returns to every value)
//   0, 1, 1, 1, ..., 1, 2, 0, ...  (stutter at 1 then continue)
//   0, 1, 1, 1, 1, ...             (stutter at 1 forever — x = 0 never recurs)
//
// Property always_ev_one (PASS):
//   always eventually (x = 1)
//   Every trace must revisit x = 1 infinitely often.
//   From 0, the only successor is 1.  From 1, either stay at 1 or go to 2;
//   from 2 the only successor is 0, which goes to 1.  So every infinite
//   trace passes through x = 1 on every cycle through the graph.
//   No lasso can avoid x = 1, so this property holds on all traces.
//
// Property always_ev_zero (FAIL):
//   always eventually (x = 0)
//   There exists a lasso trace 0, [1, 1, 1, ...] where the cycle {1}
//   never contains x = 0.  Once x = 1, the stutter transition keeps
//   x = 1 forever, so x = 0 is never reached again.
//   The model checker should report a counterexample with a cycle at x = 1.

module round_007

let x: 0..2

init {
  x = 0
}

transition advance {
  x' = (x + 1) mod 3
}

transition stutter_at_one {
  x = 1 and x' = 1
}

// PASS: x = 1 is unavoidable on every cycle through the graph
property always_ev_one {
  always eventually (x = 1)
}

// FAIL: the stutter at x = 1 allows a trace that never returns to x = 0
property always_ev_zero {
  always eventually (x = 0)
}
