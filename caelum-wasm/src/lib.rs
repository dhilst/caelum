//! WebAssembly bindings for the Caelum LTL model checker.
//!
//! The kernel is environment-agnostic, so this crate is a thin shim. It offers
//! two flavours of check:
//!
//! * [`check_spec`] / [`check_spec_multi`] — synchronous, using the pure-Rust
//!   `varisat` SAT backend compiled into the wasm module.
//! * [`check_spec_z3`] — asynchronous, offloading solving to the browser's
//!   z3.js. The native Z3 cannot link into a wasm module, and z3.js's solve is
//!   async, so we drive it two-pass: encode the property to SMT-LIB2 (Pass A,
//!   capturing the script), `await` z3.js, then replay the returned model to
//!   decode the trace (Pass B). Both passes re-run the deterministic encoder,
//!   so variable ids line up.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use caelum_kernel::bmc::setup::prepare as prepare_bmc;
use caelum_kernel::bmc::unroll::check_property_with_solver;
use caelum_kernel::bmc::{check_with_bmc, BmcOptions, SmtOracle, SmtScriptSolver, SolverBackend};
use caelum_kernel::checker::{check_properties, CheckReport, CheckStatus};
use caelum_kernel::diagnostics::{CaelumError, Result};
use caelum_kernel::loader::{load_spec_with, LoadedSpec, ModuleId, ModuleResolver};
use caelum_kernel::model::{build_graph_with_options, BuildOptions};
use caelum_kernel::sema::check_source_file;
use caelum_kernel::syntax::{parse_source, Item, SourceFile};

// ---------------------------------------------------------------------------
// Synchronous, in-module varisat path
// ---------------------------------------------------------------------------

/// In-memory resolver: an import string maps directly to a module's source
/// text (a virtual filesystem). Import ids are the raw import strings.
struct MemResolver {
    files: HashMap<String, String>,
}

impl ModuleResolver for MemResolver {
    fn resolve(&self, _importer: &ModuleId, import: &str) -> Result<ModuleId> {
        Ok(import.to_string())
    }

    fn read(&self, id: &ModuleId) -> Result<String> {
        self.files
            .get(id)
            .cloned()
            .ok_or_else(|| CaelumError::ReadFile {
                path: id.clone(),
                message: "module not found in virtual filesystem".into(),
            })
    }
}

/// Check a single-file spec with the in-module varisat backend. `opts_json` is
/// a JSON object; all fields optional:
/// `{ "engine": "explicit"|"bmc", "bmc_depth": u, "prove": bool, "max_states": u }`.
/// Returns a JSON report string (or `{ "error": ... }`).
#[wasm_bindgen]
pub fn check_spec(source: &str, opts_json: &str) -> String {
    let mut files = HashMap::new();
    files.insert("<root>".to_string(), source.to_string());
    run(&"<root>".to_string(), files, opts_json)
}

/// Check a multi-file spec. `files_json` maps module id → source text; `root`
/// names the entry module.
#[wasm_bindgen]
pub fn check_spec_multi(files_json: &str, root: &str, opts_json: &str) -> String {
    let files: HashMap<String, String> = match serde_json::from_str(files_json) {
        Ok(files) => files,
        Err(err) => return error_json(&format!("invalid files_json: {err}")),
    };
    run(&root.to_string(), files, opts_json)
}

fn run(root: &ModuleId, files: HashMap<String, String>, opts_json: &str) -> String {
    let opts = match parse_opts(opts_json) {
        Ok(opts) => opts,
        Err(msg) => return error_json(&msg),
    };
    match run_inner(root, &files, &opts) {
        Ok(value) => value.to_string(),
        Err(err) => error_json(&err.to_string()),
    }
}

fn run_inner(
    root: &ModuleId,
    files: &HashMap<String, String>,
    opts: &serde_json::Value,
) -> Result<serde_json::Value> {
    let resolver = MemResolver {
        files: files.clone(),
    };
    let spec = load_spec_with(root, &resolver)?;
    check_source_file(&spec.source)?;

    let engine = opts.get("engine").and_then(|v| v.as_str()).unwrap_or("explicit");
    let report = match engine {
        "bmc" => {
            let depth = opts.get("bmc_depth").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let prove = opts.get("prove").and_then(|v| v.as_bool()).unwrap_or(false);
            check_with_bmc(
                &spec.source,
                &BmcOptions { depth, prove },
                SolverBackend::Varisat,
            )?
        }
        _ => {
            let max_states =
                opts.get("max_states").and_then(|v| v.as_u64()).unwrap_or(100_000) as usize;
            let graph = build_graph_with_options(&spec.source, &BuildOptions { max_states })?;
            check_properties(&spec.source, &graph)?
        }
    };

    Ok(report_json(&spec, &report))
}

// ---------------------------------------------------------------------------
// Asynchronous z3.js path (two-pass: capture script → await z3 → replay model)
// ---------------------------------------------------------------------------

/// Captures the SMT-LIB2 script produced during encoding, then returns a
/// throwaway `unsat` so the (discarded) first pass short-circuits without
/// touching a model.
#[derive(Clone)]
struct CaptureOracle {
    script: Rc<RefCell<Option<String>>>,
}

impl SmtOracle for CaptureOracle {
    fn solve(&self, script: &str) -> Result<String> {
        *self.script.borrow_mut() = Some(script.to_string());
        Ok("unsat".to_string())
    }
}

/// Replays a previously obtained solver answer (from z3.js) without solving.
struct ReplayOracle {
    answer: String,
}

