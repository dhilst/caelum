Tutorial: A Traffic-Light Controller
====================================

This tutorial builds a working specification from scratch, introducing each
language feature as it becomes needed. It assumes you can program but have never
used LTL or formal logic — every symbol is introduced with a JavaScript analogy.

You can do the whole tutorial **in your browser**: each specification below is a
live editor (see :doc:`using-the-editor`). Press **Check ▶** or ``Ctrl-Enter`` to
run it. Nothing needs to be installed until you want to run specs from the command
line (:doc:`cli-reference`).

What we'll build, and why
-------------------------

We are going to specify a **crossroad traffic-light controller**: two lights
facing perpendicular directions of an intersection, cycling through red, green,
and yellow. It is a small example, but it is a genuinely good one for model
checking, because a traffic light has real correctness requirements that are easy
to state and disastrous to get wrong:

- A **safety** requirement: the two lights must **never both be green** at the same
  time. If that rule is ever broken, cars collide. Safety properties are the "this
  bad thing never happens" rules from :doc:`when-to-use`.
- A **liveness** requirement: each direction must **keep getting a green light** —
  neither side may be starved forever. Liveness properties are the "this good
  thing keeps happening" rules.

Here is the mental model to carry through the tutorial:

- The **system is a state machine.** The lights and a countdown timer are its
  *state*; the rules for switching colours are its *transitions*.
- The **requirements are properties.** We write down mutual exclusion and fairness
  as temporal-logic properties, and Caelum checks them against *every* way the
  lights can ever cycle — not just the runs we happened to imagine.

The intersection is small enough to explore exhaustively, concrete enough to
picture, and its safety rule is one everybody already understands. That makes it an
ideal first model. Let's start with the notation, then build the machine one piece
at a time.

Notation Primer
---------------

Before we start writing specifications, let's get familiar with the symbols
Caelum uses. If you've never worked with formal logic notation, this section
will get you up to speed.

Notation Primer
---------------

Caelum specifications use symbols from logic and set theory. Every symbol
has an ASCII alternative you can type on a regular keyboard — you never need
to type Unicode by hand.

Logical Connectives
^^^^^^^^^^^^^^^^^^^

These work like the boolean operators you already know from programming.

**Conjunction** ``∧`` (ASCII: ``/\`` or keyword: ``and``) — logical AND.
Both sides must be true.

In JavaScript:

.. code-block:: javascript

   // Caelum:  x = 0 ∧ y = 1
   // means:
   if (x === 0 && y === 1) { /* ... */ }

**Disjunction** ``∨`` (ASCII: ``\/`` or keyword: ``or``) — logical OR.
At least one side must be true.

.. code-block:: javascript

   // Caelum:  x = 0 ∨ y = 1
   // means:
   if (x === 0 || y === 1) { /* ... */ }

**Negation** ``¬`` (ASCII: ``~`` or keyword: ``not``) — logical NOT.
Flips true to false and vice versa.

.. code-block:: javascript

   // Caelum:  ¬ (x = 0)
   // means:
   if (!(x === 0)) { /* ... */ }

**Implication** ``→`` (ASCII: ``->``) — "if ... then ...".
``A → B`` means whenever A is true, B must also be true. It is only false
when A is true and B is false.

.. code-block:: javascript

   // Caelum:  x = 0 → y = 1
   // means:
   if (x === 0) {
     assert(y === 1);
   }

**Not equal** ``≠`` (ASCII: ``!=``) — same as ``!=`` in most languages.

**Less or equal** ``≤`` (ASCII: ``<=``), **greater or equal** ``≥``
(ASCII: ``>=``) — same as in most languages.

Set Membership
^^^^^^^^^^^^^^

**Element of** ``∈`` (ASCII: ``:``).
In Caelum, ``let x ∈ 0..3`` means "x is a variable whose value is
an element of the set {0, 1, 2, 3}." You can read it as "x is in 0 to 3."
The ASCII alternative is a plain colon: ``let x : 0..3``.

Temporal Operators
^^^^^^^^^^^^^^^^^^

These are specific to model checking — they describe properties that hold
across *time* (sequences of states), not just a single state.

**Always** ``□`` (ASCII: ``[]`` or keyword: ``always``).
``□ P`` means P is true in *every* reachable state. Think of it as
a loop that checks every state your system can ever reach:

.. code-block:: javascript

   // Caelum:  □ (x >= 0)
   // Conceptually:
   for (const state of allReachableStates) {
     assert(state.x >= 0);
   }

**Eventually** ``◇`` (ASCII: ``<>`` or keyword: ``eventually``).
``◇ P`` means P becomes true at *some* point in the future. No matter
where you are, you can always reach a state where P holds:

.. code-block:: javascript

   // Caelum:  ◇ (x = 0)
   // Conceptually:
   // Starting from any state, there exists a future state where x === 0

**Always eventually** ``□ ◇`` is a common pattern. ``□ ◇ P`` means P
keeps happening forever — the system never permanently stops reaching P.

**Next** ``◯`` (ASCII: ``()`` or keyword: ``next``).
``◯ P`` means P is true in the *next* state (one transition step ahead).

Quick Reference
^^^^^^^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Meaning
     - Unicode
     - ASCII
     - Keyword
   * - and
     - ``∧``
     - ``/\``
     - ``and``
   * - or
     - ``∨``
     - ``\/``
     - ``or``
   * - not
     - ``¬``
     - ``~``
     - ``not``
   * - implies
     - ``→``
     - ``->``
     -
   * - always
     - ``□``
     - ``[]``
     - ``always``
   * - eventually
     - ``◇``
     - ``<>``
     - ``eventually``
   * - next
     - ``◯``
     - ``()``
     - ``next``
   * - element of
     - ``∈``
     - ``:``
     -
   * - not equal
     - ``≠``
     - ``!=``
     -
   * - less or equal
     - ``≤``
     - ``<=``
     -
   * - greater or equal
     - ``≥``
     - ``>=``
     -

