# Caelum Specification

## 1. Purpose

`caelum` is a command-line LTL model checker.
It reads one root specification file with the `.lum` extension, resolves any imports,
parses the combined specification, builds a normalized internal representation, and
checks whether the declared temporal properties hold for the transition system
described by the specification.

The language is intentionally small:

- It uses propositional logic plus temporal operators.
- It has no quantifiers.
- It models state using named variables.
- It models transitions using primed next-state variables, such as `x' = x + 1`.
- It supports modular specifications through imports.
- It is designed for deterministic CLI usage and machine-readable output.

The implementation language is Rust. Parsing is performed with `pest`, and the CLI
is implemented with `clap`.

## 2. Goals

The project must provide:

1. A `.lum` specification language for finite-state temporal propositional models.
2. A parser that accepts keyword, ASCII, and Unicode temporal operator spellings.
3. A normalized AST where temporal operators are stored in canonical keyword form.
4. Pretty printers that can emit keyword, ASCII, or Unicode operator syntax.
5. Import resolution across multiple `.lum` files.
6. Static validation for declarations, duplicate names, type errors, and malformed
   temporal formulas.
7. A model checker for temporal properties over finite transition systems.
8. Useful diagnostics with file, line, and column spans.
9. A CLI shaped primarily as:

   ```text
   caelum <spec>.lum
   ```

## 3. Non-Goals

The initial language does not support:

- First-order quantifiers.
- Functions over unbounded domains.
- Infinite integer domains.
- Real numbers.
- Probabilistic transitions.
- Timed automata clocks.
- Concurrent process syntax as a first-class construct.
- Fairness constraints in the first release.
- SMT solving in the first release.

These may be added later, but the first implementation should remain a finite-state
explicit model checker.

## 4. Terminology

| Term | Meaning |
| --- | --- |
| Specification | A root `.lum` file plus all imported `.lum` files. |
| Module | One `.lum` file after parsing. |
| State variable | A declared variable whose value is part of a system state. |
| Current-state expression | An expression over unprimed variables. |
| Next-state expression | An expression that may refer to primed variables. |
| Primed variable | A variable followed by `'`, referring to that variable's value in the successor state. |
| Transition relation | A formula constraining a pair of current and next states. |
| Initial predicate | A formula describing legal initial states. |
| Invariant | A formula expected to hold in every reachable state. |
| Temporal property | A formula using temporal operators such as `always`, `eventually`, `next`, or `until`. |
| Trace | A sequence of states generated from an initial state through transitions. |
| Counterexample | A concrete trace showing why a property is false. |

## 5. Source Files

### 5.1 File Extension

All source files must use the `.lum` extension.

The CLI must reject a root input file with any other extension unless a future
compatibility flag explicitly permits it.

### 5.2 Encoding

Source files must be valid UTF-8.

Unicode temporal operators are accepted:

- `□` for `always`
- `◇` for `eventually`
- `◯` for `next`
- `𝒰` for `until`

ASCII syntax is also accepted:

- `[]` for `always`
- `<>` for `eventually`
- `()` for `next`
- `U` for `until`

All accepted temporal spellings must normalize to the canonical operator identity
during parsing. The canonical source-level names are:

- `always`
- `eventually`
- `next`
- `until`

The internal AST must not preserve the spelling used in the source except through
optional span metadata or trivia used by diagnostics.

### 5.3 Line Endings

The parser must accept LF and CRLF line endings. Diagnostics should report line and
column positions after normalizing CRLF to a single logical newline.

### 5.4 Comments

The language supports line comments and block comments:

```tpl
// line comment

/*
   block comment
*/
```

Comments are ignored by the parser except for preserving source locations.

## 6. CLI Contract

### 6.1 Main Invocation

```text
caelum <spec>.lum
```

By default, this command:

1. Loads the root `.lum` file.
2. Resolves imports.
3. Parses all modules.
4. Performs semantic validation.
5. Builds the transition system.
6. Checks all declared properties.
7. Prints a human-readable report.
8. Exits with a status code representing success or failure.

### 6.2 Exit Codes

| Code | Meaning |
| ---: | --- |
| 0 | The specification parsed and all checks passed. |
| 1 | One or more properties failed. |
| 2 | Parse error. |
| 3 | Semantic validation error. |
| 4 | Import or file loading error. |
| 5 | Model construction error, such as infinite or unsupported domain. |
| 6 | Internal error. |

If multiple classes of errors occur, the earliest pipeline stage determines the exit
code. For example, parse errors are reported before semantic errors.

### 6.3 Required CLI Options

The initial CLI must support:

```text
caelum <spec>.lum
caelum check <spec>.lum
caelum parse <spec>.lum
caelum fmt <spec>.lum
```

