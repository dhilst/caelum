Tutorial
========

This tutorial walks through building a traffic light intersection controller
step by step, introducing each language feature as it becomes needed.

Step 1: A Single Traffic Light
------------------------------

Start with one traffic light that cycles through red, green, and yellow.
Declare a named type so the colours are self-documenting:

.. code-block:: text

   type Color = enum { red, green, yellow }

   let light ∈ Color
   let timer ∈ 0..5

   init {
     light = red ∧ timer = 5
   }

``type Color = enum { ... }`` declares a named set of values.
Variables declared with ``let light ∈ Color`` can hold any value from that set.
The timer is an integer in the range 0 to 5 (inclusive).

Step 2: Adding Transitions
--------------------------

Transitions describe how the system evolves. Each transition has a guard
(when it can fire) and an effect (what changes). Primed variables like
``light'`` refer to the value in the next state:

.. code-block:: text

   transition tick {
     timer > 0 ∧ timer' = timer - 1 ∧ light' = light
   }

   transition go_green {
     light = red ∧ timer = 0 ∧ light' = green ∧ timer' = 5
   }

   transition go_yellow {
     light = green ∧ timer = 0 ∧ light' = yellow ∧ timer' = 2
   }

   transition go_red {
     light = yellow ∧ timer = 0 ∧ light' = red ∧ timer' = 5
   }

The ``tick`` transition counts the timer down while keeping the light colour.
The other transitions fire when the timer expires, changing colour and
resetting the timer. Yellow gets a shorter timer (2 instead of 5).

Step 3: Safety Properties
-------------------------

Safety properties assert that something bad *never* happens.
The ``□`` (always) operator means "in every reachable state":

.. code-block:: text

   property timer_bounded {
     □ (timer ≥ 0 ∧ timer ≤ 5)
   }

   property yellow_is_short {
     □ (light = yellow → timer ≤ 2)
   }

``timer_bounded`` checks the timer never leaves its range.
``yellow_is_short`` checks that whenever the light is yellow, the timer
is at most 2 — ensuring yellow phases are always short.

Step 4: Liveness Properties
---------------------------

Liveness properties assert that something good *eventually* happens.
The ``□ ◇`` (always eventually) pattern means "this keeps happening forever":

.. code-block:: text

   property always_cycles {
     □ ◇ (light = red)
   }

This says the light always eventually returns to red — it never gets stuck.

Step 5: Two Lights at an Intersection
--------------------------------------

Now the payoff of named types: declare two variables with the same ``Color`` type.
Both ``traf1`` and ``traf2`` share the same enum variants (``red``, ``green``,
``yellow``):

.. code-block:: text

   type Color = enum { red, green, yellow }

   let traf1 ∈ Color
   let traf2 ∈ Color
   let timer ∈ 0..5

   init {
     traf1 = green ∧ traf2 = red ∧ timer = 5
   }

Without the ``type`` keyword, you would need globally unique variant names
for each light (e.g. ``t1_red``, ``t2_red``), or fall back to integer encoding.
Named types keep the specification readable.

Add transitions that cycle between the two lights:

.. code-block:: text

   transition tick {
     timer > 0 ∧ timer' = timer - 1 ∧ traf1' = traf1 ∧ traf2' = traf2
   }

   transition traf1_to_yellow {
     traf1 = green ∧ timer = 0 ∧ traf1' = yellow ∧ traf2' = red ∧ timer' = 2
   }

   transition swap_to_traf2 {
     traf1 = yellow ∧ timer = 0 ∧ traf1' = red ∧ traf2' = green ∧ timer' = 5
   }

   transition traf2_to_yellow {
     traf2 = green ∧ timer = 0 ∧ traf2' = yellow ∧ traf1' = red ∧ timer' = 2
   }

   transition swap_to_traf1 {
     traf2 = yellow ∧ timer = 0 ∧ traf2' = red ∧ traf1' = green ∧ timer' = 5
   }

Step 6: Mutual Exclusion
-------------------------

The critical safety property: both lights must never be green at the same time.

.. code-block:: text

   property mutual_exclusion {
     □ ¬ (traf1 = green ∧ traf2 = green)
   }

   property one_moving_at_most {
     □ (traf1 ≠ red → traf2 = red)
   }

``mutual_exclusion`` directly forbids both-green.
``one_moving_at_most`` is stronger: if one light is not red, the other must be.

Step 7: Fairness Properties
----------------------------

Fairness ensures both directions get a turn:

.. code-block:: text

   property traf1_green_implies_traf2_next {
     □ (traf1 = green → ◇ (traf2 = green))
   }

   property traf2_green_implies_traf1_next {
     □ (traf2 = green → ◇ (traf1 = green))
   }

After one light is green, the other eventually gets green too.

Step 8: Invalid Properties
--------------------------

The ``invalid`` keyword marks properties that should *not* hold. Caelum
reports PASS when the property fails (as expected) and FAIL when it
unexpectedly holds:

.. code-block:: text

   invalid both_green {
     ◇ (traf1 = green ∧ traf2 = green)
   }

This claims both lights are eventually green simultaneously. Since the
system prevents that, the property fails — and Caelum reports PASS.

Running the Full Specification
------------------------------

Save the complete specification as ``intersection.lum`` and run:

.. code-block:: bash

   caelum intersection.lum

You should see all properties pass. The full specification is available at
``examples/traffic_light_intersection.lum`` in the repository.

Summary of Language Features Used
----------------------------------

.. list-table::
   :header-rows: 1

   * - Feature
     - Syntax
     - Purpose
   * - Named type
     - ``type Color = enum { ... }``
     - Reusable domain shared by multiple variables
   * - Variable
     - ``let x ∈ Color``
     - State variable with a finite domain
   * - Init block
     - ``init { ... }``
     - Constrains the initial state
   * - Transition
     - ``transition name { ... }``
     - Defines state evolution with primed variables
   * - Safety property
     - ``property name { □ ... }``
     - Asserts an invariant over all reachable states
   * - Liveness property
     - ``property name { □ ◇ ... }``
     - Asserts something keeps happening
   * - Invalid property
     - ``invalid name { ... }``
     - Asserts a property does *not* hold
