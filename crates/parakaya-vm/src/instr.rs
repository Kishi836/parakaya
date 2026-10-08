#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MemArg {
    pub offset: u32, //offset is a 32-bit unsigned integer
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    // Define the different instruction variants here
    I32Const(i32),
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32RemS,
    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LeS,
    I32GtS,
    I32GeS,
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    Drop,
    Nop,
    Unreachable,
    Block,
    Loop,
    If,
    Else,
    Br(u32),
    BrIf(u32),
    Return,
    Call(u32),
    I32Load(MemArg),
    I32Store(MemArg),
    End,
}
