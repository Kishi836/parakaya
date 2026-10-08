//! M1 exit criterion: real `.wasm` bytes (compiled from WAT) → decode → execute.

use parakaya_vm::{Trap, Value, Vm};

#[test]
fn factorial_from_wat() {
    let bytes = wat::parse_str(
        r#"(module
             (func $fac (export "fac") (param i32) (result i32) (local i32)
               local.get 0
               i32.const 1
               i32.le_s
               if
                 i32.const 1
                 local.set 1
               else
                 local.get 0
                 local.get 0
                 i32.const 1
                 i32.sub
                 call $fac
                 i32.mul
                 local.set 1
               end
               local.get 1))"#,
    )
    .unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(
        vm.invoke("fac", &[Value::I32(10)]),
        Ok(Some(Value::I32(3628800)))
    );
}

#[test]
fn memory_counter_from_wat() {
    let bytes = wat::parse_str(
        r#"(module
             (memory 1)
             (func (export "bump") (result i32)
               i32.const 0
               i32.const 0
               i32.load
               i32.const 1
               i32.add
               i32.store
               i32.const 0
               i32.load))"#,
    )
    .unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(vm.invoke("bump", &[]), Ok(Some(Value::I32(1))));
}

#[test]
fn unknown_export_traps() {
    let bytes = wat::parse_str(r#"(module (func (export "f")))"#).unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(vm.invoke("nope", &[]), Err(Trap::UnknownExport));
}
