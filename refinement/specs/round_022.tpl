// Round 22: Bool negation property
//
// System: single boolean variable `b` that toggles between true and false.
// Init: b = false.
// Transition: toggle (false -> true, true -> false).
//
// Properties:
//   tautology         — always (b or not b)            — PASS
//   neg_implies_event — always (not b -> eventually b) — PASS (toggle ensures b returns)
//   double_neg_equiv  — always (b <-> not not b)       — PASS
//   always_not_b      — always (not b)                 — FAIL (b is true in some states)

module round_022

let b: bool

init {
  b = false
}

transition toggle {
  (b = false and b' = true) or (b = true and b' = false)
}

// PASS: excluded middle — b is always either true or false
property tautology {
  always (b or not b)
}

// PASS: whenever b is false, the toggle makes it true in the next step
property neg_implies_event {
  always (not b -> eventually b)
}

// PASS: double negation equivalence on booleans
property double_neg_equiv {
  always (b <-> not not b)
}

// FAIL: b toggles, so it is true in some states
property always_not_b {
  always (not b)
}
