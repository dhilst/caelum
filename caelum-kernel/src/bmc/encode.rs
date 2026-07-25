//! Encode caelum expressions into propositional CNF, walking the AST and
//! returning a `SymVal` that represents the expression's value(s) under any
//! satisfying SAT assignment.

use std::collections::HashMap;

use crate::diagnostics::{CaelumError, Result};
use crate::syntax::{BinaryOp, Expr, UnaryOp};

use super::solver::{SatLit, Solver};
use super::sym::SymVal;

/// A resolved variable domain materialised as concrete values.
#[derive(Debug, Clone)]
pub enum ResolvedDomain {
    Bool,
    Int(Vec<i64>),
    Enum {
        type_name: String,
        variants: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub enum ConstValue {
    Bool(bool),
    Int(i64),
    Enum { type_name: String, variant: String },
}

pub struct Encoder<'a> {
    solver: &'a mut dyn Solver,
    /// Resolved constants from `const` declarations.
    constants: HashMap<String, ConstValue>,
    /// Each enum variant name maps to its parent enum type name.
    enum_variant_type: HashMap<String, String>,
    /// Resolved domains for each declared variable.
    domains: HashMap<String, ResolvedDomain>,
    /// Memoised state-variable encodings: `(var_name, time)` → SymVal.
    var_cache: HashMap<(String, usize), SymVal>,
    /// Reusable always-true literal.
    true_lit: SatLit,
}

impl<'a> Encoder<'a> {
    pub fn new(
        solver: &'a mut dyn Solver,
        constants: HashMap<String, ConstValue>,
        enum_variant_type: HashMap<String, String>,
        domains: HashMap<String, ResolvedDomain>,
    ) -> Self {
        let true_var = solver.new_var();
        solver.add_clause(&[true_var]);
        Self {
            solver,
            constants,
            enum_variant_type,
            domains,
            var_cache: HashMap::new(),
            true_lit: true_var,
        }
    }

    fn false_lit(&self) -> SatLit {
        -self.true_lit
    }

    /// Constant literal that is always true under any model — useful for
    /// callers that need to inject a unit identity into a chain encoding.
    pub fn true_lit_value(&self) -> SatLit {
        self.true_lit
    }

    /// Get or allocate the SymVal for a state variable at a given time.
    pub fn var_at_time(&mut self, name: &str, time: usize) -> Result<SymVal> {
        if let Some(value) = self.var_cache.get(&(name.to_string(), time)) {
            return Ok(value.clone());
        }
        let domain = self
            .domains
            .get(name)
            .cloned()
            .ok_or_else(|| CaelumError::Model {
                message: format!("BMC encoder: unknown state variable `{name}`"),
            })?;
        let value = self.alloc_for_domain(&domain);
        self.var_cache
            .insert((name.to_string(), time), value.clone());
        Ok(value)
    }

    fn alloc_for_domain(&mut self, domain: &ResolvedDomain) -> SymVal {
        match domain {
            ResolvedDomain::Bool => SymVal::Bool(self.solver.new_var()),
            ResolvedDomain::Int(values) => {
                let lits: Vec<SatLit> = values.iter().map(|_| self.solver.new_var()).collect();
                self.add_exactly_one(&lits);
                let pairs: Vec<(i64, SatLit)> = values.iter().copied().zip(lits).collect();
                SymVal::Int(pairs)
            }
            ResolvedDomain::Enum {
                type_name,
                variants,
            } => {
                let lits: Vec<SatLit> = variants.iter().map(|_| self.solver.new_var()).collect();
                self.add_exactly_one(&lits);
                let pairs: Vec<(String, SatLit)> = variants.iter().cloned().zip(lits).collect();
                SymVal::Enum {
                    type_name: type_name.clone(),
                    values: pairs,
                }
            }
        }
    }

    fn add_exactly_one(&mut self, lits: &[SatLit]) {
        // At least one true.
        self.solver.add_clause(lits);
        // At most one true: pairwise.
        for i in 0..lits.len() {
            for j in (i + 1)..lits.len() {
                self.solver.add_clause(&[-lits[i], -lits[j]]);
            }
        }
    }

