use std::path::{Path, PathBuf};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::diagnostics::{Result, CaelumError};

use super::ast::*;

#[derive(Parser)]
#[grammar = "syntax/grammar.pest"]
struct CaelumParser;

pub fn parse_source(source: &str) -> Result<SourceFile> {
    parse_source_file(Path::new("<memory>"), source)
}

pub fn parse_source_file(path: &Path, source: &str) -> Result<SourceFile> {
    let mut pairs = CaelumParser::parse(Rule::file, source).map_err(|err| CaelumError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let file = pairs.next().expect("pest returned no file pair");
    parse_file(path, file)
}

fn parse_file(path: &Path, pair: Pair<'_, Rule>) -> Result<SourceFile> {
    let mut module = None;
    let mut imports = Vec::new();
    let mut items = Vec::new();

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::module_decl => module = Some(parse_module_decl(child)),
            Rule::import_decl => imports.push(parse_import_decl(path, child)?),
            Rule::const_decl => items.push(Item::Const(parse_const_decl(child)?)),
            Rule::var_decl => items.push(Item::Var(parse_var_decl(child)?)),
            Rule::init_block => items.push(Item::Init(parse_init_block(child)?)),
            Rule::transition_block => {
                items.push(Item::Transition(parse_transition_block(child)?));
            }
            Rule::property_block => items.push(Item::Property(parse_property_block(child)?)),
            Rule::invalid_block => items.push(Item::Property(parse_invalid_block(child)?)),
            Rule::EOI => {}
            rule => {
                return Err(CaelumError::Parse {
                    path: PathBuf::from(path),
                    message: format!("unexpected top-level rule: {rule:?}"),
                });
            }
        }
    }

    Ok(SourceFile {
        module,
        imports,
        items,
    })
}

fn parse_module_decl(pair: Pair<'_, Rule>) -> ModuleName {
    let module_name = pair
        .into_inner()
        .next()
        .expect("module_decl must contain module_name");

    ModuleName {
        parts: module_name
            .into_inner()
            .map(|part| part.as_str().to_owned())
            .collect(),
    }
}

fn parse_import_decl(path: &Path, pair: Pair<'_, Rule>) -> Result<ImportDecl> {
    let string = pair
        .into_inner()
        .next()
        .expect("import_decl must contain string_lit");

    Ok(ImportDecl {
        path: unescape_string(path, string.as_str())?,
    })
}

fn parse_const_decl(pair: Pair<'_, Rule>) -> Result<ConstDecl> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("const_decl must contain ident")
        .as_str()
        .to_owned();
    let expr = parse_expr_pair(inner.next().expect("const_decl must contain expr"))?;

    Ok(ConstDecl { name, expr })
}

fn parse_var_decl(pair: Pair<'_, Rule>) -> Result<VarDecl> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("var_decl must contain ident")
        .as_str()
        .to_owned();
    let domain = parse_domain(inner.next().expect("var_decl must contain domain"))?;

    Ok(VarDecl { name, domain })
}

fn parse_init_block(pair: Pair<'_, Rule>) -> Result<InitBlock> {
    let expr = parse_block_expr(pair.into_inner().next().expect("init must contain block"))?;
    Ok(InitBlock { expr })
}

fn parse_transition_block(pair: Pair<'_, Rule>) -> Result<TransitionBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("transition must contain name")
        .as_str()
        .to_owned();
    let expr = parse_block_expr(inner.next().expect("transition must contain block"))?;

    Ok(TransitionBlock { name, expr })
}

fn parse_property_block(pair: Pair<'_, Rule>) -> Result<PropertyBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("property must contain name")
        .as_str()
        .to_owned();
    let expr = parse_block_expr(inner.next().expect("property must contain block"))?;

    Ok(PropertyBlock {
        kind: PropertyKind::Property,
        name,
        expr,
    })
}

fn parse_invalid_block(pair: Pair<'_, Rule>) -> Result<PropertyBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("invalid must contain name")
        .as_str()
        .to_owned();
    let expr = parse_block_expr(inner.next().expect("invalid must contain block"))?;

    Ok(PropertyBlock {
        kind: PropertyKind::Invalid,
        name,
        expr,
    })
}

fn parse_block_expr(pair: Pair<'_, Rule>) -> Result<Expr> {
    parse_expr_pair(pair.into_inner().next().expect("block must contain expr"))
}

