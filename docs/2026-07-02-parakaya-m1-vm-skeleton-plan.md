# Parakaya M1 — VM Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust WASM interpreter skeleton that loads a real `.wasm` binary (i32 subset) and executes it correctly — Milestone 1 exit criterion: "hand-written WASM subset executes."

**Architecture:** Flat-instruction interpreter with an explicit program counter, shared value stack, explicit control stack, and explicit call-frame stack — **no Rust recursion for guest calls** — so that in M2, snapshotting is "serialize these structs." Branch targets are precomputed at load time into a jump table.

**Tech Stack:** Rust (stable, edition 2024), cargo workspace. Dev-dependency: `wat` (compiles WebAssembly text to binary inside tests). No runtime dependencies.

## Global Constraints

- **Deadline context:** M1 = July 2026. Exit criterion: end-to-end test running WAT-compiled factorial + memory programs passes in CI.
- **Snapshot-readiness (spec §5):** all VM state lives in plain owned data structures (`Vec`, primitives). Forbidden in VM state: references, `Rc`/`RefCell`, raw pointers, host recursion for guest calls.
- `#![forbid(unsafe_code)]` in every crate.
- M1 instruction subset: i32 const/arithmetic/comparison, locals, `drop`/`nop`/`unreachable`, `block`/`loop`/`if`/`else` (empty blocktype `0x40` only), `br`/`br_if`/`return`/`call`, `i32.load`/`i32.store`, `end`. i64/f32/f64/globals/tables deferred to M2 (`Value` enum still carries all four variants to avoid churn).
- Functions return 0 or 1 values (WASM MVP): public API returns `Option<Value>`.
- Every task: rustfmt clean, `cargo clippy -- -D warnings` clean, conventional-commit messages.
- Repo path: `C:\Users\Admin\OneDrive\PES UNIVERSITY CSE\Projects\Parakaya`. All commands below run
  from there (PowerShell).
- **OneDrive guard:** build artifacts must NOT sync. A local-only (git-ignored) `.cargo\config.toml`
  redirects the target dir to `C:\Users\Admin\.cargo-target\parakaya`. Never commit this file — it
  holds an absolute Windows path that would break Linux CI.

## File Structure

```
...\Projects\Parakaya\
├── Cargo.toml                     workspace root
├── README.md                      name, tagline, status
├── docs\
│   ├── 2026-07-02-parakaya-design.md      (copied from brainstorm folder)
│   └── DECISION-HISTORY.md                (copied from brainstorm folder)
├── .github\workflows\ci.yml       fmt + clippy + test on push
└── crates\parakaya-vm\
    ├── Cargo.toml
    ├── src\
    │   ├── lib.rs                 public API: Vm::instantiate / invoke
    │   ├── value.rs               Value
    │   ├── error.rs               Trap, DecodeError
    │   ├── instr.rs               Instr, MemArg
    │   ├── module.rs              Module, FuncType, Function, Export, MemoryDecl
    │   ├── jumps.rs               prescan: matching end/else for every opener
    │   ├── exec.rs                the interpreter loop (state = plain data)
    │   └── decode.rs              .wasm binary parser (LEB128 + sections)
    └── tests\
        └── end_to_end.rs          WAT → decode → execute integration tests
```

---

### Task 1: Repository, workspace, CI

**Files:**
- Create: `Cargo.toml`, `crates\parakaya-vm\Cargo.toml`, `crates\parakaya-vm\src\lib.rs`, `README.md`, `.github\workflows\ci.yml`, `.gitignore`
- Copy: spec + decision history into `docs\`

**Interfaces:**
- Produces: compiling workspace with one library crate `parakaya-vm`; CI running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.

- [ ] **Step 1: Create repo in the Projects folder**

```powershell
New-Item -ItemType Directory -Force "C:\Users\Admin\OneDrive\PES UNIVERSITY CSE\Projects\Parakaya"
Set-Location "C:\Users\Admin\OneDrive\PES UNIVERSITY CSE\Projects\Parakaya"
git init -b main
New-Item -ItemType Directory -Force docs, .cargo
Copy-Item "..\_next-project-brainstorm\2026-07-02-parakaya-design.md" docs\
Copy-Item "..\_next-project-brainstorm\DECISION-HISTORY.md" docs\
Copy-Item "..\_next-project-brainstorm\2026-07-02-parakaya-m1-vm-skeleton-plan.md" docs\
```

`.cargo\config.toml` (local-only, git-ignored — keeps build churn out of OneDrive):
```toml
[build]
target-dir = "C:/Users/Admin/.cargo-target/parakaya"
```

Then run `/setup-matt-pocock-skills` inside this repo (scaffolds issue-tracker/triage-labels/
domain-docs config for the engineering skills) — per global setup notes, once per new big repo.

- [ ] **Step 2: Workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/parakaya-vm"]
```

