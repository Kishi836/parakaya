# Parakaya Snapshot/Restore Implementation Plan (rescope D11–D12)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **For this repo:** teaching mode — the user types every task by hand from the snippets below;
> the AI teaches the concepts first and reviews the diff at each task boundary. Do not dispatch
> implementer subagents unless the user asks.

**Goal:** Pause a running WASM program after N instructions, save its whole state to one file,
and resume it — in a new process or on another machine — with a bit-identical result.

**Architecture:** `exec::run` splits into `start` (build the state) and `resume` (run ≤ N steps,
return `Done` or `Paused`). `snapshot.rs` hand-serializes `Interp` plus the program bytes into a
fixed-width little-endian file with an FNV-1a checksum. A small `parakaya` binary wraps both as
`run … --pause-after N out` and `resume out`.

**Tech Stack:** Rust stable (edition 2024), std only. Dev-dependency `wat` (already present).

**Spec:** `docs/2026-10-12-parakaya-snapshot-design.md`

**Verified:** every code block below was built in a throwaway worktree on 2026-10-12 —
33 tests green, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
(`fnv1a_matches_reference_values` was added afterwards — its vectors were checked against a
standalone build — making 34.)

## Global Constraints

- **Zero runtime dependencies.** No serde, bincode, clap. `wat` stays dev-only.
- `#![forbid(unsafe_code)]` stays in `lib.rs`.
- Snapshot integers are **little-endian, fixed width; every `usize`, count and length → `u64`**.
- Guest state stays plain owned data (M1 rule): no references, `Rc`/`RefCell`, or host recursion
  in `Interp`.
- Every task ends with `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all clean.
- **Named-file staging only** (`git add <paths>`), never `git add -A` / `commit -am`.
- **Commit messages are plain conventional commits — no Claude attribution lines.**
- Run everything from the repo root:
  `C:\Users\Admin\OneDrive\PES UNIVERSITY CSE\Projects\Parakaya`. The git-ignored
  `.cargo\config.toml` keeps `target\` out of OneDrive — never commit it.

## Review Focus

Inputs the spec implies but its five headline tests don't pin, most likely to bite first. Each
one has a test in the task that owns the code:

1. **The user passes the wrong file to `resume`** (e.g. the `.wasm` itself) → exit code 1 and a
   readable error, no panic. *Task 4: `resuming_a_non_snapshot_fails_cleanly`.*
2. **`--pause-after` larger than the whole program** → prints the result, writes no file.
   *Task 4: `budget_longer_than_the_program_just_finishes`.*
3. **Chained hops** (resume a snapshot, pause it again, resume that) → same final result.
   *Task 4: `pause_then_resume_matches_a_straight_run`.*
4. **A program that traps after the pause point** → the same trap an unpaused run gives.
   *Task 1: `trap_after_pause_still_traps`.*
5. **A crafted snapshot with a huge length field** → an error, not an overflow panic or a
   multi-GB allocation. *Task 2: `huge_length_does_not_panic`.*

## File Structure

```
crates\parakaya-vm\
├── Cargo.toml                 + [[bin]] parakaya                        (Task 4)
├── src\
│   ├── exec.rs                start / resume / Outcome; PartialEq       (Task 1)
│   ├── lib.rs                 Vm::start / Vm::resume; re-exports        (Tasks 1, 2)
│   ├── error.rs               + SnapshotError, From<DecodeError>        (Task 2)
│   ├── decode.rs              Reader → pub(crate), checked_add          (Task 2)
│   ├── snapshot.rs            NEW: save / load / fnv1a + unit tests     (Task 2)
│   └── main.rs                NEW: the parakaya CLI                     (Task 4)
└── tests\
    ├── common\mod.rs          NEW: shared WAT programs FAC, COUNTER     (Task 3)
    ├── end_to_end.rs          uses common::*                            (Task 3)
    ├── snapshot_equivalence.rs NEW: pause-at-every-step sweep           (Task 3)
    └── cli.rs                 NEW: drives the real binary               (Task 4)
README.md                      status + CLI usage                        (Task 4)
```

Schedule: **D11 = Tasks 1–3**, **D12 = Task 4**.

---

### Task 1: Pausable execution

**Files:**
- Modify: `crates\parakaya-vm\src\exec.rs` (derives on `CtrlFrame`/`Frame`/`Interp`; replace
  `run`; two new tests at the end of `mod tests`)
- Modify: `crates\parakaya-vm\src\lib.rs`

**Interfaces:**
- Consumes: existing `push_frame`, `step`, `crate::jumps::compute`, test helper `memory_module`.
- Produces:
  - `exec::Outcome { Done(Option<Value>), Paused }` — `Debug, PartialEq`
  - `exec::start(&Module, u32, &[Value]) -> Result<Interp, Trap>`
  - `exec::resume(&Module, &mut Interp, u64) -> Result<Outcome, Trap>`
  - `Interp`, `CtrlFrame`, `Frame` gain `PartialEq`
  - `Vm::start(&self, &str, &[Value]) -> Result<Interp, Trap>`
  - `Vm::resume(&self, &mut Interp, u64) -> Result<Outcome, Trap>`
  - `parakaya_vm::{Interp, Outcome}` re-exported

**Teach before typing:**
- `Outcome` is a **tagged union** (C: `struct { enum tag; union { ... } }`) — `Done` carries
  data, `Paused` doesn't. `match` forces you to handle both.
- `#[derive(PartialEq)]` on `Interp` only compiles if **every field type** is `PartialEq` — so
  `CtrlFrame` and `Frame` need it too. The compiler will tell you exactly this if you forget.
