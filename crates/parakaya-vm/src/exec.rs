use crate::error::Trap;
use crate::instr::Instr;
use crate::value::Value;

/// One open `block`/`loop`/`if`. `height` is the value-stack depth at entry,
/// restored when a branch exits through this frame.
#[derive(Copy, Clone, Debug)]
pub struct CtrlFrame {
    pub opener_pc: usize,
    pub end_pc: usize,
    pub is_loop: bool,
    pub height: usize,
}

#[derive(Debug, Default)]
pub struct Interp {
    pub values: Vec<Value>,
    pub ctrls: Vec<CtrlFrame>,
    pub locals: Vec<Value>,
    pub pc: usize,
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

pub fn eval_body(instrs: &[Instr], n_locals: usize) -> Result<Option<Value>, Trap> {
    let mut it = Interp {
        locals: vec![Value::I32(0); n_locals],
        ..Interp::default()
    };
    let jumps = crate::jumps::compute(instrs);
    while it.pc < instrs.len() {
        match &instrs[it.pc] {
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
                let j = jumps[&it.pc];
                it.ctrls.push(CtrlFrame {
                    opener_pc: it.pc,
                    end_pc: j.end_pc,
                    is_loop: matches!(instrs[it.pc], Instr::Loop),
                    height: it.values.len(),
                });
            }
            Instr::If => {
                let j = jumps[&it.pc];
                let cond = it.pop_i32()?;
                if cond == 0 && j.else_pc.is_none() {
                    // no arm runs: skip past End without ever opening a frame
                    it.pc = j.end_pc;
                } else {
                    it.ctrls.push(CtrlFrame {
                        opener_pc: it.pc,
                        end_pc: j.end_pc,
                        is_loop: false,
                        height: it.values.len(),
                    });
                    if cond == 0 {
                        it.pc = j.else_pc.unwrap(); // else-arm starts after Else
                    }
                }
            }
            Instr::Else => {
                // reached only by falling out of the then-arm: close the if, skip past End
                let frame = it.ctrls.pop().ok_or(Trap::StackUnderflow)?;
                it.pc = frame.end_pc;
            }
            Instr::Br(n) => branch(&mut it, *n)?,
            Instr::BrIf(n) => {
                if it.pop_i32()? != 0 {
                    branch(&mut it, *n)?;
                }
            }
            Instr::End => {
                if it.ctrls.pop().is_none() {
                    break; // function-body End
                }
            }
            Instr::LocalGet(i) => {
                let v = it.locals[*i as usize];
                it.push(v);
            }
            Instr::LocalSet(i) => {
                let v = it.pop()?;
                it.locals[*i as usize] = v;
            }
            Instr::LocalTee(i) => {
                let v = it.pop()?;
                it.locals[*i as usize] = v;
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
        it.pc += 1;
    }
    Ok(it.values.pop())
}

/// Branch to the control frame `depth` levels out. Loops re-enter at their opener;
/// blocks/ifs exit past their End. The pc lands one before the next instruction
/// (the dispatch loop increments it).
fn branch(it: &mut Interp, depth: u32) -> Result<(), Trap> {
    let idx = it
        .ctrls
        .len()
        .checked_sub(depth as usize + 1)
        .ok_or(Trap::StackUnderflow)?;
    let target = it.ctrls[idx];
    it.values.truncate(target.height);
    if target.is_loop {
        it.ctrls.truncate(idx + 1); // loop frame stays; continue from opener
        it.pc = target.opener_pc;
    } else {
        it.ctrls.truncate(idx); // block/if frame exits
        it.pc = target.end_pc;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
