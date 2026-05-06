// Round 25: If-then pattern via implication (->)
//
// System: a 4-state cyclic counter modeling a request-response state machine.
//   x: 0..3, cycling through 0 -> 1 -> 2 -> 3 -> 0 -> ...
//   Init: x = 0.
//   Transition: x' = (x + 1) mod 4
//
// Trace from x = 0: 0, 1, 2, 3, 0, 1, 2, 3, ...
//
// State interpretation (request-response pattern):
//   0 = idle, 1 = request_sent, 2 = processing, 3 = response_ready
//
// Properties exercising implication (->):
//
// 1. idle_to_request (PASS):
//    always (x = 0 -> next (x = 1))
//    From idle we always transition to request_sent.
//
// 2. processing_returns_idle (PASS):
//    always (x = 2 -> eventually (x = 0))
//    From processing we eventually return to idle.
//
// 3. range_guard (PASS):
//    always (x >= 2 -> x <= 3)
//    Trivially true: if x >= 2 then x is 2 or 3, both <= 3.
//
// 4. response_wraps (PASS):
//    always (x = 3 -> next (x = 0))
//    From response_ready we wrap back to idle.
//
// 5. idle_skips_processing (FAIL):
//    always (x = 0 -> next (x = 2))
//    From idle the next state is 1 (request_sent), not 2 (processing).
//    This tests that implication correctly propagates a failing consequent.

module round_025

let x: 0..3

init {
  x = 0
}

transition step {
  x' = (x + 1) mod 4
}

// PASS: from idle (x=0), we always go to request_sent (x=1)
property idle_to_request {
  always (x = 0 -> next (x = 1))
}

// PASS: from processing (x=2), we eventually return to idle (x=0)
property processing_returns_idle {
  always (x = 2 -> eventually (x = 0))
}

// PASS: if x >= 2 then x <= 3 (trivially true in domain 0..3)
property range_guard {
  always (x >= 2 -> x <= 3)
}

// PASS: from response_ready (x=3), we wrap back to idle (x=0)
property response_wraps {
  always (x = 3 -> next (x = 0))
}

// from idle (x=0), the next state is x=1, not x=2
invalid idle_skips_processing {
  always (x = 0 -> next (x = 2))
}
