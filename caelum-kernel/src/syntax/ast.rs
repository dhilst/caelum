use serde::Serialize;

use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub module: Option<ModuleName>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleName {
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDecl {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    TypeDecl(TypeDecl),
    Const(ConstDecl),
    Var(VarDecl),
    Init(InitBlock),
    Transition(TransitionBlock),
    Property(PropertyBlock),
    Fairness(FairnessDecl),
}

/// A `fairness { ... }` block. Each constraint names a transition (a declared
/// base name before elaboration, a concrete instance name after) and how
/// strongly it must be scheduled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FairnessDecl {
    pub constraints: Vec<FairnessConstraint>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FairnessConstraint {
    pub strength: FairnessStrength,
    pub transition: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FairnessStrength {
    /// Justice: a continuously-enabled transition is eventually taken.
    Weak,
    /// Compassion: an infinitely-often-enabled transition is eventually taken.
    Strong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    pub name: String,
    pub domain: Domain,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub name: String,
    /// `Some` for an indexed declaration `let status[node ∈ Node] ∈ D`. The
    /// index parameter ranges over a finite domain; elaboration flattens the
    /// declaration into one scalar variable per index value.
    pub index: Option<TransitionParam>,
    pub domain: Domain,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitBlock {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionBlock {
    pub name: String,
    /// Formal parameters, each ranging over a finite domain. A parameterized
    /// transition is expanded during elaboration into one concrete transition
    /// per tuple in the Cartesian product of the parameter domains.
    pub params: Vec<TransitionParam>,
    pub expr: Expr,
    pub span: Span,
}

/// A named binding over a finite domain, shared by transition parameters and
/// indexed variable declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionParam {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertyKind {
    Property,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyBlock {
    pub kind: PropertyKind,
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Domain {
    Bool,
    IntRange {
        start: DomainBound,
        end: DomainBound,
    },
    Enum {
        variants: Vec<String>,
    },
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainBound {
    Int(i64),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    Name(String),
    PrimedName(String),
    /// An indexed reference such as `status[node]` or, when `primed`, its
    /// next-state form `status[node]'`. Elaboration substitutes the index and
    /// rewrites this into a scalar `Name`/`PrimedName`.
    Indexed {
        name: String,
        index: Box<Expr>,
        primed: bool,
    },
    /// `unchanged(x, y except idx, ...)` — sugar for a conjunction of
    /// `v' = v` frame conditions, eliminated during elaboration.
    Unchanged(Vec<UnchangedTarget>),
    /// `∀ x ∈ D: body` / `∃ x ∈ D: body` over a finite domain, expanded during
    /// elaboration into a conjunction / disjunction over the domain's elements.
    Quantifier {
        kind: QuantKind,
        var: String,
        domain: Domain,
        body: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// One argument of an `unchanged(...)` expression: a state variable, optionally
/// with `except idx` to preserve every index of an indexed variable except one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnchangedTarget {
    pub name: String,
    pub except: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantKind {
    Forall,
    Exists,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Always,
    Eventually,
    Next,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Implies,
    Iff,
    Until,
}

impl SourceFile {
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}