`crates\parakaya-vm\Cargo.toml`:
```toml
[package]
name = "parakaya-vm"
version = "0.1.0"
edition = "2024"

[dev-dependencies]
wat = "1"
```

`crates\parakaya-vm\src\lib.rs`:
```rust
#![forbid(unsafe_code)]

pub fn crate_wired() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles() {
        assert!(super::crate_wired());
    }
}
```

`.gitignore`:
```
/target
/.cargo/config.toml
```

`README.md`:
```markdown
# Parakaya

> Return by Death — live process migration across heterogeneous devices.

A running program hops between your PC, Raspberry Pi, phone, and a cloud VM —
mid-execution, across CPU architectures, surviving power loss at any instant.

Status: M1 (VM skeleton) in progress. Design: `docs/2026-07-02-parakaya-design.md`.
```

- [ ] **Step 3: Verify it builds and tests pass**

Run: `cargo test`
Expected: `test tests::workspace_compiles ... ok`

- [ ] **Step 4: CI workflow**

`.github\workflows\ci.yml`:
```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: "rustfmt, clippy" }
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
```

- [ ] **Step 5: Commit (and create the GitHub repo when ready)**

```powershell
git add -A
git commit -m "chore: workspace scaffold, CI, design docs"
```

---

### Task 2: Values and straight-line arithmetic execution

**Files:**
- Create: `src\value.rs`, `src\error.rs`, `src\instr.rs`, `src\exec.rs`
- Modify: `src\lib.rs`

**Interfaces:**
- Produces:
  - `Value::{I32(i32), I64(i64), F32(f32), F64(f64)}` (`Copy, Clone, Debug, PartialEq`)
  - `Trap::{Unreachable, DivByZero, OutOfBoundsMemory, StackUnderflow, TypeMismatch}`
  - `Instr` (M1 subset per Global Constraints), `MemArg { offset: u32 }`
  - `exec::Interp { values: Vec<Value>, ... }` with `fn push(&mut self, v: Value)` / `fn pop_i32(&mut self) -> Result<i32, Trap>`
  - free fn used by tests until Task 5: `exec::eval_body(instrs: &[Instr], n_locals: usize) -> Result<Option<Value>, Trap>`

- [ ] **Step 1: Write the failing test** (in `src\exec.rs` `#[cfg(test)]`)

