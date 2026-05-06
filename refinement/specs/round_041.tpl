// Round 41, Tier 2: Min/max tracking — high-water mark (UNICODE syntax)
//
// System: a value variable (val: 0..3) that moves non-deterministically
// (increment, decrement, or stay), paired with a high-water mark variable
// (hwm: 0..3) that always holds the maximum value val has ever reached.
//
// This models a classic "peak tracker" pattern: hwm is monotonically
// non-decreasing and always satisfies hwm >= val.
//
// Variables:
//   val : 0..3   -- current value, changes non-deterministically
//   hwm : 0..3   -- high-water mark, tracks the maximum of val seen so far
//
// Init: val = 0, hwm = 0
//
// Transitions (non-deterministic):
//   inc_new_max:  val < 3 ∧ val = hwm
//                 -> val' = val + 1, hwm' = val + 1
//                 Value increments past the current high-water mark,
//                 establishing a new maximum.
//
//   inc_below:    val < 3 ∧ val < hwm
//                 -> val' = val + 1, hwm' = hwm
//                 Value increments but stays at or below current hwm.
//                 (val + 1 <= hwm since val < hwm, so val+1 <= hwm)
//
//   dec:          val > 0
//                 -> val' = val - 1, hwm' = hwm
//                 Value decrements; hwm unchanged (already >= val > val').
//
//   stay:         val' = val, hwm' = hwm
//                 Value stays the same; hwm unchanged.
//
// Note: val can only exceed hwm by incrementing from val = hwm, which is
// handled by inc_new_max. In all other transitions, hwm' >= val' is maintained.
//
// Reachable states (val, hwm) with constraint hwm >= val:
//   hwm=0: (0,0)
//   hwm=1: (0,1), (1,1)
//   hwm=2: (0,2), (1,2), (2,2)
//   hwm=3: (0,3), (1,3), (2,3), (3,3)
//   Total: 10 reachable states.
//
// Example trace from (0, 0):
//   (0,0) ->inc_new_max-> (1,1) ->inc_new_max-> (2,2) ->dec-> (1,2)
//   ->dec-> (0,2) ->inc_below-> (1,2) ->inc_below-> (2,2)
//   ->inc_new_max-> (3,3) ->dec-> (2,3) ->dec-> (1,3) ->stay-> (1,3) ...
//
// Key observations:
//   1. hwm >= val is an invariant (holds in all reachable states).
//   2. hwm is monotonically non-decreasing: hwm' >= hwm in every transition.
//   3. Once hwm reaches 3, it stays at 3 forever (monotone + bounded domain).
//   4. hwm can eventually reach 3 on some paths (via repeated inc_new_max),
//      but NOT on all paths (stay can loop forever at (0,0)).
//   5. val always eventually returns to 0 is NOT guaranteed (stay can loop).
//   6. From (0,0), eventually hwm >= 1 is NOT guaranteed (stay loops at (0,0)).
//
// ALL operators use UNICODE syntax:
//   □ always, ◇ eventually, ◯ next, 𝒰 until
//   ∧ and, ∨ or, ¬ not
//   -> implies, <-> iff

module round_041

let val: 0..3
let hwm: 0..3

init {
  val = 0 ∧ hwm = 0
}

// Value increments to a new maximum (val was equal to hwm, both advance)
transition inc_new_max {
  val < 3 ∧ val = hwm
  ∧ val' = val + 1
  ∧ hwm' = val + 1
}

// Value increments but stays within the existing high-water mark
transition inc_below {
  val < 3 ∧ val + 1 <= hwm
  ∧ val' = val + 1
  ∧ hwm' = hwm
}

// Value decrements; high-water mark stays
transition dec {
  val > 0
  ∧ val' = val - 1
  ∧ hwm' = hwm
}

// Value and high-water mark both stay unchanged
transition stay {
  val' = val ∧ hwm' = hwm
}

// --- PROPERTY 1 (PASS) ---
// The high-water mark is always at least as large as the current value.
// This is the core tracking invariant: hwm >= val in every reachable state.
property hwm_dominates_val {
  □ (hwm >= val)
}

