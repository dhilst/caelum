//! Symbolic-value abstraction used while encoding caelum expressions to CNF.
//!
//! `SymVal::Bool(lit)` represents a boolean expression: the literal is true
//! iff the expression is true. `SymVal::Int(values)` represents an integer
//! expression by a one-hot list `(int_value, lit)`; exactly one literal is
//! true under any model, and that literal's paired integer is the value.
//! `SymVal::Enum` is the same idea for enum domains, paired with the variant
//! name. The encoder maintains the one-hot-exactly-one invariant by
//! emitting at-most-one and at-least-one clauses where needed.

use super::solver::SatLit;

#[derive(Debug, Clone)]
pub enum SymVal {
    Bool(SatLit),
    Int(Vec<(i64, SatLit)>),
    Enum {
        type_name: String,
        values: Vec<(String, SatLit)>,
    },
}

impl SymVal {
    pub fn as_bool(&self) -> Option<SatLit> {
        match self {
            SymVal::Bool(lit) => Some(*lit),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<&[(i64, SatLit)]> {
        match self {
            SymVal::Int(values) => Some(values),
            _ => None,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            SymVal::Bool(_) => "bool",
            SymVal::Int(_) => "int",
            SymVal::Enum { .. } => "enum",
        }
    }
}
