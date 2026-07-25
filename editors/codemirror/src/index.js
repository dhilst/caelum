// Public entry point for the Caelum CodeMirror editor bundle.
//
// `mountCaelumEditor(parent, opts)` builds an editor with Caelum syntax
// highlighting, live model checking (via an already-initialized caelum-wasm
// module), a "Check" button, a results pane, and a counterexample trace table.
// The page owns wasm loading and passes the module in as `opts.wasm`, so a
// single wasm instance can be shared across many editors on one page.

import { EditorState } from "@codemirror/state";
import {
  EditorView, keymap, lineNumbers, highlightActiveLine,
  highlightActiveLineGutter, drawSelection, highlightSpecialChars,
} from "@codemirror/view";
import { history, defaultKeymap, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { lintGutter, lintKeymap } from "@codemirror/lint";
import { caelum } from "./caelum-lang.js";
import { runCaelumCheck } from "./lint.js";

export { caelum, caelumStreamLanguage, caelumHighlightStyle } from "./caelum-lang.js";
export { runCaelumCheck } from "./lint.js";

// A compact, light editor surface. The highlight palette in caelum-lang.js is
// tuned for a light background; we keep the editor light even on dark pages so
// tokens stay legible, and give it a card border to sit inside prose.
const baseTheme = EditorView.theme({
  "&": {
    fontSize: "14px",
    border: "1px solid rgba(128,128,128,0.35)",
    borderRadius: "6px",
    backgroundColor: "#fafafa",
    color: "#383a42",
  },
  ".cm-content": {
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
    caretColor: "#383a42",
  },
  ".cm-gutters": {
    backgroundColor: "#f0f0f0",
    color: "#a0a1a7",
    border: "none",
  },
  ".cm-activeLine": { backgroundColor: "rgba(0,0,0,0.03)" },
  ".cm-activeLineGutter": { backgroundColor: "rgba(0,0,0,0.05)" },
  "&.cm-focused": { outline: "2px solid rgba(64,120,242,0.4)" },
});

// Minimal chrome for the toolbar / results pane / trace table, injected once.
// The editor surface itself is themed via CodeMirror's `baseTheme` above.
const CHROME_CSS = `
.caelum-editor { margin: 1rem 0; }
.caelum-toolbar { display: flex; gap: .5rem; margin-top: .4rem; }
.caelum-btn {
  font: inherit; font-size: 13px; cursor: pointer;
  padding: .25rem .7rem; border-radius: 5px;
  border: 1px solid rgba(128,128,128,0.4);
  background: #4078f2; color: #fff;
}
.caelum-btn:hover { background: #345fd0; }
.caelum-btn:active { transform: translateY(1px); }
.caelum-result {
  margin-top: .4rem; padding: .35rem .6rem; border-radius: 5px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 13px; white-space: pre-wrap; min-height: 1.2em;
}
.caelum-result:empty { display: none; }
.caelum-result-ok { background: rgba(80,161,79,0.15); color: #2e7d32; }
.caelum-result-error { background: rgba(228,86,73,0.12); color: #c62828; }
.caelum-result-line { margin-top: .15rem; opacity: .85; }
.caelum-prop { margin-top: .2rem; }
.caelum-prop-badge {
  font-size: 11px; opacity: .7; margin-left: .35rem;
  border: 1px solid rgba(128,128,128,0.4); border-radius: 4px; padding: 0 .3rem;
}
.caelum-trace {
  border-collapse: collapse; margin: .35rem 0 .2rem; font-size: 12px;
  color: #383a42; background: #fff; display: block; overflow-x: auto;
}
.caelum-trace th, .caelum-trace td {
  border: 1px solid rgba(128,128,128,0.3); padding: .15rem .45rem; text-align: left;
}
.caelum-trace thead th { background: rgba(128,128,128,0.12); font-weight: 600; }
.caelum-trace td.caelum-num { text-align: right; font-variant-numeric: tabular-nums; }
.caelum-loop-start td { border-top: 2px solid #4078f2; }
.caelum-trace-loop { font-size: 12px; color: #4078f2; margin-bottom: .3rem; }
`;

let stylesInjected = false;
function injectStyles() {
  if (stylesInjected || typeof document === "undefined") return;
  const style = document.createElement("style");
  style.setAttribute("data-caelum-editor", "");
  style.textContent = CHROME_CSS;
  document.head.appendChild(style);
  stylesInjected = true;
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

// Render one counterexample as a trace table: rows are states, columns are the
// ordered union of variable names across all states, with transition labels and
// a lasso marker on the loop-back state. Cells are `{ type, value }` objects.
function renderTrace(pane, ce) {
  const states = ce.states || [];
  if (states.length === 0) return;

  // Ordered union of variable keys across every state (indexed vars like
  // `status[n1]` each get their own column).
  const cols = [];
  const seen = new Set();
  for (const state of states) {
    for (const key of Object.keys(state)) {
      if (!seen.has(key)) { seen.add(key); cols.push(key); }
    }
  }

  const table = el("table", "caelum-trace");
  const thead = el("thead");
  const headRow = el("tr");
  headRow.appendChild(el("th", null, "#"));
  headRow.appendChild(el("th", null, "transition"));
  for (const col of cols) headRow.appendChild(el("th", null, col));
  thead.appendChild(headRow);
  table.appendChild(thead);

  const transitions = ce.transitions || [];
  const tbody = el("tbody");
  for (let i = 0; i < states.length; i++) {
    const row = el("tr", i === ce.cycle_start ? "caelum-loop-start" : null);
    row.appendChild(el("td", null, i === ce.cycle_start ? i + " ⟲" : String(i)));
    const tName = transitions[i] != null ? transitions[i] : (i === 0 ? "init" : "");
    row.appendChild(el("td", null, tName));
    for (const col of cols) {
      const cell = states[i][col];
      const isInt = cell && cell.type === "Int";
      const td = el("td", isInt ? "caelum-num" : null, cell != null ? String(cell.value) : "·");
      row.appendChild(td);
    }
    tbody.appendChild(row);
  }
  table.appendChild(tbody);
  pane.appendChild(table);

  if (typeof ce.cycle_start === "number") {
    pane.appendChild(el("div", "caelum-trace-loop", "↺ loops back to state " + ce.cycle_start));
  }
}

function plural(n, singular, pluralForm) {
  return n === 1 ? singular : pluralForm;
}

function statusGlyph(status) {
  if (status === "pass" || status === "certified") return "✓";
  if (status === "fail") return "✗";
  return "•"; // skipped / other
}

function renderResult(pane, report) {
  pane.textContent = "";
  pane.className = "caelum-result";
  if (!report) return;

  // Top-level error (parse/semantic/model failure).
  if (report.error) {
    pane.classList.add("caelum-result-error");
    pane.appendChild(el("div", null, "✗ " + report.error));
    for (const d of report.diagnostics || []) {
      if (typeof d.start_line === "number") {
        pane.appendChild(el("div", "caelum-result-line", d.start_line + ":" + d.start_col + "  " + d.message));
      }
    }
    return;
  }

  const props = report.properties || [];
  const failed = props.filter((p) => p.status === "fail");
  if (report.status === "pass") {
    pane.classList.add("caelum-result-ok");
    pane.appendChild(el("div", null, "✓ all " + props.length + " " + plural(props.length, "property", "properties") + " hold"));
  } else {
    pane.classList.add("caelum-result-error");
    pane.appendChild(el("div", null, "✗ " + failed.length + " of " + props.length + " " + plural(props.length, "property", "properties") + " failed"));
  }

  for (const p of props) {
    const row = el("div", "caelum-prop", statusGlyph(p.status) + " " + p.name);
    if (p.kind && p.kind !== "property") {
      row.appendChild(el("span", "caelum-prop-badge", p.kind));
    }
    row.appendChild(el("span", "caelum-prop-badge", p.status));
    pane.appendChild(row);
    if (p.note) pane.appendChild(el("div", "caelum-result-line", p.note));
    if (p.counterexample) renderTrace(pane, p.counterexample);
  }
}

// Build and mount an editor. Options:
//   doc         initial source (string)
//   wasm        initialized caelum-wasm wrapper ({ check(source) })
//   readOnly    if true, the editor is not editable
//   showToolbar if false, omit the Check button (default true)
// Returns { view, container, check() }.
export function mountCaelumEditor(parent, opts = {}) {
  injectStyles();
  const wasm = opts.wasm;

  const container = el("div", "caelum-editor");
  const editorHost = el("div", "caelum-editor-host");
  const pane = el("div", "caelum-result");
  container.appendChild(editorHost);

  // Manual check: run the checker on demand and install its diagnostics. There
  // is *no* auto-linter watching edits, so a reported error stays visible while
  // you type, until the next check.
  const doCheck = (v) => {
    if (wasm) {
      runCaelumCheck(v, { wasm, onResult: (r) => renderResult(pane, r) });
    }
  };

  const extensions = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    highlightActiveLine(),
    bracketMatching(),
    // Ctrl-Enter / Cmd-Enter runs the checker (same as the Check button).
    keymap.of([
      { key: "Mod-Enter", run: (v) => { doCheck(v); return true; } },
    ]),
    keymap.of([...defaultKeymap, ...historyKeymap, ...lintKeymap, indentWithTab]),
    caelum(),
    baseTheme,
    EditorState.tabSize.of(2),
    EditorView.editable.of(!opts.readOnly),
  ];

  // Gutter markers for diagnostics, only when a wasm module is supplied;
  // highlighting-only doc blocks omit them.
  if (wasm) {
    extensions.push(lintGutter());
  }

  const view = new EditorView({
    state: EditorState.create({ doc: opts.doc || "", extensions }),
    parent: editorHost,
  });

  const check = () => doCheck(view);

  if (wasm && opts.showToolbar !== false) {
    const toolbar = el("div", "caelum-toolbar");
    const checkBtn = el("button", "caelum-btn", "Check ▶");
    checkBtn.type = "button";
    checkBtn.title = "Check the spec (Ctrl-Enter)";
    checkBtn.addEventListener("click", check);
    toolbar.appendChild(checkBtn);
    container.appendChild(toolbar);
    container.appendChild(pane);
  }

  parent.appendChild(container);
  // Check on load whenever a kernel is present, so results and any
  // counterexample are ready immediately.
  if (wasm) requestAnimationFrame(check);
  return { view, container, check };
}

// Convenience default for direct <script type="module"> use.
export default { mountCaelumEditor };
