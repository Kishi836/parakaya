# Parakaya — Snapshot/Restore Design (rescope D11–D12)

> Status: approved in brainstorming 2026-10-12. Implementation plan:
> `docs/2026-10-12-parakaya-snapshot-plan.md`.

## Goal

Pause a running WASM program after N instructions, write its entire state to one file, and
resume it later — in a new process or on another device — with a result identical to a run that
was never interrupted. This is the first working "Return by Death": the process dies, the file
survives, execution continues.

**Success criterion:** for every pause point N of the test programs, *pause → save → load →
finish* returns exactly what the uninterrupted run returns.

## Scope

In: pausable execution API, a versioned hand-written binary snapshot format, a `parakaya` CLI
(`run` / `resume`), and the equivalence test.

Out (deferred, with reason):

| Not doing | Why not now |
|---|---|
| LZ4 / delta snapshots | one memory page is 64 KiB; size isn't a problem yet |
| Signatures / encryption | design doc §2 non-goal for this slice; checksum covers accidental corruption |
| NaN canonicalization | no float instructions exist until M2 |
| Semantic validation on load (pc in range, valid func_idx) | same stance as M1's "no validation pass"; a hand-tampered file may panic |
| Daemon, WAL handoff, mDNS, network transfer | full design (`2026-07-02-parakaya-design.md`), after D14 |
| Serde / bincode | chosen against: zero deps, and every byte of the cross-ISA format stays explainable |

## 1. Pausable execution (`exec.rs`, `lib.rs`)

`run` currently builds an `Interp`, loops to completion and drops the state. It splits in two:

```rust
#[derive(Debug, PartialEq)]
pub enum Outcome { Done(Option<Value>), Paused }

pub fn start(module: &Module, func_idx: u32, args: &[Value]) -> Result<Interp, Trap>;
pub fn resume(module: &Module, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap>;
pub fn run(module: &Module, func_idx: u32, args: &[Value]) -> Result<Option<Value>, Trap>;
```

- `start`: argument-count check, memory allocation, first frame pushed. No instruction runs.
- `resume`: the existing `while let Some(frame)` loop plus a counter. Each iteration first checks
  for an empty frame stack (→ `Done(result)`), then the budget (→ `Paused`). So
  `resume(it, 0)` on a fresh `Interp` is `Paused`, and a budget exactly equal to the program's
  step count returns `Done`.
- `run` = `start` then `resume(u64::MAX)`; all existing tests keep passing unchanged.
- Jump tables are rebuilt inside each `resume` call (existing `ponytail:` note; moves to decode
  time in M2).
- `Interp` gains `PartialEq`. `Interp` and `Outcome` are re-exported from `lib.rs`.

`Vm` gains two thin wrappers; `invoke` stays:

```rust
pub fn start(&self, name: &str, args: &[Value]) -> Result<Interp, Trap>;   // export lookup + exec::start
pub fn resume(&self, it: &mut Interp, max_steps: u64) -> Result<Outcome, Trap>;
```

**Why this is sufficient:** all guest state already lives in `Interp` as plain owned data (M1
architecture rule). The only loop-local data is the jump tables, which are derived from the
module. Pause points are instruction boundaries (design doc §5 "Migration points").

## 2. Snapshot format (`snapshot.rs`)

All integers little-endian, fixed width. Every `usize` — including every count and length —
is written as `u64` (a full 65,536-page memory is exactly 2³² bytes, one past `u32::MAX`).

```
magic      "PKSN"                          4 bytes
version    u32 = 1
wasm       len u64, then bytes             the program travels inside the snapshot
values     count u64, each: tag u8 + payload
             tag 0 = I32 (4 B), 1 = I64 (8 B), 2 = F32 (raw bits, 4 B), 3 = F64 (raw bits, 8 B)
ctrls      count u64, each: opener_pc u64, end_pc u64, is_loop u8 (0/1), height u64
frames     count u64, each: func_idx u32, pc u64,
                            locals (count u64 + values as above),
                            value_base u64, ctrl_base u64
memory     len u64, then bytes
checksum   u64 FNV-1a over every preceding byte
```