```rust
#[test]
fn const_add() {
    let body = [Instr::I32Const(2), Instr::I32Const(3), Instr::I32Add, Instr::End];
    assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(5))));
}

#[test]
fn arithmetic_wraps_and_divides() {
    let body = [Instr::I32Const(i32::MAX), Instr::I32Const(1), Instr::I32Add, Instr::End];
    assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(i32::MIN)))); // wrapping
    let body = [Instr::I32Const(7), Instr::I32Const(2), Instr::I32DivS, Instr::End];
    assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(3))));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p parakaya-vm` → compile error (types don't exist).

- [ ] **Step 3: Implement**

`src\value.rs`:
```rust
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}
```

`src\error.rs`:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    Unreachable,
    DivByZero,
    OutOfBoundsMemory,
    StackUnderflow,
    TypeMismatch,
}
```

`src\instr.rs`:
```rust
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MemArg {
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    I32Const(i32),
    I32Add, I32Sub, I32Mul, I32DivS, I32RemS,
    I32Eqz, I32Eq, I32Ne, I32LtS, I32LeS, I32GtS, I32GeS,
    LocalGet(u32), LocalSet(u32), LocalTee(u32),
    Drop, Nop, Unreachable,
    Block, Loop, If, Else,
    Br(u32), BrIf(u32), Return, Call(u32),
    I32Load(MemArg), I32Store(MemArg),
    End,
}
```

`src\exec.rs` (core loop, extended in later tasks; binary ops via a helper to stay DRY):
```rust
use crate::error::Trap;
use crate::instr::Instr;
use crate::value::Value;

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
            Instr::I32DivS => it.bin_i32(|a, b| {
                if b == 0 { Err(Trap::DivByZero) } else { Ok(a.wrapping_div(b)) }
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
```

`src\lib.rs` — replace placeholder:
```rust
#![forbid(unsafe_code)]

pub mod error;
pub mod exec;
pub mod instr;
pub mod value;

pub use error::Trap;
pub use value::Value;
```

- [ ] **Step 4: Run tests** — `cargo test -p parakaya-vm` → both PASS. (Add a `div_by_zero_traps` test asserting `Err(Trap::DivByZero)` while here.)

- [ ] **Step 5: Commit** — `git add -A; git commit -m "feat(vm): value stack and i32 arithmetic execution"`

---

### Task 3: Locals and comparisons

**Files:** Modify: `src\exec.rs`

**Interfaces:**
- Consumes: `Interp`, `eval_body` from Task 2.
- Produces: working `LocalGet/LocalSet/LocalTee`, all i32 comparison ops (result `i32` 0/1 per WASM).

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn locals_roundtrip() {
    let body = [
        Instr::I32Const(41), Instr::LocalSet(0),
        Instr::LocalGet(0), Instr::I32Const(1), Instr::I32Add,
        Instr::LocalTee(1), Instr::Drop, Instr::LocalGet(1), Instr::End,
    ];
    assert_eq!(eval_body(&body, 2), Ok(Some(Value::I32(42))));
}

#[test]
fn comparisons_yield_0_or_1() {
    let body = [Instr::I32Const(3), Instr::I32Const(5), Instr::I32LtS, Instr::End];
    assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(1))));
    let body = [Instr::I32Const(0), Instr::I32Eqz, Instr::End];
    assert_eq!(eval_body(&body, 0), Ok(Some(Value::I32(1))));
}
```

- [ ] **Step 2: Run — fail** (hits `unimplemented!`).

- [ ] **Step 3: Implement** — add arms in the match:

```rust
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
```

- [ ] **Step 4: Run — pass.**  
- [ ] **Step 5: Commit** — `git commit -am "feat(vm): locals and i32 comparisons"`

---

### Task 4: Control flow (jump table + control stack)

**Files:**
- Create: `src\jumps.rs`
- Modify: `src\exec.rs`, `src\lib.rs` (add `pub mod jumps;`)

**Interfaces:**
- Produces:
  - `jumps::JumpInfo { end_pc: usize, else_pc: Option<usize> }`
  - `jumps::compute(instrs: &[Instr]) -> std::collections::HashMap<usize, JumpInfo>` — for every `Block`/`Loop`/`If` pc, its matching `End` (and `Else` for `If`). Panics on malformed nesting (decoder guarantees well-formed input; direct-construction tests must be well-formed).
  - `exec::CtrlFrame { opener_pc: usize, end_pc: usize, is_loop: bool, height: usize }`; `Interp.ctrls: Vec<CtrlFrame>`
  - `eval_body` gains branching semantics: `Br(n)`/`BrIf(n)` unwind `n+1` control frames; loops jump to `opener_pc`, blocks/ifs jump past `end_pc`; value stack truncated to the target frame's `height` (M1 blocks are void-typed, blocktype `0x40` only).

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn if_else_selects_branch() {
    // if (0) { 111 } else { 222 } → local 0
    let body = [
        Instr::I32Const(0), Instr::If,
        Instr::I32Const(111), Instr::LocalSet(0),
        Instr::Else,
        Instr::I32Const(222), Instr::LocalSet(0),
        Instr::End,
        Instr::LocalGet(0), Instr::End,
    ];
    assert_eq!(eval_body(&body, 1), Ok(Some(Value::I32(222))));
}

#[test]
fn loop_sums_1_to_5() {
    // local0 = counter, local1 = acc
    let body = [
        Instr::I32Const(5), Instr::LocalSet(0),
        Instr::Block,
            Instr::Loop,
                Instr::LocalGet(0), Instr::I32Eqz, Instr::BrIf(1),      // exit block when 0
                Instr::LocalGet(1), Instr::LocalGet(0), Instr::I32Add, Instr::LocalSet(1),
                Instr::LocalGet(0), Instr::I32Const(1), Instr::I32Sub, Instr::LocalSet(0),
                Instr::Br(0),                                            // continue loop
            Instr::End,
        Instr::End,
        Instr::LocalGet(1), Instr::End,
    ];
    assert_eq!(eval_body(&body, 2), Ok(Some(Value::I32(15))));
}
```

- [ ] **Step 2: Run — fail.**

- [ ] **Step 3: Implement**

`src\jumps.rs`:
```rust
use crate::instr::Instr;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug)]
pub struct JumpInfo {
    pub end_pc: usize,
    pub else_pc: Option<usize>,
}

