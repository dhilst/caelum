# Caelum

LTL model checker.

## Build & Test

- Cargo lives at `~/.cargo/bin/cargo`; if not in PATH, run `export PATH="$HOME/.cargo/bin:$PATH"` first.
- `cargo build` — build the project
- `cargo test` — run all unit tests
- `cargo run -- <spec.lum>` — check a spec file

## Harness

- CI harness binary at `ci/src/main.rs` — run with `cargo run --manifest-path ci/Cargo.toml`
- Runs `cargo test` and all `examples/**/*.lum` files in parallel (threadpool sized to CPU count)
- Each process has a 60s timeout; exits 0 only if all tests and examples pass
- Pre-push hook (`git push`) invokes the harness automatically
- GitHub Actions CI runs the harness on pull requests

## Examples layout

- `examples/simple/` — small standalone specs
- `examples/game_of_life/` — Game of Life grid specs
- `examples/refinement/` — iterative refinement rounds

## Documentation

- Sphinx docs in `docs/sphinx/` use RST format (not Markdown)
- Build: `cd docs/sphinx && uv sync --group docs && uv run sphinx-build -b html source build/html`
