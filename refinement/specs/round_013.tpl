// Round 13: Constants used inside expressions (init, transition, property)
// Tests: const values referenced in init assignments, transition arithmetic,
//        and property comparisons -- not just as domain bounds.

module round_013

const step = 2
const max = 6

let x: 0..max

init {
  x = step
}

transition advance {
  x' = (x + step) mod (max + 1)
}

property always_bounded {
  always (x < max + 1)
}

property returns_to_start {
  always (eventually (x = step))
}