pub fn compute(instrs: &[Instr]) -> HashMap<usize, JumpInfo> {
    let mut map = HashMap::new();
    let mut openers: Vec<usize> = Vec::new();
    for (pc, ins) in instrs.iter().enumerate() {
        match ins {
            Instr::Block | Instr::Loop | Instr::If => openers.push(pc),
            Instr::Else => {
                let opener = *openers.last().expect("else without if");
                map.insert(opener, JumpInfo { end_pc: usize::MAX, else_pc: Some(pc) });
            }
            Instr::End => {
                if let Some(opener) = openers.pop() {
                    let else_pc = map.get(&opener).and_then(|j| j.else_pc);
                    map.insert(opener, JumpInfo { end_pc: pc, else_pc });
                }
                // an End with no opener terminates the function body: ignore
            }
            _ => {}
        }
    }
    map
}
```

`src\exec.rs` — add to `Interp`: `pub ctrls: Vec<CtrlFrame>`; new struct:
```rust
#[derive(Copy, Clone, Debug)]
pub struct CtrlFrame {
    pub opener_pc: usize,
    pub end_pc: usize,
    pub is_loop: bool,
    pub height: usize,
}
```
`eval_body` computes `let jumps = crate::jumps::compute(instrs);` up front, and the match gains:
```rust
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
    it.ctrls.push(CtrlFrame {
        opener_pc: it.pc, end_pc: j.end_pc, is_loop: false, height: it.values.len(),
    });
    if cond == 0 {
        // jump to else-arm start, or to End if no else
        it.pc = j.else_pc.unwrap_or(j.end_pc);
    }
}
Instr::Else => {
    // reached only by falling out of the then-arm: skip to End
    let frame = it.ctrls.last().ok_or(Trap::StackUnderflow)?;
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
```
with the helper (free fn in `exec.rs`):
```rust
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
        it.ctrls.truncate(idx);     // block/if frame exits
        it.pc = target.end_pc;
    }
    Ok(())
}
```

- [ ] **Step 4: Run — pass** (also re-run all previous tests).  
- [ ] **Step 5: Commit** — `git commit -am "feat(vm): structured control flow via jump table + control stack"`

---

### Task 5: Functions, call frames, module (no host recursion)

**Files:**
- Create: `src\module.rs`
- Modify: `src\exec.rs` (replace `eval_body` internals with frame-based `run`), `src\lib.rs` (add `pub mod module;`)

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `module::FuncType { params: usize, results: usize }` (M1: i32-only, so arity suffices; real valtypes in M2)
  - `module::Function { ty: FuncType, n_locals: usize, body: Vec<Instr> }`
  - `module::Export { name: String, func_idx: u32 }`
  - `module::MemoryDecl { min_pages: u32 }`
  - `module::Module { types: Vec<FuncType>, funcs: Vec<Function>, exports: Vec<Export>, memory: Option<MemoryDecl> }`
  - `exec::Frame { func_idx: u32, pc: usize, locals: Vec<Value>, value_base: usize, ctrl_base: usize }`
  - `exec::run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap>` — drives an explicit `Vec<Frame>`; `Call` pushes a frame, `Return`/body-`End` pops one. **The host never recurses.**
  - `eval_body(instrs, n_locals)` retained as a thin wrapper (wraps instrs in a single-function module) so Tasks 2–4 tests keep passing unchanged.

- [ ] **Step 1: Failing tests**

```rust
fn single_export_module(funcs: Vec<Function>) -> Module {
    Module {
        types: funcs.iter().map(|f| f.ty).collect(),
        exports: vec![Export { name: "main".into(), func_idx: (funcs.len() - 1) as u32 }],
        funcs,
        memory: None,
    }
}

#[test]
fn call_add_function() {
    // f0(a,b) = a+b ; main() = f0(2,3)
    let add = Function {
        ty: FuncType { params: 2, results: 1 },
        n_locals: 0,
        body: vec![Instr::LocalGet(0), Instr::LocalGet(1), Instr::I32Add, Instr::End],
    };
    let main = Function {
        ty: FuncType { params: 0, results: 1 },
        n_locals: 0,
        body: vec![Instr::I32Const(2), Instr::I32Const(3), Instr::Call(0), Instr::End],
    };
    let m = single_export_module(vec![add, main]);
    assert_eq!(run(&m, 1, &[]), Ok(Some(Value::I32(5))));
}

