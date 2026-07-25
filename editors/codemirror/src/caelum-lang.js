// CodeMirror 6 language support for Caelum (.lum).
//
// A token-level `StreamLanguage` hand-ported from the PEG grammar at
// caelum-kernel/src/syntax/grammar.pest. It is intentionally NOT a full parser:
// it recognizes comments, keywords (with sub-classes), ASCII + Unicode
// operators, numbers, strings, and identifiers (including primed `x'`), which is
// ample for documentation snippets. Highlighting and model checking share one
// source of truth for *checking* (caelum-wasm); this file only styles.

import { StreamLanguage, LanguageSupport, HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

// --- Keyword tables (grammar.pest) ------------------------------------------

// Structural / declaration keywords.
const KW_STRUCT = new Set([
  "module", "import", "type", "const", "let", "init", "transition",
  "property", "invalid", "fairness", "weak", "strong", "unchanged", "except",
]);
// Built-in domain/type keywords.
const TYPE_BUILTIN = new Set(["bool", "enum"]);
// Boolean literals.
const CONST_BUILTIN = new Set(["true", "false"]);
// Quantifier keywords (ASCII spellings; Unicode ∀ ∃ handled below).
const KW_BINDER = new Set(["forall", "exists"]);
// Word-spelled operators. `mod` is arithmetic; the rest are logical/temporal
// and share the "logic" colour with their symbolic aliases.
const OP_WORD_LOGIC = new Set([
  "and", "or", "not", "always", "eventually", "next", "until", "U",
]);
const OP_WORD_ARITH = new Set(["mod"]);

// Keywords after which the next identifier names a *declaration*.
const DECL_DEF = new Set(["const", "let", "transition", "property", "invalid"]);
const DECL_TYPE = new Set(["type"]);
const DECL_MODULE = new Set(["module", "import"]);

// Single-code-point Unicode operator glyphs (all in the BMP). The astral `𝒰`
// (until) is a surrogate pair, so it is matched by string below, not here.
const UNI_BINDER = new Set(["∀", "∃"]);
const UNI_LOGIC = new Set(["¬", "∧", "∨", "→", "↔", "□", "◇", "◯"]);
const UNI_OP = new Set(["≠", "≤", "≥", "∈"]);

const IDENT_START = /[A-Za-z_]/;
const IDENT_CHAR = /[A-Za-z0-9_]/;

function startState() {
  // `expect` colours the next identifier: "def" (var/const/transition/property
  // name), "type" (type name), "module" (module/import target), or null.
  // `inBlockComment` tracks an open `/* … */` across lines.
  return { expect: null, inBlockComment: false };
}

function token(stream, state) {
  // Inside an open block comment: consume until `*/`.
  if (state.inBlockComment) {
    if (stream.match(/^.*?\*\//)) {
      state.inBlockComment = false;
    } else {
      stream.skipToEnd();
    }
    return "comment";
  }

  if (stream.eatSpace()) return null;

  const ch = stream.peek();

  // Comments: `// … EOL` and `/* … */` (possibly multi-line).
  if (stream.match("//")) {
    stream.skipToEnd();
    return "comment";
  }
  if (stream.match("/*")) {
    if (!stream.match(/^.*?\*\//)) {
      state.inBlockComment = true;
      stream.skipToEnd();
    }
    return "comment";
  }

  // Numbers.
  if (/[0-9]/.test(ch)) {
    stream.match(/^[0-9]+/);
    return "number";
  }

  // Strings (import paths): `"…"` with `\"` escapes.
  if (ch === '"') {
    stream.next();
    while (!stream.eol()) {
      const c = stream.next();
      if (c === "\\") { stream.next(); continue; }
      if (c === '"') break;
    }
    return "string";
  }

  // Identifiers / keywords, with an optional trailing prime (`x'`).
  if (IDENT_START.test(ch)) {
    let word = "";
    while (!stream.eol() && IDENT_CHAR.test(stream.peek())) word += stream.next();
    // A trailing `'` binds to the identifier (next-state reference).
    if (stream.peek() === "'") stream.next();

    const reserved =
      KW_STRUCT.has(word) || TYPE_BUILTIN.has(word) || CONST_BUILTIN.has(word) ||
      KW_BINDER.has(word) || OP_WORD_LOGIC.has(word) || OP_WORD_ARITH.has(word);

    // A pending declaration name wins over the generic identifier colour,
    // but reserved words are never treated as names.
    if (state.expect && !reserved) {
      const kind = state.expect;
      state.expect = null;
      return kind === "module" ? "module" : kind === "type" ? "typeDef" : "def";
    }

    if (KW_STRUCT.has(word)) {
      if (DECL_MODULE.has(word)) state.expect = "module";
      else if (DECL_TYPE.has(word)) state.expect = "type";
      else if (DECL_DEF.has(word)) state.expect = "def";
      else state.expect = null;
      return "keyword";
    }
    if (TYPE_BUILTIN.has(word)) return "typeName";
    if (CONST_BUILTIN.has(word)) return "bool";
    if (KW_BINDER.has(word)) return "binder";
    if (OP_WORD_LOGIC.has(word)) return "logic";
    if (OP_WORD_ARITH.has(word)) return "operator";

    state.expect = null;
    return "variable";
  }

  // Astral Unicode operator: 𝒰 (until) is a surrogate pair.
  if (stream.match("𝒰")) return "logic";

  // Single-code-point Unicode operators.
  if (UNI_BINDER.has(ch)) { stream.next(); return "binder"; }
  if (UNI_LOGIC.has(ch)) { stream.next(); return "logic"; }
  if (UNI_OP.has(ch)) { stream.next(); return "operator"; }

  // Multi-character ASCII operators (longest first).
  if (stream.match("<->")) return "logic";       // iff
  if (stream.match("->")) return "logic";        // implies
  if (stream.match("[]")) return "logic";        // always
  if (stream.match("<>")) return "logic";        // eventually
  if (stream.match("()")) return "logic";        // next
  if (stream.match("/\\")) return "logic";       // and
  if (stream.match("\\/")) return "logic";       // or
  if (stream.match("!=")) return "operator";
  if (stream.match("<=")) return "operator";
  if (stream.match(">=")) return "operator";
  if (stream.match("..")) return "operator";     // range

  // Single-character operators and punctuation.
  if (ch === "~") { stream.next(); return "logic"; }             // not
  if ("+-*/=<>".includes(ch)) { stream.next(); return "operator"; }
  if ("(){}[],;:.".includes(ch)) { stream.next(); return "punctuation"; }

  // Anything else: consume one char so we never stall.
  stream.next();
  return null;
}

// Map our token names to @lezer/highlight tags.
const tokenTable = {
  comment: t.lineComment,
  keyword: t.keyword,
  typeName: t.typeName,
  bool: t.bool,
  number: t.number,
  string: t.string,
  binder: t.operatorKeyword,
  logic: t.logicOperator,
  operator: t.operator,
  def: t.definition(t.variableName),
  typeDef: t.definition(t.typeName),
  module: t.namespace,
  variable: t.variableName,
  punctuation: t.punctuation,
};

export const caelumStreamLanguage = StreamLanguage.define({
  name: "caelum",
  startState,
  token,
  tokenTable,
  languageData: {
    commentTokens: { line: "//", block: { open: "/*", close: "*/" } },
  },
});

// A theme-agnostic highlight style. Colours are chosen to read on both light
// and dark backgrounds; the editor theme sets the surrounding surface.
export const caelumHighlightStyle = HighlightStyle.define([
  { tag: t.lineComment, color: "#6a737d", fontStyle: "italic" },
  { tag: t.keyword, color: "#a626a4" },
  { tag: t.typeName, color: "#c18401" },
  { tag: t.bool, color: "#986801" },
  { tag: t.number, color: "#986801" },
  { tag: t.string, color: "#50a14f" },
  { tag: t.operatorKeyword, color: "#0184bc", fontWeight: "bold" },
  { tag: t.logicOperator, color: "#0184bc", fontWeight: "bold" },
  { tag: t.operator, color: "#4078f2" },
  { tag: t.definition(t.variableName), color: "#4078f2", fontWeight: "bold" },
  { tag: t.definition(t.typeName), color: "#c18401", fontWeight: "bold" },
  { tag: t.namespace, color: "#50a14f" },
  { tag: t.variableName, color: "#383a42" },
  { tag: t.punctuation, color: "#696c77" },
]);

// The full language extension: grammar + highlight style.
export function caelum() {
  return new LanguageSupport(caelumStreamLanguage, [
    syntaxHighlighting(caelumHighlightStyle),
  ]);
}
