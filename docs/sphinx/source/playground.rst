Playground
==========

A full-page editor for experimenting with Caelum. Unlike the inline editors
scattered through these docs (see :doc:`using-the-editor`), this is a blank
scratchpad with room to work and the ability to load and share specifications by
URL. It is seeded with a small modular counter. Edit anything and press
**Check ▶** (or ``Ctrl-Enter``) to re-run the model checker.

.. note::

   The checker is `caelum-kernel <https://github.com/dhilst/caelum>`_ compiled to
   WebAssembly, using the pure-Rust *varisat* backend. Your spec never leaves the
   page — everything runs locally in your browser.

.. raw:: html

   <style>
   .caelum-load-bar { display: flex; gap: .5rem; margin: 1rem 0 .3rem; }
   .caelum-load-bar input { flex: 1; font: inherit; font-size: 13px; padding: .3rem .5rem;
     border: 1px solid rgba(128,128,128,0.4); border-radius: 5px; }
   .caelum-load-bar button { font: inherit; font-size: 13px; cursor: pointer; padding: .3rem .8rem;
     border-radius: 5px; border: 1px solid rgba(128,128,128,0.4); background: #4078f2; color: #fff; }
   </style>
   <div class="caelum-load-bar">
     <input id="caelum-load-url" type="text"
            placeholder="Load a .lum from a URL — a staged example or a raw.githubusercontent.com link" />
     <button id="caelum-load-btn" type="button">Load</button>
   </div>
   <div id="caelum-playground" data-seed-url="_static/examples/counter.lum"></div>

.. note::

   **Load and share specs by URL.** Paste any URL into the box above and press
   **Load** to open that ``.lum`` file — a staged example such as
   ``_static/examples/traffic_light.lum``, or an absolute link like a raw GitHub
   file. The address updates to ``playground.html?q=<url>``, so *that link is
   itself shareable*: send it to anyone and the playground loads the spec
   automatically. (Cross-origin hosts must allow it via CORS; raw GitHub does.)

.. tip::

   When a property fails, the results pane shows a **counterexample trace** — one
   row per state, with the transition taken between states and, for a liveness
   (``□ ◇``) property, a ``⟲`` marker on the state the infinite loop returns to.
   A parse or type error is underlined inline at the offending location.
