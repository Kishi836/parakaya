use crate::error::Trap;
use crate::instr::Instr;
use crate::jumps::JumpInfo;
use crate::module::{FuncType, Function, Module};
use crate::value::Value;
use std::collections::HashMap;

/// One open `block`/`loop`/`if`. `height` is the value-stack depth at entry,
/// restored when a branch exits through this frame.
#[derive(Copy, Clone, Debug)]
pub struct CtrlFrame {
    pub opener_pc: usize,
    pub end_pc: usize,
    pub is_loop: bool,
    pub height: usize,
}

/// One guest function activation. `value_base`/`ctrl_base` mark where this call's
/// slice of the shared value and control stacks begins.
#[derive(Clone, Debug)]
pub struct Frame {
    pub func_idx: u32,
    pub pc: usize,
    pub locals: Vec<Value>,
    pub value_base: usize,
    pub ctrl_base: usize,
}

/// The entire VM state — plain owned data, so M2 snapshots are "serialize this struct".
#[derive(Debug, Default)]
pub struct Interp {
    pub values: Vec<Value>,
    pub ctrls: Vec<CtrlFrame>,
    pub frames: Vec<Frame>,
}

impl Interp {
    pub fn push(&mut self, v: Value) {
        self.values.push(v);
    }
    pub fn pop(&mut self) -> Result<Value, Trap> {
        self.values.pop().ok_or(Trap::StackUnderflow)
    }
    pub fn pop_i32(&mut self) -> Result<i32, Trap> {
        match self.pop()? {
            Value::I32(x) => Ok(x),
            _ => Err(Trap::TypeMismatch),
        }
    }
    fn bin_i32(&mut self, f: impl Fn(i32, i32) -> Result<i32, Trap>) -> Result<(), Trap> {
        let b = self.pop_i32()?;
        let a = self.pop_i32()?;
        self.push(Value::I32(f(a, b)?));
        Ok(())
    }
}

/// Test helper: run `instrs` as a lone `() -> i32` function with `n_locals` locals.
pub fn eval_body(instrs: &[Instr], n_locals: usize) -> Result<Option<Value>, Trap> {
    let ty = FuncType {
        params: 0,
        results: 1,
    };
    let module = Module {
        types: vec![ty],
        funcs: vec![Function {
            ty,
            n_locals,
            body: instrs.to_vec(),
        }],
        ..Module::default()
    };
    run(&module, 0, &[])
}

/// Call `func_idx` with `args`. Guest calls push `Frame`s onto `Interp.frames` and this
/// loop keeps stepping the top one — the host never recurses, however deep the guest goes.
pub fn run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap> {
    if args.len() != module.funcs[func_idx as usize].ty.params {
        return Err(Trap::TypeMismatch);
    }
    // ponytail: every function's jump table is rebuilt per run; move to decode time in M2.
    let jumps: Vec<HashMap<usize, JumpInfo>> = module
        .funcs
        .iter()
        .map(|f| crate::jumps::compute(&f.body))
        .collect();
    let mut it = Interp::default();
    push_frame(&mut it, module, func_idx, args.to_vec());
    while let Some(frame) = it.frames.last() {
        let f = frame.func_idx as usize;
        step(&mut it, module, &module.funcs[f].body, &jumps[f])?;
    }
    Ok(it.values.pop())
}

