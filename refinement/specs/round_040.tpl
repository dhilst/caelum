// Round 40, Tier 2: Wrap-around counter with overflow detection (ASCII syntax)
//
// System: a modular counter cnt: 0..3 that increments every step and
// wraps from 3 back to 0, paired with a boolean overflow flag (ovf)
// that is set to true EXACTLY when the wrap-around occurs (cnt goes
// from 3 to 0), and cleared on every other step.
//
// This models a classic hardware counter with a carry/overflow output.
// The key interaction is that ovf DETECTS the wrap-around event:
//   ovf' = true   iff   cnt = 3   (i.e., next step wraps)
//   ovf' = false  iff   cnt < 3   (i.e., next step is a normal increment)
//
// Variables:
//   cnt : 0..3   -- modular counter (increments mod 4)
//   ovf : bool   -- overflow flag (true when wrap-around just happened)
//
// Init: cnt = 0, ovf = false
//
// Transition (deterministic, single transition):
//   step:  cnt' = (cnt + 1) mod 4
//          ovf' = (cnt = 3)   -- flag set when counter was at max
//
// Trace (deterministic, period-4 cycle after initial state):
//   s0: cnt=0, ovf=false  ->  s1  (initial state, visited once)
//   s1: cnt=1, ovf=false  ->  s2
//   s2: cnt=2, ovf=false  ->  s3
//   s3: cnt=3, ovf=false  ->  s4
//   s4: cnt=0, ovf=true   ->  s1  (wrap! overflow flag set)
//   s1: cnt=1, ovf=false  ->  s2  (cycle of length 4: s1->s2->s3->s4->s1)
//
// Key observations:
//   1. The system is fully deterministic (exactly one transition).
//   2. ovf = true occurs exactly when cnt = 0 AND the previous cnt was 3.
//      In the cycle, ovf = true only at state s4 = (0, true).
//   3. After the initial transient, the cycle visits:
//      (1,F) -> (2,F) -> (3,F) -> (0,T) -> (1,F) ...
//   4. The initial state (0, false) is NOT revisited after s0, because
//      every subsequent time cnt = 0, ovf = true (not false).
//   5. cnt = 0 always eventually recurs (every 4 steps in the cycle).
//   6. ovf = true always eventually recurs (every 4 steps in the cycle).
//   7. ovf = true implies cnt = 0 (overflow only when counter just wrapped).
//   8. cnt = 0 does NOT imply ovf = true (at s0, cnt=0 but ovf=false).
//      However, after the initial state, cnt=0 always coincides with ovf=true.
//
// ALL operators use ASCII syntax:
//   [] always, <> eventually, () next, U until
//   /\ and, \/ or, ~ not, -> implies, <-> iff

module round_040

let cnt: 0..3
let ovf: bool

init {
  cnt = 0 /\ ovf = false
}

// Deterministic step: counter increments mod 4, overflow flag set on wrap
// We split into two transitions based on the wrap condition to set ovf correctly.

// Normal increment: cnt < 3, so no wrap, ovf' = false
transition step_normal {
  cnt < 3
  /\ cnt' = cnt + 1
  /\ ovf' = false
}

// Wrap-around: cnt = 3, so counter wraps to 0, ovf' = true
transition step_wrap {
  cnt = 3
  /\ cnt' = 0
  /\ ovf' = true
}

// --- PROPERTY 1 (PASS) ---
// Counter is always in bounds (domain invariant).
property cnt_bounded {
  [] (cnt >= 0 /\ cnt <= 3)
}

// --- PROPERTY 2 (PASS) ---
// Overflow flag implies counter is zero.
// ovf = true only occurs when the counter just wrapped from 3 to 0.
// Trace check: ovf = true only at s4 = (0, true). cnt = 0 there. Holds.
property ovf_implies_zero {
  [] (ovf = true -> cnt = 0)
}

// --- PROPERTY 3 (PASS) ---
// Counter always eventually returns to zero (the cycle visits cnt=0 every 4 steps).
property cnt_always_eventually_zero {
  [] <> (cnt = 0)
}

// --- PROPERTY 4 (PASS) ---
// The overflow flag is raised infinitely often (every 4 steps in the cycle).
property ovf_recurs {
  [] <> (ovf = true)
}

// --- PROPERTY 5 (PASS) ---
// Whenever cnt = 3, the next state has cnt = 0 and ovf = true.
// This is the core wrap-around detection property: step_wrap fires.
property wrap_detected {
  [] (cnt = 3 -> () (cnt = 0 /\ ovf = true))
}

// --- PROPERTY 6 (PASS) ---
// Whenever cnt < 3, the next state has ovf = false and cnt = cnt + 1.
// step_normal fires: cnt' = cnt + 1, ovf' = false.
// We verify for each value of cnt < 3:
property normal_step_increments {
  [] (
    (cnt = 0 -> () (cnt = 1 /\ ovf = false))
    /\ (cnt = 1 -> () (cnt = 2 /\ ovf = false))
    /\ (cnt = 2 -> () (cnt = 3 /\ ovf = false))
  )
}

// --- PROPERTY 7 (PASS) ---
// The overflow flag is true for exactly one step, then cleared.
// ovf = true -> next(ovf = false), because from (0, true), cnt = 0 < 3,
// so step_normal fires: cnt' = 1, ovf' = false.
property ovf_lasts_one_step {
  [] (ovf = true -> () (ovf = false))
}

// --- PROPERTY 8 (PASS) ---
// The counter cycles through all values: from cnt = 0, eventually cnt = 3.
// Trace: cnt=0 -> 1 -> 2 -> 3 within 3 steps.
property zero_reaches_max {
  [] (cnt = 0 -> <> (cnt = 3))
}

// --- INVALID 1 (expected FAIL) ---
// "The overflow flag is never raised."
// FALSE: ovf = true at state s4 = (0, true) after the first full cycle.
// Trace: (0,F) -> (1,F) -> (2,F) -> (3,F) -> (0,T). ovf = true.
invalid ovf_never_true {
  [] (ovf = false)
}

// --- INVALID 2 (expected FAIL) ---
// "Whenever cnt = 0, ovf is true."
// FALSE at the initial state: s0 = (0, false). cnt = 0 but ovf = false.
invalid zero_always_has_ovf {
  [] (cnt = 0 -> ovf = true)
}

// --- INVALID 3 (expected FAIL) ---
// "Once overflow occurs, it stays true forever."
// FALSE: ovf = true lasts exactly one step, then ovf' = false.
// Trace: ... -> (0,T) -> (1,F). ovf goes from true to false.
invalid ovf_stays_forever {
  [] (ovf = true -> [] (ovf = true))
}

// --- INVALID 4 (expected FAIL) ---
// "The counter never wraps: cnt is always strictly less than 3 or ovf is always false."
// We express this as "cnt never reaches 3": always cnt < 3.
// FALSE: cnt reaches 3 after 3 steps from init.
// Trace: (0,F) -> (1,F) -> (2,F) -> (3,F). cnt = 3.
invalid cnt_never_max {
  [] (cnt < 3)
}
