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

Variables not mentioned in a transition retain their current value (frame condition).

Properties
----------

Declare temporal properties to check:

.. code-block:: text

   property in_range {
     □ (x >= 0 ∧ x <= max)
   }

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