fn parse_domain(pair: Pair<'_, Rule>) -> Result<Domain> {
    match pair.as_rule() {
        Rule::bool_domain => Ok(Domain::Bool),
        Rule::int_range => {
            let mut inner = pair.into_inner();
            let start = parse_domain_bound(inner.next().expect("range must contain start"))?;
            let end = parse_domain_bound(inner.next().expect("range must contain end"))?;
            Ok(Domain::IntRange { start, end })
        }
        Rule::enum_domain => Ok(Domain::Enum {
            variants: pair
                .into_inner()
                .map(|variant| variant.as_str().to_owned())
                .collect(),
        }),
        rule => Err(CaelumError::Parse {
            path: PathBuf::from("<memory>"),
            message: format!("unexpected domain rule: {rule:?}"),
        }),
    }
}

fn parse_domain_bound(pair: Pair<'_, Rule>) -> Result<DomainBound> {
    match pair.as_rule() {
        Rule::int_lit => Ok(DomainBound::Int(parse_i64(pair)?)),
        Rule::ident => Ok(DomainBound::Name(pair.as_str().to_owned())),
        rule => Err(CaelumError::Parse {
            path: PathBuf::from("<memory>"),
            message: format!("unexpected range bound rule: {rule:?}"),
        }),
    }
}

fn parse_expr_pair(pair: Pair<'_, Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::equivalence => parse_left_assoc(pair, binary_op_from_pair),
        Rule::implication => parse_right_assoc(pair, BinaryOp::Implies),
        Rule::until_expr => parse_right_assoc(pair, BinaryOp::Until),
        Rule::or_expr => parse_left_assoc(pair, binary_op_from_pair),
        Rule::and_expr => parse_left_assoc(pair, binary_op_from_pair),
        Rule::comparison => parse_optional_binary(pair),
        Rule::additive => parse_left_assoc(pair, binary_op_from_pair),
        Rule::multiplicative => parse_left_assoc(pair, binary_op_from_pair),
        Rule::unary => parse_unary(pair),
        Rule::primary | Rule::literal => {
            parse_expr_pair(pair.into_inner().next().expect("wrapper must contain expr"))
        }
        Rule::bool_lit => Ok(Expr::Bool(pair.as_str() == "true")),
        Rule::int_lit => Ok(Expr::Int(parse_i64(pair)?)),
        Rule::ident => Ok(Expr::Name(pair.as_str().to_owned())),
        Rule::primed_ident => {
            let name = pair
                .into_inner()
                .next()
                .expect("primed ident must contain ident")
                .as_str()
                .to_owned();
            Ok(Expr::PrimedName(name))
        }
        rule => Err(CaelumError::Parse {
            path: PathBuf::from("<memory>"),
            message: format!("unexpected expression rule: {rule:?}"),
        }),
    }
}

fn parse_unary(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut ops = Vec::new();
    let mut primary = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::unary_op => ops.push(unary_op_from_pair(child)),
            _ => primary = Some(parse_expr_pair(child)?),
        }
    }

    let mut expr = primary.expect("unary must contain primary");
    for op in ops.into_iter().rev() {
        expr = Expr::Unary {
            op,
            expr: Box::new(expr),
        };
    }

    Ok(expr)
}

fn parse_left_assoc(
    pair: Pair<'_, Rule>,
    op_parser: fn(Pair<'_, Rule>) -> BinaryOp,
) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let mut expr = parse_expr_pair(inner.next().expect("binary expression must contain lhs"))?;

    while let Some(op_pair) = inner.next() {
        let rhs = parse_expr_pair(inner.next().expect("binary expression must contain rhs"))?;
        expr = Expr::Binary {
            op: op_parser(op_pair),
            lhs: Box::new(expr),
            rhs: Box::new(rhs),
        };
    }

    Ok(expr)
}

