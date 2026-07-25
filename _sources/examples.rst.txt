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

Crossroad Traffic Light
-----------------------

Two traffic lights sharing a named ``Color`` type, with mutual exclusion
and fairness properties. This example demonstrates the ``type`` keyword
for reusable enum domains.

.. code-block:: text

   type Color = enum { red, green, yellow }

   let traf1 ∈ Color
   let traf2 ∈ Color
   let timer ∈ 0..5

   init {
     traf1 = green ∧ traf2 = red ∧ timer = 5
   }

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

   property mutual_exclusion {
     □ ¬ (traf1 = green ∧ traf2 = green)
   }

   property traf1_eventually_green {
     □ ◇ (traf1 = green)
   }

   invalid both_green {
     ◇ (traf1 = green ∧ traf2 = green)
   }

**Properties:**

- ``mutual_exclusion``: Both lights are never green simultaneously.
- ``traf1_eventually_green``: Traffic light 1 always eventually gets a green phase.
- ``both_green`` (invalid): Claims both lights are eventually green at once — correctly
  fails because the transitions prevent it.

See ``examples/crossroad_traffic_light.lum`` for the full specification with
all safety, liveness, and fairness properties. The :doc:`tutorial` walks through
building this example step by step.

Cluster Provisioning
--------------------

A multi-node provisioning model that exercises every multi-entity feature at
once: indexed state, parameterized transitions, ``unchanged`` (including
``except``), and quantifiers. Each node advances
``defined → image_assigned → powering_on → provisioned``.

.. code-block:: text

   type Node = enum { n1, n2 }
   type Image = enum { no_image, compute, storage }
   type Power = enum { off, on }
   type ProvisionStatus = enum { defined, image_assigned, powering_on, provisioned }

   let assigned_image[node ∈ Node] ∈ Image
   let requested_power[node ∈ Node] ∈ Power
   let actual_power[node ∈ Node] ∈ Power
   let status[node ∈ Node] ∈ ProvisionStatus

   init {
     ∀ node ∈ Node:
       assigned_image[node] = no_image ∧
       requested_power[node] = off ∧
       actual_power[node] = off ∧
       status[node] = defined
   }

   transition assign_image(node ∈ Node, image ∈ Image) {
     image ≠ no_image ∧
     status[node] = defined ∧
     assigned_image[node]' = image ∧
     status[node]' = image_assigned ∧
     unchanged(assigned_image except node, status except node,
               requested_power, actual_power)
   }

   property provisioned_nodes_have_images {
     □ (∀ node ∈ Node:
          status[node] = provisioned → assigned_image[node] ≠ no_image)
   }

**Properties:**

- ``provisioned_nodes_have_images``: A provisioned node always has a real image.
- ``provisioned_nodes_are_powered``: A provisioned node is always powered on.

See ``examples/cluster/cluster.lum`` for the full specification (all four
transitions plus an ``idle`` stutter that keeps a fully-provisioned cluster from
deadlocking). Counterexample traces name the firing instance, e.g.
``--(assign_image(n2, compute))-->``.

Node-level *liveness* such as
``□ (status[node] = powering_on → ◇ status[node] = provisioned)`` does **not**
hold under arbitrary interleaving without a fairness assumption, since a path may
keep servicing other nodes forever. Adding ``fairness { weak node_powers_on }``
rules out those unfair runs, and the property then holds. See
``examples/cluster/cluster_liveness.lum`` for the runnable version, which the
explicit engine proves:

.. code-block:: text

   fairness {
     weak node_powers_on
   }

   property provisioning_finishes {
     □ (∀ node ∈ Node:
          status[node] = powering_on → ◇ (status[node] = provisioned))
   }
