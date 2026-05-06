// Round 37, Tier 2: Mutual exclusion pattern (ASCII operator syntax)
//
// System: two processes (p1 and p2) competing for a critical section.
// Each process has three states: idle, trying, critical.
// Because the engine requires globally unique enum variant names,
// we prefix variants: p1 uses {p1_idle, p1_try, p1_crit} and
// p2 uses {p2_idle, p2_try, p2_crit}.
//
// Variables:
//   p1 : enum { p1_idle, p1_try, p1_crit }
//   p2 : enum { p2_idle, p2_try, p2_crit }
//
// Init: p1 = p1_idle, p2 = p2_idle
//
// Protocol rules:
//   - A process can move from idle to trying at any time.
//   - A process can move from trying to critical ONLY if the other
//     process is NOT in the critical section (mutual exclusion enforced
//     at entry).
//   - A process in critical moves to idle (releases the lock).
//   - Each transition updates one process and frames the other (p2' = p2
//     or p1' = p1), modeling interleaved concurrency.
//
// Transitions (one process moves per step, the other is framed):
//
//   p1_request:  p1 = p1_idle                       -> p1' = p1_try,  p2' = p2
//   p1_enter:    p1 = p1_try /\ ~(p2 = p2_crit)    -> p1' = p1_crit, p2' = p2
//   p1_release:  p1 = p1_crit                       -> p1' = p1_idle, p2' = p2
//
//   p2_request:  p2 = p2_idle                       -> p2' = p2_try,  p1' = p1
//   p2_enter:    p2 = p2_try /\ ~(p1 = p1_crit)    -> p2' = p2_crit, p1' = p1
//   p2_release:  p2 = p2_crit                       -> p2' = p2_idle, p1' = p1
//
// Reachable states from (idle, idle):
//   Step 0: { (idle, idle) }
//   Step 1: { (try, idle), (idle, try) }
//   Step 2 from (try, idle):
//     p1_enter:   (crit, idle)
//     p2_request: (try, try)
//   Step 2 from (idle, try):
//     p2_enter:   (idle, crit)
//     p1_request: (try, try)
//   Step 3 from (crit, idle):
//     p1_release: (idle, idle)         -- already seen
//     p2_request: (crit, try)
//   Step 3 from (try, try):
//     p1_enter:   (crit, try)          -- p2 not crit, so p1 can enter
//     p2_enter:   (try, crit)          -- p1 not crit, so p2 can enter
//   Step 3 from (idle, crit):
//     p2_release: (idle, idle)         -- already seen
//     p1_request: (try, crit)
//   Step 4 from (crit, try):
//     p1_release: (idle, try)          -- already seen
//     (p2_enter blocked: p1 = crit)
//   Step 4 from (try, crit):
//     p2_release: (try, idle)          -- already seen
//     (p1_enter blocked: p2 = crit)
//
// Full reachable states (9 minus unreachable):
//   (idle, idle), (idle, try), (idle, crit),
//   (try, idle), (try, try), (try, crit),
//   (crit, idle), (crit, try)
//
// NOT reachable: (crit, crit) -- the mutual exclusion property!
//   When p1 = crit, p2_enter is blocked (guard ~(p1 = p1_crit) fails).
//   When p2 = crit, p1_enter is blocked (guard ~(p2 = p2_crit) fails).
//   So both processes can never be in critical simultaneously.
//
// Key observations:
//   1. MUTUAL EXCLUSION: ~(p1 = p1_crit /\ p2 = p2_crit) always holds.
//   2. From (try, try), exactly one process can enter critical,
//      but not both (non-deterministic choice, but exclusive).
//   3. A process in critical can stay critical for another step if the
//      other process makes a move (frame condition), but will eventually
//      leave via release.
//   4. A trying process never retreats to idle; it can only stay trying
//      or advance to critical (forward-only protocol).
//   5. Starvation IS possible: one process can stay in trying forever
//      if the other process keeps cycling (non-determinism allows this).
//      So [] <> (p1 = p1_idle) does NOT hold and is NOT claimed.
//
// Properties (ASCII syntax):
//   [] always, <> eventually, () next, U until
//   /\ and, \/ or, ~ not, -> implies, <-> iff

module round_037

let p1: enum { p1_idle, p1_try, p1_crit }
let p2: enum { p2_idle, p2_try, p2_crit }

init {
  p1 = p1_idle /\ p2 = p2_idle
}

// Process 1 transitions (p2 is framed)
transition p1_request {
  p1 = p1_idle /\ p1' = p1_try /\ p2' = p2
}

transition p1_enter {
  p1 = p1_try /\ ~(p2 = p2_crit) /\ p1' = p1_crit /\ p2' = p2
}