`caelum <spec>.lum` is equivalent to `caelum check <spec>.lum`.

### 6.4 Printer Options

The parser normalizes all temporal operators to canonical internal operator kinds.
When printing formulas, the user can select an output syntax.

```text
--print-keywords
--print-ascii-operators
--print-unicode-operators
```

The default is:

```text
--print-unicode-operators
```

For compatibility with the requirement text, the misspelled flag
`--print-unicode-oeprators` may be accepted as a hidden deprecated alias, but all
documentation and help output should use the corrected spelling
`--print-unicode-operators`.

Printer behavior:

| Internal operator | Keyword output | ASCII output | Unicode output |
| --- | --- | --- | --- |
| Not | `not P` | `~ P` | `¬ P` |
| And | `P and Q` | `P /\ Q` | `P ∧ Q` |
| Or | `P or Q` | `P \/ Q` | `P ∨ Q` |
| Always | `always P` | `[] P` | `□ P` |
| Eventually | `eventually P` | `<> P` | `◇ P` |
| Next | `next P` | `() P` | `◯ P` |
| Until | `P until Q` | `P U Q` | `P 𝒰 Q` |
| Type membership | `x : X` | `x : X` | `x ∈ X` |

### 6.5 Output Format Options

The first release should support:

```text
--format human
--format json
```

The default is `human`.

JSON output should be stable enough for tests and editor integration.

### 6.6 Additional CLI Options

Recommended initial options:

```text
--max-states <N>
--max-depth <N>
--show-trace
--no-color
--color <auto|always|never>
--include-path <DIR>
--dump-ast
--dump-normalized
--dump-graph
```

Meanings:

- `--max-states <N>` limits explicit state exploration.
- `--max-depth <N>` limits bounded trace search where applicable.
- `--show-trace` prints counterexample traces.
- `--include-path <DIR>` adds a directory to the import search path.
- `--dump-ast` prints the parsed AST.
- `--dump-normalized` prints the normalized specification using the selected printer.
- `--dump-graph` prints the reachable transition graph in a simple text format.

## 7. Language Overview

A `.lum` file contains declarations and property checks.

Example:

```tpl
module counter

let x: 0..3

init {
  x = 0
}

transition inc {
  x' = (x + 1) mod 4
}

property wraps {
  always eventually x = 0
}
```

Equivalent temporal syntax examples:

```tpl
property keywords {
  always eventually x = 0
}

property ascii {
  [] <> x = 0
}

property unicode {
  □ ◇ x = 0
}
```

All three properties above must produce the same internal AST.

## 8. Module System

### 8.1 Module Declaration

Each file may declare a module:

```tpl
module my_module
```

If omitted, the module name is derived from the file stem.

Module names use identifiers separated by dots:

```tpl
module examples.counter
```

### 8.2 Imports

Imports appear at the top level:

```tpl
import "common.lum"
import "arith/ring.lum"
```

Imports are resolved relative to:

1. The importing file's directory.
2. Directories passed through `--include-path`, in order.

The root file's directory is always an implicit include path.

### 8.3 Import Rules

The import resolver must:

- Require imported files to have the `.lum` extension.
- Canonicalize filesystem paths before cycle detection.
- Load each canonical file at most once.
- Detect import cycles.
- Report the full import chain when a cycle or missing file occurs.

Example diagnostic:

```text
error[import-cycle]: import cycle detected
  --> specs/a.lum:3:8
   |
 3 | import "b.lum"
   |        ^^^^^^^
   |
   = import chain:
     specs/a.lum
     specs/b.lum
     specs/c.lum
     specs/a.lum
```

### 8.4 Visibility

The first release uses a simple global namespace after import resolution.

Rules:

- Top-level declarations from imported modules are visible to the importing module.
- Duplicate top-level names are errors unless they are the same module loaded through
  the same canonical path.
- Later releases may add qualified imports or explicit exports.

## 9. Lexical Structure

### 9.1 Identifiers

Identifiers start with an ASCII letter or `_`, followed by ASCII letters, digits, or
`_`.

```text
[A-Za-z_][A-Za-z0-9_]*
```

Reserved words cannot be used as ordinary identifiers.

### 9.2 Reserved Words

The following words are reserved:

```text
and
bool
const
else
eventually
false
if
import
init
int
mod
module
next
not
or
property
then
transition
true
until
let
always
```

The single uppercase token `U` is reserved as the ASCII `until` operator.

### 9.3 Literals

Supported literals:

```tpl
true
false
0
123
-12
```

Negative integer literals are parsed as unary negation applied to a positive literal
unless the grammar can support signed literals without ambiguity.

### 9.4 Strings

Strings are used only for imports in the first release.

```tpl
import "path/to/file.lum"
```

