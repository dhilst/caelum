use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Bool(bool),
    Int(i64),
    Enum(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct State {
    pub values: Vec<Value>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(value) => write!(f, "{value}"),
            Value::Int(value) => write!(f, "{value}"),
            Value::Enum(value) => f.write_str(value),
        }
    }
}
