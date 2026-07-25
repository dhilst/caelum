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

#[cfg(feature = "bmc-smtlib")]
pub mod smtlib_backend {
    //! A `Solver` that emits an SMT-LIB2 script and delegates the actual
    //! solving to a caller-provided [`SmtOracle`] — the browser's z3.js or the
    //! native `z3` binary. No solver is linked in; this is pure string
    //! generation plus a tiny model parser, so it builds anywhere (incl. wasm).
    //!
    //! Each DIMACS variable K becomes a boolean constant `vK`; each clause an
    //! `(assert (or ...))`. The engine is batch (all clauses, then one solve),
    //! which maps to a single script ending in `(check-sat)`/`(get-value ...)`.

    use std::collections::HashMap;

    use super::*;

    /// Runs an SMT-LIB2 script and returns Z3's raw textual output (the
    /// `check-sat` line followed by any `get-value` output).
    pub trait SmtOracle {
        fn solve(&self, script: &str) -> Result<String>;
    }

    pub struct SmtScriptSolver<O: SmtOracle> {
        oracle: O,
        decls: String,
        asserts: String,
        n: i32,
        model: HashMap<i32, bool>,
    }

    impl<O: SmtOracle> SmtScriptSolver<O> {
        pub fn new(oracle: O) -> Self {
            Self {
                oracle,
                decls: String::new(),
                asserts: String::new(),
                n: 0,
                model: HashMap::new(),
            }
        }

        fn lit(l: SatLit) -> String {
            let var = l.unsigned_abs();
            if l > 0 {
                format!("v{var}")
            } else {
                format!("(not v{var})")
            }
        }
    }

    impl<O: SmtOracle> Solver for SmtScriptSolver<O> {
        fn new_var(&mut self) -> i32 {
            self.n += 1;
            self.decls
                .push_str(&format!("(declare-const v{} Bool)\n", self.n));
            self.n
        }

        fn add_clause(&mut self, clause: &[SatLit]) {
            match clause.len() {
                0 => self.asserts.push_str("(assert false)\n"),
                1 => self
                    .asserts
                    .push_str(&format!("(assert {})\n", Self::lit(clause[0]))),
                _ => {
                    let lits: Vec<String> = clause.iter().map(|&l| Self::lit(l)).collect();
                    self.asserts
                        .push_str(&format!("(assert (or {}))\n", lits.join(" ")));
                }
            }
        }

        fn solve(&mut self) -> Result<bool> {
            // Lead with `(reset)` so the script is self-contained: oracles that
            // reuse a solver/context across calls (e.g. one z3.js context for
            // several properties) won't collide on re-declared vars or a
            // re-set logic. Fresh-process oracles (the CLI's `z3 -in`) are
            // unaffected.
            let mut script = String::from("(reset)\n(set-logic QF_UF)\n");
            script.push_str(&self.decls);
            script.push_str(&self.asserts);
            script.push_str("(check-sat)\n");
            if self.n > 0 {
                let vars: Vec<String> = (1..=self.n).map(|k| format!("v{k}")).collect();
                script.push_str(&format!("(get-value ({}))\n", vars.join(" ")));
            }

            let output = self.oracle.solve(&script)?;
            let trimmed = output.trim_start();
            // Order matters: "unsat" starts with "sat" only after the "un".
            if trimmed.starts_with("unsat") {
                return Ok(false);
            }
            if trimmed.starts_with("unknown") {
                return Err(CaelumError::Model {
                    message: "smtlib oracle returned unknown".into(),
                });
            }
            if !trimmed.starts_with("sat") {
                return Err(CaelumError::Model {
                    message: format!("smtlib oracle returned unexpected output: {trimmed}"),
                });
            }
            self.model.clear();
            parse_model(trimmed, &mut self.model);
            Ok(true)
        }

        fn model_value(&self, var: i32) -> bool {
            assert!(var > 0, "model_value expects positive variable index");
            *self.model.get(&var).unwrap_or(&false)
        }
    }

    /// Parse `((v1 true) (v2 false) ...)` pairs out of Z3's `get-value` output.
    /// Missing variables default to false (matching the SAT backends).
    fn parse_model(output: &str, model: &mut HashMap<i32, bool>) {
        let cleaned = output.replace('(', " ").replace(')', " ");
        let tokens: Vec<&str> = cleaned.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            if let Some(rest) = tokens[i].strip_prefix('v') {
                if let Ok(var) = rest.parse::<i32>() {
                    if let Some(&value) = tokens.get(i + 1) {
                        match value {
                            "true" => {
                                model.insert(var, true);
                                i += 2;
                                continue;
                            }
                            "false" => {
                                model.insert(var, false);
                                i += 2;
                                continue;
                            }
                            _ => {}
                        }
                    }
                }
            }
            i += 1;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Records the script and replays a canned Z3 answer.
        struct FakeOracle {
            answer: String,
        }
        impl SmtOracle for FakeOracle {
            fn solve(&self, _script: &str) -> Result<String> {
                Ok(self.answer.clone())
            }
        }

        #[test]
        fn parses_sat_model_and_reads_values() {
            let mut s = SmtScriptSolver::new(FakeOracle {
                answer: "sat\n((v1 true) (v2 false) (v3 true))\n".into(),
            });
            let a = s.new_var();
            let b = s.new_var();
            let c = s.new_var();
            s.add_clause(&[a, -b]);
            assert!(s.solve().expect("solve"));
            assert!(s.model_value(a));
            assert!(!s.model_value(b));
            assert!(s.model_value(c));
        }

        #[test]
        fn reports_unsat() {
            let mut s = SmtScriptSolver::new(FakeOracle {
                answer: "unsat\n".into(),
            });
            let _ = s.new_var();
            assert!(!s.solve().expect("solve"));
        }
    }
}