transition p1_release {
  p1 = p1_crit /\ p1' = p1_idle /\ p2' = p2
}

// Process 2 transitions (p1 is framed)
transition p2_request {
  p2 = p2_idle /\ p2' = p2_try /\ p1' = p1
}

transition p2_enter {
  p2 = p2_try /\ ~(p1 = p1_crit) /\ p2' = p2_crit /\ p1' = p1
}

transition p2_release {
  p2 = p2_crit /\ p2' = p2_idle /\ p1' = p1
}

// --- PROPERTY 1 (PASS) ---
// MUTUAL EXCLUSION: both processes are never in the critical section
// at the same time. This is the central property of the protocol.
// It holds because p1_enter requires ~(p2 = p2_crit) and p2_enter
// requires ~(p1 = p1_crit), so (crit, crit) is unreachable.
property mutual_exclusion {
  [] ~(p1 = p1_crit /\ p2 = p2_crit)
}

// --- PROPERTY 2 (PASS) ---
// If p1 is critical, then in the next step p1 is either idle (released)
// or still critical (if a p2 transition fired and p1 was framed).
property critical_to_idle_or_stay {
  [] (p1 = p1_crit -> () (p1 = p1_idle \/ p1 = p1_crit))
}

// --- PROPERTY 3 (PASS) ---
// Symmetry: same property for p2.
property critical_to_idle_or_stay_p2 {
  [] (p2 = p2_crit -> () (p2 = p2_idle \/ p2 = p2_crit))
}

// --- PROPERTY 4 (PASS) ---
// If p1 is trying and p2 is idle, then next p1 is either critical
// (p1_enter fired) or still trying (a p2 transition fired and p1
// was framed). p1 cannot go to idle from trying.
property trying_with_idle_other {
  [] (p1 = p1_try /\ p2 = p2_idle -> () (p1 = p1_crit \/ p1 = p1_try))
}

// --- PROPERTY 5 (PASS) ---
// Process states only move forward in the protocol: idle -> try -> crit.
// A trying process cannot go backwards to idle; it can only stay trying
// (framed by the other process's move) or advance to critical.
// From p1 = p1_try: p1_enter -> p1_crit, or p2 moves and p1 stays p1_try.
// No transition sets p1' = p1_idle when p1 = p1_try.
property trying_never_retreats {
  [] (p1 = p1_try -> () (p1 = p1_try \/ p1 = p1_crit))
}

// --- PROPERTY 6 (PASS) ---
// Symmetry: same forward-only property for p2.
property trying_never_retreats_p2 {
  [] (p2 = p2_try -> () (p2 = p2_try \/ p2 = p2_crit))
}

// --- PROPERTY 7 (PASS) ---
// At most one process is in the critical section at any time.
// This is equivalent to mutual exclusion but expressed differently:
// if p1 is critical, then p2 is either idle or trying (not critical).
property exclusion_alt {
  [] (p1 = p1_crit -> (p2 = p2_idle \/ p2 = p2_try))
}

// --- INVALID 1 (expected FAIL) ---
// "Both processes can be in the critical section at the same time."
// This is false because (crit, crit) is unreachable: no trace reaches
// a state where p1 = p1_crit /\ p2 = p2_crit.
invalid both_critical {
  <> (p1 = p1_crit /\ p2 = p2_crit)
}

// --- INVALID 2 (expected FAIL) ---
// "Once p1 enters critical, it stays critical forever."
// False because p1_release transitions p1 from critical to idle.
// Trace: (idle,idle) -> (try,idle) -> (crit,idle) -> (idle,idle).
invalid critical_forever {
  [] (p1 = p1_crit -> [] (p1 = p1_crit))
}

// --- INVALID 3 (expected FAIL) ---
// "p1 is always idle" -- false because p1 can transition to trying.
// Trace: (idle,idle) -> (try,idle). p1 is not idle.
invalid p1_always_idle {
  [] (p1 = p1_idle)
}

// --- INVALID 4 (expected FAIL) ---
// "If both processes are trying, they both enter critical next step."
// False because only one can enter at a time (one transition fires per
// step), and the mutual exclusion guard prevents both being critical.
// From (try, try): p1_enter -> (crit, try). p2 is still trying.
invalid both_enter_simultaneously {
  [] (p1 = p1_try /\ p2 = p2_try -> () (p1 = p1_crit /\ p2 = p2_crit))
}

// --- INVALID 5 (expected FAIL) ---
// "p1 always eventually returns to idle." (starvation freedom)
// False because p1 can be starved: from (try, try), if p2_enter always
// fires, we get the cycle (try, try) -> (try, crit) -> (try, idle) ->
// (try, try) -> ... and p1 stays in p1_try forever.
invalid no_starvation_p1 {
  [] <> (p1 = p1_idle)
}
