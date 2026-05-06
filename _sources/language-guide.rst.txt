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

Variables
---------

Variables are declared with a name and a finite domain:

.. code-block:: text

   let x ∈ 0..3                          // integer range
   let flag : bool                        // boolean
   let mode : enum { idle, busy, done }   // enumeration

The type separator can be ``:`` or ``∈``.

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