You can freely mix Unicode and ASCII in the same file. The formatter
(``caelum fmt``) can convert between styles.

Step 1: A Single Traffic Light
------------------------------

We are going to specify a crossroad traffic light system with two traffic
lights: ``traf1`` and ``traf2``. The full specification is available at
`examples/crossroad_traffic_light.lum <https://github.com/dhilst/caelum/blob/master/examples/crossroad_traffic_light.lum>`_.

Let's start simple with just one light that cycles through red, green, and
yellow. Declare a named type so the colours are self-documenting:

.. code-block:: text

   type Color = enum { red, green, yellow }

   let light ∈ Color
   let timer ∈ 0..5

   init {
     light = red ∧ timer = 5
   }

``type Color = enum { ... }`` declares a named set of values.
Variables declared with ``let light ∈ Color`` (read: "light is in Color")
can hold any value from that set.
The timer is an integer in the range 0 to 5 (inclusive).

The ``init`` block says the system starts with ``light = red`` **and**
``timer = 5``. The ``∧`` is just "and" — you could also write
``light = red and timer = 5``.

Step 2: Adding Transitions
--------------------------

Transitions describe how the system evolves. Each transition has a guard
(when it can fire) and an effect (what changes). Primed variables like
``light'`` refer to the value in the *next* state:

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

Read ``timer > 0 ∧ timer' = timer - 1`` as: "if timer is greater than 0,
then in the next state timer equals timer minus 1." The ``∧`` chains
multiple conditions together (just like ``&&`` in JavaScript).

The ``tick`` transition counts the timer down while keeping the light colour.
The other transitions fire when the timer expires, changing colour and
resetting the timer. Yellow gets a shorter timer (2 instead of 5).

Step 3: Safety Properties
-------------------------

Safety properties assert that something bad *never* happens.
The ``□`` ("always") operator means "in every reachable state":

.. code-block:: text

   property timer_bounded {
     □ (timer ≥ 0 ∧ timer ≤ 5)
   }

Read this as: "it is **always** the case that timer is between 0 and 5."
You could write it with ASCII as ``[] (timer >= 0 /\ timer <= 5)``
or with keywords as ``always (timer >= 0 and timer <= 5)``.

The implication operator ``→`` ("if ... then") is useful for conditional
safety properties:

.. code-block:: text

   property yellow_is_short {
     □ (light = yellow → timer ≤ 2)
   }

Read this as: "it is always the case that **if** the light is yellow,
**then** the timer is at most 2." This ensures yellow phases are always short.

Step 4: Liveness Properties
---------------------------

Liveness properties assert that something good *eventually* happens.
The ``□ ◇`` ("always eventually") pattern means "this keeps happening
forever":

.. code-block:: text

   property always_cycles {
     □ ◇ (light = red)
   }

Read this as: "it is **always** the case that the light **eventually**
becomes red." In other words, the light never gets permanently stuck —
no matter what state you're in, red will come around again.

Step 5: Two Lights at an Intersection
--------------------------------------

Now the payoff of named types: declare two variables with the same ``Color``
type. Both ``traf1`` and ``traf2`` share the same enum variants (``red``,
``green``, ``yellow``):

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

Read this as: "it is **always** the case that it is **not** true that both
traf1 and traf2 are green." Or more naturally: "both lights are never green
at the same time."

.. code-block:: text

   property one_moving_at_most {
     □ (traf1 ≠ red → traf2 = red)
   }

``one_moving_at_most`` is stronger: "if traf1 is **not equal** to red,
**then** traf2 must be red." In other words, at most one light can be
non-red at any time.

Step 7: Fairness Properties
----------------------------

Fairness ensures both directions get a turn. The ``→`` combined with ``◇``
lets us say "if this happens, then that will eventually happen":

.. code-block:: text

   property traf1_green_implies_traf2_next {
     □ (traf1 = green → ◇ (traf2 = green))
   }

   property traf2_green_implies_traf1_next {
     □ (traf2 = green → ◇ (traf1 = green))
   }

Read the first one as: "it is **always** the case that **if** traf1 is green,
**then eventually** traf2 will be green too." This guarantees neither
direction is starved.

Step 8: Invalid Properties
--------------------------

The ``invalid`` keyword marks properties that should *not* hold. Caelum
reports PASS when the property fails (as expected) and FAIL when it
unexpectedly holds:

.. code-block:: text

   invalid both_green {
     ◇ (traf1 = green ∧ traf2 = green)
   }

This claims "**eventually** both lights are green simultaneously." Since the
system prevents that, the property fails — and Caelum reports PASS.

Running the Full Specification
------------------------------

Here is the complete specification. Press **Check ▶** (or ``Ctrl-Enter``) to run
it in your browser — every safety and liveness property should pass, and each
``invalid`` block should fail as expected (which Caelum reports as a pass):

.. code-block:: lum

   module examples.crossroad_traffic_light

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

   property one_moving_at_most {
     □ (traf1 ≠ red → traf2 = red)
   }

   property traf1_eventually_green {
     □ ◇ (traf1 = green)
   }

   property traf2_eventually_green {
     □ ◇ (traf2 = green)
   }

   invalid both_green {
     ◇ (traf1 = green ∧ traf2 = green)
   }

You can also run it from the command line. Save the specification as
``crossroad.lum`` and run:

.. code-block:: bash

   caelum crossroad.lum

The full specification, with every safety, liveness, and fairness property, is
available at ``examples/simple/crossroad_traffic_light.lum`` in the repository.

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
