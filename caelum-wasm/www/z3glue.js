// Bridges caelum-wasm's `check_spec_z3` to Z3 compiled to WebAssembly.
//
// `check_spec_z3(source, opts, solveFn)` calls `solveFn(script)` once per
// property and awaits a Promise<string> of Z3's raw `check-sat`/`get-value`
// output. Each script from the kernel begins with `(reset)`, so a single
// reused context is safe.
//
// z3-solver is loaded from local node_modules (run `npm install` in this
// directory). Serving it same-origin avoids the COEP/CORP issues a CDN import
// would hit under cross-origin isolation.
import { init } from './node_modules/z3-solver/build/browser.js';

let solveFn = null;

/// Returns an async `(script) => Promise<string>` solve callback, initialising
/// Z3 (and one shared context) on first use.
export async function makeSolve() {
  if (solveFn) return solveFn;
  const { Z3 } = await init();
  const cfg = Z3.mk_config();
  const ctx = Z3.mk_context(cfg);
  solveFn = async (script) => await Z3.eval_smtlib2_string(ctx, script);
  return solveFn;
}