Strings support these escapes:

| Escape | Meaning |
| --- | --- |
| `\\` | Backslash |
| `\"` | Quote |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |

Import strings must not resolve outside normal filesystem access rules, but the
language itself does not sandbox paths.

## 10. Types and Domains

### 10.1 Supported Types

The first release supports finite domains only:

```tpl
bool
0..10
-3..3
enum { idle, busy, done }
```

`int` may be accepted only when bounded by an explicit domain annotation or a future
solver backend. For the first explicit-state model checker, unbounded `int` is a
semantic error.

### 10.2 Variable Declarations

```tpl
let flag: bool
let x: 0..3
let mode: enum { idle, busy, done }
```

The Unicode spelling may use the set-theoretic element-of symbol:

```tpl
let flag ∈ bool
let x ∈ 0..3
```

Both `:` and `∈` normalize to the same variable declaration AST. The default
Unicode printer emits `∈`; keyword and ASCII printer modes emit `:`.

Every state variable contributes to the state vector.

### 10.3 Constants

```tpl
const limit = 3
const enabled = true
```

Constants are compile-time values. They do not change across states and cannot be
primed.

### 10.4 Domain Finiteness

All state variables must have finite domains. The model checker must compute the
cartesian product of all domains conceptually, but it should generate reachable
states on demand instead of eagerly materializing the full state space when possible.

If the full domain size exceeds `--max-states`, the tool should fail before or during
exploration with a clear diagnostic unless the explored reachable set remains within
the configured limit.

## 11. Expressions

### 11.1 Expression Categories

The language has:

- Boolean expressions.
- Integer expressions over bounded integers.
- Enum value expressions.
- Temporal formulas.

A temporal formula is a boolean-valued formula that may include temporal operators.

### 11.1.1 Type Membership Syntax

The syntax:

```tpl
x : X
x ∈ X
```

is read as "`x` is of type/domain `X`" in declaration contexts. In the first
implementation this syntax is used for state variable declarations:

```tpl
let x : 0..3
let y ∈ bool
```

It is not an expression-level proposition in the first release. If expression-level
type tests are added later, they must not conflict with variable declarations.

### 11.2 Current and Next Variables

Current-state variable:

```tpl
x
```

Next-state variable:

```tpl
x'
```

Primed variables are legal only in transition formulas. They are illegal in:

- `init` blocks.
- `property` blocks.
- Constant declarations.
- Variable declarations.

Examples:

```tpl
transition step {
  x' = x + 1
}
```

```tpl
property bad {
  always x' = x
}
```

The second example is invalid because properties describe traces using temporal
operators, not direct primed next-state syntax.

### 11.3 Operators

The language supports:

| Category | Operators |
| --- | --- |
| Arithmetic | `+`, `-`, `*`, `/`, `mod` |
| Comparison | `=`, `!=`, `<`, `<=`, `>`, `>=` |
| Boolean | `not`, `~`, `¬`, `and`, `/\`, `∧`, `or`, `\/`, `∨`, `->`, `<->` |
| Temporal prefix | `always`, `eventually`, `next`, `[]`, `<>`, `()`, `□`, `◇`, `◯` |
| Temporal infix | `until`, `U`, `𝒰` |
| Grouping | `(`, `)` |

### 11.4 Equality

Equality is written with one equals sign:

```tpl
x = 0
```

Assignment is not a separate syntactic category. In a transition block,
`x' = expr` is a formula constraining the next value of `x`.

### 11.5 Operator Precedence

From highest precedence to lowest:

