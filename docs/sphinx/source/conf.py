from pygments.lexer import RegexLexer, words
from pygments.token import Comment, Keyword, Name, Number, Operator, Punctuation, String, Text

project = "Caelum"
copyright = "2025, Daniel Hilst"
author = "Daniel Hilst"

extensions = [
    "myst_parser",
]

templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

html_theme = "furo"
html_static_path = ["_static"]
html_title = "Caelum"
# Loaded as an ES module so it can `import` the wasm + editor bundles. The
# interactive editors are wired up by _static/caelum-init.js, which loads the
# caelum-wasm ESM (caelum_wasm.js + caelum_wasm_bg.wasm) and the bundled
# CodeMirror editor (caelum-editor.js) — all three are copied into _static/ by
# docs/sphinx/build.sh (locally) or the CI `docs` job. If those assets are
# absent the pages still render with static Pygments highlighting.
html_js_files = [("caelum-init.js", {"type": "module"})]

source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}


# --- A minimal Pygments lexer for `.lum` code blocks ------------------------
# This only provides the static (pre-JS / no-JS) highlighting fallback; once the
# page's JavaScript runs, each `.. code-block:: lum` is upgraded into a live
# CodeMirror editor by caelum-init.js.
class LumLexer(RegexLexer):
    name = "Caelum"
    aliases = ["lum", "caelum"]
    filenames = ["*.lum"]

    tokens = {
        "root": [
            (r"//.*$", Comment.Single),
            (r"/\*", Comment.Multiline, "block"),
            (words((
                "module", "import", "type", "const", "let", "init",
                "transition", "property", "invalid", "fairness", "weak",
                "strong", "unchanged", "except", "forall", "exists",
            ), suffix=r"\b"), Keyword),
            (words(("bool", "enum"), suffix=r"\b"), Keyword.Type),
            (words(("true", "false"), suffix=r"\b"), Name.Constant),
            (words((
                "and", "or", "not", "always", "eventually", "next", "until",
                "mod",
            ), suffix=r"\b"), Operator.Word),
            (r"[0-9]+", Number),
            (r'"(\\.|[^"\\])*"', String),
            (r"∀|∃|¬|∧|∨|→|↔|□|◇|◯|𝒰|≠|≤|≥|∈|\.\.|<->|->|<>|\[\]|\(\)|"
             r"/\\|\\/|!=|<=|>=|[+\-*/=<>~]", Operator),
            (r"[A-Za-z_][A-Za-z0-9_]*'?", Name),
            (r"[(){}\[\],;:.]", Punctuation),
            (r"\s+", Text),
            (r".", Text),
        ],
        "block": [
            (r"[^*]+", Comment.Multiline),
            (r"\*/", Comment.Multiline, "#pop"),
            (r"\*", Comment.Multiline),
        ],
    }


def setup(app):
    app.add_lexer("lum", LumLexer)
    return {"parallel_read_safe": True, "parallel_write_safe": True}
