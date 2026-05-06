// Round 35, Tier 2: Larger domain 0..5 with two variables (KEYWORD syntax)
//
// System: two counters x and y on the domain 0..5, coupled via modular
// arithmetic.  This exercises a 6x6 = 36 state space (larger than prior
// rounds which used 0..3 or 0..4).
//
// Variables:
//   x : 0..5   -- primary counter, advances by (y + 1) mod 6
//   y : 0..5   -- secondary counter, mirrors x via modular offset
//
// Init: x = 0, y = 0
//
// Transitions:
//   step_fwd:  x' = (x + y + 1) mod 6,  y' = (y + 1) mod 6
//              Both advance; x jumps by (y+1), y increments by 1.
//
//   swap:      x' = y,  y' = x
//              Exchange the two counters.
//
//   reset_x:   x > 0 -> x' = 0, y' = y
//              Reset x to 0, keeping y.
//
// Trace from (0, 0):
//   step_fwd: x' = (0+0+1) mod 6 = 1, y' = 1 -> (1, 1)
//   step_fwd: x' = (1+1+1) mod 6 = 3, y' = 2 -> (3, 2)
//   step_fwd: x' = (3+2+1) mod 6 = 0, y' = 3 -> (0, 3)
//   step_fwd: x' = (0+3+1) mod 6 = 4, y' = 4 -> (4, 4)
//   step_fwd: x' = (4+4+1) mod 6 = 3, y' = 5 -> (3, 5)
//   step_fwd: x' = (3+5+1) mod 6 = 3, y' = 0 -> (3, 0)
//   step_fwd: x' = (3+0+1) mod 6 = 4, y' = 1 -> (4, 1)
//   ...
//
//   swap from (1,1): x' = 1, y' = 1 -> (1, 1) (no change, symmetric)
//   swap from (3,2): x' = 2, y' = 3 -> (2, 3)
//   reset_x from (3,2): x' = 0, y' = 2 -> (0, 2)
//
// Reachable states grow quickly due to swap and reset_x branching.
// The full reachable set covers a significant portion of the 36 states.
//
// Key observations:
//   1. x + y can range from 0 to 10 in the full domain, but reachable
//      states may be constrained.
//   2. y cycles through 0..5 via step_fwd (increments mod 6).
//   3. swap is symmetric; if (a,b) is reachable then (b,a) is too.
//   4. reset_x ensures (0, k) is reachable for any reachable y = k.
//   5. x + y <= 10 always holds because both are in 0..5 (max 5+5=10).
//
// ALL operators use KEYWORD syntax:
//   always, eventually, next, until, and, or, not

module round_035

let x: 0..5
let y: 0..5

init {
  x = 0 and y = 0
}

// Both counters advance: x jumps by (y+1), y increments by 1 (all mod 6)
transition step_fwd {
  x' = (x + y + 1) mod 6 and y' = (y + 1) mod 6
}

// Swap the two counters
transition swap {
  x' = y and y' = x
}

// Reset x to 0 when it is positive, y unchanged
transition reset_x {
  x > 0 and x' = 0 and y' = y
}

// --- PROPERTY 1 (PASS) ---
// The sum x + y never exceeds 10.
// Since both x, y are in 0..5, the maximum is 5 + 5 = 10.
// This is a domain-level invariant that must always hold.
property sum_bounded {
  always (x + y <= 10)
}

// --- PROPERTY 2 (PASS) ---
// From the initial state (0,0), the next state via step_fwd has x = 1.
// step_fwd: x' = (0+0+1) mod 6 = 1. swap: x' = 0. So not ALL next states
// have x = 1 -- swap gives x' = 0. Let me pick something universally true.
//
// Actually: from (0, 0), step_fwd -> (1,1), swap -> (0,0), no reset_x (x=0).
// So next states are (1,1) and (0,0). Both satisfy x = y.
// From (0,0) next state always has x = y.
property init_preserves_equality {
  x = 0 and y = 0 -> next (x = y)
}

// --- PROPERTY 3 (PASS) ---
// Whenever x = 0 and y = 0, stepping forward gives x = 1 and y = 1.
// step_fwd from (0,0): x' = 1, y' = 1. swap from (0,0): x' = 0, y' = 0.
// So we say: from (0,0), next state has x <= 1 and y <= 1.
// step_fwd -> (1,1): 1<=1, 1<=1. swap -> (0,0): 0<=1, 0<=1. Both hold.
property from_origin_small_step {
  always (x = 0 and y = 0 -> next (x <= 1 and y <= 1))
}

// --- PROPERTY 4 (PASS) ---
// After reset_x fires, x = 0 in the next state. But we don't know which
// transition fires. Let's state a domain invariant instead:
// x and y are always each at most 5 (domain constraint).
property domain_upper_bound {
  always (x <= 5 and y <= 5)
}

// --- PROPERTY 5 (PASS) ---
// Whenever x = y, swapping preserves x = y (since swap just exchanges
// equal values). But other transitions may also fire, so we state:
// if x = y, then next state from swap has x = y (and from step_fwd or
// reset_x it might not). We need a universally true property.
//
// Let's use: always (x >= 0 and y >= 0). Both are in 0..5, so always true.
property domain_lower_bound {
  always (x >= 0 and y >= 0)
}

// --- INVALID 1 (expected FAIL) ---
// "x and y are always equal"
// False because step_fwd from (1,1) gives (3,2): x=3, y=2, not equal.
// Trace: (0,0) -> (1,1) -> (3,2). At (3,2), x != y.
invalid always_equal {
  always (x = y)
}

// --- INVALID 2 (expected FAIL) ---
// "x + y is always less than 5"
// False because step_fwd reaches states where x + y >= 5.
// Trace: (0,0) -> (1,1) -> (3,2) -> (0,3) -> (4,4). At (4,4), x+y = 8 >= 5.
invalid sum_always_small {
  always (x + y < 5)
}

// --- INVALID 3 (expected FAIL) ---
// "y never reaches 5"
// False because step_fwd increments y mod 6, so y cycles through all
// values 0..5. Trace continuing from above:
// (0,0) -> (1,1) -> (3,2) -> (0,3) -> (4,4) -> (3,5). y = 5.
invalid y_never_five {
  always (not (y = 5))
}

// --- INVALID 4 (expected FAIL) ---
// "once x becomes 0, it stays 0 forever"
// False because step_fwd can move x away from 0.
// Trace: (0,0) -> (1,1). x was 0, then became 1.
invalid x_zero_forever {
  always (x = 0 -> always (x = 0))
}