| Precedence | Operators | Associativity |
| ---: | --- | --- |
| 1 | grouping, literals, identifiers, primed identifiers | n/a |
| 2 | unary `not`, `~`, `¬`, unary `-`, temporal prefix operators | right |
| 3 | `*`, `/`, `mod` | left |
| 4 | `+`, `-` | left |
| 5 | `<`, `<=`, `>`, `>=`, `=`, `!=` | non-associative |
| 6 | `and`, `/\`, `∧` | left |
| 7 | `or`, `\/`, `∨` | left |
| 8 | `until`, `U`, `𝒰` | right |
| 9 | `->` | right |
| 10 | `<->` | left |

Temporal `until` binds weaker than `and` and `or` so that:

```tpl
a and b until c
```

parses as:

```tpl
(a and b) until c
```

Users should use parentheses when mixing temporal and boolean connectives in
non-obvious formulas.

### 11.6 Canonical Classical Logic Operators

The parser must normalize all classical propositional operator spellings to the same
AST operators:

| Source | Canonical AST |
| --- | --- |
| `not p` | `Not(p)` |
| `~ p` | `Not(p)` |
| `¬ p` | `Not(p)` |
| `p and q` | `And(p, q)` |
| `p /\ q` | `And(p, q)` |
| `p ∧ q` | `And(p, q)` |
| `p or q` | `Or(p, q)` |
| `p \/ q` | `Or(p, q)` |
| `p ∨ q` | `Or(p, q)` |

### 11.7 Canonical Temporal Operators

The parser must normalize all temporal spellings to one of these AST variants:

```rust
TemporalOp::Always
TemporalOp::Eventually
TemporalOp::Next
TemporalOp::Until
```

Examples:

| Source | Canonical AST |
| --- | --- |
| `always p` | `Always(p)` |
| `[] p` | `Always(p)` |
| `□ p` | `Always(p)` |
| `eventually p` | `Eventually(p)` |
| `<> p` | `Eventually(p)` |
| `◇ p` | `Eventually(p)` |
| `next p` | `Next(p)` |
| `() p` | `Next(p)` |
| `◯ p` | `Next(p)` |
| `p until q` | `Until(p, q)` |
| `p U q` | `Until(p, q)` |
| `p 𝒰 q` | `Until(p, q)` |

## 12. Top-Level Declarations

### 12.1 File Grammar Sketch

The high-level structure is:

```text
file        = module_decl? import_decl* item*
item        = const_decl | var_decl | init_block | transition_block | property_block
```

### 12.2 Constants

```tpl
const name = expression
```

Constants must be acyclic. A constant may refer only to constants declared earlier
in import order or in the same module.

### 12.3 Variables

```tpl
let name: domain
```

Variable names must be unique across the resolved specification.

### 12.4 Init Blocks

```tpl
init {
  expression
}
```

Multiple `init` blocks are allowed. Their formulas are conjoined.

Equivalent:

```tpl
init {
  x = 0
}

init {
  y = false
}
```

and:

```tpl
init {
  x = 0 and y = false
}
```

### 12.5 Transition Blocks

```tpl
transition name {
  expression
}
```

Multiple transition blocks are allowed. The default composition is disjunction:
a valid transition may satisfy any one named transition block.

Example:

```tpl
transition inc {
  x' = x + 1
}

transition reset {
  x' = 0
}
```

This means the system may either increment or reset.

If users need conjunctive transition constraints shared by every transition, they
should write:

```tpl
transition step {
  (x' = x + 1 or x' = 0) and y' = y
}
```

Later versions may add explicit `action` and `constraint` declarations.

### 12.6 Unchanged Variables

The first release requires every transition relation to determine or constrain all
next-state variables explicitly enough for state generation.

Recommended shorthand for future support:

```tpl
unchanged x
unchanged { x, y, z }
```

This shorthand is not required in the first release unless implemented deliberately.
Without it, users write:

```tpl
x' = x
```

### 12.7 Property Blocks

```tpl
property name {
  temporal_formula
}
```

Property names must be unique.

Properties are checked independently. A failed property should not prevent checking
later properties unless the checker cannot continue due to resource limits or an
internal error.

## 13. Suggested Formal Grammar

The exact `pest` grammar may differ, but it should follow this shape.

```pest
WHITESPACE = _{ " " | "\t" | NEWLINE }
COMMENT    = _{ line_comment | block_comment }

line_comment  = _{ "//" ~ (!NEWLINE ~ ANY)* }
block_comment = _{ "/*" ~ (!"*/" ~ ANY)* ~ "*/" }

file = { SOI ~ module_decl? ~ import_decl* ~ item* ~ EOI }

module_decl = { "module" ~ module_name }
module_name = { ident ~ ("." ~ ident)* }

import_decl = { "import" ~ string_lit }

item = _{
    const_decl
  | var_decl
  | init_block
  | transition_block
  | property_block
}

const_decl = { "const" ~ ident ~ "=" ~ expr }
var_decl   = { "let" ~ ident ~ type_sep ~ domain }
type_sep   = { ":" | "∈" }

domain = _{
    "bool"
  | int_range
  | enum_domain
}

int_range   = { int_lit ~ ".." ~ int_lit }
enum_domain = { "enum" ~ "{" ~ ident ~ ("," ~ ident)* ~ ","? ~ "}" }

init_block       = { "init" ~ block_expr }
transition_block = { "transition" ~ ident ~ block_expr }
property_block   = { "property" ~ ident ~ block_expr }

block_expr = { "{" ~ expr ~ "}" }

expr = { equivalence }

equivalence = { implication ~ ("<->" ~ implication)* }
implication = { until_expr ~ ("->" ~ implication)? }
until_expr  = { or_expr ~ (until_op ~ until_expr)? }
or_expr     = { and_expr ~ (or_op ~ and_expr)* }
and_expr    = { comparison ~ (and_op ~ comparison)* }
comparison  = { additive ~ (comp_op ~ additive)? }
additive    = { multiplicative ~ (("+" | "-") ~ multiplicative)* }
multiplicative = { unary ~ (("*" | "/" | "mod") ~ unary)* }
unary       = { unary_op* ~ primary }
primary     = { literal | primed_ident | ident | "(" ~ expr ~ ")" }

