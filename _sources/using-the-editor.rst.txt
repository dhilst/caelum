Trying Caelum in Your Browser
=============================

You do not have to install anything to use Caelum. The whole model checker is
compiled to WebAssembly and runs inside this documentation. Every ``.lum`` example
you see is a **live editor**, and the entire :doc:`tutorial` can be completed
without leaving your browser.

Every example is editable and runnable
--------------------------------------

Wherever these docs show a ``.lum`` specification, that grey block is a real code
editor, not a static snippet. You can:

- **Edit it in place** — change a value, break a rule, add a property.
- **Run it** — press the **Check ▶** button or ``Ctrl-Enter`` (``Cmd-Enter`` on a
  Mac).

A block gets a **Check ▶** button when it contains at least one ``property`` (or
``invalid``) to verify. Smaller fragments that only show a type or a transition
are still editable and syntax-highlighted, but there is nothing to check, so they
have no button.

Try it now — press **Check ▶**:

.. code-block:: lum

   let x ∈ 0..2

   init { x = 0 }

   transition step { x' = (x + 1) mod 3 }

   property stays_small { □ (x ≤ 2) }

Everything runs locally
-----------------------

The checker is `caelum-kernel <https://github.com/dhilst/caelum>`_ compiled to
WebAssembly, using the pure-Rust *varisat* backend. Your specification is checked
**entirely in your browser** — it never leaves the page, there is no server, and
no account is required. Close the tab and it is gone.

Reading the results
-------------------

When you run a spec, a results pane appears beneath the editor.

**Passing.** Each property is listed with a pass marker. A pass means Caelum
explored every reachable state and found no way to violate the property.

**Failing — the counterexample trace.** When a property *can* be broken, Caelum
shows a **counterexample**: a table with one row per state, the variable values in
each, and the transition taken between rows. It is a concrete, replayable story of
how the property fails. For a liveness (``□ ◇``) property the trace ends in a
loop, and a ``⟲`` marker shows the state the loop returns to — the system cycles
there forever without the good thing ever happening.

Try this one. The property claims ``x`` is never ``2``, but the counter reaches
``2`` on the third step, so it fails — press **Check ▶** and read the trace:

.. code-block:: lum

   let x ∈ 0..2

   init { x = 0 }

   transition step { x' = (x + 1) mod 3 }

   property never_two { □ (x ≠ 2) }

**Errors — inline underlines.** If a spec does not parse or has a type error,
Caelum underlines the exact spot and explains the problem, the same way an editor
flags a syntax error. Try deleting the closing brace ``}`` from the property above
and running it again.

The ``invalid`` keyword
-----------------------

Sometimes you *want* to confirm that a property does **not** hold — for example, to
show a bad state is unreachable. Marking a block ``invalid`` flips the reporting:
Caelum reports a **pass** when the property fails as expected, and a **fail** if it
unexpectedly holds.

.. code-block:: lum

   let x ∈ 0..2

   init { x = 0 }

   transition step { x' = (x + 1) mod 3 }

   invalid never_reaches_two { □ (x ≠ 2) }

Here ``x`` *does* reach ``2``, so the property is genuinely false — which is
exactly what ``invalid`` asserts, so this reports a pass.

Where to go next
----------------

- The :doc:`tutorial` builds a complete traffic-light controller step by step,
  right in these editors.
- The :doc:`playground` is a full-page blank editor for your own experiments, with
  the ability to load and share specifications by URL.
