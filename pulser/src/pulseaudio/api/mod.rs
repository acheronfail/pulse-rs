pub mod command;
pub mod structs;
pub mod volume;

pub use command::*;
use serde::Serialize;
pub use structs::*;
pub use volume::*;

#[derive(Debug, Clone, Serialize)]
pub enum PAIdent {
    Index(u32),
    Name(String),
}
