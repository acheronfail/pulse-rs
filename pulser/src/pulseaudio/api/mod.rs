pub mod command;
pub mod structs;
pub mod volume;

use std::fmt::Display;

pub use command::*;
use serde::Serialize;
pub use structs::*;
pub use volume::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PAIdent {
    Index(u32),
    Name(String),
}

impl Display for PAIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PAIdent::Index(idx) => f.write_fmt(format_args!("#{}", idx)),
            PAIdent::Name(name) => f.write_fmt(format_args!("{}", name)),
        }
    }
}
