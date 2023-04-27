pub mod command;
pub mod structs;
pub mod volume;

pub use command::*;
pub use structs::*;
pub use volume::*;

#[derive(Debug, Clone)]
pub enum PAIdent {
    Index(u32),
    Name(String),
}
