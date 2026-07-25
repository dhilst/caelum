# caelum-wasm

WebAssembly bindings for the Caelum LTL model checker.

## API

- `check_spec(source, optsJson) -> reportJson` — synchronous, using the
  pure-Rust **varisat** backend compiled into the module. `optsJson`:
  `{ "engine": "explicit"|"bmc", "bmc_depth": u, "prove": bool, "max_states": u }`.
- `check_spec_multi(filesJson, root, optsJson) -> reportJson` — same, for a
  multi-file spec (`filesJson` maps module id → source; `root` is the entry).
- `check_spec_z3(source, optsJson, solveFn) -> Promise<reportJson>` —
  **asynchronous**, offloading each property's SMT-LIB2 to Z3 running as a
  second wasm module. `solveFn` is `(script: string) => Promise<string>`
  returning Z3's raw `check-sat`/`get-value` output.

The native Z3 cannot link into a wasm module (it needs a C++ runtime), so the
z3.js path is **two modules**: caelum-wasm encodes to SMT-LIB2, z3.js solves.
Because z3.js's solve is async and the BMC engine is synchronous, `check_spec_z3`
runs each property twice — Pass A captures the deterministic SMT-LIB2 script,
z3.js solves it, Pass B replays the returned model to decode the trace.

## Build

```sh
# browser (ES modules)
wasm-pack build caelum-wasm --target web --out-dir pkg
# or nodejs (CommonJS)
wasm-pack build caelum-wasm --target nodejs --out-dir pkg
```

## Browser demo (`www/`)

```sh
wasm-pack build caelum-wasm --target web --out-dir pkg
cd caelum-wasm/www && npm install z3-solver   # provides z3.js same-origin
node serve.mjs                                # COOP/COEP headers for z3 threads
# open http://localhost:8080/www/
```

The demo has three engines: explicit (varisat), BMC/varisat, and BMC/z3.js.

## Node example

```js
import { init } from 'z3-solver';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const wasm = require('./pkg/caelum_wasm.js'); // nodejs target

const { Z3 } = await init();
const ctx = Z3.mk_context(Z3.mk_config());
const solve = async (s) => await Z3.eval_smtlib2_string(ctx, s);

const spec = `let x: 0..2
init { x = 0 }
transition step { x' = (x + 1) mod 3 }
property never_two { [] (x != 2) }`;

console.log(await wasm.check_spec_z3(spec, '{"bmc_depth":5}', solve));
// -> {"status":"fail", ...counterexample reaching x=2...}
```

The z3.js integration is verified end-to-end in Node (see the project's
scratchpad tests); the browser demo uses the same code path.