- Why `Vm::start`/`resume` take `&self` (shared borrow) while `invoke` takes `&mut self`: the new
  methods never change the `Vm`; all mutation happens in the `Interp` you own. That separation is
  what lets one `Vm` drive many paused runs — and what makes the state snapshot-able.
- `unreachable!()` documents "this can't happen" and panics if it ever does.
- The loop checks "frames empty?" **before** "budget spent?" — so a budget of exactly the
  program's step count returns `Done`, not `Paused`. Test 1 pins this.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `exec.rs`, after
  `out_of_bounds_traps`:

```rust
    #[test]
    fn resume_budget_is_exact() {
        // 4 instructions: const, const, add, End
        let m = memory_module(vec![
            Instr::I32Const(2),
            Instr::I32Const(3),
            Instr::I32Add,
            Instr::End,
        ]);
        let mut it = start(&m, 0, &[]).unwrap();
        assert_eq!(resume(&m, &mut it, 0), Ok(Outcome::Paused)); // nothing ran
        assert_eq!(resume(&m, &mut it, 3), Ok(Outcome::Paused)); // const, const, add
        assert_eq!(it.values, vec![Value::I32(5)]);
        assert_eq!(
            resume(&m, &mut it, 1), // the 4th step (End) returns
            Ok(Outcome::Done(Some(Value::I32(5))))
        );
    }

    #[test]
    fn trap_after_pause_still_traps() {
        let m = memory_module(vec![
            Instr::I32Const(1),
            Instr::I32Const(0),
            Instr::I32DivS,
            Instr::End,
        ]);
        let mut it = start(&m, 0, &[]).unwrap();
        assert_eq!(resume(&m, &mut it, 2), Ok(Outcome::Paused));
        assert_eq!(resume(&m, &mut it, u64::MAX), Err(Trap::DivByZero));
    }
```

- [ ] **Step 2: Run — expect red**

Run: `cargo test -p parakaya-vm resume_budget`
Expected: compile errors `E0425 cannot find function 'start'` / `'resume'` and
`E0433 ... Outcome`.

- [ ] **Step 3: Add `PartialEq` to the three state structs** in `exec.rs`:

```rust
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CtrlFrame {
```
```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
```
```rust
/// The entire VM state — plain owned data, so a snapshot is "serialize this struct".
#[derive(Debug, Default, PartialEq)]
pub struct Interp {
```

- [ ] **Step 4: Replace the whole `run` function** (doc comment included) with:

```rust
/// What a `resume` call ended with.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The called function returned (with its result, if it has one).
    Done(Option<Value>),
    /// The step budget ran out first; the `Interp` can be resumed (or snapshotted).
    Paused,
}

/// Set up a call to `func_idx` with `args` without executing anything.
pub fn start(module: &Module, func_idx: u32, args: &[Value]) -> Result<Interp, Trap> {
    if args.len() != module.funcs[func_idx as usize].ty.params {
        return Err(Trap::TypeMismatch);
    }
    let mut it = Interp::default();
    if let Some(mem) = module.memory {
        it.memory = vec![0; mem.min_pages as usize * 65536];
    }
    push_frame(&mut it, module, func_idx, args.to_vec());
    Ok(it)
}

/// Execute at most `max_steps` instructions. Guest calls push `Frame`s onto `it.frames` and
/// this loop keeps stepping the top one — the host never recurses, so all state stays in `it`
/// and a `Paused` interpreter can be continued later, even in another process.
pub fn resume(module: &Module, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap> {
    // ponytail: every function's jump table is rebuilt per resume; move to decode time in M2.
    let jumps: Vec<HashMap<usize, JumpInfo>> = module
        .funcs
        .iter()
        .map(|f| crate::jumps::compute(&f.body))
        .collect();
    let mut steps = 0;
    while let Some(frame) = it.frames.last() {
        if steps == max_steps {
            return Ok(Outcome::Paused);
        }
        let f = frame.func_idx as usize;
        step(it, module, &module.funcs[f].body, &jumps[f])?;
        steps += 1;
    }
    Ok(Outcome::Done(it.values.pop()))
}

/// Call `func_idx` with `args` and run it to completion.
pub fn run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap> {
    let mut it = start(module, func_idx, args)?;
    match resume(module, &mut it, u64::MAX)? {
        Outcome::Done(result) => Ok(result),
        Outcome::Paused => unreachable!("u64::MAX steps never runs out"),
    }
}
```

