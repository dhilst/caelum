// Round 27, Tier 2: Bool + int range (ASCII operator syntax)
//
// System: a bool `flag` and an int `count: 0..3`.
//   - flag toggles every step
//   - when flag is true, count increments (mod 4)
//   - when flag is false, count stays the same
//
// Init: flag = true, count = 0
//
// Trace (deterministic):
//   s0: flag=true,  count=0
//   s1: flag=false, count=1
//   s2: flag=true,  count=1
//   s3: flag=false, count=2
//   s4: flag=true,  count=2
//   s5: flag=false, count=3
//   s6: flag=true,  count=3
//   s7: flag=false, count=0
//   s8: flag=true,  count=0   (= s0, cycle of length 8)
//
// ALL operators use ASCII syntax: [], <>, (), U, /\, \/, ~, ->, <->

module round_027

let flag: bool
let count: 0..3

init {
  flag = true /\ count = 0
}

transition step {
  flag' = ~ flag
  /\ (flag = true -> count' = (count + 1) mod 4)
  /\ (flag = false -> count' = count)
}

// PASS: count is always in the range 0..3 (always true by domain)
property count_in_range {
  [] (count >= 0 /\ count <= 3)
}

// PASS: the system always eventually returns to count = 0
// (the 8-state cycle visits count=0 twice: s0 and s7)
property count_returns_to_zero {
  [] <> (count = 0)
}

// PASS: when count=0, eventually count reaches 3
property zero_reaches_three {
  [] ((count = 0) -> <> (count = 3))
}

// PASS: flag toggles -- if flag is true now, it is false next
property flag_toggles {
  [] ((flag = true) <-> () (flag = false))
}

// PASS: eventually count reaches 3
property eventually_three {
  <> (count = 3)
}

// PASS: the cycle always revisits the initial state (flag=true, count=0)
property revisit_initial {
  [] (<> (flag = true /\ count = 0))
}

// FAIL (via invalid): flag and count=0 do NOT always hold together
// flag is true only half the time, and count=0 only at s0 and s7
invalid flag_always_with_zero {
  [] (flag = true /\ count = 0)
}

// FAIL (via invalid): count is NOT always strictly increasing
// count stays the same when flag=false, so "always count < () count" is false
// (Note: () count means "next count")
// Actually we can't write count < () count directly since () is prefix.
// Let's express: "it's always the case that count never equals 0 after step 0"
// which is false since count revisits 0.
invalid count_never_zero_again {
  [] ((count = 0) -> [] (count = 0))
}

// FAIL (via invalid): both variables cannot be simultaneously "stuck"
// flag=true and count=3 don't persist forever because flag toggles
invalid stuck_at_max {
  <> ([] (flag = true /\ count = 3))
}
