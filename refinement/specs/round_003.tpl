// Round 3: Single enum variable with equality checks
// Tests: enum domain, init block, cycling transitions through enum variants,
//        always property that passes (tautology), always property that fails (state != done)

module round_003

let state: enum { idle, running, done }

init {
  state = idle
}

// Cycle: idle -> running -> done -> idle -> ...
transition to_running {
  state = idle and state' = running
}

transition to_done {
  state = running and state' = done
}

transition to_idle {
  state = done and state' = idle
}

// PASS: state is always one of the three valid variants (tautology for an enum)
property always_valid {
  always (state = idle or state = running or state = done)
}

// FAIL: state starts idle, moves to running, then reaches done.
// Once state = done, this property is violated.
property never_done {
  always (state != done)
}
