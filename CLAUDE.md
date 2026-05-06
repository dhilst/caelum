# tplgine

Temporal propositional logic engine and model checker.

## Build & Test

- `cargo build` — build the project
- `cargo test` — run all unit tests
- `cargo run -- <spec.tpl>` — check a spec file

## Harness

1. All tests must pass (`cargo test`)
2. All `.tpl` files in `examples/` and `refinement/specs/` must pass (`cargo run -- <file>` exits 0)
3. Harness is executed before `git push` as a pre-push hook