unary_op    = { not_op | "-" | always_op | eventually_op | next_op }
not_op      = { "not" | "~" | "¬" }
or_op       = { "or" | "\\/" | "∨" }
and_op      = { "and" | "/\\" | "∧" }
always_op   = { "always" | "[]" | "□" }
eventually_op = { "eventually" | "<>" | "◇" }
next_op     = { "next" | "()" | "◯" }
until_op    = { "until" | "U" | "𝒰" }

comp_op = { "=" | "!=" | "<=" | "<" | ">=" | ">" }

primed_ident = { ident ~ "'" }
ident        = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
int_lit      = @{ "-"? ~ ASCII_DIGIT+ }
literal      = { "true" | "false" | int_lit }
string_lit   = @{ "\"" ~ string_char* ~ "\"" }
string_char  = { "\\\"" | "\\\\" | "\\n" | "\\r" | "\\t" | (!"\"" ~ ANY) }
```

Notes:

- `COMMENT` should be included in the grammar's silent whitespace handling.
- Keywords must be rejected as identifiers during semantic token validation if pest
  does not enforce that directly.
- The grammar should avoid ambiguity between grouping `(` `)` and ASCII `next` `()`.
  `()` is a zero-width-looking two-character operator followed by an expression, so
  `() p` means `next p`, while `(p)` means grouping.

## 14. AST Model

The Rust AST should separate parsed syntax from checked semantic objects.

### 14.1 Parsed AST

Suggested parsed AST:

```rust
pub struct SourceFile {
    pub path: PathBuf,
    pub module: Option<ModuleName>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

pub enum Item {
    Const(ConstDecl),
    Var(VarDecl),
    Init(InitBlock),
    Transition(TransitionBlock),
    Property(PropertyBlock),
}

pub enum Expr {
    Bool(bool),
    Int(i64),
    Name(NameRef),
    PrimedName(NameRef),
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

pub enum UnaryOp {
    Not,
    Neg,
    Always,
    Eventually,
    Next,
}

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
```

Every AST node that can produce diagnostics should carry a source span. This may be
done by wrapping nodes in a `Spanned<T>` type.

### 14.2 Checked Model

After semantic analysis, produce a checked model:

```rust
pub struct Model {
    pub variables: Vec<Variable>,
    pub constants: Vec<Constant>,
    pub init: CheckedExpr,
    pub transitions: Vec<Transition>,
    pub properties: Vec<Property>,
}
```

The checked model should resolve names to stable IDs instead of strings.

```rust
pub struct VariableId(pub usize);
pub struct ConstantId(pub usize);
pub struct PropertyId(pub usize);
```

This allows evaluation to avoid repeated hash lookups.

## 15. Semantic Validation

Semantic analysis must reject:

- Unknown identifiers.
- Duplicate variable, constant, transition, or property names.
- Use of reserved words as identifiers.
- Use of primed variables outside transitions.
- Use of unprimed next-state-only values where not allowed.
- Constants that depend on variables.
- Cyclic constant definitions.
- Type mismatches.
- Arithmetic over booleans.
- Boolean operators over integers or enum values.
- Comparisons between incompatible types.
- Temporal operators over non-boolean expressions.
- Unbounded state variables.
- Empty domains.
- Division or modulo by statically known zero.
- Imported modules with duplicate conflicting declarations.

### 15.1 Formula Placement Rules

| Location | Current variables | Primed variables | Temporal operators |
| --- | --- | --- | --- |
| `const` | No | No | No |
| `init` | Yes | No | No |
| `transition` | Yes | Yes | No |
| `property` | Yes | No | Yes |

Temporal operators are not allowed in `transition` formulas in the first release.
Transitions describe the next-state relation directly through primed variables.

### 15.2 Transition Completeness

For explicit state generation, the checker must be able to enumerate all successor
states from a current state.

An implementation may use either:

1. Brute-force enumeration of all possible next states and filtering by the
   transition relation.
2. A more direct assignment extraction strategy for formulas shaped like
   `x' = expr`.

The first release should prefer correctness through brute-force filtering because
it handles arbitrary finite formulas. Optimizations can be added later.

### 15.3 Deadlocks

A state is deadlocked if it is reachable and has no successor.

Default behavior:

- Deadlocks are model errors unless explicitly allowed by a future flag.
- The checker should report a trace to the deadlocked state.

Rationale: Standard LTL semantics are usually defined over infinite traces. Treating
deadlock as an error avoids silently inventing stuttering behavior.

Future option:

```text
--deadlock <error|stutter>
```

Where `stutter` treats a deadlocked state as having a self-loop.

## 16. Evaluation Semantics

### 16.1 States

A state is a total assignment from state variables to values.

```rust
pub struct State {
    pub values: Vec<Value>,
}

pub enum Value {
    Bool(bool),
    Int(i64),
    Enum(EnumValueId),
}
```

### 16.2 Initial States

A state is initial if it satisfies the conjunction of all `init` formulas.

If no `init` block exists, all states in the finite domain product are initial.
The tool should warn because this is often accidental.

### 16.3 Successor States

Given current state `s` and candidate next state `t`, a transition formula is
evaluated as follows:

- Unprimed variable `x` reads from `s`.
- Primed variable `x'` reads from `t`.
- Constants read from the constant environment.

`t` is a successor of `s` if any transition block evaluates to `true`.

If no transition block exists, the model has no successors and therefore every
initial state is deadlocked.

### 16.4 Arithmetic

Arithmetic uses signed 64-bit integers for the first release.

During state-space construction, all variable values must remain inside their
declared finite domains. A transition that computes a value outside a variable's
domain simply does not match any candidate next state unless it is written as a
relation that can still be satisfied.

Example:

```tpl
let x: 0..3

transition inc {
  x' = x + 1
}
```

At `x = 3`, there is no successor through `inc` because `4` is outside `0..3`.

Users can define wraparound explicitly:

```tpl
transition inc {
  x' = (x + 1) mod 4
}
```

### 16.5 Boolean Logic

The propositional fragment uses classical two-valued boolean logic.

Truth tables:

| Expression | Meaning |
| --- | --- |
| `not P`, `~ P`, `¬ P` | true iff `P` is false |
| `P and Q`, `P /\ Q`, `P ∧ Q` | true iff both are true |
| `P or Q`, `P \/ Q`, `P ∨ Q` | true iff at least one is true |
| `P -> Q` | equivalent to `not P or Q` |
| `P <-> Q` | true iff `P` and `Q` have the same truth value |

## 17. Temporal Semantics

Temporal properties are interpreted over infinite traces:

```text
s0, s1, s2, ...
```

Each adjacent pair must satisfy the transition relation.

For a trace `π` and position `i`:

| Formula | Semantics |
| --- | --- |
| `always P` | `P` holds at every position `j >= i`. |
| `eventually P` | There exists a position `j >= i` where `P` holds. |
| `next P` | `P` holds at position `i + 1`. |
| `P until Q` | There exists `j >= i` where `Q` holds, and for every `k` with `i <= k < j`, `P` holds. |

Dualities that should hold:

```text
eventually P == true until P
always P == not eventually not P
```

These dualities may be used internally for model checking.

### 17.1 Path Quantification

Properties are universally quantified over all traces from all initial states.

```tpl
property safe {
  always x >= 0
}
```

Means:

```text
For every initial state and every possible trace from that state,
always x >= 0 holds.
```

A property fails if there exists at least one valid trace that violates it.

### 17.2 Branching vs Linear Time

The property language is linear-time temporal logic over paths, not CTL.

There are no explicit path quantifiers such as `A`, `E`, `forall paths`, or
`exists path`.

The checker's top-level property interpretation is universal across all paths.

## 18. Model Checking Strategy

### 18.1 Initial Implementation

The first implementation should:

1. Build the reachable transition graph from all initial states.
2. Reject deadlocks unless deadlock stuttering is explicitly implemented.
3. Translate each LTL property into a negated automaton or use a direct graph
   algorithm for the supported operators.
4. Search for an accepting cycle representing a counterexample.
5. Print pass/fail results per property.

For a small first release, a direct algorithm over the closure of subformulas is
acceptable if it is well-tested. A standard automata-based approach is preferable
for long-term correctness.

### 18.2 Reachability

Reachability algorithm:

```text
worklist = all initial states
visited = empty set

while worklist is not empty:
    s = pop(worklist)
    if s in visited:
        continue
    add s to visited
    successors = enumerate successors of s
    for each t in successors:
        add edge s -> t
        if t not in visited:
            push t
```

The checker must enforce `--max-states` during this process.

### 18.3 Invariant Shortcut

Properties of the form:

```tpl
always P
```

Where `P` is a non-temporal state formula may be checked as a reachable-state
invariant:

```text
for each reachable state s:
    if P(s) is false:
        report path to s
```

This shortcut is optional but recommended.

### 18.4 Eventually Checks

For:

```tpl
eventually P
```

The universal property fails if there exists a reachable cycle from an initial state
where `P` never holds.

The checker can detect this by finding a reachable strongly connected component
with at least one cycle and no state satisfying `P`.

### 18.5 Until Checks

For:

```tpl
P until Q
```

The property fails if there exists a trace where:

- `Q` never occurs, and
- `P` holds until the trace reaches a position where `P` is false before `Q`, or
- the trace remains forever in a cycle where `P` holds and `Q` never holds.

A robust implementation should translate `not (P until Q)` or use recursive LTL
model checking over the reachable graph.

### 18.6 Nested Temporal Formulas

Nested formulas must be supported:

```tpl
always (request -> eventually grant)
always (request -> next (request until grant))
```

The checker must not special-case only top-level `always`, `eventually`, `next`, and
`until` if nested formulas are accepted by the parser.

If the first implementation only supports a restricted model-checking fragment, the
parser may still accept the full grammar, but semantic validation must produce a
clear unsupported-feature diagnostic instead of silently checking incorrectly.

## 19. Counterexamples

When a property fails, the tool should print a counterexample trace.

For finite graph LTL checks, counterexamples usually have lasso shape:

```text
prefix: s0 -> s1 -> ... -> sm
cycle:  sm -> ... -> sn -> sm
```

Human output example:

```text
FAIL property eventually_zero

counterexample:
  s0: x = 1
  s1: x = 2
  s2: x = 3
  cycle starts at s1
```

JSON output should include:

```json
{
  "property": "eventually_zero",
  "result": "fail",
  "counterexample": {
    "states": [
      { "x": 1 },
      { "x": 2 },
      { "x": 3 }
    ],
    "cycle_start": 1
  }
}
```

## 20. Diagnostics

Diagnostics must be precise and actionable.

Every parse and semantic error should include:

- Severity.
- Error code.
- Message.
- File path.
- Line and column.
- Source excerpt.
- Optional help text.

Example:

```text
error[type-mismatch]: expected boolean expression
  --> specs/counter.lum:12:10
   |
12 | property p { always (x + 1) }
   |                      ^^^^^ expected bool, found int
   |
   = help: comparisons such as `x + 1 = 0` produce boolean formulas
```

Recommended crates:

- `miette`
- `thiserror`
- `ariadne`

Only one diagnostic rendering crate should be selected for the implementation.

## 21. Formatting and Printing

### 21.1 Normalized Printing

The formatter prints a normalized version of the specification.

Input:

```tpl
property p {
  [] <> x = 0
}
```

Default output:

```tpl
property p {
  □ ◇ x = 0
}
```

With `--print-keywords`:

```tpl
property p {
  always eventually x = 0
}
```

With `--print-ascii-operators`:

```tpl
property p {
  [] <> x = 0
}
```

### 21.2 Parentheses

The printer must preserve meaning by inserting parentheses where required by
precedence.

Example:

```tpl
always (p or q)
```

Must not print as:

```tpl
always p or q
```

Unless that expression has identical parse semantics under the defined precedence.

### 21.3 Stable Formatting

Formatting should be idempotent:

```text
caelum fmt spec.lum > formatted.lum
caelum fmt formatted.lum > formatted2.lum
diff formatted.lum formatted2.lum
```

Should produce no differences.

## 22. JSON Report Schema

The exact schema may evolve before stabilization, but the initial shape should be:

```json
{
  "tool": "caelum",
  "status": "pass",
  "root": "specs/counter.lum",
  "stats": {
    "modules": 1,
    "variables": 1,
    "states": 4,
    "transitions": 4,
    "properties": 2
  },
  "properties": [
    {
      "name": "wraps",
      "status": "pass"
    }
  ],
  "diagnostics": []
}
```

On failure:

```json
{
  "tool": "caelum",
  "status": "fail",
  "properties": [
    {
      "name": "bad",
      "status": "fail",
      "counterexample": {
        "states": [
          { "x": 1 },
          { "x": 2 }
        ],
        "cycle_start": 0
      }
    }
  ],
  "diagnostics": []
}
```

## 23. Rust Project Structure

Recommended crate layout:

```text
caelum/
  Cargo.toml
  docs/
    SPEC.md
  src/
    main.rs
    cli.rs
    lib.rs
    syntax/
      mod.rs
      ast.rs
      grammar.pest
      parser.rs
      printer.rs
    loader/
      mod.rs
      imports.rs
    sema/
      mod.rs
      names.rs
      types.rs
      check.rs
    model/
      mod.rs
      state.rs
      eval.rs
      graph.rs
    checker/
      mod.rs
      ltl.rs
      invariant.rs
      counterexample.rs
    diagnostics/
      mod.rs
```

### 23.1 Crate Responsibilities

| Module | Responsibility |
| --- | --- |
| `cli` | Clap argument definitions and command dispatch. |
| `syntax` | Pest grammar, parser, AST, and pretty printer. |
| `loader` | File loading, UTF-8 validation, import resolution, cycle detection. |
| `sema` | Name resolution, type checking, formula placement rules. |
| `model` | State representation, expression evaluation, transition graph construction. |
| `checker` | Temporal property verification and counterexample generation. |
| `diagnostics` | Shared error types and source reporting helpers. |

### 23.2 Suggested Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
pest = "2"
pest_derive = "2"
thiserror = "2"
miette = { version = "7", features = ["fancy"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = "2"
```

Actual versions should be selected when the project is initialized.

## 24. Testing Requirements

### 24.1 Parser Tests

Parser tests must cover:

- Keyword temporal operators.
- ASCII temporal operators.
- Unicode temporal operators.
- Normalization to identical AST nodes.
- Operator precedence.
- Primed variables.
- Import declarations.
- Comments.
- Invalid syntax with useful errors.

### 24.2 Semantic Tests

Semantic tests must cover:

- Unknown names.
- Duplicate declarations.
- Type mismatches.
- Primed variables outside transitions.
- Temporal operators inside transitions.
- Unbounded domains.
- Empty ranges.
- Import cycles.

### 24.3 Model Checker Tests

Checker tests must cover:

- Passing invariants.
- Failing invariants with counterexamples.
- Passing eventuality properties.
- Failing eventuality properties due to cycles.
- `next` properties.
- `until` properties.
- Nested temporal formulas.
- Deadlock detection.
- Resource limit handling.

### 24.4 Golden Tests

Golden tests should validate:

- Human diagnostics.
- JSON output.
- Normalized formatter output for keyword, ASCII, and Unicode printer modes.

## 25. Example Specifications

### 25.1 Counter

```tpl
module examples.counter

let x: 0..3

init {
  x = 0
}

transition inc {
  x' = (x + 1) mod 4
}

property always_in_range {
  always (x >= 0 and x <= 3)
}

property returns_to_zero {
  always eventually x = 0
}
```

### 25.2 Request Grant

```tpl
module examples.request_grant

let request: bool
let grant: bool

init {
  not request and not grant
}

transition idle {
  request' = true and grant' = false
}

transition serve {
  request' = false and grant' = true
}

transition clear {
  request' = false and grant' = false
}

property every_request_gets_grant {
  always (request -> eventually grant)
}
```

### 25.3 Import

`common.lum`:

```tpl
module common

const max = 3
```

`main.lum`:

```tpl
module main

import "common.lum"

let x: 0..max

init {
  x = 0
}

transition step {
  x' = (x + 1) mod (max + 1)
}

property wraps {
  □ ◇ x = 0
}
```

## 26. Implementation Milestones

### Milestone 1: Project Skeleton

- Create Rust crate.
- Add `clap` CLI with `check`, `parse`, and `fmt`.
- Add baseline diagnostics.
- Add CI-friendly test command.

### Milestone 2: Parser and Printer

- Implement `pest` grammar.
- Build parsed AST.
- Normalize temporal operators.
- Implement keyword, ASCII, and Unicode printers.
- Add parser and formatting tests.

### Milestone 3: Imports and Semantic Analysis

- Implement import resolver.
- Implement name resolution.
- Implement type checking.
- Enforce formula placement rules.
- Add semantic diagnostics.

### Milestone 4: Explicit Model Construction

- Represent finite domains.
- Enumerate initial states.
- Enumerate successor states.
- Build reachable graph.
- Detect deadlocks.

### Milestone 5: Property Checking

- Implement invariant checking.
- Implement full supported temporal checking.
- Generate counterexample traces.
- Add JSON output.

### Milestone 6: Hardening

- Add golden tests.
- Add resource limits.
- Improve diagnostics.
- Document language examples.
- Stabilize CLI help text.

## 27. Compatibility Rules

Before a stable `1.0` release, the language may change if the changes are documented.
After `1.0`, compatibility should follow these rules:

- Existing valid specs should continue to parse.
- Existing operator spellings must remain accepted.
- Default printing may remain Unicode, but explicit printer flags must be stable.
- JSON output should be versioned if breaking changes are introduced.

## 28. Open Design Questions

The first implementation should make explicit decisions for:

- Whether transition blocks are always disjunctive or whether a separate global
  transition constraint syntax should exist.
- Whether deadlocks are always errors or whether stuttering should be configurable
  in the initial release.
- Whether enum values live in a global namespace or are scoped to their enum domain.
- Whether `int` should be rejected entirely until a symbolic backend exists.
- Whether imports should support module names in addition to file paths.
- Whether formatter output should preserve comments.

Until these are resolved, the defaults in this specification should be treated as
the implementation target.
