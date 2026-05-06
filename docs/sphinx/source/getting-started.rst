Getting Started
===============

Prerequisites
-------------

- `Rust toolchain <https://rustup.rs/>`_ (edition 2021 or later)

Installation
------------

.. code-block:: bash

   git clone https://github.com/dhilst/tlpengine.git
   cd tlpengine
   cargo build --release

The binary is at ``target/release/caelum``.

Your First Specification
------------------------

Create a file called ``counter.lum``:

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

Run it:

.. code-block:: bash

   caelum counter.lum

Both properties should pass. The counter wraps around from ``max`` back to 0,
so it always returns to zero and always stays in range.

Understanding Output
--------------------

On success, Caelum prints a summary and exits with code 0:

.. code-block:: text

   checking counter.lum
     in_range ........... PASS
     returns_to_zero .... PASS

On failure, Caelum shows which property failed and exits with code 1. Use
``--show-trace`` to see the counterexample trace.

Next Steps
----------

- Read the :doc:`language-guide` for the full syntax
- See more :doc:`examples`
- Check the :doc:`cli-reference` for all available options
