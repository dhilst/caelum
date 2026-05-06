use serde::Serialize;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    pub name: String,
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitBlock {
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionBlock {
    pub name: String,
    pub expr: Expr,
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