    pub fn assert(&mut self, lit: SatLit) {
        self.solver.add_clause(&[lit]);
    }

    /// Allocate a fresh propositional variable (used by callers that need
    /// to add their own clauses, e.g. lasso closure literals).
    pub fn solver_new_var(&mut self) -> SatLit {
        self.solver.new_var()
    }

    /// Add a raw CNF clause, exposed for callers that build their own
    /// constraints over solver-allocated literals.
    pub fn add_clause(&mut self, clause: &[SatLit]) {
        self.solver.add_clause(clause);
    }

    /// Boolean and: returns a fresh literal `r` with `r ↔ a ∧ b`.
    pub fn band(&mut self, a: SatLit, b: SatLit) -> SatLit {
        let r = self.solver.new_var();
        // r → a, r → b
        self.solver.add_clause(&[-r, a]);
        self.solver.add_clause(&[-r, b]);
        // a ∧ b → r
        self.solver.add_clause(&[-a, -b, r]);
        r
    }

    /// Boolean or: returns a fresh literal `r` with `r ↔ a ∨ b`.
    pub fn bor(&mut self, a: SatLit, b: SatLit) -> SatLit {
        let r = self.solver.new_var();
        self.solver.add_clause(&[-a, r]);
        self.solver.add_clause(&[-b, r]);
        self.solver.add_clause(&[-r, a, b]);
        r
    }

    /// Big or: introduce one literal `r` with `r ↔ ⋁ lits`.
    pub fn bor_many(&mut self, lits: &[SatLit]) -> SatLit {
        if lits.is_empty() {
            return self.false_lit();
        }
        if lits.len() == 1 {
            return lits[0];
        }
        let r = self.solver.new_var();
        for &l in lits {
            self.solver.add_clause(&[-l, r]);
        }
        let mut clause: Vec<SatLit> = vec![-r];
        clause.extend_from_slice(lits);
        self.solver.add_clause(&clause);
        r
    }

    /// Big and: introduce one literal `r` with `r ↔ ⋀ lits`.
    pub fn band_many(&mut self, lits: &[SatLit]) -> SatLit {
        if lits.is_empty() {
            return self.true_lit;
        }
        if lits.len() == 1 {
            return lits[0];
        }
        let r = self.solver.new_var();
        for &l in lits {
            self.solver.add_clause(&[-r, l]);
        }
        let mut clause: Vec<SatLit> = lits.iter().map(|l| -l).collect();
        clause.push(r);
        self.solver.add_clause(&clause);
        r
    }

    /// Iff: r ↔ (a ↔ b).
    pub fn biff(&mut self, a: SatLit, b: SatLit) -> SatLit {
        // (a ∧ b) ∨ (¬a ∧ ¬b)
        let pos = self.band(a, b);
        let neg = self.band(-a, -b);
        self.bor(pos, neg)
    }

    /// Encode equality between two state-variable SymVals.  Used by the
    /// lasso wraparound to assert that the loop closure state matches some
    /// earlier state.
    pub fn symval_equal(&mut self, a: &SymVal, b: &SymVal) -> Result<SatLit> {
        match (a, b) {
            (SymVal::Bool(la), SymVal::Bool(lb)) => Ok(self.biff(*la, *lb)),
            (SymVal::Int(va), SymVal::Int(vb)) => {
                let mut conjs = Vec::new();
                for (val_a, lit_a) in va {
                    for (val_b, lit_b) in vb {
                        if val_a == val_b {
                            conjs.push(self.band(*lit_a, *lit_b));
                        }
                    }
                }
                Ok(self.bor_many(&conjs))
            }
            (SymVal::Enum { values: va, .. }, SymVal::Enum { values: vb, .. }) => {
                let mut conjs = Vec::new();
                for (val_a, lit_a) in va {
                    for (val_b, lit_b) in vb {
                        if val_a == val_b {
                            conjs.push(self.band(*lit_a, *lit_b));
                        }
                    }
                }
                Ok(self.bor_many(&conjs))
            }
            _ => Err(CaelumError::Model {
                message: format!(
                    "BMC encoder: cannot equate {} with {} between states",
                    a.kind_label(),
                    b.kind_label()
                ),
            }),
        }
    }

