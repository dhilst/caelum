// Round 6: Single integer variable exercising `until` (𝒰)
//
// System: x ranges over 0..3. Init at x = 0.
// Deterministic transition: x increments modulo 4.
//   x = 0 -> x = 1 -> x = 2 -> x = 3 -> x = 0 -> ...
//
// Property until_pass (PASS):
//   (x < 3) until (x = 3)
//   The deterministic trace from x = 0 is: 0, 1, 2, 3, 0, 1, 2, 3, ...
//   At position 0: x=0, x=1, x=2 are all < 3, and at position 3 x = 3.
//   So there exists j=3 where Q holds, and for all k in [0,3), P holds.
//   This satisfies the until semantics.
//
// Property until_fail (FAIL):
//   (x = 0) until (x = 3)
//   The trace is: 0, 1, 2, 3, 0, ...
//   Q (x=3) first holds at position 3, but P (x=0) must hold at all
//   positions before that. At position 1, x=1 so P is false, violating
//   the "while" condition before Q is reached.

module round_006

let x: 0..3

init {
  x = 0
}

transition inc {
  x' = (x + 1) mod 4
}

// PASS: x stays < 3 until it reaches 3
property until_pass {
  (x < 3) until (x = 3)
}

// FAIL: x = 0 does NOT hold continuously until x = 3
property until_fail {
  (x = 0) until (x = 3)
}
