#![forbid(unsafe_code)]

pub mod error;
pub mod exec;
pub mod instr;
pub mod value;

pub use error::Trap;
pub use value::Value;
