# Caelum

LTL model checker.

## Workspace layout

Cargo workspace with three crates:

- `caelum-kernel/` — environment-agnostic core (parser, sema, model, checker,
  bmc, diagnostics). No filesystem/stdout/process; compiles to
  `wasm32-unknown-unknown`. The import loader is abstracted behind the
  `ModuleResolver` trait.
- `caelum-cli/` — clap CLI + `main.rs` + `StdFsResolver` (all env coupling).
  Produces the `caelum` binary. Backend features forward to the kernel.
- `caelum-wasm/` — `wasm-bindgen` cdylib (`check_spec`, `check_spec_z3`). See
  `caelum-wasm/README.md`.
- `ci/` — the parallel test harness (crate name `harness`).

## Build & Test

- Cargo lives at `~/.cargo/bin/cargo`; if not in PATH, run `export PATH="$HOME/.cargo/bin:$PATH"` first.
- `cargo build` — build the native crates (kernel/cli/ci; wasm is excluded from default-members)
- `cargo test` — run all unit tests
- `cargo run -p caelum-cli -- <spec.lum>` — check a spec file
- `cargo test -p caelum-cli --features smtlib` — test the SMT-LIB2 path against the `z3` binary
- `wasm-pack build caelum-wasm --target web` — build the wasm module

## Solver backends (BMC engine)

`--solver z3` (default, native libz3) · `varisat` (pure Rust, wasm-viable) ·
`cadical` · `smtlib` (emits SMT-LIB2 to the external `z3` binary; feature-gated).
In the browser, `caelum-wasm` uses varisat in-module or offloads SMT-LIB2 to
z3.js.

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
