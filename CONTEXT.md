# Parakaya — Domain Context

Seeded from the design spec (`docs/2026-07-02-parakaya-design.md`). Maintained via `/grill-with-docs`.

## What this project is

A system in which running WASM programs migrate live between heterogeneous devices
(x86-64 PC ↔ aarch64 Raspberry Pi ↔ Android phone ↔ cloud VM), surviving power loss at any instant.

## Glossary

| Term | Definition | Avoid calling it |
|---|---|---|
| **soul** | A running Parakaya process: the VM execution state (stacks, frames, memory, pc, ABI state) | "job", "container" |
| **body** | A device running the Parakaya daemon; hosts souls | "node" (ok in protocol code), "server" |
| **summon** | User-initiated migration: pulling a soul to the device you're on (`pk summon`) | "push migration" |
| **evacuation** | Self-preserving migration triggered by a dying host (battery critical / shutdown signal) | "failover" |
| **snapshot** | The serialized, versioned, cross-ISA representation of a soul at one instruction boundary | "checkpoint" (research-lingo ok in papers/README) |
| **handoff** | The two-phase, WAL-backed transfer of a soul between bodies (INTENT → PREPARED → COMMITTED) | "transfer" |
| **epoch** | Monotonic fencing counter incremented per migration; stale-epoch messages are rejected | "version", "term" |
| **tombstone** | Source-side marker proving a soul moved on; prevents zombie resurrection | — |
| **WAL** | Per-daemon write-ahead log making every handoff step crash-recoverable | — |
| **shell** | The thin per-device UI host: blits a soul's framebuffer, forwards input | "viewer" |
| **guardian** | The flagship workload: simulated-ECG anomaly detector that never drops a beat | — |
| **the ABI** | The guest contract: framebuffer region, input queue, hostcalls (time/rand/kv/stream/yield). Frozen for v1 | — |

## Invariants (the two laws)

- **INV-1 "at most one soul":** a process never runs on two bodies simultaneously — not even during failures.
- **INV-2 "the soul is never lost":** once a process exists, some body can always resume it.

## Key constraints skills must respect

- VM state is plain owned data (no host recursion for guest calls, no Rc/RefCell/pointers) — snapshot-ability is structural.
- `#![forbid(unsafe_code)]` everywhere.
- Determinism: guests get time/randomness only via hostcalls (virtual clock, seeded RNG).
- The guardian is a SIMULATED demo, never a medical-device claim.