impl SmtOracle for ReplayOracle {
    fn solve(&self, _script: &str) -> Result<String> {
        Ok(self.answer.clone())
    }
}

/// Check a single-file spec by offloading each property's SMT-LIB2 to z3.js.
///
/// `solve_fn` is a JS function `(script: string) => Promise<string>` that runs
/// the script through z3 and resolves to its raw `check-sat`/`get-value`
/// output. Returns a JSON report string.
#[wasm_bindgen]
pub async fn check_spec_z3(source: String, opts_json: String, solve_fn: js_sys::Function) -> String {
    match check_spec_z3_inner(&source, &opts_json, &solve_fn).await {
        Ok(value) => value.to_string(),
        Err(message) => error_json(&message),
    }
}

async fn check_spec_z3_inner(
    source: &str,
    opts_json: &str,
    solve_fn: &js_sys::Function,
) -> std::result::Result<serde_json::Value, String> {
    let opts = parse_opts(opts_json)?;
    let depth = opts.get("bmc_depth").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let file = parse_source(source).map_err(|e| e.to_string())?;
    check_source_file(&file).map_err(|e| e.to_string())?;
    let spec = prepare_bmc(&file).map_err(|e| e.to_string())?;
    let options = BmcOptions {
        depth,
        prove: false,
    };

    let mut results = Vec::new();
    for property in &spec.properties {
        // Pass A: run the encoder to capture the SMT-LIB2 script. The oracle
        // returns a throwaway `unsat`, so this result is discarded unless the
        // property never solves (skipped/unsupported), in which case there is
        // no script and the pass-A result is authoritative.
        let cell = Rc::new(RefCell::new(None));
        let pass_a = {
            let mut capture = SmtScriptSolver::new(CaptureOracle {
                script: cell.clone(),
            });
            check_property_with_solver(&spec, property, &options, &mut capture)
                .map_err(|e| e.to_string())?
        };
        let script = cell.borrow_mut().take();
        let Some(script) = script else {
            results.push(pass_a);
            continue;
        };

        // Solve the captured script with z3.js (async).
        let promise = solve_fn
            .call1(&JsValue::NULL, &JsValue::from_str(&script))
            .map_err(|e| format!("solve_fn threw: {e:?}"))?;
        let promise: Promise = promise
            .dyn_into()
            .map_err(|_| "solve_fn must return a Promise".to_string())?;
        let answer = JsFuture::from(promise)
            .await
            .map_err(|e| format!("z3 solve rejected: {e:?}"))?
            .as_string()
            .ok_or_else(|| "z3 solve must resolve to a string".to_string())?;

        // Pass B: re-run the deterministic encoder and decode using the model
        // z3 returned. Variable ids match Pass A, so the model lines up.
        let mut replay = SmtScriptSolver::new(ReplayOracle { answer });
        let result = check_property_with_solver(&spec, property, &options, &mut replay)
            .map_err(|e| e.to_string())?;
        results.push(result);
    }

    let status = if results.iter().any(|r| r.status == CheckStatus::Fail) {
        CheckStatus::Fail
    } else {
        CheckStatus::Pass
    };
    let report = CheckReport {
        status,
        properties: results,
    };
    Ok(report_json_parts(&file, 1, &report))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn parse_opts(opts_json: &str) -> std::result::Result<serde_json::Value, String> {
    if opts_json.trim().is_empty() {
        Ok(serde_json::json!({}))
    } else {
        serde_json::from_str(opts_json).map_err(|e| format!("invalid opts_json: {e}"))
    }
}

fn var_names(source: &SourceFile) -> Vec<String> {
    source
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Var(decl) => Some(decl.name.clone()),
            _ => None,
        })
        .collect()
}

fn report_json(spec: &LoadedSpec, report: &CheckReport) -> serde_json::Value {
    report_json_parts(&spec.source, spec.files.len(), report)
}

fn report_json_parts(source: &SourceFile, files: usize, report: &CheckReport) -> serde_json::Value {
    let names = var_names(source);
    let properties: Vec<serde_json::Value> = report
        .properties
        .iter()
        .map(|property| {
            let mut object = serde_json::Map::new();
            object.insert("name".into(), serde_json::json!(property.name));
            object.insert("kind".into(), serde_json::json!(property.kind));
            object.insert("status".into(), serde_json::json!(property.status));
            if let Some(note) = &property.note {
                object.insert("note".into(), serde_json::json!(note));
            }
            if let Some(counterexample) = &property.counterexample {
                let states: Vec<serde_json::Value> = counterexample
                    .states
                    .iter()
                    .map(|state| {
                        let mut row = serde_json::Map::new();
                        for (name, value) in names.iter().zip(&state.values) {
                            row.insert(name.clone(), serde_json::json!(value));
                        }
                        serde_json::Value::Object(row)
                    })
                    .collect();
                let mut ce = serde_json::Map::new();
                ce.insert("states".into(), serde_json::Value::Array(states));
                if let Some(cycle_start) = counterexample.cycle_start {
                    ce.insert("cycle_start".into(), serde_json::json!(cycle_start));
                }
                object.insert("counterexample".into(), serde_json::Value::Object(ce));
            }
            serde_json::Value::Object(object)
        })
        .collect();

    serde_json::json!({
        "tool": "caelum",
        "status": report.status,
        "files": files,
        "items": source.item_count(),
        "properties": properties,
    })
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "tool": "caelum", "error": message }).to_string()
}
