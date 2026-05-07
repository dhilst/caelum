//! Bounded model checking engine for caelum.
//!
//! Encodes the unrolled transition system as a propositional SAT instance and
//! refutes it using a pluggable backend (varisat or cadical, behind feature
//! flags). v1 supports SAFETY ONLY: properties must be of the form `□ φ`
//! where `φ` is propositional with optional `◯` nesting. Liveness (`◇`,
//! `until`) returns CheckStatus::Skipped.
//!
//! Non-temporal properties are treated as state predicates over the initial
//! state (mirroring the explicit engine's interpretation).

pub mod encode;
pub mod setup;
pub mod solver;
pub mod sym;
pub mod unroll;

pub use unroll::BmcOptions;

use crate::checker::{CheckReport, CheckStatus};
use crate::diagnostics::Result;
use crate::syntax::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverBackend {
    #[cfg(feature = "bmc-varisat")]
    Varisat,
    #[cfg(feature = "bmc-cadical")]
    Cadical,
}

impl Default for SolverBackend {
    fn default() -> Self {
        #[cfg(feature = "bmc-varisat")]
        {
            Self::Varisat
        }
        #[cfg(all(not(feature = "bmc-varisat"), feature = "bmc-cadical"))]
        {
            Self::Cadical
        }
    }
}

fn make_solver(backend: SolverBackend) -> Box<dyn solver::Solver> {
    match backend {
        #[cfg(feature = "bmc-varisat")]
        SolverBackend::Varisat => Box::new(solver::varisat_backend::VarisatSolver::new()),
        #[cfg(feature = "bmc-cadical")]
        SolverBackend::Cadical => Box::new(solver::cadical_backend::CadicalSolver::new()),
    }
}

pub fn check_with_bmc(
    file: &SourceFile,
    options: &BmcOptions,
    backend: SolverBackend,
) -> Result<CheckReport> {
    let spec = setup::prepare(file)?;
    let mut results = Vec::new();
    for property in &spec.properties {
        let mut solver = make_solver(backend);
        let result = unroll::check_property_with_solver(&spec, property, options, solver.as_mut())?;
        results.push(result);
    }

    let status = if results.iter().any(|r| r.status == CheckStatus::Fail) {
        CheckStatus::Fail
    } else {
        CheckStatus::Pass
    };

    Ok(CheckReport {
        status,
        properties: results,
    })
}

#[cfg(all(test, feature = "bmc-varisat"))]
mod tests {
    use super::*;
    use crate::checker::CheckStatus;
    use crate::sema::check_source_file;
    use crate::syntax::parse_source;

    fn check(source: &str, depth: usize) -> CheckReport {
        let file = parse_source(source).expect("parse");
        check_source_file(&file).expect("sema");
        check_with_bmc(&file, &BmcOptions { depth }, SolverBackend::Varisat).expect("bmc")
    }

    #[test]
    fn finds_safety_violation_with_counterexample() {
        // x cycles 0 -> 1 -> 2 -> 0; the invariant `x != 2` must fail.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property never_two { □ (x != 2) }
            ",
            5,
        );
        assert_eq!(report.status, CheckStatus::Fail);
        let prop = &report.properties[0];
        assert_eq!(prop.status, CheckStatus::Fail);
        let trace = prop.counterexample.as_ref().expect("counterexample");
        // The trace must reach a state with x = 2.
        let saw_two = trace
            .states
            .iter()
            .any(|s| matches!(s.values.first(), Some(crate::model::Value::Int(2))));
        assert!(saw_two);
    }

    #[test]
    fn passes_safety_within_depth() {
        let report = check(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property in_range { □ (x >= 0 ∧ x <= 3) }
            ",
            8,
        );
        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn skips_unsupported_liveness() {
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property eventually_zero { ◇ (x = 0) }
            ",
            5,
        );
        let prop = &report.properties[0];
        assert_eq!(prop.status, CheckStatus::Skipped);
        assert!(prop.note.as_deref().unwrap_or("").contains("eventually"));
    }

    #[test]
    fn invalid_block_passes_when_property_correctly_fails() {
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            invalid never_two { □ (x != 2) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn next_operator_shifts_time_index() {
        // x toggles between 0 and 1; from x = 0 the next state has x = 1.
        // `□ (x = 0 → ◯ x = 1)` is true and must pass.
        let report = check(
            r"
            let x: 0..1
            init { x = 0 }
            transition step { x' = 1 - x }
            property toggles { □ (x = 0 → ◯ (x = 1)) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }
}
