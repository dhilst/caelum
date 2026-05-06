// Round 31, Tier 2: Multiple named transition blocks (UNICODE operator syntax)
//
// System: a single counter v ∈ 0..3 with THREE named transitions that
// create non-deterministic branching:
//
//   inc:   v < 3          -> v' = v + 1
//   dec:   v > 0          -> v' = v - 1
//   reset: v > 0          -> v' = 0
//
// Init: v = 1
//
// Enabled transitions per state:
//   v=0: { inc }                 -> successors: { 1 }
//   v=1: { inc, dec, reset }    -> successors: { 2, 0, 0 } = { 0, 2 }
//   v=2: { inc, dec, reset }    -> successors: { 3, 1, 0 }
//   v=3: { dec, reset }         -> successors: { 2, 0 }
//
// All states { 0, 1, 2, 3 } are reachable from v=1.
//
// Key observations:
//   - v=0 is a "sink with one exit": only inc is enabled, so v=0 -> v=1 always.
//   - From v=2 and v=3 there exists a cycle 2->3->2->3->... that avoids 0 and 1.
//   - The system is maximally non-deterministic at v=1 and v=2 (3 transitions each).
//
// Properties (UNICODE syntax: □ always, ◇ eventually, ◯ next, ∧ and, ∨ or, ¬ not):

module round_031

let v ∈ 0..3

init {
  v = 1
}

// Increment: only when below max
transition inc {
  v < 3 ∧ v' = v + 1
}

// Decrement: only when above min
transition dec {
  v > 0 ∧ v' = v - 1
}

// Reset to zero: only when nonzero
transition reset {
  v > 0 ∧ v' = 0
}

// --- PROPERTY 1 (PASS) ---
// The counter is always within bounds. Domain invariant.
property bounded {
  □ (v >= 0 ∧ v <= 3)
}

// --- PROPERTY 2 (PASS) ---
// When v=0, the only enabled transition is inc, so next v=1.
property zero_forced_up {
  □ (v = 0 -> ◯ (v = 1))
}

// --- PROPERTY 3 (PASS) ---
// When v=3, dec gives v=2 and reset gives v=0. Either way, v < 3 next.
property max_must_decrease {
  □ (v = 3 -> ◯ (v < 3))
}

// --- INVALID 1 (expected FAIL) ---
// "v is always 0" -- false, v starts at 1 and can reach 2 and 3.
// Counterexample: the initial state itself, v=1 != 0.
invalid always_zero {
  □ (v = 0)
}

// --- INVALID 2 (expected FAIL) ---
// "v always eventually returns to 0" -- false.
// The cycle v=2 -> v=3 -> v=2 -> ... (inc from 2, dec from 3) avoids 0 forever.
// Reachable via: v=1 -> inc -> v=2, then loop.
invalid always_revisits_zero {
  □ ◇ (v = 0)
}

// --- INVALID 3 (expected FAIL) ---
// "When v=1, the next state is always v=2" -- false because dec and reset
// are also enabled at v=1, giving v=0.
// Counterexample: v=1 -> dec -> v=0.
invalid one_always_increments {
  □ (v = 1 -> ◯ (v = 2))
}

// --- INVALID 4 (expected FAIL) ---
// "v never reaches 3" -- false.
// Trace: v=1 -> inc -> v=2 -> inc -> v=3. Reached.
invalid never_reaches_max {
  □ (v < 3)
}
