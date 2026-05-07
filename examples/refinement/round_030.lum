// Round 30, Tier 2: Enum + int range (ASCII operator syntax)
//
// System: a three-mode controller { counting, paused, reset } driving
// a counter variable in 0..3.
//
//   - In "counting" mode the counter increments (saturates at 3).
//   - In "paused"   mode the counter holds its current value.
//   - In "reset"    mode the counter goes to 0.
//
// Mode transitions are non-deterministic (any mode can go to any mode),
// but the counter is entirely determined by the mode.
//
// Variables:
//   mode    : enum { counting, paused, reset }
//   counter : 0..3
//
// Init: mode = paused, counter = 0
//
// Transitions:
//   Each transition picks a next mode non-deterministically and applies
//   the counter rule for the CURRENT mode:
//     mode = counting -> counter' = min(counter + 1, 3)
//     mode = paused   -> counter' = counter
//     mode = reset    -> counter' = 0
//
// Because any mode can follow any mode, the system explores all
// combinations of (mode, counter) reachable from (paused, 0).
//
// Reachable states (steady-state, from the init (paused, 0)):
//   (paused,   0) -> any mode, counter' = 0 (paused holds)
//   (counting, 0) -> any mode, counter' = 1
//   (reset,    0) -> any mode, counter' = 0
//   (paused,   1) -> any mode, counter' = 1
//   (counting, 1) -> any mode, counter' = 2
//   (reset,    1) -> any mode, counter' = 0   (but reset,1 reachable? yes, from counting,0->paused then paused,1->reset... actually let's trace)
//
// Full reachability from (paused, 0):
//   Step 0: { (paused, 0) }
//   Step 1: counter' = 0 (paused holds), mode' in {counting, paused, reset}
//           -> { (counting, 0), (paused, 0), (reset, 0) }
//   Step 2 from (counting, 0): counter' = min(0+1,3) = 1
//           -> { (counting, 1), (paused, 1), (reset, 1) }
//   Step 2 from (paused, 0): counter' = 0  (already covered)
//   Step 2 from (reset, 0): counter' = 0   (already covered)
//   Step 3 from (counting, 1): counter' = 2
//           -> { (counting, 2), (paused, 2), (reset, 2) }
//   Step 3 from (paused, 1): counter' = 1  (already covered)
//   Step 3 from (reset, 1): counter' = 0   (already covered)
//   Step 4 from (counting, 2): counter' = 3
//           -> { (counting, 3), (paused, 3), (reset, 3) }
//   Step 4 from (paused, 2): counter' = 2  (already covered)
//   Step 4 from (reset, 2): counter' = 0   (already covered)
//   Step 5 from (counting, 3): counter' = 3 (saturated)
//           -> { (counting, 3), (paused, 3), (reset, 3) }  (already covered)
//   Step 5 from (paused, 3): counter' = 3  (already covered)
//   Step 5 from (reset, 3): counter' = 0   (already covered)
//
// All reachable states: { (m, c) | m in {counting, paused, reset}, c in {0,1,2,3} }
//   = 12 states total. All are reachable.
//
// Key observations:
//   1. reset always sends counter to 0 next step.
//   2. paused always preserves counter.
//   3. counting increments counter unless already at 3.
//   4. Since mode transitions are unconstrained, from any state we can
//      reach counter=0 within 2 steps (go to reset, then next step counter=0).
//   5. counter=3 is always reachable (go to counting repeatedly).
//
// Properties (ASCII syntax):
//   [] always, <> eventually, () next, U until
//   /\ and, \/ or, ~ not, -> implies, <-> iff

module round_030

let mode: enum { counting, paused, reset }
let counter: 0..3

init {
  mode = paused /\ counter = 0
}

// Counting: increment up to saturation at 3
transition count_to_counting {
  mode = counting /\ counter < 3
  /\ counter' = counter + 1
  /\ mode' = counting
}