    /// Encode an expression at the given time, returning a SymVal.
    pub fn encode(&mut self, expr: &Expr, time: usize) -> Result<SymVal> {
        match expr {
            Expr::Bool(value) => Ok(SymVal::Bool(if *value {
                self.true_lit
            } else {
                -self.true_lit
            })),
            Expr::Int(value) => Ok(SymVal::Int(vec![(*value, self.true_lit)])),
            Expr::Name(name) => {
                if let Some(c) = self.constants.get(name).cloned() {
                    return Ok(self.const_to_sym(c));
                }
                if let Some(type_name) = self.enum_variant_type.get(name).cloned() {
                    return Ok(self.enum_literal(&type_name, name));
                }
                self.var_at_time(name, time)
            }
            Expr::PrimedName(name) => self.var_at_time(name, time + 1),
            // ◯ φ at time t = φ at time t+1.  Bypass child encoding because the
            // shifted time must be applied before recursing.
            Expr::Unary {
                op: UnaryOp::Next,
                expr: inner,
            } => self.encode(inner, time + 1),
            Expr::Unary { op, expr } => {
                let inner = self.encode(expr, time)?;
                self.encode_unary(*op, inner)
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.encode(lhs, time)?;
                let r = self.encode(rhs, time)?;
                self.encode_binary(*op, l, r)
            }
        }
    }

    fn const_to_sym(&self, value: ConstValue) -> SymVal {
        match value {
            ConstValue::Bool(b) => SymVal::Bool(if b { self.true_lit } else { -self.true_lit }),
            ConstValue::Int(v) => SymVal::Int(vec![(v, self.true_lit)]),
            ConstValue::Enum { type_name, variant } => {
                let mut values = Vec::new();
                if let Some(ResolvedDomain::Enum { variants, .. }) =
                    self.find_enum_domain(&type_name)
                {
                    for v in variants {
                        values.push((
                            v.clone(),
                            if v == &variant {
                                self.true_lit
                            } else {
                                -self.true_lit
                            },
                        ));
                    }
                }
                SymVal::Enum { type_name, values }
            }
        }
    }

    fn find_enum_domain(&self, type_name: &str) -> Option<&ResolvedDomain> {
        self.domains
            .values()
            .find(|d| matches!(d, ResolvedDomain::Enum { type_name: t, .. } if t == type_name))
    }

    fn enum_literal(&self, type_name: &str, variant: &str) -> SymVal {
        if let Some(ResolvedDomain::Enum { variants, .. }) = self.find_enum_domain(type_name) {
            let values = variants
                .iter()
                .map(|v| {
                    (
                        v.clone(),
                        if v == variant {
                            self.true_lit
                        } else {
                            -self.true_lit
                        },
                    )
                })
                .collect();
            SymVal::Enum {
                type_name: type_name.to_string(),
                values,
            }
        } else {
            SymVal::Enum {
                type_name: type_name.to_string(),
                values: vec![(variant.to_string(), self.true_lit)],
            }
        }
    }

    fn encode_unary(&mut self, op: UnaryOp, inner: SymVal) -> Result<SymVal> {
        match op {
            UnaryOp::Not => {
                let lit = inner.as_bool().ok_or_else(|| CaelumError::Model {
                    message: format!(
                        "BMC encoder: `not` applied to non-bool ({})",
                        inner.kind_label()
                    ),
                })?;
                Ok(SymVal::Bool(-lit))
            }
            UnaryOp::Neg => {
                let values = inner.as_int().ok_or_else(|| CaelumError::Model {
                    message: format!(
                        "BMC encoder: unary `-` applied to non-int ({})",
                        inner.kind_label()
                    ),
                })?;
                let negated = values.iter().map(|(v, l)| (-v, *l)).collect();
                Ok(SymVal::Int(negated))
            }
            UnaryOp::Next => Err(CaelumError::Model {
                message: "BMC encoder: internal: Next should be handled in `encode`".into(),
            }),
            UnaryOp::Always | UnaryOp::Eventually => Err(CaelumError::Model {
                message: format!(
                    "BMC encoder: temporal operator `{}` only allowed at the top of a property",
                    match op {
                        UnaryOp::Always => "always",
                        UnaryOp::Eventually => "eventually",
                        _ => unreachable!(),
                    }
                ),
            }),
        }
    }

