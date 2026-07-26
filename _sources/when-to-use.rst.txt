What Caelum Is For
==================

Before writing any specification, it helps to understand *what kind of problem*
Caelum solves — and when reaching for a model checker pays off. This page assumes
you can program but have never used formal logic or model checking. No syntax to
memorise yet; that comes in the :doc:`tutorial`.

The kind of specification
-------------------------

Caelum works with two ingredients.

**A model of a system as a state machine.** You pick a handful of *variables* that
capture the system's state (a mode, a counter, whose turn it is) and you write
*transitions* — the rules for how those variables can change from one step to the
next. Together they define every state the system can be in and every way it can
move between states. This is the same mental model as a state diagram you might
sketch on a whiteboard, written down precisely.

**Requirements as temporal properties.** Once the machine is described, you state
what must be true about it *over time*. These properties come in two flavours:

- **Safety** — *"something bad never happens."* A safety property is a rule that
  must hold in every reachable state: two processes are never in the critical
  section at once; a booked seat is never double-sold; the buffer never overflows.
- **Liveness** — *"something good eventually happens."* A liveness property is a
  promise that the system keeps making progress: a request is always eventually
  served; every process eventually gets a turn; the system never permanently
  freezes.

"Temporal" just means these statements talk about *sequences of states* — the
future, the next step, forever — not a single snapshot. That is what Linear
Temporal Logic (LTL) gives you, and it is what makes a model checker different
from an ordinary assertion.

Why not just write tests?
-------------------------

A unit test runs your system on *one* scenario and checks the outcome. That is
great for catching the bugs you anticipated — but the nastiest bugs in stateful,
concurrent, or event-driven systems come from the ordering you *didn't* think of:
the one interleaving out of thousands where two things happen in just the wrong
order.

Caelum is **exhaustive**. It explores *every* reachable state and *every* possible
ordering of transitions, then checks your properties against all of them. If a
counterexample exists — even a fifteen-step sequence you would never have written
by hand — it finds it and shows it to you. If none exists, you have a proof that
the property holds for the whole model, not just for the cases you tried.

Where it shines
---------------

Model checking is a natural fit whenever correctness depends on *ordering, timing
of events, or concurrency* rather than on crunching data:

- **Mutual exclusion and concurrency** — locks, critical sections, race conditions.
- **Protocols and handshakes** — connection setup/teardown, retry logic, message
  ordering.
- **Controllers** — traffic lights, elevators, vending machines, any device driven
  by a state machine (the :doc:`tutorial` builds exactly one of these).
- **Lifecycle and workflow logic** — an order that must go
  ``created → paid → shipped`` and never skip a step or move backwards.
- **Distributed coordination** — leader election, consensus, replication invariants.

Where it is the wrong tool
--------------------------

Caelum is not a universal verifier. Look elsewhere when:

- The state space is **not finite** or is simply **too large** — Caelum has to be
  able to enumerate reachable states, and it stops at ``--max-states`` (100,000 by
  default). Systems dominated by large arrays, unbounded collections, or wide
  integer arithmetic blow up quickly.
- The hard part is **numerical** — heavy arithmetic or floating point. Caelum
  handles small bounded integers, not real-number computation.
- You need **real-time performance guarantees** (deadlines in milliseconds) rather
  than logical ordering guarantees.

A good practice is to model checking the *tricky core* of a system — the state
machine where ordering matters — and leave the data-heavy or performance-critical
parts to types, unit tests, and profiling. The techniques complement each other.

See it work
-----------

Here is a two-line system: a light that flips between ``off`` and ``on``. The
property claims it *always eventually* turns on again. Press **Check ▶** (or
``Ctrl-Enter``) — it should pass.

.. code-block:: lum

   let light ∈ enum { off, on }

   init { light = off }

   transition turn_on  { light = off ∧ light' = on }
   transition turn_off { light = on ∧ light' = off }

   property keeps_blinking { □ ◇ (light = on) }

Do not worry about the notation yet — ``□ ◇`` reads as "always eventually," and
the :doc:`tutorial` introduces every symbol with a JavaScript analogy. The point
for now: you described a tiny machine and a requirement, and Caelum checked the
requirement against *every* run of the machine.

Next, learn how the in-browser editor works in :doc:`using-the-editor`, then build
a real specification in the :doc:`tutorial`.
