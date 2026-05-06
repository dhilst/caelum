// Round 12: Constants used as domain bounds for integer range variables
// Tests: const declarations as lo/hi bounds in let domain, init, transition, property

module round_012

const lo = 0
const hi = 3

let x: lo..hi

init {
  x = lo
}

transition inc {
  x' = (x + 1) mod (hi + 1)
}

property always_in_range {
  always (x >= lo and x <= hi)
}

property returns_to_zero {
  always (eventually (x = lo))
}
