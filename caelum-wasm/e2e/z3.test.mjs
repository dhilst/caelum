// End-to-end test of the two-module z3.js path: caelum-wasm encodes each
// property to SMT-LIB2 and this harness solves it with real Z3 (z3-solver).
//
// Requires the nodejs-target build:
//   wasm-pack build caelum-wasm --target nodejs --out-dir pkg
//   cd caelum-wasm/e2e && npm install && node z3.test.mjs
import { init } from 'z3-solver';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const wasm = require('../pkg/caelum_wasm.js');

const { Z3 } = await init();
const ctx = Z3.mk_context(Z3.mk_config());
// One shared context is safe: each kernel script leads with `(reset)`.
const solve = async (script) => await Z3.eval_smtlib2_string(ctx, script);

let failures = 0;
function check(cond, label) {
  if (cond) {
    console.log(`ok   - ${label}`);
  } else {
    console.error(`FAIL - ${label}`);
    failures++;
  }
}

async function report(spec, depth) {
  const json = await wasm.check_spec_z3(spec, JSON.stringify({ bmc_depth: depth }), solve);
  return JSON.parse(json);
}

const failing = `let x: 0..2
init { x = 0 }
transition step { x' = (x + 1) mod 3 }
property never_two { [] (x != 2) }`;

const passing = `let x: 0..3
init { x = 0 }
transition step { x' = (x + 1) mod 4 }
property in_range { [] (x >= 0 /\\ x <= 3) }`;

const recurrence = `let x: 0..2
init { x = 0 }
transition step { x' = (x + 1) mod 3 }
property recurrent_zero { [] <> (x = 0) }`;

// Safety violation → fail, with a counterexample reaching x = 2.
{
  const r = await report(failing, 5);
  check(r.status === 'fail', 'failing safety reports fail');
  const ce = r.properties[0].counterexample;
  check(!!ce, 'failing safety yields a counterexample');
  check(ce && ce.states.some((s) => s.x && s.x.value === 2), 'counterexample reaches x = 2');
}

// Safety holds within depth → pass.
{
  const r = await report(passing, 8);
  check(r.properties[0].status === 'pass', 'passing safety reports pass');
}

// Recurrence via lasso → pass.
{
  const r = await report(recurrence, 5);
  check(r.properties[0].status === 'pass', 'recurrence lasso reports pass');
}

// Parse error is surfaced, not thrown.
{
  const r = JSON.parse(await wasm.check_spec_z3('garbage', '{}', solve));
  check(!!r.error, 'parse error is surfaced in the report');
}

if (failures > 0) {
  console.error(`\n${failures} check(s) failed`);
  process.exit(1);
}
console.log('\nall z3.js integration checks passed');
