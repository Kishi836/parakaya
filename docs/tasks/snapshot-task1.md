# Task 1 — Pausable execution (typing guide)

> Open side by side with the code: in VS Code, `Ctrl+Shift+V` on this file (or `Ctrl+K V` for a
> split preview). Hints are visible; **full solutions are folded** — click ▶ only after you've
> tried. Full plan: `docs/2026-10-12-parakaya-snapshot-plan.md` · spec:
> `docs/2026-10-12-parakaya-snapshot-design.md`.

**Goal:** split `run` into `start` (build the state, run nothing) and `resume` (run at most N
instructions, then report `Done` or `Paused`).

**Files you'll touch:** `crates\parakaya-vm\src\exec.rs`, `crates\parakaya-vm\src\lib.rs`

**Progress checklist**

- [ ] 1. Type the two failing tests
- [ ] 2. Run → red (won't compile)
- [ ] 3. Type the `Outcome` enum
- [ ] 4. Write `start`
- [ ] 5. Write `resume` → new tests green
- [ ] 6. Rewrite `run` on top of them → all 20 green
- [ ] 7. `lib.rs`: `export`, `Vm::start`, `Vm::resume`, slimmer `invoke`
- [ ] 8. `PartialEq` cascade
- [ ] 9. fmt + clippy + test → ask for review → commit

---

## Concepts first

| Rust | What it is | You know it as |
|---|---|---|
| `enum Outcome { Done(Option<Value>), Paused }` | a **tagged union**: one tag, data only on some variants | C: `struct { int tag; union {...}; }` — but the compiler forces you to check the tag |
| `start` + `resume` | split "set up" from "run" | Python generator: create it, then `next()` it — except our state is a plain struct you can save to disk (Task 2) |
| `&mut Interp` | a mutable borrow: the function may change it, the caller still owns it | C: `struct interp *it` |
| `&self` on `Vm::start/resume` | shared borrow: the `Vm` itself never changes | C: `const struct vm *` |
| `todo!()` / `unreachable!()` | panic with "not yet implemented" / "this can't happen" | Python: `raise NotImplementedError` / `assert False` |
| `#[derive(PartialEq)]` | auto-generate `==` field by field | only works if **every field's type** already has `==` |

**Why this is the heart of Parakaya:** today `run` builds state, loops, and throws the state
away. After this task the state lives in an `Interp` *you* hold between calls — so you can stop
anywhere, keep it, and continue later. Task 2 writes that `Interp` to a file.

---

## Step 1 — The failing tests (type exactly)

`exec.rs`, at the **very bottom**: after the last test `out_of_bounds_traps`, but **before** the
final `}` that closes `mod tests`.

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

- `memory_module` already exists higher up in `mod tests` — it wraps instructions into a
  one-function module.
- Test 1 pins the **exact** budget: 0 steps runs nothing; 3 steps leaves `[5]` on the stack; the
  4th step is `End`, which returns.
- Test 2 checks that a trap after a pause is the *same* trap an unpaused run gives.

## Step 2 — Run: expect red

```powershell
cargo test -p parakaya-vm resume_budget
```

**Predict first:** which names will rustc say it can't find?

<details><summary>Expected output</summary>

Compile errors, roughly: `E0425 cannot find function 'start'`, `E0425 cannot find function
'resume'`, `E0433 failed to resolve: use of undeclared type 'Outcome'` — several times each.
Red by not compiling is a valid red: the tests describe functions that don't exist yet.
</details>

## Step 3 — The `Outcome` enum (type exactly)

`exec.rs`, directly **below** the existing `run` function:

```rust
/// What a `resume` call ended with.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The called function returned (with its result, if it has one).
    Done(Option<Value>),
    /// The step budget ran out first; the `Interp` can be resumed (or snapshotted).
    Paused,
}
```

`Debug` lets `assert_eq!` print it on failure; `PartialEq` lets `assert_eq!` compare it.

## Step 4 — `start`

Below `Outcome`. Signature (type exactly):

```rust
/// Set up a call to `func_idx` with `args` without executing anything.
pub fn start(module: &Module, func_idx: u32, args: &[Value]) -> Result<Interp, Trap> {

}
```

**Hints** — this is the "build the state" half of `run`. Look at `run` right above:

1. Wrong argument count → `return Err(Trap::TypeMismatch);`
2. A fresh `Interp`; allocate memory if the module declares one
3. Push the first frame (`push_frame`)
4. Give the `Interp` back — the return type is `Result`, so wrap it

Almost every line already exists in `run`. Copy them; don't reinvent.

<details><summary>Solution</summary>

```rust
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
```
</details>

## Step 5 — `resume`

Below `start`. Signature (type exactly):

```rust
/// Execute at most `max_steps` instructions. Guest calls push `Frame`s onto `it.frames` and
/// this loop keeps stepping the top one — the host never recurses, so all state stays in `it`
/// and a `Paused` interpreter can be continued later, even in another process.
pub fn resume(module: &Module, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap> {

}
```

**Hints** — the "loop" half of `run`, plus a budget:

1. Build `jumps` exactly as `run` does. Keep the `ponytail:` comment, but say "per resume".
2. A step counter, starting at `0`.
3. `while let Some(frame) = it.frames.last()` — same loop header as `run`. Inside:
   - counter equals `max_steps`? → `return Ok(Outcome::Paused);`
   - otherwise `step(...)` once, then add 1 to the counter.
4. After the loop (no frames left) → `Ok(Outcome::Done(...))` with the popped value stack.

**Two traps:**

- **Order matters.** The budget check goes *inside* the loop, so "no frames left" is checked
  first (by the `while`). That's why a budget of exactly 4 in test 1 gives `Done`, not `Paused`.
  Walk through test 1 by hand with your loop before running it.
- `it` is **already** a `&mut Interp` in here. `run` wrote `step(&mut it, …)` because its `it`
  was an owned `Interp`. Here you pass `it` straight through. (Write `&mut it` and rustc reports
  `E0596 cannot borrow 'it' as mutable` — you'd be taking a pointer *to the pointer*. Its
  `help:` line says it exactly: "remove the `&mut`".)

<details><summary>Solution</summary>

```rust
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
```

`steps` has no type annotation: Rust infers `u64` because it's compared with `max_steps`.
</details>

**Check:**

```powershell
cargo test -p parakaya-vm
```

Expected: 20 passed, including `resume_budget_is_exact` and `trap_after_pause_still_traps`.
(Don't filter by `resume`: that name doesn't match `trap_after_pause_still_traps`, so only
one of the two would run.)

## Step 6 — Rewrite `run` on top of `start` + `resume`

`run` now duplicates both. Replace its **body** (keep the signature) so it just:

1. starts the call,
2. resumes it with an unlimited budget (`u64::MAX`),
3. `match`es the `Outcome`: `Done(result)` → `Ok(result)`; `Paused` → `unreachable!("…")`.

Also fix its doc comment: `run` no longer owns the loop.

**Teaching note:** `match` must cover **every** variant. Delete the `Paused` arm and rustc
refuses to compile (`E0004 non-exhaustive patterns`). Try it, then put it back. C's `switch`
would have let you forget it silently.

<details><summary>Solution</summary>

```rust
/// Call `func_idx` with `args` and run it to completion.
pub fn run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap> {
    let mut it = start(module, func_idx, args)?;
    match resume(module, &mut it, u64::MAX)? {
        Outcome::Done(result) => Ok(result),
        Outcome::Paused => unreachable!("u64::MAX steps never runs out"),
    }
}
```

`run` is now *above* `start`/`resume` in the file but calls them. That's fine in Rust; there
are no forward declarations like C's prototypes.
</details>

**Check:** `cargo test` → **20 unit tests + 3 end-to-end** green. The 18 old tests pass
untouched, because `run`'s signature didn't change.

## Step 7 — `lib.rs`

**7a. Re-export** — under `pub use error::{DecodeError, Trap};` add (type exactly):

```rust
pub use exec::{Interp, Outcome};
```

so users can write `parakaya_vm::Outcome` instead of `parakaya_vm::exec::Outcome`.

**7b. A private lookup helper** — inside `impl Vm`, at the bottom:

```rust
    /// The function index exported as `name`.
    fn export(&self, name: &str) -> Result<u32, Trap> {

    }
```

Hint: it's the chain in `invoke`, but it returns the `func_idx` (a `u32`), not the whole
`Export`. `Option` has `.map(|e| …)`; it goes between `.find(…)` and `.ok_or(…)`. No `?` and no
`;` at the end: the chain itself is the return value.

**7c. Shrink `invoke`** to one line using `self.export(name)?`. Update its doc comment too; the
"M2" sentence is stale. Make it:
`` /// Run export `name` to completion. Fresh state (stack, memory) per call. ``

**7d. Two new methods** — type the signatures; each body is one line:

```rust
    /// Set up a call to export `name` without running it; drive it with `resume`.
    pub fn start(&self, name: &str, args: &[Value]) -> Result<Interp, Trap> {

    }

    /// Run `it` for at most `max_steps` more instructions. `it` must come from this
    /// module (via `start`, or a snapshot of it) — another module's state may panic.
    pub fn resume(&self, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap> {

    }
```

**Teaching note:** why `&self` here but `&mut self` on `invoke`? The new methods never change
the `Vm`; all mutation happens in the `Interp` the caller owns. So one `Vm` can drive many paused
runs at once. (`invoke` didn't need `&mut` either; it's left that way so no caller breaks.)

<details><summary>Solution (all of 7b–7d)</summary>

```rust
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
```
</details>

**Check:** `cargo test`. The 3 end-to-end tests go through `invoke`, so they prove 7c didn't
break anything.

## Step 8 — The `PartialEq` cascade (let rustc teach you)

Task 2 compares whole `Interp`s with `==`. In `exec.rs`, add `PartialEq` to **`Interp`'s derive
only**:

```rust
#[derive(Debug, Default, PartialEq)]
pub struct Interp {
```

Run `cargo build` and **read the error before fixing it.** Then also change that struct's doc
comment from "so M2 snapshots are" to "so a snapshot is".

<details><summary>What rustc says, and the fix</summary>

`E0369 binary operation '==' cannot be applied to type 'Vec<CtrlFrame>'` (and the same for
`Vec<Frame>`). A derived `==` compares field by field, so every field's type needs `==` too.
`Vec<T>` has `==` only when `T` does. Add `PartialEq` to both:

```rust
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CtrlFrame {
```
```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
```

`Value` already derives `PartialEq`, which is why `Vec<Value>` didn't complain.
</details>

## Step 9 — Finish

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All three clean → **tell me "Task 1 done" and I'll review the diff.** Then commit:

```powershell
git add crates/parakaya-vm/src/exec.rs crates/parakaya-vm/src/lib.rs
git commit -m "feat(vm): pausable execution (start/resume with a step budget)"
```

**Rustc habits:** fix the **first** error only, re-run, repeat. Trust the `help:` lines. Paste
anything you're stuck on for more than a few minutes.
