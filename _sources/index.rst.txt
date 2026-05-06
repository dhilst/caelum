Caelum
======

An LTL model checker.

Caelum reads ``.lum`` specification files describing finite-state transition systems
and checks whether declared LTL properties hold over all reachable states.
When a property fails, Caelum produces a counterexample trace.

Features
--------

- **LTL model checking** with always, eventually, next, and until operators
- **Counterexample generation** showing exactly how a property fails
- **Three syntax modes**: keyword (``always``), ASCII (``[]``), and Unicode (``□``)
- **Modular specifications** with imports
- **JSON output** for integration with other tools
- **Deterministic formatter** for consistent code style

.. toctree::
   :maxdepth: 2

   getting-started
   tutorial
   language-guide
   cli-reference
   examples
   specification
