CLI Reference
=============

Usage
-----

.. code-block:: text

   caelum [OPTIONS] [SPEC]
   caelum <COMMAND> [OPTIONS] <SPEC>

When no subcommand is given, ``caelum <spec>.lum`` is equivalent to ``caelum check <spec>.lum``.

Commands
--------

check
^^^^^

Parse, validate, build the transition system, and check all properties.

.. code-block:: bash

   caelum check spec.lum
   caelum check --format json spec.lum
   caelum check --show-trace spec.lum

parse
^^^^^

Parse the specification and report any syntax errors. Does not build the model or check properties.

.. code-block:: bash

   caelum parse spec.lum
   caelum parse --format json spec.lum

fmt
^^^

Parse and print the specification in normalized form.

.. code-block:: bash

   caelum fmt spec.lum
   caelum fmt --print-keywords spec.lum
   caelum fmt --print-ascii-operators spec.lum
   caelum fmt --print-unicode-operators spec.lum

Formatting is idempotent: formatting an already-formatted file produces identical output.

Options
-------

.. list-table::
   :header-rows: 1

   * - Flag
     - Description
   * - ``--format human|json``
     - Output format (default: ``human``)
   * - ``--show-trace``
     - Display counterexample traces on failure
   * - ``--dump-graph``
     - Print the reachable transition graph
   * - ``--max-states N``
     - Limit state exploration (default: 100,000)
   * - ``--include-path DIR``
     - Additional import search directories
   * - ``--print-keywords``
     - Format with keyword operators (``always``, ``and``)
   * - ``--print-ascii-operators``
     - Format with ASCII operators (``[]``, ``/\``)
   * - ``--print-unicode-operators``
     - Format with Unicode operators (``□``, ``∧``) (default)

Exit Codes
----------

.. list-table::
   :header-rows: 1

   * - Code
     - Meaning
   * - 0
     - The specification parsed and all checks passed
   * - 1
     - One or more properties failed
   * - 2
     - Parse error
   * - 3
     - Semantic validation error
   * - 4
     - Import or file loading error
   * - 5
     - Model construction error
   * - 6
     - Internal error

If multiple error classes occur, the earliest pipeline stage determines the exit code.
