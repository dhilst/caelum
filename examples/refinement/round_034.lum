// Round 34, Tier 2: Three variables (UNICODE operator syntax)
//
// System: a traffic-light controller with three variables of different types.
//
// Variables:
//   light : enum { red, green, yellow }   -- the traffic light color
//   timer : 0..2                          -- countdown timer
//   active: bool                          -- whether the light is operational
//
// Init: light = red, timer = 2, active = true
//
// Transitions:
//   When active = true:
//     tick:        timer > 0  -> timer' = timer - 1, light' = light, active' = true
//     go_green:    light = red ∧ timer = 0 -> light' = green, timer' = 2, active' = true
//     go_yellow:   light = green ∧ timer = 0 -> light' = yellow, timer' = 1, active' = true
//     go_red:      light = yellow ∧ timer = 0 -> light' = red, timer' = 2, active' = true
//     shutdown:    timer = 0 -> light' = red, timer' = 0, active' = false
//   When active = false:
//     restart:     active = false -> light' = red, timer' = 2, active' = true
//
// Trace from init (red, 2, true):
//   (red,2,T) -> (red,1,T)                                       [tick]
//   (red,1,T) -> (red,0,T)                                       [tick]
//   (red,0,T) -> (green,2,T) | (red,0,F)                         [go_green | shutdown]
//   (green,2,T) -> (green,1,T)                                   [tick]
//   (green,1,T) -> (green,0,T)                                   [tick]
//   (green,0,T) -> (yellow,1,T) | (red,0,F)                      [go_yellow | shutdown]
//   (yellow,1,T) -> (yellow,0,T)                                  [tick]
//   (yellow,0,T) -> (red,2,T) | (red,0,F)                        [go_red | shutdown]
//   (red,0,F) -> (red,2,T)                                       [restart]
//
// Reachable states:
//   (red,2,T), (red,1,T), (red,0,T),
//   (green,2,T), (green,1,T), (green,0,T),
//   (yellow,1,T), (yellow,0,T),
//   (red,0,F)
//   Total: 9 states
//
// Key observations:
//   - yellow never has timer=2 (go_yellow sets timer' = 1)
//   - active=false only occurs at (red,0,F)
//   - light sequence is always red -> green -> yellow -> red
//   - shutdown can only happen at timer=0
//
// ALL operators use UNICODE syntax:
//   □ always, ◇ eventually, ◯ next, 𝒰 until
//   ∧ and, ∨ or, ¬ not
//   -> implies, <-> iff

module round_034

let light: enum { red, green, yellow }
let timer ∈ 0..2
let active: bool

init {
  light = red ∧ timer = 2 ∧ active = true
}

// Tick down the timer while light stays the same (active must be true)
transition tick {
  active = true ∧ timer > 0
  ∧ timer' = timer - 1 ∧ light' = light ∧ active' = true
}

// Red -> Green when timer expires
transition go_green {
  active = true ∧ light = red ∧ timer = 0
  ∧ light' = green ∧ timer' = 2 ∧ active' = true
}

// Green -> Yellow when timer expires
transition go_yellow {
  active = true ∧ light = green ∧ timer = 0
  ∧ light' = yellow ∧ timer' = 1 ∧ active' = true
}

// Yellow -> Red when timer expires
transition go_red {
  active = true ∧ light = yellow ∧ timer = 0
  ∧ light' = red ∧ timer' = 2 ∧ active' = true
}

// Shutdown: from timer=0 while active, go to inactive
transition shutdown {
  active = true ∧ timer = 0
  ∧ light' = red ∧ timer' = 0 ∧ active' = false
}

// Restart: from inactive, go back to initial-like state
transition restart {
  active = false
  ∧ light' = red ∧ timer' = 2 ∧ active' = true
}

// --- PROPERTY 1 (PASS) ---
// When inactive, the light is always red.
// The only inactive state is (red, 0, false).
property inactive_means_red {
  □ (active = false -> light = red)
}

// --- PROPERTY 2 (PASS) ---
// Yellow light never has timer = 2.
// go_yellow sets timer' = 1, and tick only decrements, so yellow
// states are (yellow, 1, T) and (yellow, 0, T) -- never timer = 2.
property yellow_timer_bounded {
  □ (light = yellow -> timer <= 1)
}

// --- PROPERTY 3 (PASS) ---
// After a green light, the next light change goes to yellow (not red or staying green).
// When green and timer = 0, go_yellow -> (yellow, 1, T) or shutdown -> (red, 0, F).
// But shutdown goes to red, so this property needs care. Let's state:
// "If light = green and timer = 0, then next state has light = yellow or active = false"
// This is true: go_yellow gives yellow, shutdown gives (red, 0, false).
property green_then_yellow_or_shutdown {
  □ (light = green ∧ timer = 0 -> ◯ (light = yellow ∨ active = false))
}

// --- PROPERTY 4 (PASS) ---
// Whenever the system shuts down, it can restart and eventually become active again.
// From (red, 0, false), restart -> (red, 2, true). So after inactive, next is always active.
property inactive_implies_next_active {
  □ (active = false -> ◯ (active = true))
}

// --- PROPERTY 5 (PASS) ---
// Timer and activity invariant: if active is false, timer must be 0.
// The only inactive state is (red, 0, false), which has timer = 0.
property inactive_timer_zero {
  □ (active = false -> timer = 0)
}

// --- INVALID 1 (expected FAIL) ---
// "Yellow light is always immediately followed by red light"
// This is false because yellow with timer = 1 ticks to yellow with timer = 0 first.
// Counterexample: (yellow, 1, T) -> (yellow, 0, T), light is still yellow.
invalid yellow_immediately_red {
  □ (light = yellow -> ◯ (light = red ∨ active = false))
}

// --- INVALID 2 (expected FAIL) ---
// "The light is never green while timer = 2"
// This is false because go_green sets light' = green and timer' = 2.
// Counterexample: (red, 0, T) -> (green, 2, T).
invalid never_green_at_two {
  □ (¬(light = green ∧ timer = 2))
}

// --- INVALID 3 (expected FAIL) ---
// "Once active becomes false, it stays false forever"
// This is false because restart transitions from inactive to active.
// Counterexample: (red, 0, F) -> (red, 2, T).
invalid inactive_forever {
  □ (active = false -> □ (active = false))
}

// --- INVALID 4 (expected FAIL) ---
// "Timer always decreases (never increases)"
// This is false because go_green, go_red, and restart all reset timer to 2.
// Counterexample: (red, 0, T) -> (green, 2, T), timer went from 0 to 2.
invalid timer_only_decreases {
  □ (timer = 0 -> ◯ (timer = 0))
}
