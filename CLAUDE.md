# Caelum

LTL model checker.

## Build & Test

- `cargo build` — build the project
- `cargo test` — run all unit tests
- `cargo run -- <spec.lum>` — check a spec file

## Harness

1. All tests must pass (`cargo test`)
2. All `.lum` files in `examples/` and `refinement/specs/` must pass (`cargo run -- <file>` exits 0)
3. Harness is executed before `git push` as a pre-push hook

## Documentation

- Sphinx docs in `docs/sphinx/` use RST format (not Markdown)
- Build: `cd docs/sphinx && uv sync --group docs && uv run sphinx-build -b html source build/html`