Note `step(it, …)` not `step(&mut it, …)`: inside `resume`, `it` is already a `&mut Interp`.

- [ ] **Step 5: Expose it on `Vm`** — in `lib.rs`, replace from `pub use error::…` to the end
  of the file with:

```rust
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

    fn export(&self, name: &str) -> Result<u32, Trap> {
        self.module
            .exports
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.func_idx)
            .ok_or(Trap::UnknownExport)
    }
}
```

- [ ] **Step 6: Run — expect green**

Run: `cargo test`
Expected: 20 unit + 3 end-to-end tests pass (all 18 old unit tests still pass — `run` kept its
signature).

- [ ] **Step 7: Lint, format, commit**

```powershell
cargo fmt; cargo clippy --all-targets -- -D warnings
git add crates/parakaya-vm/src/exec.rs crates/parakaya-vm/src/lib.rs
git commit -m "feat(vm): pausable execution (start/resume with a step budget)"
```

---

### Task 2: Snapshot format

**Files:**
- Modify: `crates\parakaya-vm\src\decode.rs` (the `Reader` struct and its first two methods)
- Modify: `crates\parakaya-vm\src\error.rs`
- Modify: `crates\parakaya-vm\src\lib.rs` (module list + re-export)
- Create: `crates\parakaya-vm\src\snapshot.rs`

**Interfaces:**
- Consumes: `Interp`, `CtrlFrame`, `Frame` with `PartialEq` (Task 1); `decode::Reader`.
- Produces:
  - `snapshot::save(&[u8], &Interp) -> Vec<u8>`
  - `snapshot::load(&[u8]) -> Result<(Vec<u8>, Interp), SnapshotError>`
  - `SnapshotError { BadMagic, BadVersion, UnexpectedEof, BadChecksum, BadTag(u8), TooLarge,
    TrailingBytes }` — `Clone, Debug, PartialEq, Eq`, re-exported as
    `parakaya_vm::SnapshotError`

**Teach before typing:**
- **Endianness.** `x.to_le_bytes()` / `T::from_le_bytes(arr)` pin byte order to little-endian
  on every CPU. C equivalent: writing a `uint32_t` byte-by-byte with shifts instead of
  `fwrite(&x, 4, 1, f)`.
- **Widening vs narrowing.** `usize as u64` can never lose bits (usize ≤ 64 bits everywhere),
  so `as` is fine on the way out. On the way in, `usize::try_from(u64)` can fail on a 32-bit Pi
  — we turn that into `TooLarge` instead of silently truncating like C's `(size_t)x` would.
