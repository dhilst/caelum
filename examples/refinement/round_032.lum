// Round 32, Tier 2: Multiple init blocks / conjunction (KEYWORD syntax)
//
// System: two integer variables x in 0..3 and y in 0..3 with multiple
// init blocks that jointly constrain the initial states.
//
// Init blocks (conjunction semantics -- ALL must hold simultaneously):
//   init { x >= 1 }       -- x in {1, 2, 3}
//   init { x <= 2 }       -- x in {0, 1, 2}
//   init { y >= 2 }       -- y in {2, 3}
//   init { x + y <= 4 }   -- restricts high combinations
//
// Combined init constraint: x in {1, 2} and y in {2, 3} and x + y <= 4
//   Valid initial states:
//     (x=1, y=2): 1+2=3 <= 4  YES
//     (x=1, y=3): 1+3=4 <= 4  YES
//     (x=2, y=2): 2+2=4 <= 4  YES
//     (x=2, y=3): 2+3=5 <= 4  NO
//   Initial states = { (1,2), (1,3), (2,2) }
//
// Transition (deterministic):
//   x' = 3 - x     (flip: 1->2, 2->1)
//   y' = 5 - y     (flip: 2->3, 3->2)
//
// Note: for init state (1,2): next is (2,3), then x+y=5 > 4.
//       for init state (1,3): next is (2,2), then (1,3), cycle.
//       for init state (2,2): next is (1,3), then (2,2), cycle.
//
// Reachable states from each initial:
//   From (1,2): (1,2) -> (2,3) -> (1,2) -> ...   cycle: {(1,2), (2,3)}
//   From (1,3): (1,3) -> (2,2) -> (1,3) -> ...   cycle: {(1,3), (2,2)}
//   From (2,2): (2,2) -> (1,3) -> (2,2) -> ...   cycle: {(2,2), (1,3)}
//
// All reachable states = { (1,2), (2,3), (1,3), (2,2) }
//
// Key facts about reachable states:
//   x is always in {1, 2}
//   y is always in {2, 3}
//   x + y is always in {3, 4, 5}
//   x = 0 never reachable, x = 3 never reachable
//   y = 0 never reachable, y = 1 never reachable
//
// Properties (KEYWORD syntax):

module round_032

let x: 0..3
let y: 0..3

// Multiple init blocks -- conjunction semantics
init { x >= 1 }
init { x <= 2 }
init { y >= 2 }
init { x + y <= 4 }

// Deterministic flip transition
transition flip {
  x' = 3 - x and y' = 5 - y
}

// --- PROPERTY 1 (PASS) ---
// x is always 1 or 2. The init blocks restrict x to {1,2}, and the
// transition 3-x maps {1,2} to {2,1}, so x stays in {1,2} forever.
property x_in_one_two {
  always (x >= 1 and x <= 2)
}

// --- PROPERTY 2 (PASS) ---
// y is always 2 or 3. The init blocks restrict y to {2,3}, and the
// transition 5-y maps {2,3} to {3,2}, so y stays in {2,3} forever.
property y_in_two_three {
  always (y >= 2 and y <= 3)
}

// --- PROPERTY 3 (PASS) ---
// The transition is an involution: applying flip twice returns to the
// same state. So if x=1 now, then next x=2, then next-next x=1.
// Equivalently: always (x = 1 -> next (x = 2)) and (x = 2 -> next (x = 1)).
property x_toggles {
  always ((x = 1 -> next (x = 2)) and (x = 2 -> next (x = 1)))
}

// --- INVALID 1 (expected FAIL) ---
// "x is always 1" -- false because from (1,y), next x = 2.
// Counterexample: any initial state with x=1, e.g. (1,2) -> (2,3).
invalid x_always_one {
  always (x = 1)
}

// --- INVALID 2 (expected FAIL) ---
// "x + y is always <= 4" -- true for the 3 initial states but false
// for (2,3) which is reachable from (1,2). x+y = 5 > 4.
// Counterexample: (1,2) -> (2,3), and 2+3=5 > 4.
invalid sum_always_le_four {
  always (x + y <= 4)
}

// --- INVALID 3 (expected FAIL) ---
// "x = 0 is eventually reachable" -- false, x stays in {1,2} forever.
// No trace starting from an initial state ever reaches x=0.
invalid x_reaches_zero {
  eventually (x = 0)
}

// --- PROPERTY 4 (PASS) ---
// The system always eventually returns to y = 2. Since y toggles
// between 2 and 3, it visits y=2 every other step.
property y_revisits_two {
  always eventually (y = 2)
}
