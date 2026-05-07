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
    /// When true, attempt k-induction on safety properties that pass the
    /// base case so we can certify them as invariants rather than just
    /// "no counterexample within k steps".
    pub prove: bool,
}

impl Default for BmcOptions {
    fn default() -> Self {
        Self {
            depth: 50,
            prove: false,
        }
    }
}

#[derive(Debug)]
enum PropShape<'a> {
    /// `□ φ` with `φ` propositional plus optional `◯`.
    Always(&'a Expr),
    /// `□ ◇ φ` with non-temporal `φ`.
    AlwaysEventually(&'a Expr),
    /// `◇ □ φ` with non-temporal `φ` — eventually-stable.
    EventuallyAlways(&'a Expr),
    /// `◇ φ` with non-temporal `φ`.
    Eventually(&'a Expr),
    /// `φ U ψ` with both operands non-temporal.
    Until(&'a Expr, &'a Expr),
    /// `□ (P → ◇ Q)` with non-temporal `P`, `Q`.
    Response { trigger: &'a Expr, target: &'a Expr },
    /// `□ (P → □ Q)` with non-temporal `P`, `Q`.
    ResponseAlways { trigger: &'a Expr, target: &'a Expr },
    /// Non-temporal: just check at time 0.
    State(&'a Expr),
    /// Outside the supported grammar.
    Unsupported(String),
}

impl PropShape<'_> {
    fn needs_lasso(&self) -> bool {
        matches!(
            self,
            PropShape::AlwaysEventually(_)
                | PropShape::EventuallyAlways(_)
                | PropShape::Eventually(_)
                | PropShape::Until(_, _)
                | PropShape::Response { .. }
                | PropShape::ResponseAlways { .. }
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
            let body_lits = encode_per_step(encoder, body, last_observed)?;
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
        PropShape::EventuallyAlways(body) => {
            // `◇ □ body` violation: `□ ◇ ¬body` — every position has a later
            // ¬body.  On a lasso this is exactly "the loop body contains a
            // ¬body somewhere": ⋁_l (lc_l ∧ ⋁_{i ∈ [l, k]} ¬body(s_i)).
            let lc = lasso.expect("lasso required for EventuallyAlways");
            let body_lits = encode_per_step(encoder, body, last_observed)?;
            let mut disjuncts = Vec::with_capacity(last_observed + 1);
            for l in 0..=last_observed {
                let mut tail_negs = Vec::with_capacity(last_observed - l + 1);
                for t in l..=last_observed {
                    tail_negs.push(-body_lits[t]);
                }
                let any_neg = encoder.bor_many(&tail_negs);
                disjuncts.push(encoder.band(lc[l], any_neg));
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
        PropShape::Response { trigger, target } => {
            // Counterexample: ∃ t,l. P(s_t) ∧ lc_l ∧ ⋀_{i ∈ [t,k]} ¬Q(s_i)
            //                     ∧ ⋀_{i ∈ [l,k]} ¬Q(s_i).
            // Split into two independent disjuncts because t and l are
            // independently quantified once tail_q_fails is computed.
            let lc = lasso.expect("lasso required for Response");
            let p_lits = encode_per_step(encoder, trigger, last_observed)?;
            let q_lits = encode_per_step(encoder, target, last_observed)?;
            let tail_q_fails = build_tail_all_fail(encoder, &q_lits);
            // ⋁_t (P(s_t) ∧ tail_q_fails[t]) — some trigger exists with bad
            // tail in the prefix.
            let mut prefix_disjuncts = Vec::with_capacity(last_observed + 1);
            for t in 0..=last_observed {
                let conj = encoder.band(p_lits[t], tail_q_fails[t]);
                prefix_disjuncts.push(conj);
            }
            let prefix_arm = encoder.bor_many(&prefix_disjuncts);
            // ⋁_l (lc_l ∧ tail_q_fails[l]) — the loop body has no Q.
            let mut loop_disjuncts = Vec::with_capacity(last_observed + 1);
            for (l, &lc_l) in lc.iter().enumerate().take(last_observed + 1) {
                let conj = encoder.band(lc_l, tail_q_fails[l]);
                loop_disjuncts.push(conj);
            }
            let loop_arm = encoder.bor_many(&loop_disjuncts);
            // Both arms must hold.
            encoder.assert(prefix_arm);
            encoder.assert(loop_arm);
        }
        PropShape::ResponseAlways { trigger, target } => {
            // `□ (P → □ Q)` violation: ∃ t. P(s_t) ∧ ◇ ¬Q from t.
            // ◇ ¬Q from t = (∃ i ∈ [t,k]. ¬Q(s_i)) ∨ loop_q_fails_anywhere.
            let lc = lasso.expect("lasso required for ResponseAlways");
            let p_lits = encode_per_step(encoder, trigger, last_observed)?;
            let q_lits = encode_per_step(encoder, target, last_observed)?;
            let tail_q_fails_any = build_tail_any_fail(encoder, &q_lits);
            // loop_q_fails_anywhere = ⋁_l (lc_l ∧ tail_q_fails_any[l]).
            let mut loop_disjuncts = Vec::with_capacity(last_observed + 1);
            for (l, &lc_l) in lc.iter().enumerate().take(last_observed + 1) {
                let conj = encoder.band(lc_l, tail_q_fails_any[l]);
                loop_disjuncts.push(conj);
            }
            let loop_failure = encoder.bor_many(&loop_disjuncts);
            // ∃ t. P(s_t) ∧ (tail_q_fails_any[t] ∨ loop_failure).
            let mut disjuncts = Vec::with_capacity(last_observed + 1);
            for t in 0..=last_observed {
                let either = encoder.bor(tail_q_fails_any[t], loop_failure);
                disjuncts.push(encoder.band(p_lits[t], either));
            }
            let any = encoder.bor_many(&disjuncts);
            encoder.assert(any);
        }
        PropShape::Unsupported(_) => unreachable!("filtered earlier"),
    }
    Ok(())
}

/// Helper: encode `expr` at every t in `[0, last]` and return one bool lit per step.
fn encode_per_step(encoder: &mut Encoder<'_>, expr: &Expr, last: usize) -> Result<Vec<SatLit>> {
    let mut out = Vec::with_capacity(last + 1);
    for t in 0..=last {
        out.push(
            encoder
                .encode(expr, t)?
                .as_bool()
                .ok_or_else(|| CaelumError::Model {
                    message: "BMC: response sub-formula did not encode to a boolean".into(),
                })?,
        );
    }
    Ok(out)
}

/// `tail_all_fail[t] = ⋀_{i ∈ [t, k]} ¬q_lits[i]`.  Built right-to-left
/// using the chain `tail[t] = ¬q[t] ∧ tail[t+1]` so the encoder reuses a
/// linear number of fresh literals instead of `O(k²)`.
fn build_tail_all_fail(encoder: &mut Encoder<'_>, q_lits: &[SatLit]) -> Vec<SatLit> {
    let n = q_lits.len();
    let mut tail = vec![encoder.true_lit_value(); n + 1];
    // tail[n] = true (empty conjunction).
    for t in (0..n).rev() {
        tail[t] = encoder.band(-q_lits[t], tail[t + 1]);
    }
    tail.truncate(n);
    tail
}

/// `tail_any_fail[t] = ⋁_{i ∈ [t, k]} ¬q_lits[i]`.
fn build_tail_any_fail(encoder: &mut Encoder<'_>, q_lits: &[SatLit]) -> Vec<SatLit> {
    let n = q_lits.len();
    let mut tail = vec![-encoder.true_lit_value(); n + 1];
    // tail[n] = false (empty disjunction).
    for t in (0..n).rev() {
        tail[t] = encoder.bor(-q_lits[t], tail[t + 1]);
    }
    tail.truncate(n);
    tail
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
                    PropShape::Unsupported("BMC supports `□ ◇ φ` only with a non-temporal φ".into())
                };
            }
            // `□ (P → ◇ Q)` or `□ (P → □ Q)` — response patterns.
            if let Expr::Binary {
                op: BinaryOp::Implies,
                lhs: trigger,
                rhs,
            } = body.as_ref()
            {
                if let Expr::Unary {
                    op: UnaryOp::Eventually,
                    expr: target,
                } = rhs.as_ref()
                {
                    if is_state_formula(trigger) && is_state_formula(target) {
                        return PropShape::Response { trigger, target };
                    }
                }
                if let Expr::Unary {
                    op: UnaryOp::Always,
                    expr: target,
                } = rhs.as_ref()
                {
                    if is_state_formula(trigger) && is_state_formula(target) {
                        return PropShape::ResponseAlways { trigger, target };
                    }
                }
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
            // `◇ □ φ` — stabilisation pattern.
            if let Expr::Unary {
                op: UnaryOp::Always,
                expr: target,
            } = phi.as_ref()
            {
                if is_state_formula(target) {
                    return PropShape::EventuallyAlways(target);
                }
            }
            if is_state_formula(phi) {
                PropShape::Eventually(phi)
            } else {
                PropShape::Unsupported(
                    "BMC supports `◇ φ` only with a non-temporal φ (or `◇ □ φ` with non-temporal φ)"
                        .into(),
                )
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

/// Outcome of attempting k-induction on a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InductionOutcome {
    /// The inductive step was UNSAT — the property is k-inductive (an
    /// invariant for all reachable states).
    Holds,
    /// The inductive step found a counterexample at depth k. The property
    /// may still be invariant (and only fail to be k-inductive at this k),
    /// but BMC can no longer certify it without strengthening or larger k.
    Inconclusive,
    /// Property shape is not a candidate for k-induction (e.g. liveness,
    /// `Invalid` block, or `□ φ` with `◯` inside `φ`).
    NotApplicable,
}

/// Returns `true` iff the property is a candidate for k-induction —
/// kind = Property, shape = Always(body) with non-temporal body and no
/// `◯` inside.  This mirrors the soundness conditions of standard
/// k-induction with simple paths.
pub fn property_eligible_for_induction(property: &PropertyBlock) -> bool {
    if property.kind != PropertyKind::Property {
        return false;
    }
    let shape = classify(&property.expr);
    match shape {
        PropShape::Always(body) => max_next_depth(body).map(|d| d == 0).unwrap_or(false),
        _ => false,
    }
}

/// Run the inductive step of k-induction with simple paths.  The base
/// case (no counterexample within `k` steps from the initial states) must
/// have already passed; this function only checks that any `(k+1)`-step
/// simple path with `body` true at every state in `[0, k]` extends to a
/// state where `body` is also true.  Returns `InductionOutcome::Holds`
/// iff the SAT instance is UNSAT.
pub fn check_induction(
    spec: &BmcSpec<'_>,
    property: &PropertyBlock,
    options: &BmcOptions,
    solver: &mut dyn Solver,
) -> Result<InductionOutcome> {
    if !property_eligible_for_induction(property) {
        return Ok(InductionOutcome::NotApplicable);
    }
    let body = match classify(&property.expr) {
        PropShape::Always(b) => b,
        _ => return Ok(InductionOutcome::NotApplicable),
    };

    let k = options.depth;
    let last_state = k + 1; // states 0..=k+1, so k+2 states total

    let mut encoder = Encoder::new(
        solver,
        spec.constants.clone(),
        spec.enum_variant_type.clone(),
        spec.domains.clone(),
    );

    // Pre-allocate state vars at every relevant timestep.
    for t in 0..=last_state {
        for var in &spec.variables {
            encoder.var_at_time(&var.name, t)?;
        }
    }

    // Encode transitions for adjacent pairs in 0..k+1.
    if !spec.transition_blocks.is_empty() {
        for t in 0..last_state {
            let mut block_lits = Vec::new();
            for block in &spec.transition_blocks {
                let lit = encoder.encode(&block.expr, t)?.as_bool().ok_or_else(|| {
                    CaelumError::Model {
                        message: "BMC: transition block did not encode to a boolean".into(),
                    }
                })?;
                block_lits.push(lit);
            }
            let any = encoder.bor_many(&block_lits);
            encoder.assert(any);
        }
    }

    // Assume body holds at every state in [0, k].
    for t in 0..=k {
        let lit = encoder
            .encode(body, t)?
            .as_bool()
            .ok_or_else(|| CaelumError::Model {
                message: "BMC: property body did not encode to a boolean".into(),
            })?;
        encoder.assert(lit);
    }

    // Search for a state at k+1 where body is false.
    let last_lit =
        encoder
            .encode(body, last_state)?
            .as_bool()
            .ok_or_else(|| CaelumError::Model {
                message: "BMC: property body did not encode to a boolean".into(),
            })?;
    encoder.assert(-last_lit);

    // Simple-path constraint: pairwise inequality across [0, k+1].
    // Each pair contributes ⋁_v (s_i.v ≠ s_j.v).
    for i in 0..=last_state {
        for j in (i + 1)..=last_state {
            let mut diffs = Vec::with_capacity(spec.variables.len());
            for var in &spec.variables {
                let a = encoder.var_at_time(&var.name, i)?;
                let b = encoder.var_at_time(&var.name, j)?;
                let eq_lit = encoder.symval_equal(&a, &b)?;
                diffs.push(-eq_lit);
            }
            let differs = encoder.bor_many(&diffs);
            encoder.assert(differs);
        }
    }

    let sat = encoder.solve()?;
    Ok(if sat {
        InductionOutcome::Inconclusive
    } else {
        InductionOutcome::Holds
    })
}
