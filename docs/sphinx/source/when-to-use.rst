What Caelum Is For
==================

Caelum is a **behavioral specification tool**. Before writing any specification,
it helps to understand what that means, where it fits among the other things
people call a "spec," and when reaching for it pays off. This page assumes you can
program but have never used formal logic or model checking. No syntax to memorise
yet; that comes in the :doc:`tutorial`.

Two layers of specification
---------------------------

It is worth separating two very different things that both get called a
"specification":

- **Abstract specifications** say *what* a system must do — independent of how it is
  built. Behavioral specs, functional (input/output) contracts, and data-model
  invariants all live here.
- **Technical specifications** say *how* it is built — the stack and languages, the
  operating system and platform, the architecture and module structure, the wire
  protocols, and the **quantitative / real-time constraints** (a request answered
  *within 200 ms*, five-nines availability).

These are **layers, not bins**: a single requirement usually has a facet in each.
*"A password is never stored in plaintext"* is an abstract safety property;
*"hash it with bcrypt"* is the technical realisation of that same rule. The
abstract layer is the contract; the technical layer is how you meet it. Neither
substitutes for the other.

"Abstract" here means *implementation-independent*, **not vague**. A behavioral
specification is often the most precise, and the only machine-checkable, artifact
in the whole project.

**Caelum occupies the behavioral corner of the abstract layer.** It says nothing
about your stack, your deadlines, or your architecture — only about how your system
is allowed to *behave over time*.

What a behavioral specification captures
----------------------------------------

A behavioral spec has two ingredients.

**A model of the system as a state transition system.** You pick a handful of
*variables* that capture the system's state (a mode, a counter, whose turn it is)
and you write *transitions* — the rules for how those variables may change from one
step to the next. Together they define every state the system can be in and every
way it can move between states. This is the same mental model as a state diagram
you might sketch on a whiteboard, written down precisely. It is exactly how you
model a **distributed system**: each node's state, and the steps by which the whole
system evolves.

**Global, temporal properties.** Once the machine is described, you ask questions
about how it behaves *across whole runs*, not in a single snapshot. These come in
two flavours:

- **Safety** — *"something bad never happens."* A rule that must hold in every
  reachable state: two nodes are never leaders at once; a booked seat is never
  double-sold; the buffer never overflows.
- **Liveness** — *"something good eventually happens."* A promise that the system
  keeps making progress: a request is always eventually served; every node
  eventually gets a turn; the system never permanently freezes.

"Temporal" just means these statements talk about *sequences of states* — the
next step, the future, forever — not a single moment. That is what Linear Temporal
Logic (LTL) gives you, and it is what makes a model checker different from an
ordinary assertion. Behavioral specification is **particularly well-suited to
distributed systems**, where the critical safety and liveness properties are
precisely the ones that are hardest to get right by hand.

(Other formalisms sit alongside Caelum in the abstract layer — design-by-contract,
theorem provers such as Dafny, Coq, or Lean, refinement types, other model
checkers such as TLA⁺ and SPIN. Caelum's slice is explicit-state LTL model
checking.)

Why not just test?
------------------

To get the same confidence from testing a distributed system, you would have to
deploy it, drive it through *all possible orderings* of state transitions, and
analyse every resulting trace. The number of interleavings explodes
combinatorially, so this is infeasible for most real systems — the nastiest bugs
are exactly the one-in-a-thousand orderings you would never think to script.

Caelum turns that around. You specify the system as a state transition system,
write down the properties that must hold, and the model checker **verifies every
possible transition path** for you — no deployment required. If a path leads to a
state that violates a property, the check **fails and hands you a counterexample**:
a concrete, replayable trace showing exactly how it goes wrong. If no such path
exists, you have a proof that the property holds for the *whole model*, not just the
cases you happened to try.

Where it fits best
------------------

Behavioral specification earns its keep whenever correctness depends on *ordering,
concurrency, and reachable state* rather than on crunching data — most of all in
distributed and concurrent systems:

- **Mutual exclusion and concurrency** — locks, critical sections, race conditions.
- **Protocols and handshakes** — connection setup/teardown, retry logic, message
  ordering.
- **Distributed coordination** — leader election, consensus, replication invariants.
- **Controllers** — traffic lights, elevators, vending machines, any device driven
  by a state machine (the :doc:`tutorial` builds exactly one of these).
- **Lifecycle and workflow logic** — an order that must go
  ``created → paid → shipped`` and never skip a step or move backwards.

A single source of truth
------------------------

Once a behavioral specification is written and checked, it becomes a **single
source of truth** for how the system is allowed to evolve and which global and
temporal properties must hold. That has leverage beyond the check itself:

- It **guides the implementation** — engineers build against an unambiguous
  description of the required behaviour.
- It **guides the testing** — the properties tell you precisely *what* must be
  tested, and counterexamples become ready-made regression cases.
- Because it stays **abstract and compact**, it is easy to read, review, and keep
  in sync — and easy for AI agents to ingest and reason about, including deriving
  tests directly from the properties.

What it does not solve
----------------------

A compact, checked, abstract description of behaviour is powerful, but it is only
one layer of the lifecycle. It deliberately leaves several things open:

- **The implementation still has to follow the spec.** A behavioral spec states
  what must be true, not how to achieve it or whether a given implementation
  satisfies it. Someone — human or AI — must check that the code conforms; a good
  way is to derive tests from the properties that must hold.
- **You cannot mechanically derive the implementation from it.** It will not tell
  you which stack to use or which functions and methods to write — only which
  behaviour they must exhibit. Those choices are the *technical* layer.
- **Liveness is qualitative, not quantitative.** A behavioral spec says something
  *will* happen, never *when*. A request handled after five years still counts as
  "eventually handled" — obviously wrong by real-world expectations, yet perfectly
  consistent with the behavioral spec. Deadlines are quantitative / real-time
  constraints, which plain temporal logic cannot express; they belong to the
  technical layer (SLOs, load tests, timed models).
- **Caelum checks the model, not reality.** If the model misrepresents the real
  system, every property can pass and still tell you nothing true. The fidelity of
  the model-to-world mapping is on the author.

There are also hard limits on what Caelum can chew on: the state space must be
**finite** and small enough to enumerate (it stops at ``--max-states``, 100,000 by
default), and the hard part must be *logical*, not *numerical* — small bounded
integers, not heavy arithmetic or floating point. A good practice is to model-check
the tricky behavioural core and leave the data-heavy and performance-critical parts
to types, unit tests, and profiling. The layers complement each other.

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
for now: you described a tiny state transition system and a global property, and
Caelum checked that property against *every* run of the machine.

Next, learn how the in-browser editor works in :doc:`using-the-editor`, then build
a real specification in the :doc:`tutorial`.