/// Execute the instruction at the top frame's pc.
fn step(
    it: &mut Interp,
    module: &Module,
    body: &[Instr],
    jumps: &HashMap<usize, JumpInfo>,
) -> Result<(), Trap> {
    let top = it.frames.len() - 1;
    let mut pc = it.frames[top].pc;
    match &body[pc] {
        Instr::I32Const(x) => it.push(Value::I32(*x)),
        Instr::I32Add => it.bin_i32(|a, b| Ok(a.wrapping_add(b)))?,
        Instr::I32Sub => it.bin_i32(|a, b| Ok(a.wrapping_sub(b)))?,
        Instr::I32Mul => it.bin_i32(|a, b| Ok(a.wrapping_mul(b)))?,
        Instr::I32DivS => it.bin_i32(|a, b| match (a, b) {
            (_, 0) => Err(Trap::DivByZero),
            (i32::MIN, -1) => Err(Trap::IntegerOverflow),
            _ => Ok(a / b),
        })?,
        Instr::I32RemS => it.bin_i32(|a, b| {
            if b == 0 {
                Err(Trap::DivByZero)
            } else {
                Ok(a.wrapping_rem(b))
            }
        })?,
        Instr::Nop => {}
        Instr::Drop => {
            it.pop()?;
        }
        Instr::Unreachable => return Err(Trap::Unreachable),
        Instr::Block | Instr::Loop => {
            let j = jumps[&pc];
            it.ctrls.push(CtrlFrame {
                opener_pc: pc,
                end_pc: j.end_pc,
                is_loop: matches!(body[pc], Instr::Loop),
                height: it.values.len(),
            });
        }
        Instr::If => {
            let j = jumps[&pc];
            let cond = it.pop_i32()?;
            if cond == 0 && j.else_pc.is_none() {
                // no arm runs: skip past End without ever opening a frame
                pc = j.end_pc;
            } else {
                it.ctrls.push(CtrlFrame {
                    opener_pc: pc,
                    end_pc: j.end_pc,
                    is_loop: false,
                    height: it.values.len(),
                });
                if cond == 0 {
                    pc = j.else_pc.unwrap(); // else-arm starts after Else
                }
            }
        }
        Instr::Else => {
            // reached only by falling out of the then-arm: close the if, skip past End
            let frame = it.ctrls.pop().ok_or(Trap::StackUnderflow)?;
            pc = frame.end_pc;
        }
        Instr::Br(n) => return branch(it, module, *n),
        Instr::BrIf(n) => {
            if it.pop_i32()? != 0 {
                return branch(it, module, *n);
            }
        }
        Instr::Return => return pop_frame(it, module),
        Instr::End => {
            if it.ctrls.len() == it.frames[top].ctrl_base {
                return pop_frame(it, module); // function-body End
            }
            it.ctrls.pop();
        }
        Instr::Call(idx) => {
            let params = module.funcs[*idx as usize].ty.params;
            let split = it
                .values
                .len()
                .checked_sub(params)
                .ok_or(Trap::StackUnderflow)?;
            let args = it.values.split_off(split);
            it.frames[top].pc = pc + 1; // resume after the call
            push_frame(it, module, *idx, args);
            return Ok(());
        }
        Instr::LocalGet(i) => {
            let v = it.frames[top].locals[*i as usize];
            it.push(v);
        }
        Instr::LocalSet(i) => {
            let v = it.pop()?;
            it.frames[top].locals[*i as usize] = v;
        }
        Instr::LocalTee(i) => {
            let v = it.pop()?;
            it.frames[top].locals[*i as usize] = v;
            it.push(v);
        }
        Instr::I32Eqz => {
            let x = it.pop_i32()?;
            it.push(Value::I32((x == 0) as i32));
        }
        Instr::I32Eq => it.bin_i32(|a, b| Ok((a == b) as i32))?,
        Instr::I32Ne => it.bin_i32(|a, b| Ok((a != b) as i32))?,
        Instr::I32LtS => it.bin_i32(|a, b| Ok((a < b) as i32))?,
        Instr::I32LeS => it.bin_i32(|a, b| Ok((a <= b) as i32))?,
        Instr::I32GtS => it.bin_i32(|a, b| Ok((a > b) as i32))?,
        Instr::I32GeS => it.bin_i32(|a, b| Ok((a >= b) as i32))?,
        other => unimplemented!("instr {:?} arrives in a later task", other),
    }
    it.frames[top].pc = pc + 1;
    Ok(())
}

fn push_frame(it: &mut Interp, module: &Module, func_idx: u32, mut locals: Vec<Value>) {
    let f = &module.funcs[func_idx as usize];
    locals.resize(f.ty.params + f.n_locals, Value::I32(0)); // params first, then zeroed locals
    it.frames.push(Frame {
        func_idx,
        pc: 0,
        locals,
        value_base: it.values.len(),
        ctrl_base: it.ctrls.len(),
    });
}

/// Return from the top frame: keep its result (if any), discard everything else it pushed.
fn pop_frame(it: &mut Interp, module: &Module) -> Result<(), Trap> {
    let frame = it.frames.pop().ok_or(Trap::StackUnderflow)?;
    let result = if module.funcs[frame.func_idx as usize].ty.results == 1 {
        Some(it.pop()?)
    } else {
        None
    };
    it.values.truncate(frame.value_base);
    it.ctrls.truncate(frame.ctrl_base);
    if let Some(v) = result {
        it.push(v);
    }
    Ok(())
}

