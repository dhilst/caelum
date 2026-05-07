//! Take a parsed/checked SourceFile and a property, build the BMC formula,
//! solve it, and produce a PropertyResult.
//!
//! v2 supports both safety and (single-level) liveness:
//!   - `□ φ` with non-temporal `φ` (optionally containing `◯`)        — safety
//!   - non-temporal `φ`                                                — safety
//!   - `□ ◇ φ` with non-temporal `φ`                                   — recurrence
//!   - `◇ φ` with non-temporal `φ`                                     — reachability
//!   - `φ U ψ` with non-temporal `φ`, `ψ`                              — until
//!
//! Anything outside this grammar (including `◯` inside a liveness body or
//! more deeply nested temporal operators) returns `CheckStatus::Skipped`
//! with a diagnostic note rather than a wrong answer.
//!
//! Liveness uses the standard finite-state lasso encoding: an extra
//! wraparound state s_{k+1} is introduced together with one closure
//! literal per timestep, exactly one of which is true; that literal's
//! index identifies the loop start.

use crate::checker::{CheckStatus, Counterexample, PropertyResult};
use crate::diagnostics::{CaelumError, Result};
use crate::model::{State, Value};
use crate::syntax::{BinaryOp, Expr, PropertyBlock, PropertyKind, UnaryOp};

