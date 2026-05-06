Examples
========

Counter
-------

A simple modular counter that wraps around.

.. code-block:: text

   module examples.counter

   const max = 3

   let x ∈ 0..max

   init {
     x = 0
   }

   transition step {
     x' = (x + 1) mod (max + 1)
   }

   property in_range {
     □ (x >= 0 ∧ x <= max)
   }

   property returns_to_zero {
     □ ◇ (x = 0)
   }

**Properties:**

- ``in_range``: The counter always stays within ``0..max``. Passes because the domain enforces it.
- ``returns_to_zero``: The counter always eventually returns to zero. Passes because it cycles.

Failing Invariant
-----------------

Demonstrates a property that fails with a counterexample.

.. code-block:: text

   module examples.failing_invariant

   let x ∈ 0..2

   init {
     x = 0
   }

   transition step {
     x' = (x + 1) mod 3
   }

   property never_two {
     □ (x ≠ 2)
   }

The property ``never_two`` claims ``x`` is never 2, but the counter reaches 2 on the third step.
Run with ``--show-trace`` to see the counterexample:

.. code-block:: bash

   caelum --show-trace examples/failing_invariant.lum

Implication and Equivalence
---------------------------

.. code-block:: text

   let x ∈ 0..1

   init { x = 0 }

   transition toggle { x' = 1 - x }

   property implies_example {
     □ (x = 0 → ◯ x = 1)
   }

   property iff_example {
     □ (x = 0 ↔ ◯ x = 1)
   }

Both properties pass: when ``x = 0``, the next state always has ``x = 1``, and vice versa.
