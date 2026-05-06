use std::collections::HashMap;

use crate::diagnostics::{Result, TplError};
use crate::syntax::{BinaryOp, Expr, UnaryOp};

use super::state::{State, Value};

#[derive(Debug, Clone)]
pub struct EvalEnv {
    pub constants: HashMap<String, Value>,
    pub enum_values: HashMap<String, Value>,
    pub variables: HashMap<String, usize>,
}

impl EvalEnv {
    pub fn new(
        constants: HashMap<String, Value>,
        enum_values: HashMap<String, Value>,
        variables: HashMap<String, usize>,
    ) -> Self {
        Self {
            constants,
            enum_values,
            variables,
        }
    }
}

pub fn eval_expr(
    expr: &Expr,
    env: &EvalEnv,
    current: Option<&State>,
    next: Option<&State>,
) -> Result<Value> {
    match expr {
        Expr::Bool(value) => Ok(Value::Bool(*value)),
        Expr::Int(value) => Ok(Value::Int(*value)),
        Expr::Name(name) => eval_name(name, env, current),
        Expr::PrimedName(name) => eval_primed_name(name, env, next),
        Expr::Unary { op, expr } => {
            let value = eval_expr(expr, env, current, next)?;
            eval_unary(*op, value)
        }
        Expr::Binary { op, lhs, rhs } => {
            let lhs = eval_expr(lhs, env, current, next)?;
            let rhs = eval_expr(rhs, env, current, next)?;
            eval_binary(*op, lhs, rhs)
        }
    }
}

pub fn expect_bool(value: Value, context: &str) -> Result<bool> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(TplError::Model {
            message: format!("{context} evaluated to non-boolean value `{other}`"),
        }),
    }
}

pub fn expect_int(value: Value, context: &str) -> Result<i64> {
    match value {
        Value::Int(value) => Ok(value),
        other => Err(TplError::Model {
            message: format!("{context} evaluated to non-integer value `{other}`"),
        }),
    }
}

fn eval_name(name: &str, env: &EvalEnv, current: Option<&State>) -> Result<Value> {
    if let Some(value) = env.constants.get(name) {
        return Ok(value.clone());
    }
    if let Some(value) = env.enum_values.get(name) {
        return Ok(value.clone());
    }
    if let Some(index) = env.variables.get(name) {
        let state = current.ok_or_else(|| TplError::Model {
            message: format!("cannot read state variable `{name}` without current state"),
        })?;
        return Ok(state.values[*index].clone());
    }
    Err(TplError::Model {
        message: format!("unknown runtime name `{name}`"),
    })
}

fn eval_primed_name(name: &str, env: &EvalEnv, next: Option<&State>) -> Result<Value> {
    let index = env.variables.get(name).ok_or_else(|| TplError::Model {
        message: format!("unknown primed runtime name `{name}'`"),
    })?;
    let state = next.ok_or_else(|| TplError::Model {
        message: format!("cannot read primed variable `{name}'` without next state"),
    })?;
    Ok(state.values[*index].clone())
}

fn eval_unary(op: UnaryOp, value: Value) -> Result<Value> {
    match op {
        UnaryOp::Not => Ok(Value::Bool(!expect_bool(value, "negation")?)),
        UnaryOp::Neg => Ok(Value::Int(-expect_int(value, "unary minus")?)),
        UnaryOp::Always | UnaryOp::Eventually | UnaryOp::Next => Err(TplError::Model {
            message: "temporal operator cannot be evaluated during model construction".to_owned(),
        }),
    }
}

fn eval_binary(op: BinaryOp, lhs: Value, rhs: Value) -> Result<Value> {
    match op {
        BinaryOp::Add => Ok(Value::Int(
            expect_int(lhs, "addition lhs")? + expect_int(rhs, "addition rhs")?,
        )),
        BinaryOp::Sub => Ok(Value::Int(
            expect_int(lhs, "subtraction lhs")? - expect_int(rhs, "subtraction rhs")?,
        )),
        BinaryOp::Mul => Ok(Value::Int(
            expect_int(lhs, "multiplication lhs")? * expect_int(rhs, "multiplication rhs")?,
        )),
        BinaryOp::Div => {
            let lhs = expect_int(lhs, "division lhs")?;
            let rhs = expect_int(rhs, "division rhs")?;
            if rhs == 0 {
                return Err(TplError::Model {
                    message: "division by zero".to_owned(),
                });
            }
            Ok(Value::Int(lhs / rhs))
        }
        BinaryOp::Mod => {
            let lhs = expect_int(lhs, "modulo lhs")?;
            let rhs = expect_int(rhs, "modulo rhs")?;
            if rhs == 0 {
                return Err(TplError::Model {
                    message: "modulo by zero".to_owned(),
                });
            }
            Ok(Value::Int(lhs % rhs))
        }
        BinaryOp::Eq => Ok(Value::Bool(lhs == rhs)),
        BinaryOp::Ne => Ok(Value::Bool(lhs != rhs)),
        BinaryOp::Lt => Ok(Value::Bool(
            expect_int(lhs, "less-than lhs")? < expect_int(rhs, "less-than rhs")?,
        )),
        BinaryOp::Le => Ok(Value::Bool(
            expect_int(lhs, "less-or-equal lhs")? <= expect_int(rhs, "less-or-equal rhs")?,
        )),
        BinaryOp::Gt => Ok(Value::Bool(
            expect_int(lhs, "greater-than lhs")? > expect_int(rhs, "greater-than rhs")?,
        )),
        BinaryOp::Ge => Ok(Value::Bool(
            expect_int(lhs, "greater-or-equal lhs")? >= expect_int(rhs, "greater-or-equal rhs")?,
        )),
        BinaryOp::And => Ok(Value::Bool(
            expect_bool(lhs, "and lhs")? && expect_bool(rhs, "and rhs")?,
        )),
        BinaryOp::Or => Ok(Value::Bool(
            expect_bool(lhs, "or lhs")? || expect_bool(rhs, "or rhs")?,
        )),
        BinaryOp::Implies => Ok(Value::Bool(
            !expect_bool(lhs, "implication lhs")? || expect_bool(rhs, "implication rhs")?,
        )),
        BinaryOp::Iff => Ok(Value::Bool(
            expect_bool(lhs, "equivalence lhs")? == expect_bool(rhs, "equivalence rhs")?,
        )),
        BinaryOp::Until => Err(TplError::Model {
            message: "until cannot be evaluated during model construction".to_owned(),
        }),
    }
}