fn parse_right_assoc(pair: Pair<'_, Rule>, op: BinaryOp) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let lhs = parse_expr_pair(inner.next().expect("binary expression must contain lhs"))?;

    if inner.next().is_some() {
        let rhs = parse_expr_pair(inner.next().expect("binary expression must contain rhs"))?;
        Ok(Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    } else {
        Ok(lhs)
    }
}

fn parse_optional_binary(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let lhs = parse_expr_pair(inner.next().expect("comparison must contain lhs"))?;

    if let Some(op_pair) = inner.next() {
        let rhs = parse_expr_pair(inner.next().expect("comparison must contain rhs"))?;
        Ok(Expr::Binary {
            op: binary_op_from_pair(op_pair),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    } else {
        Ok(lhs)
    }
}

fn unary_op_from_pair(pair: Pair<'_, Rule>) -> UnaryOp {
    let op = pair
        .into_inner()
        .next()
        .expect("unary_op must contain concrete operator");

    match op.as_rule() {
        Rule::not_op => UnaryOp::Not,
        Rule::neg_op => UnaryOp::Neg,
        Rule::always_op => UnaryOp::Always,
        Rule::eventually_op => UnaryOp::Eventually,
        Rule::next_op => UnaryOp::Next,
        rule => unreachable!("unexpected unary operator rule: {rule:?}"),
    }
}

fn binary_op_from_pair(pair: Pair<'_, Rule>) -> BinaryOp {
    match pair.as_rule() {
        Rule::iff_op => BinaryOp::Iff,
        Rule::until_op => BinaryOp::Until,
        Rule::or_op => BinaryOp::Or,
        Rule::and_op => BinaryOp::And,
        Rule::implies_op => BinaryOp::Implies,
        Rule::add_op => match pair.as_str() {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            op => unreachable!("unexpected additive operator: {op}"),
        },
        Rule::mul_op => match pair.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            "mod" => BinaryOp::Mod,
            op => unreachable!("unexpected multiplicative operator: {op}"),
        },
        Rule::comp_op => match pair.as_str() {
            "=" => BinaryOp::Eq,
            "!=" | "≠" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            ">=" => BinaryOp::Ge,
            op => unreachable!("unexpected comparison operator: {op}"),
        },
        rule => unreachable!("unexpected binary operator rule: {rule:?}"),
    }
}

fn parse_i64(pair: Pair<'_, Rule>) -> Result<i64> {
    pair.as_str().parse::<i64>().map_err(|err| CaelumError::Parse {
        path: PathBuf::from("<memory>"),
        message: format!("invalid integer literal `{}`: {err}", pair.as_str()),
    })
}

fn unescape_string(path: &Path, raw: &str) -> Result<String> {
    let mut chars = raw[1..raw.len() - 1].chars();
    let mut out = String::new();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let escaped = chars.next().ok_or_else(|| CaelumError::Parse {
            path: path.to_path_buf(),
            message: "unterminated string escape".to_owned(),
        })?;

        match escaped {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                return Err(CaelumError::Parse {
                    path: path.to_path_buf(),
                    message: format!("unsupported string escape: \\{other}"),
                });
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_property_expr(source: &str) -> Expr {
        let file = parse_source(source).expect("source should parse");
        let Item::Property(property) = &file.items[0] else {
            panic!("expected first item to be a property");
        };
        property.expr.clone()
    }

    #[test]
    fn normalizes_prefix_temporal_operators() {
        let keywords = first_property_expr("property p { always eventually next x = 0 }");
        let ascii = first_property_expr("property p { [] <> () x = 0 }");
        let unicode = first_property_expr("property p { □ ◇ ◯ x = 0 }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn normalizes_until_operators() {
        let keywords = first_property_expr("property p { x = 0 until x = 1 }");
        let ascii = first_property_expr("property p { x = 0 U x = 1 }");
        let unicode = first_property_expr("property p { x = 0 𝒰 x = 1 }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn normalizes_classical_logic_operators() {
        let keywords = first_property_expr("property p { not a and b or c }");
        let ascii = first_property_expr(r"property p { ~ a /\ b \/ c }");
        let unicode = first_property_expr("property p { ¬ a ∧ b ∨ c }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn ascii_next_parens_disambiguated_from_grouping() {
        // `()` as next operator followed by a parenthesized expression
        let expr = first_property_expr("property p { () (x = 1) }");
        let Expr::Unary { op, expr: inner } = &expr else {
            panic!("expected Unary, got {expr:?}");
        };
        assert_eq!(*op, UnaryOp::Next);
        // The inner expression should be `x = 1` (the grouping parens are stripped)
        let Expr::Binary { op: inner_op, lhs, .. } = inner.as_ref() else {
            panic!("expected Binary inside Next, got {inner:?}");
        };
        assert_eq!(*inner_op, BinaryOp::Eq);
        assert!(matches!(lhs.as_ref(), Expr::Name(n) if n == "x"));
    }

    #[test]
    fn all_ascii_operators_parse_together() {
        // Combines every ASCII operator form in one spec:
        //   [] (always), <> (eventually), () (next),
        //   ~ (not), /\ (and), \/ (or), U (until)
        let source = r"
            property p { [] <> () ~ a /\ b \/ c U d }
        ";
        let expr = first_property_expr(source);
        // Until has the lowest precedence, so it becomes the outermost node.
        // LHS = `[] <> () ~ a /\ b \/ c`, RHS = `d`
        let Expr::Binary { op, rhs, .. } = &expr else {
            panic!("expected Binary(Until) at top level, got {expr:?}");
        };
        assert_eq!(*op, BinaryOp::Until);
        assert!(matches!(rhs.as_ref(), Expr::Name(n) if n == "d"));
    }

    #[test]
    fn unicode_element_of_type_sep_and_until_in_full_spec() {
        // End-to-end: `∈` as the type separator for VarDecl with IntRange,
        // and `𝒰` as the Until operator inside a property.
        let file = parse_source(
            r#"
            let x ∈ 0..2
            init { x = 0 }
            transition step { x' = x + 1 }
            property reaches_max { (x < 2) 𝒰 (x = 2) }
            "#,
        )
        .expect("spec with ∈ and 𝒰 should parse");

        // The first item must be a VarDecl with an IntRange domain.
        let Item::Var(ref var) = file.items[0] else {
            panic!("expected first item to be Var, got {:?}", file.items[0]);
        };
        assert_eq!(var.name, "x");
        assert_eq!(
            var.domain,
            Domain::IntRange {
                start: DomainBound::Int(0),
                end: DomainBound::Int(2),
            }
        );

        // The property expression must be a Binary Until node.
        let Item::Property(ref prop) = file.items[3] else {
            panic!("expected fourth item to be Property, got {:?}", file.items[3]);
        };
        assert_eq!(prop.name, "reaches_max");
        let Expr::Binary { op, ref lhs, ref rhs } = prop.expr else {
            panic!("expected Binary expr in property, got {:?}", prop.expr);
        };
        assert_eq!(op, BinaryOp::Until);

        // LHS: x < 2
        let Expr::Binary { op: lhs_op, .. } = lhs.as_ref() else {
            panic!("expected Binary in LHS of Until, got {:?}", lhs);
        };
        assert_eq!(*lhs_op, BinaryOp::Lt);

        // RHS: x = 2
        let Expr::Binary { op: rhs_op, .. } = rhs.as_ref() else {
            panic!("expected Binary in RHS of Until, got {:?}", rhs);
        };
        assert_eq!(*rhs_op, BinaryOp::Eq);
    }

    #[test]
    fn parses_declarations_and_primed_transition_variables() {
        let file = parse_source(
            r#"
            module examples.counter
            import "common.lum"

            const max = 3
            let x ∈ 0..max
            let mode: enum { idle, busy, done }

            init { x = 0 }
            transition inc { x' = (x + 1) mod 4 }
            property wraps { □ ◇ x = 0 }
            "#,
        )
        .expect("source should parse");

        assert_eq!(
            file.module,
            Some(ModuleName {
                parts: vec!["examples".into(), "counter".into()]
            })
        );
        assert_eq!(file.imports[0].path, "common.lum");
        assert_eq!(file.item_count(), 6);
    }

    #[test]
    fn normalizes_implies_iff_ne_unicode_operators() {
        let ascii_implies = first_property_expr("property p { x = 0 -> x = 1 }");
        let unicode_implies = first_property_expr("property p { x = 0 → x = 1 }");
        assert_eq!(ascii_implies, unicode_implies);

        let ascii_iff = first_property_expr("property p { x = 0 <-> x = 1 }");
        let unicode_iff = first_property_expr("property p { x = 0 ↔ x = 1 }");
        assert_eq!(ascii_iff, unicode_iff);

        let ascii_ne = first_property_expr("property p { x != 0 }");
        let unicode_ne = first_property_expr("property p { x ≠ 0 }");
        assert_eq!(ascii_ne, unicode_ne);
    }
}