// --- PROPERTY 2 (PASS) ---
// The high-water mark is monotonically non-decreasing: it never goes down.
// In every transition, hwm' >= hwm (either hwm' = hwm or hwm' = val + 1 > hwm).
// We express this as: always, next hwm >= current hwm.
// For each possible current value of hwm, the next hwm is at least as large.
property hwm_monotone {
  □ (
    (hwm = 0 -> ◯ (hwm >= 0))
    ∧ (hwm = 1 -> ◯ (hwm >= 1))
    ∧ (hwm = 2 -> ◯ (hwm >= 2))
    ∧ (hwm = 3 -> ◯ (hwm >= 3))
  )
}

// --- PROPERTY 3 (PASS) ---
// Once hwm reaches 3, it stays at 3 forever.
// Since hwm is monotone and 3 is the domain maximum, hwm = 3 is absorbing.
property hwm_3_is_absorbing {
  □ (hwm = 3 -> □ (hwm = 3))
}

// --- PROPERTY 4 (PASS) ---
// Domain bounds: both variables are always within 0..3.
property domain_bounds {
  □ (val >= 0 ∧ val <= 3 ∧ hwm >= 0 ∧ hwm <= 3)
}

// --- PROPERTY 5 (PASS) ---
// Initially, val equals hwm (both are 0).
// From the init block, val = 0 and hwm = 0, so val = hwm.
property init_val_eq_hwm {
  val = hwm
}

// --- PROPERTY 6 (PASS) ---
// Whenever val = hwm and val < 3, the next state either has val = hwm
// (if stay fires) or val = hwm again (if inc_new_max fires, both go to val+1)
// or val < hwm (if dec fires and val > 0). In all cases, hwm' >= val'.
// More precisely: if val = hwm, then in the next state hwm >= val still holds.
// This is just a specialization of property 1, but let's verify the "frontier"
// case explicitly: when val = hwm (val is at its peak), the invariant is maintained.
property peak_maintained {
  □ (val = hwm -> ◯ (hwm >= val))
}

// --- PROPERTY 7 (PASS) ---
// If hwm = 0 then val = 0 (since val >= 0 and hwm >= val, hwm = 0 forces val = 0).
property hwm_zero_forces_val_zero {
  □ (hwm = 0 -> val = 0)
}

// --- INVALID 1 (expected FAIL) ---
// "val and hwm are always equal."
// FALSE: after inc_new_max from (0,0) to (1,1), dec can fire giving (0,1).
// Now val = 0 but hwm = 1; they differ.
// Trace: (0,0) -> inc_new_max -> (1,1) -> dec -> (0,1). val != hwm.
invalid always_equal {
  □ (val = hwm)
}

// --- INVALID 2 (expected FAIL) ---
// "hwm is always 0."
// FALSE: inc_new_max from (0,0) gives (1,1), so hwm = 1.
// Trace: (0,0) -> inc_new_max -> (1,1). hwm = 1, not 0.
invalid hwm_always_zero {
  □ (hwm = 0)
}

// --- INVALID 3 (expected FAIL) ---
// "The high-water mark always eventually reaches 3."
// FALSE: the stay transition can loop forever at (0,0), keeping hwm = 0.
// There exist infinite paths where only stay fires: (0,0) -> (0,0) -> ...
invalid hwm_always_reaches_max {
  □ (◇ (hwm = 3))
}

// --- INVALID 4 (expected FAIL) ---
// "Once hwm reaches 1, val never returns to 0."
// FALSE: after reaching hwm = 1, val can still decrement back to 0.
// Trace: (0,0) -> inc_new_max -> (1,1) -> dec -> (0,1). val = 0 with hwm = 1.
invalid val_never_returns_to_zero {
  □ (hwm >= 1 -> val > 0)
}

// --- INVALID 5 (expected FAIL) ---
// "hwm can decrease: eventually hwm goes from a positive value back to 0."
// FALSE: hwm is monotonically non-decreasing (property 2), so once positive
// it never returns to 0. But let's express this as "eventually hwm=1 and
// eventually later hwm=0", which should fail.
// We express: eventually (hwm = 1 and eventually hwm = 0).
invalid hwm_decreases {
  ◇ (hwm = 1 ∧ ◇ (hwm = 0))
}
