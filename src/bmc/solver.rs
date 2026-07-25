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

#[cfg(feature = "bmc-z3")]
pub mod z3_backend {
    use super::*;
    use z3::ast::Bool;
    use z3::{Model, SatResult, Solver as Z3Backend};

    /// Z3 used purely as a propositional SAT backend: each DIMACS variable is
    /// a fresh boolean constant, each clause a disjunction asserted into the
    /// solver. `vars[i]` corresponds to DIMACS variable `i + 1`.
    pub struct Z3Solver {
        solver: Z3Backend,
        vars: Vec<Bool>,
        model: Option<Model>,
    }

    impl Z3Solver {
        pub fn new() -> Self {
            Self {
                solver: Z3Backend::new(),
                vars: Vec::new(),
                model: None,
            }
        }

        fn lit(&self, l: SatLit) -> Bool {
            let var = &self.vars[(l.unsigned_abs() as usize) - 1];
            if l > 0 {
                var.clone()
            } else {
                var.not()
            }
        }
    }

    impl Default for Z3Solver {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Solver for Z3Solver {
        fn new_var(&mut self) -> i32 {
            self.vars.push(Bool::fresh_const("v"));
            self.vars.len() as i32
        }

        fn add_clause(&mut self, clause: &[SatLit]) {
            if clause.is_empty() {
                // An empty clause is unsatisfiable.
                self.solver.assert(Bool::from_bool(false));
                return;
            }
            let lits: Vec<Bool> = clause.iter().map(|&l| self.lit(l)).collect();
            self.solver.assert(Bool::or(&lits));
        }

        fn solve(&mut self) -> Result<bool> {
            match self.solver.check() {
                SatResult::Sat => {
                    self.model = self.solver.get_model();
                    Ok(true)
                }
                SatResult::Unsat => Ok(false),
                SatResult::Unknown => Err(CaelumError::Model {
                    message: "z3 solver returned unknown".into(),
                }),
            }
        }

        fn model_value(&self, var: i32) -> bool {
            assert!(var > 0, "model_value expects positive variable index");
            self.model
                .as_ref()
                .and_then(|m| m.eval(&self.vars[(var as usize) - 1], true))
                .and_then(|b| b.as_bool())
                .unwrap_or(false)
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
