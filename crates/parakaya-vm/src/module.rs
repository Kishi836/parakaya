use crate::instr::Instr;

/// M1 is i32-only, so a signature is just its arity. Real valtypes arrive in M2.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FuncType {
    pub params: usize,
    pub results: usize,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub ty: FuncType,
    /// Declared locals, not counting params.
    pub n_locals: usize,
    pub body: Vec<Instr>,
}

#[derive(Clone, Debug)]
pub struct Export {
    pub name: String,
    pub func_idx: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct MemoryDecl {
    pub min_pages: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub types: Vec<FuncType>,
    pub funcs: Vec<Function>,
    pub exports: Vec<Export>,
    pub memory: Option<MemoryDecl>,
}
