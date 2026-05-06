# Caelum

> An LTL model checker.

Caelum reads `.lum` specification files describing finite-state transition systems
and checks whether declared LTL properties hold over all reachable states.
When a property fails, Caelum produces a counterexample trace.

## Quick Start

```bash
cargo build --release
cargo run -- examples/counter.lum
```

### Example

```tpl
module examples.counter

const max = 3

let x ∈ 0..max

init {
  x = 0
}

transition step {
  x' = (x + 1) mod (max + 1)
}

property in_range {
  □ (x >= 0 ∧ x <= max)
}

property returns_to_zero {
  □ ◇ (x = 0)
}
```

## Operator Syntax

Caelum supports three equivalent syntaxes for every operator:

| Operator    | Keyword      | ASCII   | Unicode |
|-------------|-------------|---------|---------|
| always      | `always`    | `[]`    | `□`     |
| eventually  | `eventually`| `<>`    | `◇`     |
| next        | `next`      | `()`    | `◯`     |
| until       | `until`     | `U`     | `𝒰`     |
| and         | `and`       | `/\`    | `∧`     |
| or          | `or`        | `\/`    | `∨`     |
| not         | `not`       | `~`     | `¬`     |
| implies     |             | `->`    | `→`     |
| iff         |             | `<->`   | `↔`     |
| not equal   |             | `!=`    | `≠`     |

## CLI

```
caelum <spec>.lum              # check a specification (default)
caelum check <spec>.lum        # explicit check
caelum parse <spec>.lum        # parse only
caelum fmt <spec>.lum          # format a specification
```

### Flags

| Flag | Description |
|------|-------------|
| `--format human\|json` | Output format (default: human) |
| `--show-trace` | Display counterexample traces |
| `--dump-graph` | Print the reachable transition graph |
| `--max-states N` | Limit state exploration (default: 100,000) |
| `--include-path DIR` | Additional import search directories |
| `--print-keywords` | Format output with keyword operators |
| `--print-ascii-operators` | Format output with ASCII operators |
| `--print-unicode-operators` | Format output with Unicode operators (default) |

### Exit Codes

| Code | Meaning |
|-----:|---------|
| 0 | All checks passed |
| 1 | One or more properties failed |
| 2 | Parse error |
| 3 | Semantic validation error |
| 4 | Import or file loading error |
| 5 | Model construction error |
| 6 | Internal error |

## Building and Testing

```bash
cargo build          # build
cargo test           # run all tests
```

## Documentation

Full documentation is available at [https://dhilst.github.io/tlpengine/](https://dhilst.github.io/tlpengine/).

## License

MIT