    fn encode_binary(&mut self, op: BinaryOp, lhs: SymVal, rhs: SymVal) -> Result<SymVal> {
        match op {
            BinaryOp::And => {
                let a = lhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("and lhs", &lhs))?;
                let b = rhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("and rhs", &rhs))?;
                Ok(SymVal::Bool(self.band(a, b)))
            }
            BinaryOp::Or => {
                let a = lhs.as_bool().ok_or_else(|| self.bool_err("or lhs", &lhs))?;
                let b = rhs.as_bool().ok_or_else(|| self.bool_err("or rhs", &rhs))?;
                Ok(SymVal::Bool(self.bor(a, b)))
            }
            BinaryOp::Implies => {
                let a = lhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("implication lhs", &lhs))?;
                let b = rhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("implication rhs", &rhs))?;
                Ok(SymVal::Bool(self.bor(-a, b)))
            }
            BinaryOp::Iff => {
                let a = lhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("equivalence lhs", &lhs))?;
                let b = rhs
                    .as_bool()
                    .ok_or_else(|| self.bool_err("equivalence rhs", &rhs))?;
                Ok(SymVal::Bool(self.biff(a, b)))
            }
            BinaryOp::Eq => self.encode_eq(lhs, rhs, true),
            BinaryOp::Ne => self.encode_eq(lhs, rhs, false),
            BinaryOp::Lt => self.encode_int_cmp(lhs, rhs, |a, b| a < b),
            BinaryOp::Le => self.encode_int_cmp(lhs, rhs, |a, b| a <= b),
            BinaryOp::Gt => self.encode_int_cmp(lhs, rhs, |a, b| a > b),
            BinaryOp::Ge => self.encode_int_cmp(lhs, rhs, |a, b| a >= b),
            BinaryOp::Add => self.encode_int_op(lhs, rhs, |a, b| a + b),
            BinaryOp::Sub => self.encode_int_op(lhs, rhs, |a, b| a - b),
            BinaryOp::Mul => self.encode_int_op(lhs, rhs, |a, b| a * b),
            BinaryOp::Div => self.encode_int_op_checked(lhs, rhs, "division", |a, b| {
                if b == 0 {
                    None
                } else {
                    Some(a / b)
                }
            }),
            BinaryOp::Mod => self.encode_int_op_checked(lhs, rhs, "modulo", |a, b| {
                if b == 0 {
                    None
                } else {
                    Some(a % b)
                }
            }),
            BinaryOp::Until => Err(CaelumError::Model {
                message: "BMC encoder: `until` operator is unsupported in BMC v1".into(),
            }),
        }
    }

    fn bool_err(&self, ctx: &str, v: &SymVal) -> CaelumError {
        CaelumError::Model {
            message: format!("BMC encoder: {ctx} expects bool, got {}", v.kind_label()),
        }
    }

    fn encode_eq(&mut self, lhs: SymVal, rhs: SymVal, equal: bool) -> Result<SymVal> {
        let lit = match (&lhs, &rhs) {
            (SymVal::Bool(a), SymVal::Bool(b)) => self.biff(*a, *b),
            (SymVal::Int(a), SymVal::Int(b)) => {
                let mut conjs = Vec::new();
                for (va, la) in a {
                    for (vb, lb) in b {
                        if va == vb {
                            conjs.push(self.band(*la, *lb));
                        }
                    }
                }
                self.bor_many(&conjs)
            }
            (SymVal::Enum { values: a, .. }, SymVal::Enum { values: b, .. }) => {
                let mut conjs = Vec::new();
                for (va, la) in a {
                    for (vb, lb) in b {
                        if va == vb {
                            conjs.push(self.band(*la, *lb));
                        }
                    }
                }
                self.bor_many(&conjs)
            }
            _ => {
                return Err(CaelumError::Model {
                    message: format!(
                        "BMC encoder: cannot compare {} and {}",
                        lhs.kind_label(),
                        rhs.kind_label()
                    ),
                })
            }
        };
        Ok(SymVal::Bool(if equal { lit } else { -lit }))
    }

    fn encode_int_cmp(
        &mut self,
        lhs: SymVal,
        rhs: SymVal,
        pred: impl Fn(i64, i64) -> bool,
    ) -> Result<SymVal> {
        let a = lhs.as_int().ok_or_else(|| CaelumError::Model {
            message: format!(
                "BMC encoder: int comparison lhs must be int, got {}",
                lhs.kind_label()
            ),
        })?;
        let b = rhs.as_int().ok_or_else(|| CaelumError::Model {
            message: format!(
                "BMC encoder: int comparison rhs must be int, got {}",
                rhs.kind_label()
            ),
        })?;
        let mut conjs = Vec::new();
        for (va, la) in a {
            for (vb, lb) in b {
                if pred(*va, *vb) {
                    conjs.push(self.band(*la, *lb));
                }
            }
        }
        Ok(SymVal::Bool(self.bor_many(&conjs)))
    }

    fn encode_int_op(
        &mut self,
        lhs: SymVal,
        rhs: SymVal,
        op: impl Fn(i64, i64) -> i64,
    ) -> Result<SymVal> {
        self.encode_int_op_checked(lhs, rhs, "arithmetic", |a, b| Some(op(a, b)))
    }

    fn encode_int_op_checked(
        &mut self,
        lhs: SymVal,
        rhs: SymVal,
        kind: &str,
        op: impl Fn(i64, i64) -> Option<i64>,
    ) -> Result<SymVal> {
        let a = lhs.as_int().ok_or_else(|| CaelumError::Model {
            message: format!(
                "BMC encoder: {kind} lhs must be int, got {}",
                lhs.kind_label()
            ),
        })?;
        let b = rhs.as_int().ok_or_else(|| CaelumError::Model {
            message: format!(
                "BMC encoder: {kind} rhs must be int, got {}",
                rhs.kind_label()
            ),
        })?;
        // For each pair of (va, la), (vb, lb), introduce a conj lit and group by result value.
        let mut groups: HashMap<i64, Vec<SatLit>> = HashMap::new();
        for (va, la) in a {
            for (vb, lb) in b {
                if let Some(result) = op(*va, *vb) {
                    let conj = self.band(*la, *lb);
                    groups.entry(result).or_default().push(conj);
                }
                // else: this combination is impossible (e.g. division by zero); skip.
            }
        }
        let mut pairs: Vec<(i64, SatLit)> = Vec::new();
        for (val, lits) in groups {
            let or_lit = self.bor_many(&lits);
            pairs.push((val, or_lit));
        }
        // Sort for stable output.
        pairs.sort_by_key(|(v, _)| *v);
        Ok(SymVal::Int(pairs))
    }

    pub fn lit_value(&self, lit: SatLit) -> bool {
        if lit > 0 {
            self.solver.model_value(lit)
        } else {
            !self.solver.model_value(-lit)
        }
    }

    pub fn solve(&mut self) -> Result<bool> {
        self.solver.solve()
    }

    pub fn read_state(&self, var: &str, t: usize) -> Option<crate::model::Value> {
        let symval = self.var_cache.get(&(var.to_string(), t))?;
        Some(match symval {
            SymVal::Bool(lit) => crate::model::Value::Bool(self.lit_value(*lit)),
            SymVal::Int(values) => {
                let v = values
                    .iter()
                    .find(|(_, l)| self.lit_value(*l))
                    .map(|(v, _)| *v)
                    .unwrap_or(0);
                crate::model::Value::Int(v)
            }
            SymVal::Enum { values, .. } => {
                let v = values
                    .iter()
                    .find(|(_, l)| self.lit_value(*l))
                    .map(|(v, _)| v.clone())
                    .unwrap_or_default();
                crate::model::Value::Enum(v)
            }
        })
    }
}
