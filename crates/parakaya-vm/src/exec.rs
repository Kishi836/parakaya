use crate::error::Trap;
use crate::value::Value;
use crate::instr::Instr;

#[derive(Debug, Default)]
pub struct Interp {
    pub values: Vec<Value>,
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
                if b == 0 { Err(Trap::DivByZero) } else { Ok(a.wrapping_rem(b)) }
            })?,
            Instr::Nop => {}
            Instr::Drop => {
                it.pop()?;
            }
            Instr::Unreachable => return Err(Trap::Unreachable),
            Instr::End => break,
            other => unimplemented!("instr {:?} arrives in a later task", other),
        }
        it.pc += 1;
    }
    Ok(it.values.pop())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_add() {
        let body = [Instr::I32Const(2), Instr::I32Const(3), Instr::I32Add, Instr::End]; // 2 + 3 = 5
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(5)))); // assert that the result of evaluating the body is equal to 5
    }

    #[test]
    fn arithmetic_wraps_and_divides() {
        let body = [Instr::I32Const(i32::MAX), Instr::I32Const(1), Instr::I32Add, Instr::End]; // i32::MAX + 1 = i32::MIN (wraps around)
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(i32::MIN)))); // wrapping
        let body = [Instr::I32Const(7), Instr::I32Const(2), Instr::I32DivS, Instr::End]; // 7 / 2 = 3
        assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(3)))); // assert that the result of evaluating the body is equal to 3
    }
}
