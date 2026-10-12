#![forbid(unsafe_code)]

pub mod decode;
pub mod error;
pub mod exec;
pub mod instr;
pub mod jumps;
pub mod module;
pub mod value;

pub use error::{DecodeError, Trap};
pub use exec::{Interp, Outcome};
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

    /// Run export `name` to completion. Fresh state (stack, memory) per call.
    pub fn invoke(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>, Trap> {
        exec::run(&self.module, self.export(name)?, args)
    }

    /// Set up a call to export `name` without running it; drive it with `resume`.
    pub fn start(&self, name: &str, args: &[Value]) -> Result<Interp, Trap> {
        exec::start(&self.module, self.export(name)?, args)
    }

    /// Run `it` for at most `max_steps` more instructions. `it` must come from this
    /// module (via `start`, or a snapshot of it) — another module's state may panic.
    pub fn resume(&self, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap> {
        exec::resume(&self.module, it, max_steps)
    }

    /// The function index exported as `name`.
    fn export(&self, name: &str) -> Result<u32, Trap> {
        self.module
            .exports
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.func_idx)
            .ok_or(Trap::UnknownExport)
    }
}
