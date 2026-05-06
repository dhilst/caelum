// Round 36, Tier 2: Non-deterministic transitions (KEYWORD syntax)
//
// System: a producer-consumer model with a bounded buffer.
//
// Variables:
//   buf   : 0..3   -- number of items in the buffer
//   ready : bool   -- whether the consumer is ready to consume
//
// Init: buf = 0, ready = false
//
// Transitions (any enabled transition can fire non-deterministically):
//
//   produce:  buf < 3             -> buf' = buf + 1, ready' = ready
//             Producer adds an item when buffer is not full.
//
//   consume:  ready and buf > 0   -> buf' = buf - 1, ready' = false
//             Consumer takes an item (and becomes unready afterwards).
//
//   prepare:  not ready           -> ready' = true, buf' = buf
//             Consumer becomes ready (only when not already ready).
//
//   slack:    ready and buf = 0   -> ready' = false, buf' = buf
//             Consumer gives up waiting when buffer is empty, becomes unready.
//
// Successor map from init (buf=0, ready=false):
//   (0, false) -> { (1, false) via produce,
//                    (0, true)  via prepare }
//
//   (1, false) -> { (2, false) via produce,
//                    (1, true)  via prepare }
//
//   (0, true)  -> { (1, true)  via produce,
//                    (0, false) via slack }
//
//   (1, true)  -> { (2, true)  via produce,
//                    (0, false) via consume }
//
//   (2, false) -> { (3, false) via produce,
//                    (2, true)  via prepare }
//
//   (2, true)  -> { (3, true)  via produce,
//                    (1, false) via consume }
//
//   (3, false) -> { (3, true)  via prepare }
//             (produce blocked: buf=3; consume blocked: not ready)
//
//   (3, true)  -> { (2, false) via consume }
//             (produce blocked: buf=3; slack blocked: buf>0; prepare blocked: ready)
//
//   (1, false) already covered above
//   (0, false) already covered above
//
// Full reachable states: all 8 combinations (buf in 0..3, ready in {true,false})
//
// Key observations for non-deterministic branching:
//   1. From (0, false), the system can EITHER produce (grow buffer) OR prepare
//      (ready the consumer). This fork leads to very different paths.
//   2. From (1, true), the system can produce (buf grows to 2) OR consume
//      (buf drops to 0 and ready becomes false). Maximally divergent outcomes.
//   3. (3, false) is a "bottleneck" state: only prepare is enabled, so the
//      next state is deterministically (3, true).
//   4. (3, true) is also deterministic: only consume fires -> (2, false).
//   5. The system always eventually returns to (0, false) on SOME path,
//      but NOT on ALL paths (produce can keep filling buffer faster than
//      consume drains it, cycling through high-buffer states).
//
// Properties use KEYWORD syntax:
//   always, eventually, next, until, and, or, not

module round_036

let buf: 0..3
let ready: bool

init {
  buf = 0 and ready = false
}

// Producer adds one item when buffer is not full; consumer state unchanged
transition produce {
  buf < 3 and buf' = buf + 1 and ready' = ready
}

// Consumer takes one item when ready and buffer non-empty; becomes unready
transition consume {
  ready = true and buf > 0 and buf' = buf - 1 and ready' = false
}

// Consumer becomes ready (only when currently not ready); buffer unchanged
transition prepare {
  ready = false and ready' = true and buf' = buf
}

// Consumer gives up when ready but buffer is empty; becomes unready
transition slack {
  ready = true and buf = 0 and ready' = false and buf' = buf
}

// --- PROPERTY 1 (PASS) ---
// Buffer is always within bounds. This is a domain invariant.
property buf_bounded {
  always (buf >= 0 and buf <= 3)
}

// --- PROPERTY 2 (PASS) ---
// When the buffer is full and consumer is not ready, the only enabled
// transition is prepare, so the consumer must become ready next.
// From (3, false): produce blocked (buf=3), consume blocked (not ready),
// slack blocked (buf != 0). Only prepare fires -> (3, true).
property full_buf_forces_prepare {
  always (buf = 3 and not ready = true -> next (ready = true))
}

// --- PROPERTY 3 (PASS) ---
// When the buffer is full and consumer IS ready, the only enabled transition
// is consume, so the buffer must decrease next.
// From (3, true): produce blocked (buf=3), prepare blocked (already ready),
// slack blocked (buf != 0). Only consume fires -> (2, false).
property full_and_ready_must_consume {
  always (buf = 3 and ready = true -> next (buf = 2 and not ready = true))
}

// --- PROPERTY 4 (PASS) ---
// After consuming, the consumer is always unready (consume sets ready' = false).
// More precisely: if ready and buf > 0 (consume can fire), then any next state
// where buf decreased must have ready = false. But with non-determinism, produce
// could also fire. Let's state something universally true:
// When buf = 3 and ready = true, next step always results in buf < 3.
// (Only consume is enabled, giving buf = 2 < 3.)
property consume_from_full_decreases {
  always (buf = 3 and ready = true -> next (buf < 3))
}

// --- PROPERTY 5 (PASS) ---
// From the initial state (0, false), the next state is either (1, false)
// via produce or (0, true) via prepare. In both cases buf <= 1.
property init_small_step {
  buf = 0 and not ready = true -> next (buf <= 1)
}

// --- INVALID 1 (expected FAIL) ---
// "The buffer is always eventually empty."
// This is false because the system can cycle through high-buffer states
// without ever reaching buf = 0. For example:
//   (0,false) -> produce -> (1,false) -> produce -> (2,false) -> produce
//   -> (3,false) -> prepare -> (3,true) -> consume -> (2,false) -> produce
//   -> (3,false) -> prepare -> (3,true) -> consume -> (2,false) -> ...
// The cycle (2,false) -> (3,false) -> (3,true) -> (2,false) avoids buf=0.
invalid always_eventually_empty {
  always eventually (buf = 0)
}

// --- INVALID 2 (expected FAIL) ---
// "When buffer has items and consumer is ready, the consumer always consumes."
// This is false because produce is also enabled when buf < 3 and ready = true.
// From (1, true): produce -> (2, true) is possible, not just consume -> (0, false).
// So it's not the case that next buf < 1.
invalid ready_always_consumes {
  always (ready = true and buf = 1 -> next (buf = 0))
}

// --- INVALID 3 (expected FAIL) ---
// "The consumer is never ready when the buffer is full."
// False because (3, true) is reachable:
//   (0,false) -> (1,false) -> (2,false) -> (2,true) -> (3,true)
// Or more directly:
//   ... -> (3,false) -> prepare -> (3,true).
invalid never_ready_when_full {
  always (buf = 3 -> not ready = true)
}

// --- INVALID 4 (expected FAIL) ---
// "Once the consumer becomes ready, it stays ready until the buffer is full."
// False because consume and slack both set ready' = false before buf reaches 3.
// Trace: (0,false) -> prepare -> (0,true) -> slack -> (0,false).
// Consumer became ready then immediately became unready, buf never reached 3.
invalid ready_persists_until_full {
  always (ready = true -> ready = true until buf = 3)
}
