# Parakaya

> Return by Death — live process migration across heterogeneous devices.

A running program hops between your PC, Raspberry Pi, phone, and a cloud VM —
mid-execution, across CPU architectures, surviving power loss at any instant.

## Status

**M1 (VM skeleton) — done.** `crates/parakaya-vm` is a from-scratch WebAssembly
interpreter with no `unsafe`:

- decodes real `.wasm` binaries (i32 subset: arithmetic, comparisons, locals,
  `block`/`loop`/`if`/`br`/`br_if`, calls, linear memory)
- keeps the guest call stack as plain data (`Vec<Frame>`), never host recursion —
  so the whole machine state can be paused and serialized
- traps instead of crashing on divide-by-zero, overflow, out-of-bounds memory

**Next: snapshot/restore** — pause after N instructions, write the VM state to a
file, resume it later or on another device.

```sh
cargo test   # unit tests + end-to-end runs of WAT programs (factorial, memory counter)
```

Design: `docs/2026-07-02-parakaya-design.md` · decisions: `docs/DECISION-HISTORY.md`.