#[test]
fn recursive_factorial() {
    // f0(n) = if n<=1 {1} else { n * f0(n-1) }  — guest recursion, host loop
    let fac = Function {
        ty: FuncType { params: 1, results: 1 },
        n_locals: 1, // local1 = result
        body: vec![
            Instr::LocalGet(0), Instr::I32Const(1), Instr::I32LeS,
            Instr::If,
                Instr::I32Const(1), Instr::LocalSet(1),
            Instr::Else,
                Instr::LocalGet(0),
                Instr::LocalGet(0), Instr::I32Const(1), Instr::I32Sub,
                Instr::Call(0), Instr::I32Mul, Instr::LocalSet(1),
            Instr::End,
            Instr::LocalGet(1), Instr::End,
        ],
    };
    let m = single_export_module(vec![fac]);
    assert_eq!(run(&m, 0, &[Value::I32(10)]), Ok(Some(Value::I32(3628800))));
}
```

- [ ] **Step 2: Run — fail.**

- [ ] **Step 3: Implement**

`src\module.rs`:
```rust
use crate::instr::Instr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FuncType {
    pub params: usize,
    pub results: usize,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub ty: FuncType,
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

impl Default for FuncType {
    fn default() -> Self {
        FuncType { params: 0, results: 0 }
    }
}
```

`src\exec.rs` — restructure (state stays plain data; this is the shape M2 serializes):
```rust
#[derive(Clone, Debug)]
pub struct Frame {
    pub func_idx: u32,
    pub pc: usize,
    pub locals: Vec<Value>,
    pub value_base: usize,
    pub ctrl_base: usize,
}

pub fn run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap> {
    let mut it = Interp::default();
    push_frame(&mut it, module, func_idx, args)?;
    // per-frame jump tables, computed lazily and cached
    let mut jump_cache: Vec<Option<std::collections::HashMap<usize, JumpInfo>>> =
        vec![None; module.funcs.len()];

    while let Some(frame_idx) = it.frames.len().checked_sub(1) {
        let func_idx = it.frames[frame_idx].func_idx as usize;
        let body = &module.funcs[func_idx].body;
        let pc = it.frames[frame_idx].pc;
        if pc >= body.len() {
            pop_frame(&mut it, module)?;
            continue;
        }
        let jumps = jump_cache[func_idx]
            .get_or_insert_with(|| crate::jumps::compute(body))
            .clone();
        step(&mut it, module, body, &jumps)?; // one instruction; may push/pop frames
    }
    Ok(it.values.pop())
}
```
where `step` contains the Task 2–4 match plus:
```rust
Instr::Call(idx) => {
    let ty = module.funcs[*idx as usize].ty;
    let split = it.values.len() - ty.params;
    let args: Vec<Value> = it.values.split_off(split);
    frame.pc += 1;                       // resume after the call
    push_frame(it, module, *idx, &args)?;
    return Ok(());                        // don't double-increment pc
}
Instr::Return => {
    pop_frame(it, module)?;
    return Ok(());
}
Instr::End if it.ctrls.len() == frame.ctrl_base => {
    pop_frame(it, module)?;
    return Ok(());
}
```
and the helpers:
```rust
fn push_frame(it: &mut Interp, module: &Module, func_idx: u32, args: &[Value]) -> Result<(), Trap> {
    let f = &module.funcs[func_idx as usize];
    let mut locals = args.to_vec();
    locals.resize(f.ty.params + f.n_locals, Value::I32(0));
    it.frames.push(Frame {
        func_idx,
        pc: 0,
        locals,
        value_base: it.values.len(),
        ctrl_base: it.ctrls.len(),
    });
    Ok(())
}

fn pop_frame(it: &mut Interp, module: &Module) -> Result<(), Trap> {
    let frame = it.frames.pop().ok_or(Trap::StackUnderflow)?;
    let f = &module.funcs[frame.func_idx as usize];
    let result = if f.ty.results == 1 { Some(it.pop()?) } else { None };
    it.values.truncate(frame.value_base);
    it.ctrls.truncate(frame.ctrl_base);
    if let Some(v) = result {
        it.push(v);
    }
    Ok(())
}
```
(`Interp` gains `pub frames: Vec<Frame>`; locals now live per-frame — `LocalGet/Set/Tee` arms index `it.frames.last_mut().unwrap().locals`. `eval_body` becomes a wrapper building a one-function `Module` and calling `run`.)

- [ ] **Step 4: Run all tests — pass.** Factorial proves guest recursion with a host-side loop: `Frame` count grows, Rust call depth doesn't.  
- [ ] **Step 5: Commit** — `git commit -am "feat(vm): functions and explicit call frames (no host recursion)"`

---

### Task 6: Linear memory

**Files:** Modify: `src\exec.rs`, `src\module.rs` (already has `MemoryDecl`)

**Interfaces:**
- Produces: `Interp.memory: Vec<u8>` (sized `min_pages * 65536` at start of `run` when `module.memory` is `Some`); `I32Load`/`I32Store` little-endian with `MemArg.offset`, bounds-checked → `Trap::OutOfBoundsMemory`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn memory_store_load_roundtrip() {
    let f = Function {
        ty: FuncType { params: 0, results: 1 },
        n_locals: 0,
        body: vec![
            Instr::I32Const(16), Instr::I32Const(-7), Instr::I32Store(MemArg { offset: 4 }),
            Instr::I32Const(16), Instr::I32Load(MemArg { offset: 4 }),
            Instr::End,
        ],
    };
    let mut m = single_export_module(vec![f]);
    m.memory = Some(MemoryDecl { min_pages: 1 });
    assert_eq!(run(&m, 0, &[]), Ok(Some(Value::I32(-7))));
}

#[test]
fn out_of_bounds_traps() {
    let f = Function {
        ty: FuncType { params: 0, results: 1 },
        n_locals: 0,
        body: vec![
            Instr::I32Const(65533), Instr::I32Load(MemArg { offset: 0 }), // 65533+4 > 65536
            Instr::End,
        ],
    };
    let mut m = single_export_module(vec![f]);
    m.memory = Some(MemoryDecl { min_pages: 1 });
    assert_eq!(run(&m, 0, &[]), Err(Trap::OutOfBoundsMemory));
}
```

- [ ] **Step 2: Run — fail.**

- [ ] **Step 3: Implement** — `Interp` gains `pub memory: Vec<u8>`; `run` sizes it; match arms:

```rust
Instr::I32Load(m) => {
    let addr = it.pop_i32()? as u32 as usize + m.offset as usize;
    let bytes: [u8; 4] = it
        .memory
        .get(addr..addr + 4)
        .ok_or(Trap::OutOfBoundsMemory)?
        .try_into()
        .unwrap();
    it.push(Value::I32(i32::from_le_bytes(bytes)));
}
Instr::I32Store(m) => {
    let val = it.pop_i32()?;
    let addr = it.pop_i32()? as u32 as usize + m.offset as usize;
    it.memory
        .get_mut(addr..addr + 4)
        .ok_or(Trap::OutOfBoundsMemory)?
        .copy_from_slice(&val.to_le_bytes());
}
```

- [ ] **Step 4: Run — pass.**  
- [ ] **Step 5: Commit** — `git commit -am "feat(vm): linear memory with bounds-checked i32 load/store"`

---

### Task 7: Binary decoder (.wasm → Module)

**Files:**
- Create: `src\decode.rs`
- Modify: `src\lib.rs` (add `pub mod decode;`), `src\error.rs`

**Interfaces:**
- Consumes: `Module`, `Function`, `FuncType`, `Export`, `MemoryDecl`, `Instr`, `MemArg`.
- Produces:
  - `DecodeError::{BadMagic, UnexpectedEof, UnsupportedOpcode(u8), UnsupportedSection(u8), Malformed(&'static str)}` (add to `error.rs`)
  - `decode::module(bytes: &[u8]) -> Result<Module, DecodeError>`
  - internal: `Reader { bytes: &[u8], pos: usize }` with `u8()`, `u32_leb()`, `i32_leb()`, `bytes(n)`, `name()` — each returning `Result<_, DecodeError>`

- [ ] **Step 1: Failing test** (uses `wat` dev-dependency — real binary bytes, no hand-hexing)

```rust
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
    assert_eq!(m.funcs[0].ty, FuncType { params: 2, results: 1 });
    assert_eq!(m.exports[0].name, "add");
    assert_eq!(
        m.funcs[0].body,
        vec![Instr::LocalGet(0), Instr::LocalGet(1), Instr::I32Add, Instr::End]
    );
}
```

- [ ] **Step 2: Run — fail.**

- [ ] **Step 3: Implement `src\decode.rs`**

```rust
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
                    m.types.push(FuncType { params: n_params, results: n_results });
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
                        m.exports.push(Export { name, func_idx: idx });
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
                        .ok_or(DecodeError::Malformed("code without func decl"))? as usize;
                    let ty = *m.types.get(ty_idx).ok_or(DecodeError::Malformed("bad type idx"))?;
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

fn decode_instrs(r: &mut Reader) -> Result<Vec<Instr>, DecodeError> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    loop {
        let op = r.u8()?;
        let ins = match op {
            0x00 => Instr::Unreachable,
            0x01 => Instr::Nop,
            0x02 => { block_type(r)?; depth += 1; Instr::Block }
            0x03 => { block_type(r)?; depth += 1; Instr::Loop }
            0x04 => { block_type(r)?; depth += 1; Instr::If }
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
    Ok(MemArg { offset: r.u32_leb()? })
}
```

`src\error.rs` — append:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    UnexpectedEof,
    UnsupportedOpcode(u8),
    UnsupportedSection(u8),
    Malformed(&'static str),
}
```

- [ ] **Step 4: Run — pass.** Add one negative test: `decode::module(b"nope")` → `Err(DecodeError::BadMagic)`.  
- [ ] **Step 5: Commit** — `git commit -am "feat(vm): wasm binary decoder for the M1 subset"`

---

### Task 8: Public API + end-to-end (M1 exit criterion)

**Files:**
- Modify: `src\lib.rs`
- Create: `tests\end_to_end.rs`

**Interfaces:**
- Produces (the crate's public face, used by every later milestone):
  - `Vm::instantiate(bytes: &[u8]) -> Result<Vm, DecodeError>`
  - `Vm::invoke(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>, Trap>` (M1: fresh state per invoke; persistent instance state arrives with snapshots in M2)
  - `Trap::UnknownExport` variant (add to `error.rs`)

- [ ] **Step 1: Failing integration test** — `tests\end_to_end.rs`:

```rust
use parakaya_vm::{Value, Vm};

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
    assert_eq!(vm.invoke("fac", &[Value::I32(10)]), Ok(Some(Value::I32(3628800))));
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
```

- [ ] **Step 2: Run — fail.**

- [ ] **Step 3: Implement** — `src\lib.rs`:

```rust
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

pub struct Vm {
    module: module::Module,
}

impl Vm {
    pub fn instantiate(bytes: &[u8]) -> Result<Vm, DecodeError> {
        Ok(Vm { module: decode::module(bytes)? })
    }

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
```

(and add `UnknownExport` to `Trap` in `error.rs`.)

- [ ] **Step 4: Run everything** — `cargo test` → all unit + integration tests PASS; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt`.

- [ ] **Step 5: Commit, push, verify CI, update README status**

```powershell
git add -A
git commit -m "feat(vm): public instantiate/invoke API; M1 exit criterion green"
```
Create the GitHub repo (`gh repo create parakaya --public --source . --push` or via web), confirm the Actions run is green, and change README status line to: `Status: M1 complete — WASM i32 subset executes from real .wasm binaries. Next: M2 (full interpreter + spec test suite).`

---

## Self-Review Notes

- **Spec coverage (M1 slice only):** spec §5 "snapshot-readiness" → Global Constraints + Task 5's explicit frames; spec §10 M1 exit criterion → Task 8. Spec-suite, floats, globals, ABI, framebuffer, daemon: deliberately M2+ (spec §10), not gaps.
- **Type consistency check:** `eval_body(&[Instr], usize) -> Result<Option<Value>, Trap>` (Tasks 2–4), `run(&Module, u32, &[Value]) -> Result<Option<Value>, Trap>` (Tasks 5–8), `Vm::invoke(&str, &[Value]) -> Result<Option<Value>, Trap>` — consistent. `single_export_module` helper defined in Task 5 and reused in Task 6 tests (same file).
- **Known M1 simplifications (intentional, documented here so nobody "fixes" them silently):** blocks are void-typed; jump tables computed per-function at run time (moves to decode-time in M2); `invoke` is stateless across calls until snapshots land; no validation pass (decoder trusts well-formed modules; the spec test suite in M2 forces validation).
