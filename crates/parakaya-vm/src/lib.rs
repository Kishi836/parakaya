#![forbid(unsafe_code)]

pub mod decode;
pub mod error;
pub mod exec;
pub mod instr;
pub mod jumps;
pub mod module;
pub mod value;

pub use error::{DecodeError, Trap};
pub use value::Value;

/// A loaded WASM module, ready to run its exports.
pub struct Vm {
    module: module::Module,
}

impl Vm {
    pub fn instantiate(bytes: &[u8]) -> Result<Vm, DecodeError> {
        Ok(Vm {
            module: decode::module(bytes)?,
        })
    }

    /// Run export `name`. M1: fresh state (stack, memory) per call; persistent instance
    /// state arrives with snapshots in M2.
    pub fn invoke(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>, Trap> {
        let export = self
            .module
            .exports
            .iter()
            .find(|e| e.name == name)
            .ok_or(Trap::UnknownExport)?;
        exec::run(&self.module, export.func_idx, args)
    }
}
