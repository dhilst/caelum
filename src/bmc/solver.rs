//! Solver trait shared by the BMC engine, with backend implementations.
//!
//! Lits are DIMACS-style i32: positive = the variable, negative = negated.
//! Variables are 1-indexed externally; backends translate to their own
//! representation. Variable 0 is reserved (illegal) so that `-v` is a valid
//! literal.

use crate::diagnostics::{CaelumError, Result};

pub type SatLit = i32;

pub trait Solver {
    /// Allocate a fresh propositional variable, return its DIMACS index (≥ 1).
    fn new_var(&mut self) -> i32;
    /// Add a CNF clause.
    fn add_clause(&mut self, clause: &[SatLit]);
    /// Run the solver; return `true` for SAT, `false` for UNSAT.
    fn solve(&mut self) -> Result<bool>;
    /// After `solve` returned SAT, query the assignment of a positive
    /// variable index. Calling on negative or zero is undefined.
    fn model_value(&self, var: i32) -> bool;
}

#[cfg(feature = "bmc-varisat")]
pub mod varisat_backend {
    use super::*;
    use varisat::{ExtendFormula, Lit, Var};

    pub struct VarisatSolver {
        solver: varisat::Solver<'static>,
        next_var: i32,
        last_model: Vec<bool>,
    }

    impl VarisatSolver {
        pub fn new() -> Self {
            Self {
                solver: varisat::Solver::new(),
                next_var: 0,
                last_model: Vec::new(),
            }
        }
    }

    impl Default for VarisatSolver {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Solver for VarisatSolver {
        fn new_var(&mut self) -> i32 {
            self.next_var += 1;
            self.next_var
        }

        fn add_clause(&mut self, clause: &[SatLit]) {
            let lits: Vec<Lit> = clause
                .iter()
                .map(|&l| {
                    let var = Var::from_index((l.unsigned_abs() as usize) - 1);
                    Lit::from_var(var, l > 0)
                })
                .collect();
            self.solver.add_clause(&lits);
        }

        fn solve(&mut self) -> Result<bool> {
            let result = self.solver.solve().map_err(|e| CaelumError::Model {
                message: format!("varisat solver error: {e}"),
            })?;
            if result {
                let model = self.solver.model().unwrap_or_default();
                self.last_model = vec![false; (self.next_var + 1) as usize];
                for lit in model {
                    let idx = lit.var().index() + 1;
                    if idx < self.last_model.len() {
                        self.last_model[idx] = lit.is_positive();
                    }
                }
            }
            Ok(result)
        }

        fn model_value(&self, var: i32) -> bool {
            assert!(var > 0, "model_value expects positive variable index");
            *self.last_model.get(var as usize).unwrap_or(&false)
        }
    }
}

#[cfg(feature = "bmc-cadical")]
pub mod cadical_backend {
    use super::*;

    pub struct CadicalSolver {
        solver: cadical::Solver,
        next_var: i32,
    }

    impl CadicalSolver {
        pub fn new() -> Self {
            Self {
                solver: cadical::Solver::default(),
                next_var: 0,
            }
        }
    }

    impl Default for CadicalSolver {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Solver for CadicalSolver {
        fn new_var(&mut self) -> i32 {
            self.next_var += 1;
            self.next_var
        }

        fn add_clause(&mut self, clause: &[SatLit]) {
            self.solver.add_clause(clause.iter().copied());
        }

        fn solve(&mut self) -> Result<bool> {
            match self.solver.solve() {
                Some(true) => Ok(true),
                Some(false) => Ok(false),
                None => Err(CaelumError::Model {
                    message: "cadical solver returned unknown".into(),
                }),
            }
        }

        fn model_value(&self, var: i32) -> bool {
            assert!(var > 0, "model_value expects positive variable index");
            self.solver.value(var).unwrap_or(false)
        }
    }
}
