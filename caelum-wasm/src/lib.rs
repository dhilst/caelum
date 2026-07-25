//! WebAssembly bindings for the Caelum LTL model checker.
//!
//! The kernel is environment-agnostic, so this crate is a thin shim: it takes
//! spec source as a string (there is no filesystem in the browser), drives the
//! same in-memory pipeline the CLI uses, and returns a JSON report string.
//!
//! Only the pure-Rust `varisat` SAT backend is compiled in — the native
//! `cadical`/`z3` backends cannot link into a wasm module. Z3 in the browser is
//! provided separately via z3.js (see the SMT-LIB2 oracle path).

use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use caelum_kernel::bmc::{check_with_bmc, BmcOptions, SolverBackend};
use caelum_kernel::checker::{check_properties, CheckReport};
use caelum_kernel::diagnostics::{CaelumError, Result};
use caelum_kernel::loader::{load_spec_with, LoadedSpec, ModuleId, ModuleResolver};
use caelum_kernel::model::{build_graph_with_options, BuildOptions};
use caelum_kernel::sema::check_source_file;
use caelum_kernel::syntax::{Item, SourceFile};

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

/// Check a single-file spec. `opts_json` is a JSON object; all fields optional:
/// `{ "engine": "explicit"|"bmc", "bmc_depth": u, "prove": bool, "max_states": u }`.
/// Returns a JSON report string (or `{ "error": ... }`).
#[wasm_bindgen]
pub fn check_spec(source: &str, opts_json: &str) -> String {
    let mut files = HashMap::new();
    files.insert("<root>".to_string(), source.to_string());
    run(&"<root>".to_string(), files, opts_json)
}

/// Check a multi-file spec. `files_json` is a JSON object mapping module id →
/// source text; `root` names the entry module within it.
#[wasm_bindgen]
pub fn check_spec_multi(files_json: &str, root: &str, opts_json: &str) -> String {
    let files: HashMap<String, String> = match serde_json::from_str(files_json) {
        Ok(files) => files,
        Err(err) => return error_json(&format!("invalid files_json: {err}")),
    };
    run(&root.to_string(), files, opts_json)
}

fn run(root: &ModuleId, files: HashMap<String, String>, opts_json: &str) -> String {
    let opts: serde_json::Value = if opts_json.trim().is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_str(opts_json) {
            Ok(value) => value,
            Err(err) => return error_json(&format!("invalid opts_json: {err}")),
        }
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

    Ok(report_to_json(&spec, &report))
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

fn report_to_json(spec: &LoadedSpec, report: &CheckReport) -> serde_json::Value {
    let names = var_names(&spec.source);
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
        "files": spec.files.len(),
        "items": spec.source.item_count(),
        "properties": properties,
    })
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "tool": "caelum", "error": message }).to_string()
}