use super::encode::{Encoder, ResolvedDomain};
use super::setup::BmcSpec;
use super::solver::{SatLit, Solver};

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
    /// `□ φ` with `φ` propositional plus optional `◯`.
    Always(&'a Expr),
    /// `□ ◇ φ` with non-temporal `φ`.
    AlwaysEventually(&'a Expr),
    /// `◇ φ` with non-temporal `φ`.
    Eventually(&'a Expr),
    /// `φ U ψ` with both operands non-temporal.
    Until(&'a Expr, &'a Expr),
    /// Non-temporal: just check at time 0.
    State(&'a Expr),
    /// Outside the supported grammar.
    Unsupported(String),
}

impl PropShape<'_> {
    fn needs_lasso(&self) -> bool {
        matches!(
            self,
            PropShape::AlwaysEventually(_) | PropShape::Eventually(_) | PropShape::Until(_, _)
        )
    }
}

pub fn check_property_with_solver(
    spec: &BmcSpec<'_>,
    property: &PropertyBlock,
    options: &BmcOptions,
    solver: &mut dyn Solver,
) -> Result<PropertyResult> {
    let shape = classify(&property.expr);

    if let PropShape::Unsupported(message) = shape {
        return Ok(PropertyResult {
            name: property.name.clone(),
            kind: property.kind,
            status: CheckStatus::Skipped,
            counterexample: None,
            note: Some(message),
        });
    }

    let next_depth = max_next_depth_for_shape(&shape)?;
    let needs_lasso = shape.needs_lasso();
    let k = options.depth;

    if k < next_depth {
        return Err(CaelumError::Model {
            message: format!(
                "BMC depth {k} is less than property's required next-depth {next_depth}",
            ),
        });
    }

    // For safety, we explore states up to time `k`; the property body uses
    // states up to `k + next_depth` because of `◯`. For liveness, we need
    // one extra wraparound state beyond `k` for the loop closure.
    let last_observed = if needs_lasso { k } else { k - next_depth };
    let total_steps = if needs_lasso { k + 1 } else { k };

    let mut encoder = Encoder::new(
        solver,
        spec.constants.clone(),
        spec.enum_variant_type.clone(),
        spec.domains.clone(),
    );

    // Pre-allocate variables for every relevant timestep so the encoder
    // cache is populated and the solver-variable layout is deterministic.
    for t in 0..=total_steps {
        for var in &spec.variables {
            encoder.var_at_time(&var.name, t)?;
        }
    }

    encode_init(&mut encoder, spec)?;
    encode_transitions(&mut encoder, spec, total_steps)?;

    let lasso = if needs_lasso {
        Some(encode_lasso(&mut encoder, spec, k)?)
    } else {
        None
    };

    encode_violation(&mut encoder, spec, &shape, last_observed, lasso.as_ref())?;

    let sat = encoder.solve()?;

    if sat {
        let cycle_start = lasso.as_ref().and_then(|l| find_cycle_start(&encoder, l));
        // For lasso traces we keep the wraparound state too — the human
        // printer will mark `cycle starts at s{cycle_start}`.
        let trace_len = if needs_lasso { k + 1 } else { k + next_depth };
        let trace = decode_trace(spec, &encoder, trace_len);
        let counterexample = Counterexample {
            states: trace,
            cycle_start,
        };

        let status = match property.kind {
            PropertyKind::Property => CheckStatus::Fail,
            PropertyKind::Invalid => CheckStatus::Pass,
        };
        Ok(PropertyResult {
            name: property.name.clone(),
            kind: property.kind,
            status,
            counterexample: if status == CheckStatus::Fail {
                Some(counterexample)
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

fn encode_init(encoder: &mut Encoder<'_>, spec: &BmcSpec<'_>) -> Result<()> {
    for block in &spec.init_blocks {
        let lit = encoder
            .encode(&block.expr, 0)?
            .as_bool()
            .ok_or_else(|| CaelumError::Model {
                message: "BMC: init block did not encode to a boolean".into(),
            })?;
        encoder.assert(lit);
    }
    Ok(())
}

fn encode_transitions(
    encoder: &mut Encoder<'_>,
    spec: &BmcSpec<'_>,
    total_steps: usize,
) -> Result<()> {
    if spec.transition_blocks.is_empty() {
        return Ok(());
    }
    for t in 0..total_steps {
        let mut block_lits = Vec::new();
        for block in &spec.transition_blocks {
            let lit =
                encoder
                    .encode(&block.expr, t)?
                    .as_bool()
                    .ok_or_else(|| CaelumError::Model {
                        message: "BMC: transition block did not encode to a boolean".into(),
                    })?;
            block_lits.push(lit);
        }
        let any = encoder.bor_many(&block_lits);
        encoder.assert(any);
    }
    Ok(())
}

/// Allocate one closure literal per candidate loop start, constrain
/// exactly one to be true, and assert that whichever one is true forces
/// `s_{k+1} == s_l` (per-variable equality).  Returns the closure
/// literals indexed by `l`.
fn encode_lasso(encoder: &mut Encoder<'_>, spec: &BmcSpec<'_>, k: usize) -> Result<Vec<SatLit>> {
    let mut lc = Vec::with_capacity(k + 1);
    for _ in 0..=k {
        lc.push(encoder.solver_new_var());
    }
    // At least one closure must hold (lasso shape required for infinite trace).
    encoder.add_clause(&lc);
    // At most one — pairwise mutex.
    for i in 0..lc.len() {
        for j in (i + 1)..lc.len() {
            encoder.add_clause(&[-lc[i], -lc[j]]);
        }
    }
    // For each candidate loop start l: lc_l ⇒ s_{k+1} == s_l.
    for (l, &lc_l) in lc.iter().enumerate() {
        let mut equalities = Vec::with_capacity(spec.variables.len());
        for var in &spec.variables {
            let here = encoder.var_at_time(&var.name, k + 1)?;
            let target = encoder.var_at_time(&var.name, l)?;
            equalities.push(encoder.symval_equal(&here, &target)?);
        }
        let all_equal = encoder.band_many(&equalities);
        // lc_l → all_equal  ≡  ¬lc_l ∨ all_equal
        encoder.add_clause(&[-lc_l, all_equal]);
    }
    Ok(lc)
}

#[allow(clippy::needless_range_loop)]
fn encode_violation(
    encoder: &mut Encoder<'_>,
    _spec: &BmcSpec<'_>,
    shape: &PropShape<'_>,
    last_observed: usize,
    lasso: Option<&Vec<SatLit>>,
) -> Result<()> {
    match shape {
        PropShape::Always(body) => {
            // Counterexample exists iff ¬body holds at some t in [0, last_observed].
            let mut violations = Vec::with_capacity(last_observed + 1);
            for t in 0..=last_observed {
                let body_lit =
                    encoder
                        .encode(body, t)?
                        .as_bool()
                        .ok_or_else(|| CaelumError::Model {
                            message: "BMC: property body did not encode to a boolean".into(),
                        })?;
                violations.push(-body_lit);
            }
            let any_violation = encoder.bor_many(&violations);
            encoder.assert(any_violation);
        }
        PropShape::State(body) => {
            // Counterexample iff ¬body at time 0.
            let body_lit =
                encoder
                    .encode(body, 0)?
                    .as_bool()
                    .ok_or_else(|| CaelumError::Model {
                        message: "BMC: property body did not encode to a boolean".into(),
                    })?;
            encoder.assert(-body_lit);
        }
        PropShape::Eventually(body) => {
            // Counterexample (□ ¬body) iff ¬body at every t in [0, k].
            // The lasso constraint already forces a valid infinite trace.
            for t in 0..=last_observed {
                let body_lit =
                    encoder
                        .encode(body, t)?
                        .as_bool()
                        .ok_or_else(|| CaelumError::Model {
                            message: "BMC: property body did not encode to a boolean".into(),
                        })?;
                encoder.assert(-body_lit);
            }
        }
        PropShape::AlwaysEventually(body) => {
            // Counterexample (◇ □ ¬body) iff ∃ l. lc_l ∧ ⋀_{t ∈ [l, k]} ¬body(s_t).
            let lc = lasso.expect("lasso required for AlwaysEventually");
            let mut body_lits = Vec::with_capacity(last_observed + 1);
            for t in 0..=last_observed {
                let body_lit =
                    encoder
                        .encode(body, t)?
                        .as_bool()
                        .ok_or_else(|| CaelumError::Model {
                            message: "BMC: property body did not encode to a boolean".into(),
                        })?;
                body_lits.push(body_lit);
            }
            let mut disjuncts = Vec::with_capacity(last_observed + 1);
            for l in 0..=last_observed {
                let mut conj = vec![lc[l]];
                for t in l..=last_observed {
                    conj.push(-body_lits[t]);
                }
                disjuncts.push(encoder.band_many(&conj));
            }
            let any = encoder.bor_many(&disjuncts);
            encoder.assert(any);
        }
        PropShape::Until(lhs, rhs) => {
            // φ U ψ holds iff ⋁_{t ∈ [0, k]} (ψ(s_t) ∧ ⋀_{i < t} φ(s_i)).
            // Counterexample: ⋀_t (¬ψ(s_t) ∨ ⋁_{i < t} ¬φ(s_i)).
            let mut phi_lits = Vec::with_capacity(last_observed + 1);
            let mut psi_lits = Vec::with_capacity(last_observed + 1);
            for t in 0..=last_observed {
                phi_lits.push(encoder.encode(lhs, t)?.as_bool().ok_or_else(|| {
                    CaelumError::Model {
                        message: "BMC: until lhs did not encode to a boolean".into(),
                    }
                })?);
                psi_lits.push(encoder.encode(rhs, t)?.as_bool().ok_or_else(|| {
                    CaelumError::Model {
                        message: "BMC: until rhs did not encode to a boolean".into(),
                    }
                })?);
            }
            for t in 0..=last_observed {
                let mut clause = vec![-psi_lits[t]];
                for i in 0..t {
                    clause.push(-phi_lits[i]);
                }
                // ¬ψ(t) ∨ ⋁_{i<t} ¬φ(i) — emit as a single clause with bor.
                let any = encoder.bor_many(&clause);
                encoder.assert(any);
            }
        }
        PropShape::Unsupported(_) => unreachable!("filtered earlier"),
    }
    Ok(())
}

fn classify(expr: &Expr) -> PropShape<'_> {
    match expr {
        Expr::Unary {
            op: UnaryOp::Always,
            expr: body,
        } => {
            // `□ ◇ φ`?
            if let Expr::Unary {
                op: UnaryOp::Eventually,
                expr: phi,
            } = body.as_ref()
            {
                return if is_state_formula(phi) {
                    PropShape::AlwaysEventually(phi)
                } else {
                    PropShape::Unsupported(
                        "BMC v2 supports `□ ◇ φ` only with a non-temporal φ".into(),
                    )
                };
            }
            // `□ φ` with optional ◯.
            if let Some(msg) = check_safety_body(body) {
                return PropShape::Unsupported(msg);
            }
            PropShape::Always(body)
        }
        Expr::Unary {
            op: UnaryOp::Eventually,
            expr: phi,
        } => {
            if is_state_formula(phi) {
                PropShape::Eventually(phi)
            } else {
                PropShape::Unsupported("BMC v2 supports `◇ φ` only with a non-temporal φ".into())
            }
        }
        Expr::Binary {
            op: BinaryOp::Until,
            lhs,
            rhs,
        } => {
            if is_state_formula(lhs) && is_state_formula(rhs) {
                PropShape::Until(lhs, rhs)
            } else {
                PropShape::Unsupported(
                    "BMC v2 supports `φ U ψ` only with non-temporal operands".into(),
                )
            }
        }
        _ => {
            if let Some(msg) = check_safety_body(expr) {
                PropShape::Unsupported(msg)
            } else {
                PropShape::State(expr)
            }
        }
    }
}

/// True iff the formula contains no temporal operators at all.
fn is_state_formula(expr: &Expr) -> bool {
    match expr {
        Expr::Bool(_) | Expr::Int(_) | Expr::Name(_) | Expr::PrimedName(_) => true,
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Always | UnaryOp::Eventually | UnaryOp::Next => false,
            UnaryOp::Not | UnaryOp::Neg => is_state_formula(inner),
        },
        Expr::Binary { op, lhs, rhs } => match op {
            BinaryOp::Until => false,
            _ => is_state_formula(lhs) && is_state_formula(rhs),
        },
    }
}

fn check_safety_body(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Bool(_) | Expr::Int(_) | Expr::Name(_) | Expr::PrimedName(_) => None,
        Expr::Unary { op, expr: inner } => match op {
            UnaryOp::Always => Some("nested `always` is unsupported by BMC v2".into()),
            UnaryOp::Eventually => Some(
                "nested `eventually` is unsupported by BMC v2 (only `□ ◇ φ` and `◇ φ` are accepted)"
                    .into(),
            ),
            UnaryOp::Next | UnaryOp::Not | UnaryOp::Neg => check_safety_body(inner),
        },
        Expr::Binary { op, lhs, rhs } => match op {
            BinaryOp::Until => Some(
                "nested `until` is unsupported by BMC v2 (only top-level `φ U ψ` is accepted)"
                    .into(),
            ),
            _ => check_safety_body(lhs).or_else(|| check_safety_body(rhs)),
        },
    }
}

fn max_next_depth_for_shape(shape: &PropShape<'_>) -> Result<usize> {
    match shape {
        PropShape::Always(body) | PropShape::State(body) => max_next_depth(body),
        // Liveness shapes require non-temporal bodies (already checked in
        // classify), so depth is 0.
        _ => Ok(0),
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

fn find_cycle_start(encoder: &Encoder<'_>, lc: &[SatLit]) -> Option<usize> {
    lc.iter()
        .enumerate()
        .find_map(|(t, lit)| encoder.lit_value(*lit).then_some(t))
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