transition count_to_paused {
  mode = counting /\ counter < 3
  /\ counter' = counter + 1
  /\ mode' = paused
}

transition count_to_reset {
  mode = counting /\ counter < 3
  /\ counter' = counter + 1
  /\ mode' = reset
}

// Counting at saturation: counter stays at 3
transition count_sat_to_counting {
  mode = counting /\ counter = 3
  /\ counter' = 3
  /\ mode' = counting
}

transition count_sat_to_paused {
  mode = counting /\ counter = 3
  /\ counter' = 3
  /\ mode' = paused
}

transition count_sat_to_reset {
  mode = counting /\ counter = 3
  /\ counter' = 3
  /\ mode' = reset
}

// Paused: counter holds
transition pause_to_counting {
  mode = paused
  /\ counter' = counter
  /\ mode' = counting
}

transition pause_to_paused {
  mode = paused
  /\ counter' = counter
  /\ mode' = paused
}

transition pause_to_reset {
  mode = paused
  /\ counter' = counter
  /\ mode' = reset
}

// Reset: counter goes to 0
transition reset_to_counting {
  mode = reset
  /\ counter' = 0
  /\ mode' = counting
}

transition reset_to_paused {
  mode = reset
  /\ counter' = 0
  /\ mode' = paused
}

transition reset_to_reset {
  mode = reset
  /\ counter' = 0
  /\ mode' = reset
}

// --- PROPERTY 1 (PASS) ---
// Reset mode always zeroes counter in the next step.
// From any (reset, c), the only transitions set counter' = 0.
property reset_zeroes_counter {
  [] (mode = reset -> () (counter = 0))
}

// --- PROPERTY 2 (PASS) ---
// Paused mode preserves the counter value.
// From any (paused, c), transitions set counter' = counter.
// We express this as: if paused and counter = k, then next counter = k.
// Since we can't quantify over k, we check each value:
property paused_holds_counter {
  [] (mode = paused -> (
    (counter = 0 -> () (counter = 0))
    /\ (counter = 1 -> () (counter = 1))
    /\ (counter = 2 -> () (counter = 2))
    /\ (counter = 3 -> () (counter = 3))
  ))
}

// --- PROPERTY 3 (PASS) ---
// Counting at saturation (counter = 3) keeps counter at 3.
// From (counting, 3), counter' = 3 in all transitions.
property counting_saturates {
  [] (mode = counting /\ counter = 3 -> () (counter = 3))
}

// --- PROPERTY 4b (PASS) ---
// Counter never exceeds 3.  (Domain invariant, but good to check.)
property counter_bounded {
  [] (counter <= 3)
}

// --- PROPERTY 4 (PASS) ---
// Counting mode with counter < 3 always increments the counter.
// We check: if counting and counter = 0, next counter = 1; etc.
property counting_increments {
  [] (
    (mode = counting /\ counter = 0 -> () (counter = 1))
    /\ (mode = counting /\ counter = 1 -> () (counter = 2))
    /\ (mode = counting /\ counter = 2 -> () (counter = 3))
  )
}

// --- INVALID 1 (expected FAIL) ---
// "counter is always 0" -- false because counting mode increments it.
// Trace: (paused,0) -> (counting,0) -> (_,1). Counter is 1, not 0.
invalid counter_always_zero {
  [] (counter = 0)
}

// --- INVALID 3 (expected FAIL) ---
// "counter always eventually returns to 0" -- false because there exists
// a trace that stays in counting mode: counter reaches 3 and loops at
// (counting,3) forever, never revisiting 0.
invalid always_revisit_zero {
  [] <> (counter = 0)
}

// --- INVALID 2 (expected FAIL) ---
// "once counter reaches 3, it stays at 3 forever"
// False because reset mode can bring it back to 0.
// Trace: (counting,2) -> (reset,3) -> (_,0). Counter goes from 3 to 0.
invalid counter_3_persists {
  [] (counter = 3 -> [] (counter = 3))
}
