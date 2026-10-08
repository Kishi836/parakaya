//! `.wasm` binary → `Module`, for the M1 subset. Trusts structure beyond what it must
//! check to stay in bounds; real validation arrives with the spec test suite in M2.

use crate::error::DecodeError;
use crate::instr::{Instr, MemArg};
use crate::module::{Export, FuncType, Function, MemoryDecl, Module};

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.bytes.get(self.pos).ok_or(DecodeError::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let s = self
            .bytes
            .get(self.pos..self.pos + n)
            .ok_or(DecodeError::UnexpectedEof)?;
        self.pos += n;
        Ok(s)
    }

    /// Unsigned LEB128: 7 bits per byte, low group first, high bit = "more follows".
    fn u32_leb(&mut self) -> Result<u32, DecodeError> {
        let mut result: u32 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            result |= u32::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 35 {
                return Err(DecodeError::Malformed("u32 LEB too long"));
            }
        }
    }

    /// Signed LEB128: as unsigned, then sign-extend from bit 6 of the last byte.
    fn i32_leb(&mut self) -> Result<i32, DecodeError> {
        let mut result: i32 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            result |= i32::from(b & 0x7F) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                if shift < 32 && b & 0x40 != 0 {
                    result |= -1i32 << shift; // sign-extend
                }
                return Ok(result);
            }
            if shift >= 35 {
                return Err(DecodeError::Malformed("i32 LEB too long"));
            }
        }
    }

    fn name(&mut self) -> Result<String, DecodeError> {
        let len = self.u32_leb()? as usize;
        let raw = self.bytes(len)?;
        String::from_utf8(raw.to_vec()).map_err(|_| DecodeError::Malformed("bad utf8 name"))
    }
}

pub fn module(bytes: &[u8]) -> Result<Module, DecodeError> {
    let mut r = Reader { bytes, pos: 0 };
    if r.bytes(4)? != b"\0asm" {
        return Err(DecodeError::BadMagic);
    }
    if r.bytes(4)? != [1, 0, 0, 0] {
        return Err(DecodeError::BadMagic);
    }

    let mut m = Module::default();
    let mut func_type_idx: Vec<u32> = Vec::new();

    while r.pos < r.bytes.len() {
        let id = r.u8()?;
        let size = r.u32_leb()? as usize;
        let section_end = r.pos + size;
        if section_end > r.bytes.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        match id {
            1 => {
                // type section: 0x60 form, params, results (i32 = 0x7F only in M1)
                let count = r.u32_leb()?;
                for _ in 0..count {
                    if r.u8()? != 0x60 {
                        return Err(DecodeError::Malformed("expected func type form"));
                    }
                    let n_params = r.u32_leb()? as usize;
                    for _ in 0..n_params {
                        expect_i32_valtype(&mut r)?;
                    }
                    let n_results = r.u32_leb()? as usize;
                    for _ in 0..n_results {
                        expect_i32_valtype(&mut r)?;
                    }
                    m.types.push(FuncType {
                        params: n_params,
                        results: n_results,
                    });
                }
            }
            3 => {
                // function section: type index per function
                let count = r.u32_leb()?;
                for _ in 0..count {
                    func_type_idx.push(r.u32_leb()?);
                }
            }
            5 => {
                // memory section
                let count = r.u32_leb()?;
                if count > 1 {
                    return Err(DecodeError::Malformed("multiple memories"));
                }
                if count == 1 {
                    let flags = r.u8()?;
                    let min = r.u32_leb()?;
                    if flags == 0x01 {
                        r.u32_leb()?; // max: parsed, ignored in M1
                    }
                    m.memory = Some(MemoryDecl { min_pages: min });
                }
            }
            7 => {
                // export section (funcs only in M1)
                let count = r.u32_leb()?;
                for _ in 0..count {
                    let name = r.name()?;
                    let kind = r.u8()?;
                    let idx = r.u32_leb()?;
                    if kind == 0x00 {
                        m.exports.push(Export {
                            name,
                            func_idx: idx,
                        });
                    } // other export kinds: skipped
                }
            }
            10 => {
                // code section
                let count = r.u32_leb()?;
                for i in 0..count {
                    let _body_size = r.u32_leb()?;
                    let mut n_locals = 0usize;
                    let n_local_groups = r.u32_leb()?;
                    for _ in 0..n_local_groups {
                        let n = r.u32_leb()? as usize;
                        expect_i32_valtype(&mut r)?;
                        n_locals += n;
                    }
                    let body = decode_instrs(&mut r)?;
                    let ty_idx = *func_type_idx
                        .get(i as usize)
                        .ok_or(DecodeError::Malformed("code without func decl"))?
                        as usize;
                    let ty = *m
                        .types
                        .get(ty_idx)
                        .ok_or(DecodeError::Malformed("bad type idx"))?;
                    m.funcs.push(Function { ty, n_locals, body });
                }
            }
            0 => {
                r.pos = section_end; // custom section: skip
            }
            other => return Err(DecodeError::UnsupportedSection(other)),
        }
        if r.pos != section_end {
            return Err(DecodeError::Malformed("section size mismatch"));
        }
    }
    Ok(m)
}

