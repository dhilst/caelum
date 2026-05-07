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
        let mut result =
            unroll::check_property_with_solver(&spec, property, options, solver.as_mut())?;

        // If the base case passed and the user asked for proof, try k-induction.
        if options.prove
            && result.status == CheckStatus::Pass
            && unroll::property_eligible_for_induction(property)
        {
            let mut induction_solver = make_solver(backend);
            match unroll::check_induction(&spec, property, options, induction_solver.as_mut())? {
                unroll::InductionOutcome::Holds => {
                    result.status = CheckStatus::Certified;
                }
                unroll::InductionOutcome::Inconclusive => {
                    let note = format!(
                        "k-induction at k={} inconclusive; bounded pass only",
                        options.depth
                    );
                    result.note = Some(note);
                }
                unroll::InductionOutcome::NotApplicable => {}
            }
        }

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
        check_with_bmc(
            &file,
            &BmcOptions {
                depth,
                prove: false,
            },
            SolverBackend::Varisat,
        )
        .expect("bmc")
    }

    fn prove(source: &str, depth: usize) -> CheckReport {
        let file = parse_source(source).expect("parse");
        check_source_file(&file).expect("sema");
        check_with_bmc(
            &file,
            &BmcOptions { depth, prove: true },
            SolverBackend::Varisat,
        )
        .expect("bmc")
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
    fn passes_eventually_with_lasso() {
        // x cycles 0 -> 1 -> 2 -> 0; ◇ (x = 2) holds because x = 2 is reached.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property reaches_two { ◇ (x = 2) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn passes_recurrence_with_lasso() {
        // From any state in the 3-cycle we keep returning to x = 0.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property recurrent_zero { □ ◇ (x = 0) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn fails_recurrence_when_trapped() {
        // The system has a trap state at x = 1 reachable from x = 0; once
        // reached, x stays at 1 forever, so □ ◇ (x = 0) is false from t = 1.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition stay_zero { x = 0 ∧ x' = 1 }
            transition stay_one  { x = 1 ∧ x' = 1 }
            transition stay_two  { x = 2 ∧ x' = 2 }
            property recurrent_zero { □ ◇ (x = 0) }
            ",
            6,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let trace = report.properties[0]
            .counterexample
            .as_ref()
            .expect("counterexample");
        assert!(trace.cycle_start.is_some(), "lasso must report cycle_start");
    }

    #[test]
    fn passes_until_when_witnessed() {
        // φ U ψ where φ = (x ≥ 0), ψ = (x = 2): φ holds at every step (it's
        // a tautology in 0..2) and ψ is reached at step 2.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property guard_until { (x >= 0) until (x = 2) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn handles_response_pattern() {
        // `□ (x = 0 → ◇ (x = 1))` — response pattern recognised in v3.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property response { □ (x = 0 → ◇ (x = 1)) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
    }

    #[test]
    fn fails_response_when_target_unreachable() {
        // From x = 0 the system reaches only x = 1 (and stays). The trigger
        // x = 2 cannot fire from any reachable state, so the property is
        // vacuously true; the false counterpart is asserting target = x=99.
        let report = check(
            r"
            let x: 0..2
            init { x = 0 }
            transition stay_zero { x = 0 ∧ x' = 1 }
            transition stay_one  { x = 1 ∧ x' = 1 }
            transition stay_two  { x = 2 ∧ x' = 2 }
            property response { □ (x = 1 → ◇ (x = 2)) }
            ",
            6,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
    }

    #[test]
    fn certifies_inductive_safety() {
        // `□ (x ≤ 2)` is genuinely 1-inductive on this counter:  any state
        // satisfying x ≤ 2 transitions to x' = (x + 1) mod 3 ∈ {0, 1, 2},
        // which also satisfies the bound.
        let report = prove(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property bounded { □ (x <= 2) }
            ",
            2,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Certified);
    }

    #[test]
    fn induction_skipped_for_liveness() {
        // Liveness properties are not eligible for k-induction; --prove
        // must leave them as the base-case PASS without crashing.
        let report = prove(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property recurrent_zero { □ ◇ (x = 0) }
            ",
            5,
        );
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].note.is_none());
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
