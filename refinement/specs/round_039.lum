// Round 39, Tier 2: Counter with reset transition (KEYWORD syntax)
//
// System: a bounded counter (0..4) with a boolean reset signal.
// The counter increments normally, but a separate reset transition
// can fire non-deterministically at any time to bring the counter
// back to zero.  The reset signal variable controls this:
//
//   - When rst = false, the counter can increment (if not at max).
//   - At any time, the system can non-deterministically arm the reset
//     by setting rst = true (without changing the counter yet).
//   - When rst = true, the only allowed action is to actually reset
//     the counter to 0 and clear the signal (rst = false).
//
// This two-phase reset (arm then fire) creates interesting properties:
// the counter freezes once rst is armed, and always returns to 0 after
// being armed.
//
// Variables:
//   cnt : 0..4   -- the counter value
//   rst : bool   -- reset signal (true = armed, about to reset)
//
// Init: cnt = 0, rst = false
//
// Transitions:
//   increment:  rst = false, cnt < 4 -> cnt' = cnt + 1, rst' = false
//               Normal counting when not armed and not at max.
//
//   arm_reset:  rst = false -> cnt' = cnt, rst' = true
//               Non-deterministically arm the reset at any counter value.
//               This freezes the counter (cnt' = cnt).
//
//   do_reset:   rst = true -> cnt' = 0, rst' = false
//               Execute the reset: counter goes to 0, signal clears.
//
// Successor map from init (cnt=0, rst=false):
//   (0, false) -> { (1, false) via increment,
//                    (0, true)  via arm_reset }
//   (0, true)  -> { (0, false) via do_reset }
//   (1, false) -> { (2, false) via increment,
//                    (1, true)  via arm_reset }
//   (1, true)  -> { (0, false) via do_reset }
//   (2, false) -> { (3, false) via increment,
//                    (2, true)  via arm_reset }
//   (2, true)  -> { (0, false) via do_reset }
//   (3, false) -> { (4, false) via increment,
//                    (3, true)  via arm_reset }
//   (3, true)  -> { (0, false) via do_reset }
//   (4, false) -> { (4, true)  via arm_reset }
//              (increment blocked: cnt = 4)
//   (4, true)  -> { (0, false) via do_reset }
//
// Reachable states: all 10 combinations (cnt in 0..4, rst in {true, false}).
//
// Key observations:
//   1. From any (k, false) with k < 4, the system can either increment
//      or arm the reset. This non-deterministic choice is the core.
//   2. From any (k, true), the only transition is do_reset -> (0, false).
//      So rst = true is a "committed" state that deterministically resets.
//   3. (4, false) is a bottleneck: increment is blocked, so the only
//      option is arm_reset -> (4, true) -> do_reset -> (0, false).
//      Hence from cnt = 4, the system MUST return to 0 within 2 steps.
//   4. On ALL paths, the system eventually returns to (0, false) because:
//      - If the system keeps incrementing, it reaches (4, false) which
//        forces arm_reset -> do_reset -> (0, false).
//      - If arm_reset fires earlier, do_reset fires next -> (0, false).
//      So cnt = 0 is visited infinitely often on every trace.
//   5. However, cnt does NOT always eventually reach 4, because arm_reset
//      can fire at any lower value, sending the counter back to 0 before
//      it ever reaches 4.
//
// ALL operators use KEYWORD syntax:
//   always, eventually, next, until, and, or, not

module round_039

let cnt: 0..4
let rst: bool

init {
  cnt = 0 and rst = false
}

// Normal increment when reset is not armed and counter is below max
transition increment {
  rst = false and cnt < 4 and cnt' = cnt + 1 and rst' = false
}

// Non-deterministically arm the reset signal (counter freezes)
transition arm_reset {
  rst = false and cnt' = cnt and rst' = true
}

// Execute the reset: counter returns to 0, signal clears
transition do_reset {
  rst = true and cnt' = 0 and rst' = false
}

// --- PROPERTY 1 (PASS) ---
// Counter is always within bounds.  This is a domain invariant: cnt is
// declared as 0..4, so it always satisfies 0 <= cnt <= 4.
property cnt_bounded {
  always (cnt >= 0 and cnt <= 4)
}

// --- PROPERTY 2 (PASS) ---
// When the reset signal is armed, the counter must be 0 at the next step.
// From any state (k, true), the only enabled transition is do_reset,
// which sets cnt' = 0.  So rst = true -> next(cnt = 0) always holds.
property armed_resets_next {
  always (rst = true -> next (cnt = 0))
}

// --- PROPERTY 3 (PASS) ---
// The counter always eventually returns to zero.
// On every path: if the system keeps incrementing it must reach 4,
// which forces arm_reset then do_reset back to 0.  If arm_reset fires
// sooner, do_reset immediately returns to 0.  Either way, cnt = 0 is
// revisited infinitely often on every trace.
property always_eventually_zero {
  always eventually (cnt = 0)
}

// --- PROPERTY 4 (PASS) ---
// When rst is armed, it is cleared in the next step (do_reset fires,
// setting rst' = false).  From any (k, true), only do_reset is enabled.
property rst_cleared_next {
  always (rst = true -> next (rst = false))
}

// --- PROPERTY 5 (PASS) ---
// At counter maximum (cnt = 4) and rst = false, increment is blocked
// (cnt < 4 fails), so the only enabled transition is arm_reset.
// Therefore the next state must have rst = true.
property max_forces_arm {
  always (cnt = 4 and rst = false -> next (rst = true))
}

// --- PROPERTY 6 (PASS) ---
// Combining properties 5 and 2: from (4, false), within 2 steps the
// counter must be 0.  Stated as: cnt = 4 and not rst = true implies
// next(next(cnt = 0)).
property max_resets_in_two {
  always (cnt = 4 and rst = false -> next (next (cnt = 0)))
}

// --- INVALID 1 (expected FAIL) ---
// "The counter always eventually reaches 4."
// FALSE: arm_reset can fire at any value (e.g., at cnt = 1), sending
// the counter back to 0 before ever reaching 4.  There exist infinite
// traces where arm_reset fires every time cnt = 1:
//   (0,F) -> inc -> (1,F) -> arm -> (1,T) -> reset -> (0,F) -> inc -> (1,F) -> arm -> ...
// The counter oscillates between 0 and 1, never reaching 4.
invalid always_eventually_max {
  always eventually (cnt = 4)
}

// --- INVALID 2 (expected FAIL) ---
// "Once the counter leaves 0, it stays above 0 until it reaches 4."
// FALSE: arm_reset can fire and do_reset brings cnt back to 0 before
// reaching 4.  Trace:
//   (0,F) -> inc -> (1,F) -> arm -> (1,T) -> reset -> (0,F)
// Counter was at 1, returned to 0 without ever reaching 4.
invalid stays_up_until_max {
  always (cnt > 0 -> cnt > 0 until cnt = 4)
}

// --- INVALID 3 (expected FAIL) ---
// "The reset signal is never armed."
// FALSE: arm_reset can fire from any (k, false) state.
// Trace: (0,F) -> arm_reset -> (0,T).  Now rst = true.
invalid rst_never_armed {
  always (rst = false)
}

// --- INVALID 4 (expected FAIL) ---
// "If cnt = 0, then cnt is still 0 at the next step."
// FALSE: increment can fire from (0, false) giving (1, false).
// Trace: (0,F) -> inc -> (1,F).  cnt was 0, now it's 1.
invalid zero_is_sticky {
  always (cnt = 0 -> next (cnt = 0))
}
