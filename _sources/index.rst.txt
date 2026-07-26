Caelum
======

Caelum is an **LTL model checker**. You describe a system as a finite state
machine — some variables and the rules for how they change — and you write down
the properties it must always obey. Caelum then explores *every* state the system
can reach and checks each property against all of them. When a property can be
broken, Caelum hands you a **counterexample**: a concrete step-by-step trace
showing exactly how it goes wrong.

Think of it as exhaustive testing for the *logic* of a system. Instead of the
handful of scenarios you would think to write as unit tests, Caelum tries them
all.

.. code-block:: lum

   let x ∈ 0..3

   init { x = 0 }

   transition step { x' = (x + 1) mod 4 }

   property in_range     { □ (x ≥ 0 ∧ x ≤ 3) }
   property returns_zero { □ ◇ (x = 0) }

That block above is a live editor. Press **Check ▶** (or ``Ctrl-Enter``) to run
the checker right here in your browser — no install required.

What Caelum is
--------------

- A checker for **finite-state** systems: variables over bounded ranges, enums,
  and booleans, evolving through transitions.
- A checker of **temporal properties** written in Linear Temporal Logic (LTL) —
  statements about how a system behaves *over time*, not just in one state.
- **Exhaustive**: it reasons about every reachable state and every possible
  ordering of events, so the bug that only shows up under one unlucky interleaving
  has nowhere to hide.
- A tool that **explains failures**: every failed property comes with a
  counterexample trace you can read.

What Caelum is not
------------------

- **Not a general theorem prover.** It reasons about a specific finite model, not
  arbitrary mathematics.
- **Not for unbounded or large data.** The state space must be finite and small
  enough to explore. It is a poor fit for heavy arithmetic, floating point, or big
  data structures.
- **Not sampling-based testing.** It does not run your real program on example
  inputs; it checks a *model* of it, exhaustively.
- **Not a programming language.** A ``.lum`` file describes behaviour and
  requirements; it does not compute results.

Features
--------

- **LTL model checking** with always, eventually, next, and until operators
- **Counterexample generation** showing exactly how a property fails
- **Three syntax modes**: keyword (``always``), ASCII (``[]``), and Unicode (``□``)
- **Runs in your browser** via WebAssembly — every example in these docs is a
  live, editable, verifiable editor
- **Modular specifications** with imports
- **JSON output** for integration with other tools
- **Deterministic formatter** for consistent code style

Where to start
--------------

- Never model-checked anything before? Read :doc:`when-to-use` to learn what kind
  of problem this is good for.
- Ready to try it? :doc:`using-the-editor` explains the in-browser editor, then
  the :doc:`tutorial` builds a real specification from scratch.
- You can do the entire tutorial in your browser — installing the command-line
  tool (:doc:`cli-reference`) is only needed to run specs offline or in CI.

.. toctree::
   :maxdepth: 2
   :caption: Learn Caelum

   when-to-use
   using-the-editor
   tutorial

.. toctree::
   :maxdepth: 2
   :caption: Reference

   language-guide
   examples
   cli-reference
   playground
   specification
