// Checks the buffer with caelum-wasm and renders diagnostics as inline squiggles
// + gutter markers. Checking is **manual** — there is no auto-linter watching
// document changes; `runCaelumCheck` is called explicitly (Check button /
// Ctrl-Enter) and installs the diagnostics via `setDiagnostics`. Existing
// diagnostics stay put while you type, until the next manual check.

import { setDiagnostics } from "@codemirror/lint";

// Map a wasm diagnostic (1-based line/col, per caelum-kernel spans) to an
// absolute CodeMirror position. We deliberately use line/col rather than the raw
// byte `byte_start`/`byte_end`, because the wasm spans are UTF-8 byte offsets
// while CodeMirror positions are UTF-16 code units — they diverge as soon as a
// spec uses Unicode operators (∀, □, ∧, …). Column counts Unicode scalar values,
// which equals UTF-16 units for every glyph Caelum uses in the BMP.
function posOf(doc, line, col) {
  const clampedLine = Math.max(1, Math.min(line, doc.lines));
  const l = doc.line(clampedLine);
  return Math.max(l.from, Math.min(l.from + (col - 1), l.to));
}

// A diagnostic is located when the kernel attached a span (start_line present).
function hasSpan(d) {
  return typeof d.start_line === "number";
}

function toCmDiagnostic(doc, d) {
  let from;
  let to;
  if (hasSpan(d)) {
    from = posOf(doc, d.start_line, d.start_col);
    to = posOf(doc, d.end_line, d.end_col);
    if (to <= from) to = Math.min(from + 1, doc.length);
  } else {
    // Span-less diagnostic: anchor to the whole first line.
    from = 0;
    to = Math.min(doc.line(1).to, doc.length);
  }
  return {
    from,
    to,
    severity: d.severity === "warning" ? "warning" : "error",
    message: d.message,
  };
}

// Run the checker on the current document and install its diagnostics.
// `opts.wasm` exposes `check(source)` returning the already-parsed report object
// (`{ status, properties, diagnostics }` or `{ error, diagnostics }`).
// `opts.onResult(report)` (optional) is called with the report after every run,
// so a results pane can react. Returns the report.
export function runCaelumCheck(view, opts) {
  const wasm = opts.wasm;
  const onResult = opts.onResult;
  const doc = view.state.doc;

  let report;
  let cmDiags;
  try {
    report = wasm.check(doc.toString());
    const diags = Array.isArray(report.diagnostics) ? report.diagnostics : [];
    cmDiags = diags.map((d) => toCmDiagnostic(doc, d));
  } catch (err) {
    // A panic in the checker should surface, not silently pass.
    const message = "internal checker error: " + (err && err.message ? err.message : String(err));
    report = { status: "fail", properties: [], diagnostics: [], error: message };
    cmDiags = [{ from: 0, to: Math.min(doc.line(1).to, doc.length), severity: "error", message }];
  }
  view.dispatch(setDiagnostics(view.state, cmDiags));
  if (onResult) onResult(report);
  return report;
}
