// Round 29, Tier 2: Enum + bool interaction (KEYWORD syntax)
//
// System: a three-phase state machine { idle, running, done } with a
// boolean "fail" flag.  The fail flag non-deterministically activates
// while in the "running" state.  When fail is true the machine goes
// back to idle instead of advancing to done.
//
// Variables:
//   state : enum { idle, running, done }
//   fail  : bool
//
// Init: state = idle, fail = false
//
// Transitions:
//   start:      idle,  any fail  -> running, fail = false
//   succeed:    running, fail=false -> done,    fail = false
//   break_it:   running, fail=false -> running, fail = true   (non-det fault)
//   retry:      running, fail=true  -> idle,    fail = false  (recovery)
//   finish:     done,    any fail  -> idle,    fail = false   (restart cycle)
//
// Reachable states (state, fail):
//   (idle, false), (running, false), (running, true), (done, false)
//
// Note: (idle, true) and (done, true) are never reachable.
//
// Successor map:
//   (idle, false)    -> { (running, false) }
//   (running, false) -> { (done, false), (running, true) }
//   (running, true)  -> { (idle, false) }
//   (done, false)    -> { (idle, false) }
//
// Traces include:
//   idle,f -> run,f -> done,f -> idle,f -> ...       (success loop)
//   idle,f -> run,f -> run,t -> idle,f -> ...        (fail-and-retry loop)
//   idle,f -> run,f -> run,t -> idle,f -> run,f -> done,f -> ...  (mixed)
//
// Properties (KEYWORD syntax):
//
// 1. done_means_no_fail (PASS):
//    always (state = done -> not fail = true)
//    Reaching "done" requires fail=false (succeed transition).
//
// 2. fail_only_while_running (PASS):
//    always (fail = true -> state = running)
//    The fail flag is only set during the running state.
//
// 3. always_can_recover (PASS):
//    always eventually (state = idle)
//    From every reachable state, idle is eventually revisited:
//    - idle -> running -> done -> idle  (success path)
//    - idle -> running -> idle  (fail-retry path)
//    No state can avoid returning to idle.
//
// 4. fail_implies_next_idle (PASS):
//    always (fail = true -> next (state = idle))
//    When fail is true (only in running), the only transition is retry -> idle.
//
// 5. done_without_running (FAIL via invalid):
//    always (state = idle -> next (state = done))
//    From idle the only transition goes to running, never directly to done.
//
// 6. fail_persists (FAIL via invalid):
//    always (fail = true -> next (fail = true))
//    The retry transition clears fail, so fail=true -> next fail=false.

module round_029

let state: enum { idle, running, done }
let fail: bool

init {
  state = idle and fail = false
}

// idle -> running (clear any lingering fail, though it should already be false)
transition start {
  state = idle and state' = running and fail' = false
}

// running without failure -> done
transition succeed {
  state = running and fail = false and state' = done and fail' = false
}

// non-deterministic fault while running
transition break_it {
  state = running and fail = false and state' = running and fail' = true
}

// recovery: fail sends us back to idle
transition retry {
  state = running and fail = true and state' = idle and fail' = false
}

// done -> restart the cycle
transition finish {
  state = done and state' = idle and fail' = false
}

// PASS: done is only reachable via the succeed transition which requires fail=false
property done_means_no_fail {
  always (state = done -> not fail = true)
}

// PASS: fail=true only occurs in the running state
property fail_only_while_running {
  always (fail = true -> state = running)
}

// PASS: every trace revisits idle infinitely often
property always_can_recover {
  always eventually (state = idle)
}

// PASS: from fail=true the only transition is retry which goes to idle
property fail_implies_next_idle {
  always (fail = true -> next (state = idle))
}

// from idle, next state is always running, never done
invalid done_without_running {
  always (state = idle -> next (state = done))
}

// fail is cleared by retry, so it does not persist
invalid fail_persists {
  always (fail = true -> next (fail = true))
}
