// Round 33, Tier 2: Frame conditions (x' = x) (ASCII operator syntax)
//
// Focus: transitions where SOME variables change while others are explicitly
// held constant via the frame condition x' = x.  This tests that the engine
// correctly handles partial-update transitions.
//
// Variables:
//   phase : enum { a, b, c }   -- a cyclic mode variable
//   count : 0..2               -- an integer counter
//
// Init: phase = a, count = 0
//
// Transitions (each updates ONE variable and frames the OTHER):
//
//   advance_phase: phase cycles a -> b -> c -> a, count' = count  (count framed)
//   inc_count:     count < 2 -> count' = count + 1, phase' = phase (phase framed)
//   reset_count:   count > 0 -> count' = 0, phase' = phase        (phase framed)
//
// Successor map from init (phase=a, count=0):
//   (a,0) -> { (b,0), (a,1) }               [advance_phase, inc_count]
//     (b,0) -> { (c,0), (b,1) }             [advance_phase, inc_count]
//       (c,0) -> { (a,0), (c,1) }           [advance_phase, inc_count]
//       (b,1) -> { (c,1), (b,2), (b,0) }    [advance_phase, inc_count, reset_count]
//     (a,1) -> { (b,1), (a,2), (a,0) }      [advance_phase, inc_count, reset_count]
//       (a,2) -> { (b,2), (a,0) }           [advance_phase, reset_count]
//         (b,2) -> { (c,2), (b,0) }         [advance_phase, reset_count]
//           (c,2) -> { (a,2), (c,0) }       [advance_phase, reset_count]
//       (c,1) -> { (a,1), (c,2), (c,0) }    [advance_phase, inc_count, reset_count]
//         (c,2) already covered
//
// All reachable states: { (p, c) | p in {a,b,c}, c in {0,1,2} } = 9 states
//
// Key frame-condition observations:
//   1. advance_phase never changes count (count' = count)
//   2. inc_count and reset_count never change phase (phase' = phase)
//   3. Therefore: if phase changes in a step, count must stay the same
//   4. And: if count changes in a step, phase must stay the same
//
// Properties (ASCII syntax):
//   [] always, <> eventually, () next
//   /\ and, \/ or, ~ not, -> implies

module round_033

let phase: enum { a, b, c }
let count: 0..2

init {
  phase = a /\ count = 0
}

// Phase advancement: a -> b -> c -> a, count is FRAMED
transition advance_phase {
  phase = a /\ phase' = b /\ count' = count
}

transition advance_phase_b {
  phase = b /\ phase' = c /\ count' = count
}

transition advance_phase_c {
  phase = c /\ phase' = a /\ count' = count
}

// Count increment: only when count < 2, phase is FRAMED
transition inc_count {
  count < 2 /\ count' = count + 1 /\ phase' = phase
}

// Count reset: only when count > 0, phase is FRAMED
transition reset_count {
  count > 0 /\ count' = 0 /\ phase' = phase
}

// --- PROPERTY 1 (PASS) ---
// Frame condition for advance_phase: when phase changes from a, count is preserved.
// If phase = a and count = 0, then in the next state either:
//   - phase became b and count stayed 0 (advance_phase), OR
//   - count changed but phase stayed a (inc_count/reset_count)
// In BOTH cases, if phase' != a then count' = count.
// We express: "if phase = a and count = 0, then next (phase = b -> count = 0)"
// This holds because advance_phase frames count, and inc_count/reset_count keep phase = a.
property frame_phase_a_preserves_count_0 {
  [] (phase = a /\ count = 0 -> () (phase = b -> count = 0))
}

// --- PROPERTY 2 (PASS) ---
// Frame condition for count transitions: when count changes, phase stays.
// If phase = a and count = 1, then next (count = 2 -> phase = a).
// inc_count frames phase, so if count goes from 1 to 2, phase must still be a.
property frame_count_inc_preserves_phase {
  [] (phase = a /\ count = 1 -> () (count = 2 -> phase = a))
}

// --- PROPERTY 3 (PASS) ---
// Stronger frame property: at most one variable changes per step.
// If phase changes, count stays. If count changes, phase stays.
// Equivalently: ~(phase changes /\ count changes).
// We express this per-value:
//   if phase = a /\ count = 0, next state cannot have phase != a /\ count != 0
property at_most_one_changes_a0 {
  [] (phase = a /\ count = 0 -> () ~(~(phase = a) /\ ~(count = 0)))
}

// --- PROPERTY 4 (PASS) ---
// The count-framing works for all phase values. If phase = b and count = 2,
// then advancing phase to c preserves count: next (phase = c -> count = 2).
property frame_phase_b_preserves_count_2 {
  [] (phase = b /\ count = 2 -> () (phase = c -> count = 2))
}

// --- PROPERTY 5 (PASS) ---
// System always eventually returns to phase = a (phase cycles through a, b, c).
// From any phase, advance_phase can always fire (it has no guard on count),
// and 3 advances return to a. Non-determinism could avoid advancing, but
// reset_count/inc_count keep phase the same, so eventually advance must happen...
// Actually, this is NOT necessarily true with non-determinism -- the system
// could keep firing inc_count/reset_count forever and never advance phase.
// Let me replace this with something that IS true.
//
// Replaced: Domain invariant -- count is always in {0, 1, 2}.
property count_bounded {
  [] (count >= 0 /\ count <= 2)
}

// --- INVALID 1 (expected FAIL) ---
// "phase and count can both change simultaneously"
// This claims: from (a, 0), next state can have phase = b /\ count = 1.
// But no single transition changes both -- advance_phase frames count,
// and inc_count frames phase. So (b, 1) is NOT a successor of (a, 0).
// However, (b, 1) IS reachable in 2 steps: (a,0) -> (a,1) -> (b,1).
// The invalid claim is: always (phase = a /\ count = 0 -> () (phase = b /\ count = 1))
// This means "from (a,0), the NEXT state is ALWAYS (b,1)" which is false
// because (a,0) -> (b,0) is possible (advance_phase), and (b,0) != (b,1).
invalid both_change_simultaneously {
  [] (phase = a /\ count = 0 -> () (phase = b /\ count = 1))
}

// --- INVALID 2 (expected FAIL) ---
// "count never reaches 2" -- false because inc_count can fire twice from 0.
// Trace: (a,0) -> (a,1) -> (a,2). Count is 2.
invalid count_never_two {
  [] (~(count = 2))
}

// --- INVALID 3 (expected FAIL) ---
// "if count is 0, it stays 0 forever" -- false because inc_count can fire.
// Trace: (a,0) -> (a,1). Count changed from 0 to 1 while phase stayed a.
// This directly tests that the frame condition does NOT mean "no change ever".
invalid count_zero_forever {
  [] (count = 0 -> [] (count = 0))
}

// --- INVALID 4 (expected FAIL) ---
// "phase never returns to a once it leaves" -- false because phase cycles.
// Trace: (a,0) -> (b,0) -> (c,0) -> (a,0). Phase returned to a.
invalid phase_never_returns {
  [] (phase = b -> [] (~(phase = a)))
}