fn expect_i32_valtype(r: &mut Reader) -> Result<(), DecodeError> {
    match r.u8()? {
        0x7F => Ok(()),
        _ => Err(DecodeError::Malformed("M1 supports i32 valtypes only")),
    }
}

/// Decode one function body up to and including its final `End`.
fn decode_instrs(r: &mut Reader) -> Result<Vec<Instr>, DecodeError> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    loop {
        let op = r.u8()?;
        let ins = match op {
            0x00 => Instr::Unreachable,
            0x01 => Instr::Nop,
            0x02 => {
                block_type(r)?;
                depth += 1;
                Instr::Block
            }
            0x03 => {
                block_type(r)?;
                depth += 1;
                Instr::Loop
            }
            0x04 => {
                block_type(r)?;
                depth += 1;
                Instr::If
            }
            0x05 => Instr::Else,
            0x0B => {
                if depth == 0 {
                    out.push(Instr::End);
                    return Ok(out);
                }
                depth -= 1;
                Instr::End
            }
            0x0C => Instr::Br(r.u32_leb()?),
            0x0D => Instr::BrIf(r.u32_leb()?),
            0x0F => Instr::Return,
            0x10 => Instr::Call(r.u32_leb()?),
            0x1A => Instr::Drop,
            0x20 => Instr::LocalGet(r.u32_leb()?),
            0x21 => Instr::LocalSet(r.u32_leb()?),
            0x22 => Instr::LocalTee(r.u32_leb()?),
            0x28 => Instr::I32Load(mem_arg(r)?),
            0x36 => Instr::I32Store(mem_arg(r)?),
            0x41 => Instr::I32Const(r.i32_leb()?),
            0x45 => Instr::I32Eqz,
            0x46 => Instr::I32Eq,
            0x47 => Instr::I32Ne,
            0x48 => Instr::I32LtS,
            0x4A => Instr::I32GtS,
            0x4C => Instr::I32LeS,
            0x4E => Instr::I32GeS,
            0x6A => Instr::I32Add,
            0x6B => Instr::I32Sub,
            0x6C => Instr::I32Mul,
            0x6D => Instr::I32DivS,
            0x6F => Instr::I32RemS,
            other => return Err(DecodeError::UnsupportedOpcode(other)),
        };
        out.push(ins);
    }
}

fn block_type(r: &mut Reader) -> Result<(), DecodeError> {
    match r.u8()? {
        0x40 => Ok(()), // void — the only blocktype in M1
        _ => Err(DecodeError::Malformed("M1 supports void blocktype only")),
    }
}

fn mem_arg(r: &mut Reader) -> Result<MemArg, DecodeError> {
    r.u32_leb()?; // alignment hint: parsed, ignored
    Ok(MemArg {
        offset: r.u32_leb()?,
    })
}

#[cfg(test)]
mod tests {
    use crate::decode;
    use crate::error::DecodeError;
    use crate::instr::Instr;
    use crate::module::FuncType;

    #[test]
    fn decodes_add_module() {
        let bytes = wat::parse_str(
            r#"(module
                 (func (export "add") (param i32 i32) (result i32)
                   local.get 0
                   local.get 1
                   i32.add))"#,
        )
        .unwrap();
        let m = decode::module(&bytes).unwrap();
        assert_eq!(m.funcs.len(), 1);
        assert_eq!(
            m.funcs[0].ty,
            FuncType {
                params: 2,
                results: 1
            }
        );
        assert_eq!(m.exports[0].name, "add");
        assert_eq!(
            m.funcs[0].body,
            vec![
                Instr::LocalGet(0),
                Instr::LocalGet(1),
                Instr::I32Add,
                Instr::End
            ]
        );
    }

    #[test]
    fn rejects_bad_magic() {
        assert_eq!(decode::module(b"nope").unwrap_err(), DecodeError::BadMagic);
    }

    #[test]
    fn rejects_section_running_past_eof() {
        // valid header, then a custom section (id 0) claiming 100 bytes that aren't there
        let bytes = [0, b'a', b's', b'm', 1, 0, 0, 0, 0, 100];
        assert_eq!(
            decode::module(&bytes).unwrap_err(),
            DecodeError::UnexpectedEof
        );
    }
}
