#!/usr/bin/env bash
# Build the Caelum documentation site end to end: compile the kernel to wasm,
# bundle the CodeMirror editor, stage the runtime assets into source/_static/,
# and run Sphinx. Mirrors the CI `docs` job so the site can be built and
# previewed locally. Requires: cargo + wasm-pack, node + npm, and uv (which
# provides the Sphinx toolchain from pyproject.toml's `docs` group).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"   # docs/sphinx
ROOT="$(cd "$HERE/../.." && pwd)"       # repository root
STATIC="$HERE/source/_static"

echo "==> Building caelum-wasm (wasm32, --target web)"
wasm-pack build "$ROOT/caelum-wasm" --target web --release --out-dir pkg
cp "$ROOT/caelum-wasm/pkg/caelum_wasm.js" "$STATIC/"
cp "$ROOT/caelum-wasm/pkg/caelum_wasm_bg.wasm" "$STATIC/"

echo "==> Building the CodeMirror editor bundle"
( cd "$ROOT/editors/codemirror" && npm ci && npm run build )
cp "$ROOT/editors/codemirror/dist/caelum-editor.js" "$STATIC/"

echo "==> Staging playground example specs"
mkdir -p "$STATIC/examples"
cp "$ROOT"/examples/simple/*.lum "$STATIC/examples/"

echo "==> Running Sphinx (uv)"
cd "$HERE"
uv sync --group docs
uv run sphinx-build -b html source build/html
touch build/html/.nojekyll

echo "==> Done: $HERE/build/html/index.html"