/// Branch `depth` control frames out of the current function. Loops re-enter just after
/// their opener; blocks/ifs exit just past their End. A depth equal to the number of open
/// frames targets the function body itself — WASM treats that as a return.
fn branch(it: &mut Interp, module: &Module, depth: u32) -> Result<(), Trap> {
    let top = it.frames.len() - 1;
    let depth = depth as usize;
    let open = it.ctrls.len() - it.frames[top].ctrl_base;
    if depth == open {
        return pop_frame(it, module);
    }
    if depth > open {
        return Err(Trap::StackUnderflow); // would escape into the caller's blocks
    }
    let idx = it.ctrls.len() - 1 - depth;
    let target = it.ctrls[idx];
    it.values.truncate(target.height);
    if target.is_loop {
        it.ctrls.truncate(idx + 1); // loop frame stays; continue from opener
        it.frames[top].pc = target.opener_pc + 1;
    } else {
        it.ctrls.truncate(idx); // block/if frame exits
        it.frames[top].pc = target.end_pc + 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Export;

    #[test]
    fn div_by_zero_traps() {
        let body = [
            Instr::I32Const(1),
            Instr::I32Const(0),
            Instr::I32DivS,
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 0), Err(Trap::DivByZero));
    }

    #[test]
    fn div_overflow_traps() {
        let body = [
            Instr::I32Const(i32::MIN),
            Instr::I32Const(-1),
            Instr::I32DivS,
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 0), Err(Trap::IntegerOverflow));
    }

    #[test]
    fn const_add() {
        let body = [
            Instr::I32Const(2),
            Instr::I32Const(3),
            Instr::I32Add,
            Instr::End,
        ]; // 2 + 3 = 5
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(5)))); // assert that the result of evaluating the body is equal to 5
    }

    #[test]
    fn arithmetic_wraps_and_divides() {
        let body = [
            Instr::I32Const(i32::MAX),
            Instr::I32Const(1),
            Instr::I32Add,
            Instr::End,
        ]; // i32::MAX + 1 = i32::MIN (wraps around)
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(i32::MIN)))); // wrapping
        let body = [
            Instr::I32Const(7),
            Instr::I32Const(2),
            Instr::I32DivS,
            Instr::End,
        ]; // 7 / 2 = 3
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(3)))); // assert that the result of evaluating the body is equal to 3
    }
    #[test]
    fn locals_roundtrip() {
        let body = [
            Instr::I32Const(41),
            Instr::LocalSet(0),
            Instr::LocalGet(0),
            Instr::I32Const(1),
            Instr::I32Add,
            Instr::LocalTee(1),
            Instr::Drop,
            Instr::LocalGet(1),
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 2), Ok(Some(Value::I32(42))));
    }

    #[test]
    fn comparisons_yield_0_or_1() {
        let body = [
            Instr::I32Const(3),
            Instr::I32Const(5),
            Instr::I32LtS,
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(1))));
        let body = [Instr::I32Const(0), Instr::I32Eqz, Instr::End];
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(1))));
    }

    #[test]
    fn if_else_selects_branch() {
        // if (0) { 111 } else { 222 } → local 0
        let body = [
            Instr::I32Const(0),
            Instr::If,
            Instr::I32Const(111),
            Instr::LocalSet(0),
            Instr::Else,
            Instr::I32Const(222),
            Instr::LocalSet(0),
            Instr::End,
            Instr::LocalGet(0),
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 1), Ok(Some(Value::I32(222))));
    }

    #[test]
    fn loop_sums_1_to_5() {
        // local0 = counter, local1 = acc
        let body = [
            Instr::I32Const(5),
            Instr::LocalSet(0),
            Instr::Block,
            Instr::Loop,
            Instr::LocalGet(0),
            Instr::I32Eqz,
            Instr::BrIf(1), // exit block when 0
            Instr::LocalGet(1),
            Instr::LocalGet(0),
            Instr::I32Add,
            Instr::LocalSet(1),
            Instr::LocalGet(0),
            Instr::I32Const(1),
            Instr::I32Sub,
            Instr::LocalSet(0),
            Instr::Br(0), // continue loop
            Instr::End,
            Instr::End,
            Instr::LocalGet(1),
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 2), Ok(Some(Value::I32(15))));
    }

    #[test]
    fn finished_if_leaves_no_control_frame() {
        // A taken then-arm (and a skipped else-less if) must pop their control frame,
        // or the br below resolves its depth against a stale frame.
        // block(outer) { block(inner) { if(1){} else {}  if(0){}  br 1 }  local0 = 99 }
        let body = [
            Instr::Block,
            Instr::Block,
            Instr::I32Const(1),
            Instr::If,
            Instr::Else,
            Instr::End,
            Instr::I32Const(0),
            Instr::If,
            Instr::End,
            Instr::Br(1), // → past outer's End: skips the store
            Instr::End,
            Instr::I32Const(99),
            Instr::LocalSet(0),
            Instr::End,
            Instr::LocalGet(0),
            Instr::End,
        ];
        assert_eq!(eval_body(&body, 1), Ok(Some(Value::I32(0))));
    }

    fn single_export_module(funcs: Vec<Function>) -> Module {
        Module {
            types: funcs.iter().map(|f| f.ty).collect(),
            exports: vec![Export {
                name: "main".into(),
                func_idx: (funcs.len() - 1) as u32,
            }],
            funcs,
            memory: None,
        }
    }

    #[test]
    fn call_add_function() {
        // f0(a,b) = a+b ; main() = f0(2,3)
        let add = Function {
            ty: FuncType {
                params: 2,
                results: 1,
            },
            n_locals: 0,
            body: vec![
                Instr::LocalGet(0),
                Instr::LocalGet(1),
                Instr::I32Add,
                Instr::End,
            ],
        };
        let main = Function {
            ty: FuncType {
                params: 0,
                results: 1,
            },
            n_locals: 0,
            body: vec![
                Instr::I32Const(2),
                Instr::I32Const(3),
                Instr::Call(0),
                Instr::End,
            ],
        };
        let m = single_export_module(vec![add, main]);
        assert_eq!(run(&m, 1, &[]), Ok(Some(Value::I32(5))));
    }

    fn factorial() -> Function {
        // f0(n) = if n<=1 {1} else { n * f0(n-1) }  — guest recursion, host loop
        Function {
            ty: FuncType {
                params: 1,
                results: 1,
            },
            n_locals: 1, // local1 = result
            body: vec![
                Instr::LocalGet(0),
                Instr::I32Const(1),
                Instr::I32LeS,
                Instr::If,
                Instr::I32Const(1),
                Instr::LocalSet(1),
                Instr::Else,
                Instr::LocalGet(0),
                Instr::LocalGet(0),
                Instr::I32Const(1),
                Instr::I32Sub,
                Instr::Call(0),
                Instr::I32Mul,
                Instr::LocalSet(1),
                Instr::End,
                Instr::LocalGet(1),
                Instr::End,
            ],
        }
    }

    #[test]
    fn recursive_factorial() {
        let m = single_export_module(vec![factorial()]);
        assert_eq!(run(&m, 0, &[Value::I32(10)]), Ok(Some(Value::I32(3628800))));
    }

    #[test]
    fn br_out_of_function_returns_to_caller() {
        // f0() { 7; br 0; unreachable }  — br to the function body itself is a return.
        // main() { block { local0 = f0() } local0 } — f0's br must not touch main's block.
        let f0 = Function {
            ty: FuncType {
                params: 0,
                results: 1,
            },
            n_locals: 0,
            body: vec![
                Instr::I32Const(7),
                Instr::Br(0),
                Instr::Unreachable,
                Instr::End,
            ],
        };
        let main = Function {
            ty: FuncType {
                params: 0,
                results: 1,
            },
            n_locals: 1,
            body: vec![
                Instr::Block,
                Instr::Call(0),
                Instr::LocalSet(0),
                Instr::End,
                Instr::LocalGet(0),
                Instr::End,
            ],
        };
        let m = single_export_module(vec![f0, main]);
        assert_eq!(run(&m, 1, &[]), Ok(Some(Value::I32(7))));
    }

    #[test]
    fn run_rejects_wrong_arity() {
        let m = single_export_module(vec![factorial()]);
        assert_eq!(run(&m, 0, &[]), Err(Trap::TypeMismatch));
    }
}
