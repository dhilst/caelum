//! Take a parsed/checked SourceFile and a property, build the BMC formula,
//! solve it, and produce a PropertyResult.
//!
//! v1 supports SAFETY ONLY: properties of the form `□ φ` where `φ` may
//! contain `◯` (next) but no `◇`, `until`, or further nested `□`.
//! Non-temporal `φ` is also accepted (checked at time 0). Anything else
//! returns CheckStatus::Skipped with a diagnostic note.

use crate::checker::{CheckStatus, Counterexample, PropertyResult};
use crate::diagnostics::{CaelumError, Result};
use crate::model::{State, Value};
use crate::syntax::{BinaryOp, Expr, PropertyBlock, PropertyKind, UnaryOp};

use super::encode::{Encoder, ResolvedDomain};
use super::setup::BmcSpec;
use super::solver::Solver;

#[derive(Debug, Clone, Copy)]
pub struct BmcOptions {
    pub depth: usize,
}

impl Default for BmcOptions {
    fn default() -> Self {
        Self { depth: 50 }
    }
}

#[derive(Debug)]
enum PropShape<'a> {
    /// Top-level `□ φ`; the body is φ.
    Always(&'a Expr),
    /// Non-temporal: just check at time 0.
    State(&'a Expr),
    /// Unsupported in BMC v1.
    Unsupported(String),
}

pub fn check_property_with_solver(
    spec: &BmcSpec<'_>,
    property: &PropertyBlock,
    options: &BmcOptions,
    solver: &mut dyn Solver,
) -> Result<PropertyResult> {
    let shape = classify(&property.expr);

    let body = match shape {
        PropShape::Always(b) => b,
        PropShape::State(b) => b,
        PropShape::Unsupported(message) => {
            return Ok(PropertyResult {
                name: property.name.clone(),
                kind: property.kind,
                status: CheckStatus::Skipped,
                counterexample: None,
                note: Some(message),
            });
        }
    };

    let next_depth = max_next_depth(body)?;
    let needs_unroll = matches!(shape, PropShape::Always(_));
    let max_t = if needs_unroll {
        if options.depth < next_depth {
            return Err(CaelumError::Model {
                message: format!(
                    "BMC depth {} less than property's required next-depth {}",
                    options.depth, next_depth
                ),
            });
        }
        options.depth - next_depth
    } else {
        0
    };

    let total_steps = max_t + next_depth;

    let mut encoder = Encoder::new(
        solver,
        spec.constants.clone(),
        spec.enum_variant_type.clone(),
        spec.domains.clone(),
    );

    // 1. Pre-allocate variables for every relevant timestep so cache is populated.
    for t in 0..=total_steps {
        for var in &spec.variables {
            encoder.var_at_time(&var.name, t)?;
        }
    }

    // 2. Encode init at time 0.
    for block in &spec.init_blocks {
        let lit = encoder
            .encode(&block.expr, 0)?
            .as_bool()
            .ok_or_else(|| CaelumError::Model {
                message: "BMC: init block did not encode to a boolean".into(),
            })?;
        encoder.assert(lit);
    }

    // 3. Encode transitions for each adjacent timestep pair.
    if !spec.transition_blocks.is_empty() {
        for t in 0..total_steps {
            let mut block_lits = Vec::new();
            for block in &spec.transition_blocks {
                let lit = encoder.encode(&block.expr, t)?.as_bool().ok_or_else(|| {
                    CaelumError::Model {
                        message: "BMC: transition block did not encode to a boolean".into(),
                    }
                })?;
                block_lits.push(lit);
            }
            // Disjunction of transition blocks (per spec section 12.5).
            let any = encoder.bor_many(&block_lits);
            encoder.assert(any);
        }
    }

    // 4. Encode property body at each timestep, collect violation lits.
    let mut violations = Vec::new();
    for t in 0..=max_t {
        let body_lit = encoder
            .encode(body, t)?
            .as_bool()
            .ok_or_else(|| CaelumError::Model {
                message: "BMC: property body did not encode to a boolean".into(),
            })?;
        violations.push(-body_lit);
    }
    let any_violation = encoder.bor_many(&violations);
    encoder.assert(any_violation);

    // 5. Solve.
    let sat = encoder.solve()?;

    if sat {
        let trace = decode_trace(spec, &encoder, total_steps);
        let counterexample = Some(Counterexample {
            states: trace,
            cycle_start: None,
        });

        let status = match property.kind {
            PropertyKind::Property => CheckStatus::Fail,
            PropertyKind::Invalid => CheckStatus::Pass,
        };
        Ok(PropertyResult {
            name: property.name.clone(),
            kind: property.kind,
            status,
            counterexample: if status == CheckStatus::Fail {
                counterexample
            } else {
                None
            },
            note: None,
        })
    } else {
        let status = match property.kind {
            PropertyKind::Property => CheckStatus::Pass,
            PropertyKind::Invalid => CheckStatus::Fail,
        };
        Ok(PropertyResult {
            name: property.name.clone(),
            kind: property.kind,
            status,
            counterexample: None,
            note: None,
        })
    }
}

fn classify(expr: &Expr) -> PropShape<'_> {
    if let Expr::Unary {
        op: UnaryOp::Always,
        expr: body,
    } = expr
    {
        if let Some(msg) = check_safety_body(body) {
            return PropShape::Unsupported(msg);
        }
        return PropShape::Always(body);
    }
    if let Some(msg) = check_safety_body(expr) {
        return PropShape::Unsupported(msg);
    }
    PropShape::State(expr)
}

fn check_safety_body(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Bool(_) | Expr::Int(_) | Expr::Name(_) | Expr::PrimedName(_) => None,
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Always => Some("nested `always` is unsupported by BMC v1".into()),
            UnaryOp::Eventually => Some("`eventually` is unsupported by BMC v1 (liveness)".into()),
            UnaryOp::Next | UnaryOp::Not | UnaryOp::Neg => check_safety_body(inner),
        },
        Expr::Binary { op, lhs, rhs } => match op {
            BinaryOp::Until => Some("`until` is unsupported by BMC v1 (liveness)".into()),
            _ => check_safety_body(lhs).or_else(|| check_safety_body(rhs)),
        },
    }
}

fn max_next_depth(expr: &Expr) -> Result<usize> {
    Ok(match expr {
        Expr::Bool(_) | Expr::Int(_) | Expr::Name(_) | Expr::PrimedName(_) => 0,
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Next => max_next_depth(inner)? + 1,
            UnaryOp::Always | UnaryOp::Eventually => {
                return Err(CaelumError::Model {
                    message: "BMC: temporal operator inside property body".into(),
                });
            }
            _ => max_next_depth(inner)?,
        },
        Expr::Binary { lhs, rhs, .. } => std::cmp::max(max_next_depth(lhs)?, max_next_depth(rhs)?),
    })
}

fn decode_trace(spec: &BmcSpec<'_>, encoder: &Encoder<'_>, total_steps: usize) -> Vec<State> {
    let mut states = Vec::with_capacity(total_steps + 1);
    for t in 0..=total_steps {
        let mut row = Vec::new();
        for var in &spec.variables {
            let value = encoder.read_state(&var.name, t).unwrap_or_else(|| {
                match spec.domains.get(&var.name).expect("domain") {
                    ResolvedDomain::Bool => Value::Bool(false),
                    ResolvedDomain::Int(values) => Value::Int(values.first().copied().unwrap_or(0)),
                    ResolvedDomain::Enum { variants, .. } => {
                        Value::Enum(variants.first().cloned().unwrap_or_default())
                    }
                }
            });
            row.push(value);
        }
        states.push(State { values: row });
    }
    states
}
