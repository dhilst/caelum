//! Resolve constants, types and variable domains from a SourceFile, ready
//! for the BMC encoder. Mirrors the small slice of `model/graph.rs`'s
//! preparation code, but keeps the BMC engine independent of the explicit
//! engine's data structures.

use std::collections::HashMap;

use crate::diagnostics::{CaelumError, Result};
use crate::model::eval::{eval_expr, expect_int, EvalEnv};
use crate::model::Value;
use crate::syntax::{
    ConstDecl, Domain, DomainBound, InitBlock, Item, PropertyBlock, SourceFile, TransitionBlock,
    VarDecl,
};

use super::encode::{ConstValue, ResolvedDomain};

pub struct BmcSpec<'a> {
    pub constants: HashMap<String, ConstValue>,
    pub enum_variant_type: HashMap<String, String>,
    pub variables: Vec<&'a VarDecl>,
    pub domains: HashMap<String, ResolvedDomain>,
    pub init_blocks: Vec<&'a InitBlock>,
    pub transition_blocks: Vec<&'a TransitionBlock>,
    pub properties: Vec<&'a PropertyBlock>,
}

pub fn prepare<'a>(file: &'a SourceFile) -> Result<BmcSpec<'a>> {
    let constants_raw = collect_constants(file)?;
    let types = collect_types(file);
    let mut domains = HashMap::new();
    let mut variables = Vec::new();
    let mut enum_variant_type = HashMap::new();

    for item in &file.items {
        match item {
            Item::Var(decl) => {
                variables.push(decl);
                let resolved = resolve_domain(&decl.name, &decl.domain, &constants_raw, &types)?;
                if let ResolvedDomain::Enum {
                    type_name,
                    variants,
                } = &resolved
                {
                    for v in variants {
                        enum_variant_type.insert(v.clone(), type_name.clone());
                    }
                }
                domains.insert(decl.name.clone(), resolved);
            }
            Item::TypeDecl(decl) => {
                if let Domain::Enum { variants } = &decl.domain {
                    for v in variants {
                        enum_variant_type.insert(v.clone(), decl.name.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let mut constants = HashMap::new();
    for (name, value) in constants_raw {
        constants.insert(
            name,
            match value {
                Value::Bool(b) => ConstValue::Bool(b),
                Value::Int(i) => ConstValue::Int(i),
                Value::Enum(variant) => {
                    let type_name = enum_variant_type
                        .get(&variant)
                        .cloned()
                        .unwrap_or_else(|| variant.clone());
                    ConstValue::Enum { type_name, variant }
                }
            },
        );
    }

    let init_blocks = file
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Init(b) => Some(b),
            _ => None,
        })
        .collect();
    let transition_blocks = file
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Transition(b) => Some(b),
            _ => None,
        })
        .collect();
    let properties = file
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Property(p) => Some(p),
            _ => None,
        })
        .collect();

    Ok(BmcSpec {
        constants,
        enum_variant_type,
        variables,
        domains,
        init_blocks,
        transition_blocks,
        properties,
    })
}

fn collect_constants(file: &SourceFile) -> Result<HashMap<String, Value>> {
    let mut constants = HashMap::new();
    for item in &file.items {
        if let Item::Const(ConstDecl { name, expr }) = item {
            let env = EvalEnv::new(constants.clone(), HashMap::new(), HashMap::new());
            let value = eval_expr(expr, &env, None, None)?;
            constants.insert(name.clone(), value);
        }
    }
    Ok(constants)
}

fn collect_types(file: &SourceFile) -> HashMap<String, Domain> {
    let mut types = HashMap::new();
    for item in &file.items {
        if let Item::TypeDecl(decl) = item {
            types.insert(decl.name.clone(), decl.domain.clone());
        }
    }
    types
}

fn resolve_domain(
    var_name: &str,
    domain: &Domain,
    constants: &HashMap<String, Value>,
    types: &HashMap<String, Domain>,
) -> Result<ResolvedDomain> {
    match domain {
        Domain::Bool => Ok(ResolvedDomain::Bool),
        Domain::IntRange { start, end } => {
            let start = bound_value(start, constants)?;
            let end = bound_value(end, constants)?;
            if start > end {
                return Err(CaelumError::Model {
                    message: format!("BMC: empty integer range for `{var_name}`: {start}..{end}"),
                });
            }
            Ok(ResolvedDomain::Int((start..=end).collect()))
        }
        Domain::Enum { variants } => {
            // Inline enums use the variable name as the implicit type name.
            Ok(ResolvedDomain::Enum {
                type_name: var_name.to_string(),
                variants: variants.clone(),
            })
        }
        Domain::Named(type_name) => {
            let referenced = types.get(type_name).ok_or_else(|| CaelumError::Model {
                message: format!("BMC: unknown type `{type_name}` for variable `{var_name}`"),
            })?;
            // For a named enum we want to keep the user's type name.
            if let Domain::Enum { variants } = referenced {
                return Ok(ResolvedDomain::Enum {
                    type_name: type_name.clone(),
                    variants: variants.clone(),
                });
            }
            resolve_domain(var_name, referenced, constants, types)
        }
    }
}

fn bound_value(bound: &DomainBound, constants: &HashMap<String, Value>) -> Result<i64> {
    match bound {
        DomainBound::Int(value) => Ok(*value),
        DomainBound::Name(name) => {
            let value = constants.get(name).ok_or_else(|| CaelumError::Model {
                message: format!("BMC: unknown range bound constant `{name}`"),
            })?;
            expect_int(value.clone(), "range bound")
        }
    }
}
