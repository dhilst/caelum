Language Guide
==============

Modules
-------

Every specification can optionally declare a module name:

.. code-block:: text

   module examples.counter

Imports
-------

Specifications can import other ``.lum`` files:

.. code-block:: text

   import "common.lum"

Constants
---------

Named integer constants:

.. code-block:: text

   const max = 3

Types
-----

The ``type`` keyword declares a named type that can be shared by multiple
variables:

.. code-block:: text

   type Color = enum { red, green, yellow }
   type Counter = 0..max

The right-hand side can be an ``enum { ... }``, an integer range, or ``bool``.
Named types must be declared before use (no forward references).
Type names share the global namespace with variables, constants, and enum
variants — duplicates are rejected.

When an enum type is declared with ``type``, its variants are registered once
and shared by all variables of that type:

.. code-block:: text

   type Color = enum { red, green, yellow }
   let a ∈ Color
   let b ∈ Color

   // Both variables can use the same variant names:
   init { a = red ∧ b = green }

   // Cross-variable comparison works because they share the same type:
   property same_color { □ (a = b → a = red) }

Variables
---------

Variables are declared with a name and a finite domain:

.. code-block:: text

   let x ∈ 0..3                          // integer range
   let flag : bool                        // boolean
   let mode : enum { idle, busy, done }   // inline enumeration
   let light ∈ Color                      // named type (see above)

The type separator can be ``:`` or ``∈``.
The domain can be an inline definition or a reference to a named type.

Init Blocks
-----------

Define the initial state:

.. code-block:: text

   init {
     x = 0 ∧ flag = false
   }

Multiple ``init`` blocks are conjoined.

Transitions
-----------

Define how the system evolves. Primed variables (``x'``) denote the next-state value:

.. code-block:: text

   transition step {
     x' = (x + 1) mod (max + 1)
   }

A next-state variable that a transition does **not** constrain is left free: the
transition may move to *any* value in that variable's domain. To hold a variable
fixed you must constrain its next-state value explicitly, e.g. ``y' = y``. The
``unchanged`` shorthand (below) makes this concise.

Frame conditions with ``unchanged``
-----------------------------------

``unchanged(...)`` expands to a conjunction of ``v' = v`` frame conditions:

.. code-block:: text

   transition step {
     x' = x + 1 ∧
     unchanged(y, z)          // ≡  y' = y ∧ z' = z
   }

Arguments must be declared state variables (not constants, enum values, or primed
names). Duplicates are ignored. For an indexed variable, ``unchanged(status)``
preserves *every* index; ``unchanged(status except node)`` preserves every index
other than ``node`` (see :ref:`indexed-state`).

Parameterized transitions
-------------------------

A transition may take parameters ranging over finite domains. It is expanded at
compile time into one concrete transition per tuple in the Cartesian product of
the parameter domains:

.. code-block:: text

   type Node = enum { n1, n2 }

   transition power_on(node ∈ Node) {
     status[node]' = on ∧ unchanged(status except node)
   }

Both ``∈`` and ``:`` separate a parameter from its domain. Parameters are
immutable — they have no next-state (primed) form — and each generated instance
is named after its arguments (``power_on(n1)``, ``power_on(n2)``), which is what
counterexample traces report.

.. _indexed-state:

Indexed state
-------------

A variable may be indexed by a finite domain, declaring one entry per index:

.. code-block:: text

   let status[node ∈ Node] ∈ Power

Reference an entry with ``status[node]`` and its next-state value with
``status[node]'``. Indexed variables are flattened into one scalar variable per
index (internally named ``status[n1]``, ``status[n2]``, …).

Quantifiers
-----------

``∀`` and ``∃`` range over finite domains and expand to a conjunction or
disjunction over the domain's elements:

.. code-block:: text

   init { ∀ node ∈ Node: status[node] = off }

   property some_on { □ (∃ node ∈ Node: status[node] = on) }

The keyword forms ``forall`` and ``exists`` are also accepted.

Properties
----------

Declare temporal properties to check:

.. code-block:: text

   property in_range {
     □ (x >= 0 ∧ x <= max)
   }

Fairness
--------

Liveness properties (``◇``, ``□◇``, ``until``) often only hold if the scheduler
does not neglect a transition forever. A ``fairness`` block declares such
assumptions:

.. code-block:: text

   fairness {
     weak   node_powers_on
     strong assign_image
   }

Each entry names a transition and a strength:

- **weak** (justice): a transition that is *continuously enabled* must
  eventually be taken.
- **strong** (compassion): a transition that is *enabled infinitely often* must
  eventually be taken.

A named transition applies the constraint to every instance a parameterized
transition expands into (``node_powers_on(n1)``, ``node_powers_on(n2)``, …), so
"weak ``node_powers_on``" means *every* node eventually powers on. Fairness
restricts only the infinite paths considered for liveness; it never affects
safety (``□``) properties. The explicit engine *proves* fair liveness; the BMC
engine *refutes* it (finds fair counterexamples up to the search depth).

Operators
---------

Caelum supports three equivalent syntaxes for every operator.

Temporal Operators
^^^^^^^^^^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Operator
     - Keyword
     - ASCII
     - Unicode
   * - always
     - ``always``
     - ``[]``
     - ``□``
   * - eventually
     - ``eventually``
     - ``<>``
     - ``◇``
   * - next
     - ``next``
     - ``()``
     - ``◯``
   * - until
     - ``until``
     - ``U``
     - ``𝒰``

Logical Operators
^^^^^^^^^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Operator
     - Keyword
     - ASCII
     - Unicode
   * - and
     - ``and``
     - ``/\``
     - ``∧``
   * - or
     - ``or``
     - ``\/``
     - ``∨``
   * - not
     - ``not``
     - ``~``
     - ``¬``
   * - implies
     -
     - ``->``
     - ``→``
   * - iff
     -
     - ``<->``
     - ``↔``

Comparison Operators
^^^^^^^^^^^^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Operator
     - ASCII
     - Unicode
   * - equal
     - ``=``
     -
   * - not equal
     - ``!=``
     - ``≠``
   * - less than
     - ``<``
     -
   * - less/equal
     - ``<=``
     -
   * - greater
     - ``>``
     -
   * - greater/eq
     - ``>=``
     -

Arithmetic Operators
^^^^^^^^^^^^^^^^^^^^

``+``, ``-``, ``*``, ``/``, ``mod``

Operator Precedence
^^^^^^^^^^^^^^^^^^^

From lowest to highest:

.. list-table::
   :header-rows: 1

   * - Level
     - Operators
     - Associativity
   * - 1
     - ``<->`` / ``↔``
     - left
   * - 2
     - ``->`` / ``→``
     - right
   * - 3
     - ``until`` / ``U`` / ``𝒰``
     - right
   * - 4
     - ``or`` / ``\/`` / ``∨``
     - left
   * - 5
     - ``and`` / ``/\`` / ``∧``
     - left
   * - 6
     - ``=``, ``!=`` / ``≠``, ``<``, ``<=``, ``>``, ``>=``
     - non-associative
   * - 7
     - ``+``, ``-``
     - left
   * - 8
     - ``*``, ``/``, ``mod``
     - left
   * - 9
     - ``not`` / ``¬``, ``-`` (neg), ``□``, ``◇``, ``◯``
     - right (prefix)