- **`pub(crate)`** = visible anywhere in this crate, invisible to users of the library.
  (Between C's file-`static` and a header export.)
- **`impl From<DecodeError> for SnapshotError`** is what lets `r.bytes(4)?` compile inside a
  function returning `Result<_, SnapshotError>`: `?` calls `From::from` on the error.
- **Const generics:** `fn arr<const N: usize>(…) -> […; N]` — `N` is inferred from the caller.
  `i32::from_le_bytes` wants `[u8; 4]`, so `arr(r)?` there reads 4 bytes.
- **Field order matters in `Frame { func_idx: get_u32(&mut r)?, pc: get_usize(&mut r)?, … }`.**
  Rust evaluates struct-literal fields top to bottom, so this reads the file in the right order
  — keep the fields in the same order as `save` writes them.
- **Never `Vec::with_capacity(count)` when `count` comes from a file:** a corrupt count of 2⁶⁰
  would try to allocate exabytes. Pushing one at a time fails at EOF instead.
- **Checksum first.** `load` verifies FNV-1a over the whole body before parsing anything, so
  corrupted bytes are never interpreted.

- [ ] **Step 1: Make `Reader` shareable and overflow-safe** — in `decode.rs`, replace the
  `struct Reader` definition and its `u8` and `bytes` methods with:

```rust
/// Bounds-checked cursor over a byte slice. Shared with `snapshot.rs`.
pub(crate) struct Reader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.bytes.get(self.pos).ok_or(DecodeError::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        // checked_add: a huge length read from a file must not overflow pos + n
        let end = self.pos.checked_add(n).ok_or(DecodeError::UnexpectedEof)?;
        let s = self
            .bytes
            .get(self.pos..end)
            .ok_or(DecodeError::UnexpectedEof)?;
        self.pos = end;
        Ok(s)
    }
```

(The `u32_leb`, `i32_leb`, `name` methods below stay as they are.)

- [ ] **Step 2: Add the error type** — append to `error.rs`:

```rust
/// Why a snapshot file could not be turned back into an `Interp`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    BadMagic,
    BadVersion,
    UnexpectedEof,
    BadChecksum,
    BadTag(u8),
    TooLarge,
    TrailingBytes,
}

/// Lets `?` pass a `Reader` error up from snapshot code. The only `Reader` methods
/// snapshot code calls (`u8`, `bytes`) can fail only with `UnexpectedEof`.
impl From<DecodeError> for SnapshotError {
    fn from(_: DecodeError) -> Self {
        SnapshotError::UnexpectedEof
    }
}
```

- [ ] **Step 3: Register the module** — in `lib.rs`:

```rust
pub mod module;
pub mod snapshot;
pub mod value;

pub use error::{DecodeError, SnapshotError, Trap};
```

- [ ] **Step 4: Write `snapshot.rs` with stubs + the tests (red first)** — create
  `crates\parakaya-vm\src\snapshot.rs`:

```rust
//! `Interp` ⇄ bytes. Every field is fixed-width little-endian and every `usize` is written
//! as `u64`, so a snapshot written on one CPU loads on any other.
//! Layout: `docs/2026-10-12-parakaya-snapshot-design.md` §2.

use crate::decode::Reader;
use crate::error::SnapshotError;
use crate::exec::{CtrlFrame, Frame, Interp};
use crate::value::Value;

pub fn save(wasm: &[u8], it: &Interp) -> Vec<u8> {
    todo!()
}

pub fn load(bytes: &[u8]) -> Result<(Vec<u8>, Interp), SnapshotError> {
    todo!()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built mid-run state touching every field and every `Value` tag.
    fn sample() -> Interp {
        Interp {
            values: vec![
                Value::I32(-7),
                Value::I64(1 << 40),
                Value::F32(1.5),
                Value::F64(-0.25),
            ],
            ctrls: vec![CtrlFrame {
                opener_pc: 3,
                end_pc: 9,
                is_loop: true,
                height: 1,
            }],
            frames: vec![Frame {
                func_idx: 2,
                pc: 5,
                locals: vec![Value::I32(42)],
                value_base: 0,
                ctrl_base: 0,
            }],
            memory: vec![0xAB; 16],
        }
    }

    /// Strip the checksum, let `edit` tamper with the body, then append a fresh checksum —
    /// so the test reaches the parser instead of stopping at `BadChecksum`.
    fn tamper(edit: impl Fn(&mut Vec<u8>)) -> Vec<u8> {
        let mut body = save(b"wasm", &sample());
        body.truncate(body.len() - 8);
        edit(&mut body);
        let sum = fnv1a(&body);
        body.extend_from_slice(&sum.to_le_bytes());
        body
    }

    #[test]
    fn fnv1a_matches_reference_values() {
        // published FNV-1a 64 test vectors: any correct implementation, anywhere, agrees
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn round_trip_is_lossless() {
        let snap = save(b"wasm", &sample());
        assert_eq!(load(&snap), Ok((b"wasm".to_vec(), sample())));
    }

    #[test]
    fn flipped_byte_fails_checksum() {
        let mut snap = save(b"wasm", &sample());
        snap[10] ^= 1;
        assert_eq!(load(&snap), Err(SnapshotError::BadChecksum));
    }

    #[test]
    fn cut_short_fails() {
        let snap = save(b"wasm", &sample());
        assert_eq!(
            load(&snap[..snap.len() - 1]),
            Err(SnapshotError::BadChecksum)
        );
        assert_eq!(load(&snap[..7]), Err(SnapshotError::UnexpectedEof));
    }

    #[test]
    fn wrong_magic_and_trailing_bytes_are_rejected() {
        assert_eq!(load(&tamper(|b| b[0] = b'X')), Err(SnapshotError::BadMagic));
        assert_eq!(
            load(&tamper(|b| b.push(0))),
            Err(SnapshotError::TrailingBytes)
        );
    }

    #[test]
    fn huge_length_does_not_panic() {
        // the wasm length field sits right after magic + version (bytes 8..16)
        let snap = tamper(|b| b[8..16].copy_from_slice(&u64::MAX.to_le_bytes()));
        assert!(matches!(
            load(&snap),
            Err(SnapshotError::UnexpectedEof | SnapshotError::TooLarge)
        ));
    }
}
```

- [ ] **Step 5: Run — expect red**

Run: `cargo test -p parakaya-vm snapshot`
Expected: compiles (with unused-variable warnings), the 6 snapshot tests FAIL with
`not yet implemented`.

- [ ] **Step 6: Implement** — replace the three stubs (everything between the `use` lines and
  `#[cfg(test)]`) with:

```rust
const MAGIC: &[u8; 4] = b"PKSN";
const VERSION: u32 = 1;

/// Serialize `it`, embedding the program `wasm` so the file is self-contained.
pub fn save(wasm: &[u8], it: &Interp) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    put_u32(&mut out, VERSION);
    put_bytes(&mut out, wasm);
    put_values(&mut out, &it.values);
    put_u64(&mut out, it.ctrls.len() as u64);
    for c in &it.ctrls {
        put_u64(&mut out, c.opener_pc as u64);
        put_u64(&mut out, c.end_pc as u64);
        out.push(c.is_loop as u8);
        put_u64(&mut out, c.height as u64);
    }
    put_u64(&mut out, it.frames.len() as u64);
    for f in &it.frames {
        put_u32(&mut out, f.func_idx);
        put_u64(&mut out, f.pc as u64);
        put_values(&mut out, &f.locals);
        put_u64(&mut out, f.value_base as u64);
        put_u64(&mut out, f.ctrl_base as u64);
    }
    put_bytes(&mut out, &it.memory);
    let sum = fnv1a(&out);
    put_u64(&mut out, sum);
    out
}

/// Parse a snapshot back into `(wasm, Interp)`. The checksum is verified before anything
/// else is read, so corrupted bytes are never interpreted.
pub fn load(bytes: &[u8]) -> Result<(Vec<u8>, Interp), SnapshotError> {
    let body_len = bytes
        .len()
        .checked_sub(8)
        .ok_or(SnapshotError::UnexpectedEof)?;
    let (body, sum) = bytes.split_at(body_len);
    if fnv1a(body) != u64::from_le_bytes(sum.try_into().unwrap()) {
        return Err(SnapshotError::BadChecksum);
    }

    let mut r = Reader {
        bytes: body,
        pos: 0,
    };
    if r.bytes(4)? != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    if get_u32(&mut r)? != VERSION {
        return Err(SnapshotError::BadVersion);
    }
    let wasm = get_bytes(&mut r)?.to_vec();
    let mut it = Interp {
        values: get_values(&mut r)?,
        ..Interp::default()
    };
    for _ in 0..get_u64(&mut r)? {
        it.ctrls.push(CtrlFrame {
            opener_pc: get_usize(&mut r)?,
            end_pc: get_usize(&mut r)?,
            is_loop: r.u8()? != 0,
            height: get_usize(&mut r)?,
        });
    }
    for _ in 0..get_u64(&mut r)? {
        it.frames.push(Frame {
            func_idx: get_u32(&mut r)?,
            pc: get_usize(&mut r)?,
            locals: get_values(&mut r)?,
            value_base: get_usize(&mut r)?,
            ctrl_base: get_usize(&mut r)?,
        });
    }
    it.memory = get_bytes(&mut r)?.to_vec();
    if r.pos != body.len() {
        return Err(SnapshotError::TrailingBytes);
    }
    Ok((wasm, it))
}

/// FNV-1a, 64-bit: a few lines, no dependency, and identical on every platform
/// (unlike std's `DefaultHasher`, which may change between Rust releases).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn put_u32(out: &mut Vec<u8>, x: u32) {
    out.extend_from_slice(&x.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, x: u64) {
    out.extend_from_slice(&x.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u64(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn put_values(out: &mut Vec<u8>, values: &[Value]) {
    put_u64(out, values.len() as u64);
    for v in values {
        match v {
            Value::I32(x) => {
                out.push(0);
                out.extend_from_slice(&x.to_le_bytes());
            }
            Value::I64(x) => {
                out.push(1);
                out.extend_from_slice(&x.to_le_bytes());
            }
            // floats go out as their raw bit pattern, so even NaN payloads survive
            Value::F32(x) => {
                out.push(2);
                out.extend_from_slice(&x.to_le_bytes());
            }
            Value::F64(x) => {
                out.push(3);
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
}

/// Read exactly `N` bytes as an array; `N` is inferred from the caller
/// (e.g. `i32::from_le_bytes` wants `[u8; 4]`).
fn arr<const N: usize>(r: &mut Reader) -> Result<[u8; N], SnapshotError> {
    Ok(r.bytes(N)?.try_into().unwrap()) // bytes(N) returned exactly N bytes
}

fn get_u32(r: &mut Reader) -> Result<u32, SnapshotError> {
    Ok(u32::from_le_bytes(arr(r)?))
}

fn get_u64(r: &mut Reader) -> Result<u64, SnapshotError> {
    Ok(u64::from_le_bytes(arr(r)?))
}

/// A `u64` from the file, as this machine's `usize`. Fails instead of truncating on
/// a 32-bit host.
fn get_usize(r: &mut Reader) -> Result<usize, SnapshotError> {
    usize::try_from(get_u64(r)?).map_err(|_| SnapshotError::TooLarge)
}

fn get_bytes<'a>(r: &mut Reader<'a>) -> Result<&'a [u8], SnapshotError> {
    let n = get_usize(r)?;
    Ok(r.bytes(n)?)
}

fn get_values(r: &mut Reader) -> Result<Vec<Value>, SnapshotError> {
    let mut values = Vec::new(); // no with_capacity: the count comes from the file
    for _ in 0..get_u64(r)? {
        values.push(match r.u8()? {
            0 => Value::I32(i32::from_le_bytes(arr(r)?)),
            1 => Value::I64(i64::from_le_bytes(arr(r)?)),
            2 => Value::F32(f32::from_le_bytes(arr(r)?)),
            3 => Value::F64(f64::from_le_bytes(arr(r)?)),
            tag => return Err(SnapshotError::BadTag(tag)),
        });
    }
    Ok(values)
}
```

- [ ] **Step 7: Run — expect green**

Run: `cargo test`
Expected: 26 unit + 3 end-to-end tests pass.

- [ ] **Step 8: Lint, format, commit**

```powershell
cargo fmt; cargo clippy --all-targets -- -D warnings
git add crates/parakaya-vm/src/decode.rs crates/parakaya-vm/src/error.rs crates/parakaya-vm/src/lib.rs crates/parakaya-vm/src/snapshot.rs
git commit -m "feat(vm): versioned little-endian snapshot format with checksum"
```

---

### Task 3: The equivalence sweep (headline test)

**Files:**
- Create: `crates\parakaya-vm\tests\common\mod.rs`
- Modify: `crates\parakaya-vm\tests\end_to_end.rs` (use the shared programs)
- Create: `crates\parakaya-vm\tests\snapshot_equivalence.rs`

**Interfaces:**
- Consumes: `Vm::{instantiate, invoke, start, resume}`, `Outcome` (Task 1);
  `snapshot::{save, load}` (Task 2).
- Produces: `tests/common/mod.rs` with `pub const FAC: &str` and `pub const COUNTER: &str`
  (Task 4's CLI test uses `FAC`).

**Teach before typing:**
- Each file in `tests\` is compiled as **its own crate**. Shared helpers go in
  `tests\common\mod.rs` (a folder), not `tests\common.rs` — cargo would treat the latter as one
  more test file.
- `#![allow(dead_code)]` in `common`: `cli.rs` uses only `FAC`, and an unused `COUNTER` there
  would be a warning — which `clippy -D warnings` turns into a CI failure.
- **A test that has never failed proves nothing.** This one passes on first run (Tasks 1–2
  already work), so Step 4 deliberately breaks `save` to watch it fail, then restores it.

- [ ] **Step 1: Shared programs** — create `crates\parakaya-vm\tests\common\mod.rs`:

```rust
//! WAT programs shared by the integration tests (`mod common;` in each test file).
// Each test file is its own crate and uses only some of these.
#![allow(dead_code)]

/// Recursive factorial: `fac(n)`, one i32 param, one i32 local.
pub const FAC: &str = r#"(module
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
    local.get 1))"#;

/// Bumps the i32 at address 0 and returns it: exercises memory store/load.
pub const COUNTER: &str = r#"(module
  (memory 1)
  (func (export "bump") (result i32)
    i32.const 0
    i32.const 0
    i32.load
    i32.const 1
    i32.add
    i32.store
    i32.const 0
    i32.load))"#;
```

- [ ] **Step 2: Point `end_to_end.rs` at them** — replace the whole file with:

```rust
//! M1 exit criterion: real `.wasm` bytes (compiled from WAT) → decode → execute.

mod common;

use parakaya_vm::{Trap, Value, Vm};

#[test]
fn factorial_from_wat() {
    let bytes = wat::parse_str(common::FAC).unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(
        vm.invoke("fac", &[Value::I32(10)]),
        Ok(Some(Value::I32(3628800)))
    );
}

#[test]
fn memory_counter_from_wat() {
    let bytes = wat::parse_str(common::COUNTER).unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(vm.invoke("bump", &[]), Ok(Some(Value::I32(1))));
}

#[test]
fn unknown_export_traps() {
    let bytes = wat::parse_str(r#"(module (func (export "f")))"#).unwrap();
    let mut vm = Vm::instantiate(&bytes).unwrap();
    assert_eq!(vm.invoke("nope", &[]), Err(Trap::UnknownExport));
}
```

- [ ] **Step 3: The sweep** — create `crates\parakaya-vm\tests\snapshot_equivalence.rs`:

```rust
//! The snapshot success criterion: pause at ANY instruction, save, load into a fresh VM,
//! finish — and get exactly what a run that was never interrupted gets.

mod common;

use parakaya_vm::{Outcome, Value, Vm, snapshot};

fn sweep(wat_src: &str, export: &str, args: &[Value]) {
    let wasm = wat::parse_str(wat_src).unwrap();
    let mut vm = Vm::instantiate(&wasm).unwrap();
    let expected = vm.invoke(export, args).unwrap();

    // count the steps of an uninterrupted run: each resume(1) call runs exactly one
    let mut it = vm.start(export, args).unwrap();
    let mut total = 0;
    loop {
        total += 1;
        if vm.resume(&mut it, 1).unwrap() != Outcome::Paused {
            break;
        }
    }

    for n in 0..total {
        let mut it = vm.start(export, args).unwrap();
        assert_eq!(vm.resume(&mut it, n), Ok(Outcome::Paused), "pausing at {n}");
        let file = snapshot::save(&wasm, &it);

        // "another device": nothing survives but the snapshot bytes
        let (wasm2, mut it2) = snapshot::load(&file).unwrap();
        let vm2 = Vm::instantiate(&wasm2).unwrap();
        assert_eq!(
            vm2.resume(&mut it2, u64::MAX),
            Ok(Outcome::Done(expected)),
            "resumed from step {n} of {total}"
        );
    }
}

#[test]
fn factorial_survives_a_snapshot_at_every_step() {
    sweep(common::FAC, "fac", &[Value::I32(10)]);
}

#[test]
fn memory_survives_a_snapshot_at_every_step() {
    sweep(common::COUNTER, "bump", &[]);
}
```

Run: `cargo test --test snapshot_equivalence`
Expected: 2 passed.

- [ ] **Step 4: Prove it can fail (mutation check — do not commit this)** — in `snapshot.rs`
  `save`, temporarily change `put_bytes(&mut out, &it.memory);` to
  `put_bytes(&mut out, &vec![0; it.memory.len()]);` (snapshot "forgets" memory contents).

Run: `cargo test --test snapshot_equivalence`
Expected: `memory_survives_a_snapshot_at_every_step` FAILS —
`resumed from step 6 of 9`, `left: Ok(Done(Some(I32(0))))`, `right: Ok(Done(Some(I32(1))))`.
Explain to yourself why step 6 is the first failing pause point, then **revert the line** and
re-run until green.

- [ ] **Step 5: Lint, format, commit**

```powershell
cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
git add crates/parakaya-vm/tests/common/mod.rs crates/parakaya-vm/tests/end_to_end.rs crates/parakaya-vm/tests/snapshot_equivalence.rs
git commit -m "test(vm): snapshot at every step resumes to the uninterrupted result"
```

---

### Task 4: The `parakaya` CLI + README

**Files:**
- Modify: `crates\parakaya-vm\Cargo.toml`
- Create: `crates\parakaya-vm\src\main.rs`
- Create: `crates\parakaya-vm\tests\cli.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: `Vm::{instantiate, start, resume}`, `Outcome`, `Value` (Task 1);
  `snapshot::{save, load}` (Task 2); `common::FAC` (Task 3).
- Produces: binary `parakaya` with
  `run <prog.wasm> <export> [i32 args...] [--pause-after <N> <out>]` and
  `resume <snap> [--pause-after <N> <out>]`. Success → result line on stdout, exit 0. Pause →
  `paused after N steps -> <out>`. Any error → `error: …` on stderr, exit 1.

**Teach before typing:**
- `[[bin]] name = "parakaya"` names the executable; without it cargo would call it
  `parakaya-vm`. `main.rs` uses the library exactly as an outside user would
  (`use parakaya_vm::…`) — it can't touch anything not `pub`.
- **Slice patterns:** `[cmd, prog, export, call_args @ ..] if cmd == "run"` matches a slice of
  ≥ 3 strings and binds the rest to `call_args`. Python: `cmd, prog, export, *call_args = args`.
- **let-else:** `let [_, n, out] = tail.as_slice() else { return Err(…) };` — destructure or
  bail out.
- `.collect::<Result<Vec<_>, _>>()` turns many `Result`s into one; it stops at the first `Err`.
- `fn main() -> ExitCode` lets `main` set the process exit code without `std::process::exit`.
- `env!("CARGO_BIN_EXE_parakaya")` — cargo builds the binary before integration tests and
  tells them where it is.
- `fs::write` is not atomic: a crash mid-write leaves a torn file. That's acceptable here
  because `load` rejects it via the checksum (Review Focus 1).

- [ ] **Step 1: Write the failing CLI tests** — create `crates\parakaya-vm\tests\cli.rs`:

```rust
//! Drive the real `parakaya` binary the way a user would.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

fn parakaya(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_parakaya"))
        .args(args)
        .output()
        .unwrap()
}

/// Run and expect success; returns trimmed stdout.
fn ok(args: &[&str]) -> String {
    let out = parakaya(args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A fresh scratch dir per test (tests run in parallel) holding `fac.wasm`.
fn scratch(test: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("parakaya-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prog = dir.join("fac.wasm");
    std::fs::write(&prog, wat::parse_str(common::FAC).unwrap()).unwrap();
    (dir, prog.to_str().unwrap().to_string())
}

#[test]
fn pause_then_resume_matches_a_straight_run() {
    let (dir, prog) = scratch("hop");
    let a = dir.join("a.pksn");
    let b = dir.join("b.pksn");
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

    assert_eq!(ok(&["run", &prog, "fac", "10"]), "3628800");
    assert_eq!(
        ok(&["run", &prog, "fac", "10", "--pause-after", "40", a]),
        format!("paused after 40 steps -> {a}")
    );
    // chained hop: resume, pause again, resume again
    assert_eq!(
        ok(&["resume", a, "--pause-after", "40", b]),
        format!("paused after 40 steps -> {b}")
    );
    assert_eq!(ok(&["resume", b]), "3628800");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn budget_longer_than_the_program_just_finishes() {
    let (dir, prog) = scratch("long");
    let snap = dir.join("never.pksn");
    let snap = snap.to_str().unwrap();
    assert_eq!(
        ok(&["run", &prog, "fac", "3", "--pause-after", "1000000", snap]),
        "6"
    );
    assert!(!std::path::Path::new(snap).exists(), "no snapshot written");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn resuming_a_non_snapshot_fails_cleanly() {
    let (dir, prog) = scratch("bad");
    let out = parakaya(&["resume", &prog]); // a .wasm, not a snapshot
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("BadChecksum"));
    std::fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **Step 2: Run — expect red**

Run: `cargo test --test cli`
Expected: compile error — `environment variable 'CARGO_BIN_EXE_parakaya' not defined` (the
binary doesn't exist yet).

- [ ] **Step 3: Declare the binary** — in `crates\parakaya-vm\Cargo.toml`, after the
  `[package]` block:

```toml
[[bin]]
name = "parakaya"
path = "src/main.rs"
```

- [ ] **Step 4: Write the CLI** — create `crates\parakaya-vm\src\main.rs`:

```rust
//! `parakaya run | resume` — the command-line face of snapshot/restore.

use parakaya_vm::{Outcome, Value, Vm, snapshot};
use std::process::ExitCode;

const USAGE: &str = "usage:
  parakaya run    <prog.wasm> <export> [i32 args...] [--pause-after <N> <out.pksn>]
  parakaya resume <snap.pksn>                        [--pause-after <N> <out.pksn>]";

fn main() -> ExitCode {
    match cli(std::env::args().skip(1).collect()) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Everything after the program name. Returns the line to print on success.
fn cli(mut args: Vec<String>) -> Result<String, String> {
    // peel off the optional trailing `--pause-after <N> <out>`
    let pause = match args.iter().position(|a| a == "--pause-after") {
        Some(i) => {
            let tail = args.split_off(i);
            let [_, n, out] = tail.as_slice() else {
                return Err(USAGE.into());
            };
            let n: u64 = n.parse().map_err(|e| format!("bad step count {n}: {e}"))?;
            Some((n, out.clone()))
        }
        None => None,
    };

    let (wasm, vm, mut it) = match args.as_slice() {
        [cmd, prog, export, call_args @ ..] if cmd == "run" => {
            let wasm = std::fs::read(prog).map_err(|e| format!("{prog}: {e}"))?;
            let vm = Vm::instantiate(&wasm).map_err(|e| format!("{prog}: {e:?}"))?;
            let call_args = call_args
                .iter()
                .map(|a| a.parse().map(Value::I32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("bad i32 argument: {e}"))?;
            let it = vm
                .start(export, &call_args)
                .map_err(|t| format!("trap: {t:?}"))?;
            (wasm, vm, it)
        }
        [cmd, snap] if cmd == "resume" => {
            let bytes = std::fs::read(snap).map_err(|e| format!("{snap}: {e}"))?;
            let (wasm, it) = snapshot::load(&bytes).map_err(|e| format!("{snap}: {e:?}"))?;
            let vm = Vm::instantiate(&wasm).map_err(|e| format!("{snap}: {e:?}"))?;
            (wasm, vm, it)
        }
        _ => return Err(USAGE.into()),
    };

    let budget = pause.as_ref().map_or(u64::MAX, |(n, _)| *n);
    match vm
        .resume(&mut it, budget)
        .map_err(|t| format!("trap: {t:?}"))?
    {
        Outcome::Done(Some(Value::I32(x))) => Ok(x.to_string()),
        Outcome::Done(Some(v)) => Ok(format!("{v:?}")),
        Outcome::Done(None) => Ok("(no result)".into()),
        Outcome::Paused => {
            let (n, out) = pause.expect("only a finite budget pauses");
            // not atomic: a crash mid-write leaves a torn file, which load rejects (checksum)
            std::fs::write(&out, snapshot::save(&wasm, &it)).map_err(|e| format!("{out}: {e}"))?;
            Ok(format!("paused after {n} steps -> {out}"))
        }
    }
}
```

- [ ] **Step 5: Run — expect green**

Run: `cargo test`
Expected: 26 unit + 3 cli + 3 end_to_end + 2 snapshot_equivalence = **34 passed**.

Then try it by hand (no args → usage, exit 1):

```powershell
cargo run -q --bin parakaya
```

- [ ] **Step 6: README** — in `README.md`, replace the `**Next: snapshot/restore** …` paragraph
  and the `cargo test` block after it with:

````markdown
**Snapshot/restore — done.** Pause a program after any number of instructions, write its whole
state (stacks, call frames, memory, and the program itself) to one checksummed,
platform-neutral file, and resume it in a fresh process:

```sh
cargo install --path crates/parakaya-vm
parakaya run fac.wasm fac 10 --pause-after 40 fac.pksn   # paused after 40 steps -> fac.pksn
parakaya resume fac.pksn                                   # 3628800
```

`cargo test` pauses the test programs at **every** instruction, round-trips each snapshot
through bytes, and checks every resumed run against the uninterrupted one.

**Next:** resume a snapshot on a second device with a different CPU.
````

- [ ] **Step 7: Lint, format, commit, push**

```powershell
cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
git add crates/parakaya-vm/Cargo.toml crates/parakaya-vm/src/main.rs crates/parakaya-vm/tests/cli.rs README.md
git commit -m "feat(cli): parakaya run/resume with --pause-after snapshots"
git push origin main
```

Then confirm CI: `gh run list --limit 1` → `completed success`.