```rust
pub fn save(wasm: &[u8], it: &Interp) -> Vec<u8>;
pub fn load(bytes: &[u8]) -> Result<(Vec<u8>, Interp), SnapshotError>;

pub enum SnapshotError { BadMagic, BadVersion, UnexpectedEof, BadChecksum, BadTag(u8), TooLarge, TrailingBytes }
```

Design decisions:

- **`usize` → `u64`.** `usize` is 8 bytes on the x86-64 PC and 4 on a 32-bit ARM OS. Writing it
  raw would make the layout depend on the machine that wrote it. Load converts with
  `usize::try_from`, failing with `TooLarge` instead of truncating.
- **Program embedded.** One file to copy for the D13 hop; no module-hash matching. (Rust's
  `DefaultHasher` is not stable across builds or machines, so it could not have been the hash.)
- **FNV-1a 64 checksum, verified before parsing.** ~5 lines, no dependency, identical on every
  platform. A corrupted or truncated transfer fails loudly rather than resuming wrong state —
  silently wrong results are the worst failure a migration system can have.
- **Reader reuse.** `decode.rs`'s bounds-checked `Reader` becomes `pub(crate)`; `snapshot.rs`
  uses its `u8`/`bytes` and adds `get_u32`/`get_u64` helpers on top. One
  `From<DecodeError> for SnapshotError` (→ `UnexpectedEof`) lets `?` work across both.
  `SnapshotError` lives in `error.rs` beside `Trap` and `DecodeError`. `Reader::bytes` switches
  to `checked_add`, so a crafted huge length is an error, not an overflow panic.
- **Trailing bytes are an error** (`TrailingBytes`): a file must parse to exactly its checksum.

## 3. CLI (`src/main.rs`)

A `[[bin]] name = "parakaya"` in the existing `parakaya-vm` crate. Arguments parsed by hand from
`std::env::args` (no clap).

```
parakaya run    <prog.wasm> <export> [i32 args...] [--pause-after <N> <out.pksn>]
parakaya resume <snap.pksn>                        [--pause-after <N> <out.pksn>]
```

- Finishes → prints the result (`5040`, or `(no result)`), exit 0.
- Paused → writes the snapshot, prints `paused after N steps -> out.pksn`, exit 0.
- Program finishes before N steps → prints the result; no file written.
- `resume` accepting `--pause-after` means hops chain (PC → Pi → PC) at no extra cost.
- Any trap, decode error, snapshot error or I/O error → message on stderr, exit 1.

## 4. Testing

| # | Test | Where | Proves |
|---|---|---|---|
| 1 | **Equivalence sweep**: for factorial(10) and the memory counter, measure total steps T; for every N in 0..T: start → `resume(N)` is `Paused` → save → load → `resume(MAX)` equals the uninterrupted result | `tests/snapshot_equivalence.rs` | the success criterion, at every pause point |
| 2 | Round-trip: a mid-run `Interp` → save → load equals the original, and the wasm bytes come back | `snapshot.rs` unit | the format loses nothing |
| 3 | Corruption: one flipped byte → `BadChecksum`; file cut short → `BadChecksum` (checksum runs first); fewer than 8 bytes → `UnexpectedEof`; wrong magic with a valid checksum → `BadMagic` | `snapshot.rs` unit | load fails loudly |
| 4 | Budget edges: `resume(0)` on a fresh `Interp` is `Paused`; `resume(T)` is `Done` | `exec.rs` unit | the counter is exact |
| 5 | CLI smoke: `run … --pause-after`, then `resume`, prints the right number | `tests/cli.rs` via `env!("CARGO_BIN_EXE_parakaya")` | the end-user path works |

The cross-ISA version of test 1 (save on x86, load on ARM) is D13's manual hop. It becomes a CI
job only if an ARM runner is wired up later.

## 5. Schedule

- **D11:** §1 pausable execution + tests 4; §2 `snapshot.rs` + tests 2–3; test 1.
- **D12:** §3 CLI + test 5; README status update.

Teaching mode applies: the user types the implementation from the plan's snippets and teaching
notes; AI reviews at task boundaries.
