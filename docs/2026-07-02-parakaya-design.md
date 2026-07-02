# Parakaya — Design Specification

> **Return by Death** — live process migration across heterogeneous devices.
> Author: PES CSE, solo + frontier AI · Date: 2026-07-02 · Status: awaiting user approval
> Deadline: resume-ready by **1 June 2027**
> Decision log: `DECISION-HISTORY.md` (same folder). This spec moves into the Parakaya repo at creation.

---

## 1. Problem & Positioning

**The revival story.** In the 80s–90s, distributed-OS research (Sprite, MOSIX, Amoeba) chased live
process migration — a running program hopping between machines mid-execution. It died by ~2000:
heterogeneous CPUs meant incompatible machine code, and open resources didn't survive the jump.
WebAssembly removed the foundational wall (one portable instruction format across x86/ARM/RISC-V).
Academia noticed: MVVM (WAMR AOT-checkpoint instrumentation, LLM-agent edge↔cloud migration),
Wanco (POPL'25 SRC, compiler-injected platform-neutral snapshots), and WAMR research forks prove
the **mechanism**. Nobody has shipped the **system**: wasmtime's snapshot-migrate feature request
(issue #4002) is open since 2022; Golem Cloud does server-side fault-recovery migration only;
Apple Handoff / Android Task Continuity / Windows Resume are app-cooperative state sync for
whitelisted apps, not process migration.

**The claim Parakaya defends (phrased precisely):** *the first working user-facing system in which
ordinary running programs migrate live among heterogeneous consumer devices — gaming PC (x86-64) ↔
Raspberry Pi (aarch64) ↔ Android phone ↔ cloud VM — via user-initiated or self-preserving handoff,
with sub-second continuity and a transactional protocol under which power loss at any instant
loses nothing.*

**Honesty notes (viva/interview-grade):**
- The checkpoint/migrate *mechanism* for WASM exists in research form (MVVM, Wanco) — cited, not claimed.
- Parakaya's mechanism-level edge: interpreter state is plain data, so it can freeze at **any**
  instruction boundary; safepoint-instrumented AOT approaches can only stop at pre-injected points.
- Threat assessment (2026-07-02): ~85% the slot stays open to June 2027; ~99% the project retains
  resume value even if adjacent work ships (claims soften to "independently built + benchmarked").

## 2. Product Definition (v1)

One binary ecosystem, five components (§4), running on four device classes. A user can:

1. `pk run guardian.wasm` on any device — the process starts, its framebuffer appears in the shell.
2. `pk summon <proc>` from any other device — the process migrates live, screen and all, in under a
   second on LAN, resuming at the exact instruction.
3. Kill a device (shutdown, battery death) — the process **evacuates itself** to the nearest peer
   (auto-policy) or, on a no-warning crash, resumes from the last periodic local snapshot on reboot.
4. `pk ps` — see every process in the cluster and which body it currently inhabits.

**Demo lineup:**
- **Act 1 — flagship: the medical guardian.** A patient-monitoring anomaly detector consuming a
  *simulated* ECG stream over LAN, waveform rendered live in its framebuffer. Migrations mid-stream
  drop zero beats (proved by logged sequence numbers). All local, no cloud. *Framed strictly as a
  simulated demonstration, never a medical-device claim.*
- **Act 2 — the immortal computation.** A molecular-dynamics-style simulation (energy curve +
  molecule viz) that survives power cuts by fleeing between devices, resuming mid-iteration.
- **Act 3 — the game.** A playable 2D game hops PC → phone → Pi mid-frame. Pure fun, near-zero
  extra cost once the SDK exists.

**Non-goals (v1):** general WASI file/socket compatibility; GPU/WebGPU access; multi-process apps;
JIT/AOT execution tiers; NAT hole-punching; policy engine beyond the single evacuation rule;
security hardening beyond TLS transport + snapshot integrity hashes; iOS.

## 3. Hardware & Targets

| Body | ISA | Role |
|---|---|---|
| Gaming PC (Windows; WSL2/Linux for dev) | x86-64 | dev machine, demo body |
| Raspberry Pi 5 (~₹8k) | aarch64 | edge body, power-pull victim |
| Android phone (existing/cheap) | aarch64 | pocket body (NDK build + SurfaceView shell) |
| Cloud VM (free tier) | x86-64 | always-awake body; devices dial out to it (public IP, no NAT tricks) |

## 4. Architecture & Components

```
        ┌────────────── one "body" (each device) ───────────────┐
        │  parakaya-shell            parakaya-daemon             │
        │  ┌───────────────┐   ┌─────────────────────────────┐   │
        │  │ blit process  │◄─►│ hosts souls (VM instances)  │   │
        │  │ framebuffer;  │   │ peer table (mDNS discovery) │   │
        │  │ forward input │   │ migration endpoint (TLS)    │   │
        │  └───────────────┘   │ policy watcher (batt/shutdn)│   │
        │                      │ WAL + snapshot store        │   │
        │                      └──────────────┬──────────────┘   │
        │                      parakaya-vm    ▼                  │
        │                      ┌─────────────────────────────┐   │
        │                      │ value stack · frames · PC   │   │
        │                      │ linear memory · globals ·   │   │
        │                      │ tables  = plain data =      │   │
        │                      │ THE SNAPSHOT                │   │
        │                      └─────────────────────────────┘   │
        └────────────────────────────────────────────────────────┘
          LAN: mDNS + direct TLS        cloud: same daemon, dialed out to
```

1. **parakaya-vm** (Rust library) — WASM core interpreter. All execution state in serializable
   structures. Deterministic: no ambient time/randomness (hostcalls only).
2. **parakaya-abi + guest SDK** (Rust, compiled to wasm32) — framebuffer region, input-event queue,
   hostcalls: `now()` (virtual clock), `rand()` (seeded), `kv_get/put` (small persistent state),
   `stream_subscribe/next/ack` (named LAN streams, used by the guardian), `yield()`.
3. **parakaya-daemon** — process hosting, discovery, migration protocol (§6), WAL, periodic local
   snapshots (default 10s), policy watcher, `pk` control socket.
4. **parakaya-shell** — winit + softbuffer window (Windows/Linux/Pi); Android app with SurfaceView
   + touch forwarding; headless mode for compute workloads.
5. **pk CLI** — `run`, `ps`, `summon`, `kill`, `peers`.

**Free screen migration:** the framebuffer lives inside guest linear memory, so migrating memory
migrates the pixels; the destination shell just starts blitting. No graphics API virtualization.

## 5. VM & Snapshot Design

- **Scope:** WASM core spec (MVP) + the Parakaya ABI. No threads, no SIMD in v1 (spec-suite subsets
  for excluded proposals documented in the README).
- **Interpreter:** register/stack hybrid in the wasm3 style as a stretch; a clean stack machine
  first. Performance target: 60fps for the 2D game and guardian viz on the Pi — validated early
  (M1 exit criterion), with resolution scaling as the fallback lever.
- **Snapshot format:** versioned, self-describing binary: module hash + globals + memory (LZ4,
  optionally delta vs a prior snapshot) + tables + call stack (frames, locals, PC) + pending
  hostcall state + ABI state (input queue, stream cursors) + virtual-clock/RNG seeds + integrity
  hash. Endianness pinned little-endian (all four targets are LE; enforced by serializer tests).
- **Cross-ISA correctness rule:** a snapshot taken after N instructions on ISA A and resumed on
  ISA B must produce bit-identical results to an unmigrated run (WASM's deterministic core + NaN
  canonicalization on floats makes this achievable; validated continuously in CI).
- **Migration points:** any instruction boundary. Freeze latency is microseconds (finish current
  instruction, stop).

## 6. Daemon & Migration Protocol

**Invariants:**
- **INV-1 "at most one soul":** a process never runs on two bodies simultaneously.
- **INV-2 "the soul is never lost":** after a process exists, some body can always resume it.

**Discovery:** mDNS service `_parakaya._tcp` on LAN; static peer entry for the cloud body. Peer
table gossips process locations for `pk ps`.

**Handoff (two-phase, WAL-backed, epoch-fenced):**
1. Target requests process (summon) or source initiates (evacuation).
2. Source freezes VM at next instruction; WAL `INTENT(proc, epoch n+1, target)`.
3. Source streams compressed snapshot; target verifies integrity hash, persists snapshot,
   WAL `PREPARED`, replies PREPARED.
4. Source WAL `COMMITTED`, sends COMMIT, keeps tombstone (never runs epoch ≤ n again).
5. Target WAL `COMMITTED`, resumes VM at exact instruction; shell blits; streams re-attach from
   last-acked sequence number (source-side sensors buffer briefly → gapless).

**Failure matrix (power loss at any point):**

| Window | Outcome |
|---|---|
| before target PREPARED persists | source recovers via WAL, resumes locally; target discards partial |
| after PREPARED, before COMMIT | on reconnect: target-wins if PREPARED persisted (source tombstones); if target is permanently gone, source reclaims after lease timeout |
| after COMMIT | target owns; source tombstone prevents zombie resurrection |

Epoch fencing everywhere: daemons reject messages/commands bearing stale epochs.

**Urgent evacuation:** battery-critical / OS shutdown-signal watcher triggers the same protocol
with immediate freeze and fastest-peer selection. No separate code path.

**Local durability:** periodic (10s default) local snapshots → no-warning crashes (kernel panic)
resume from the last snapshot on reboot, no migration involved.

## 7. Flagship Workload — The Medical Guardian

- `ecg-sim` (host-side tool) streams a synthetic ECG (normal sinus + injectable anomalies:
  tachycardia, dropped beats, fibrillation patterns) as a named Parakaya stream on the LAN.
- `guardian.wasm` subscribes, runs windowed anomaly detection (classical DSP thresholds — simple,
  explainable, NOT a medical claim), renders waveform + status to its framebuffer, raises alarms
  via a hostcall (shell notification + sound).
- **Never-drops-a-beat proof:** every beat carries a sequence number; the guardian logs receipt;
  a test migrates the guardian hundreds of times during a long stream and asserts a gapless log.
- Demo beats: live waveform on Pi → summon to phone mid-stream (waveform continues) → anomaly fires
  correctly while a migration is in flight → Pi shutdown mid-monitoring → guardian evacuates, no gap.

## 8. Testing Strategy

1. **Interpreter:** W3C WASM core spec test suite in CI (public pass/fail claim) + differential
   fuzzing vs wasmtime (random programs, outputs must match).
2. **Snapshots:** cross-ISA equivalence in CI — random programs, random snapshot points, x86↔ARM
   resume, bit-identical results required.
3. **Protocol:** deterministic simulation — daemon state machines run over a simulated network;
   seeded RNG injects crashes, partitions, duplicates, power-pulls at every protocol step;
   thousands of runs per push asserting INV-1/INV-2; failures replay exactly from seed.
4. **End-to-end:** scripted 4-device runs (guardian + migrations + power-pulls) before each demo.

## 9. Verifiable Markers (resume claims with receipts)

- VM passes the W3C WASM core spec test suite; differential-fuzzed against wasmtime
- Live migration across 4 device classes / 2 ISAs, on camera
- Published handoff-latency curve (target: sub-second on LAN at guardian-sized state)
- Zero dropped beats across N migrations (logged sequence numbers)
- 1,000+ simulated power-pulls at randomized protocol steps: zero duplicated/lost processes
- Public repo + demo video ("That Time My Process Got Reincarnated as a Raspberry Pi")

## 10. Milestones (deadline 1 June 2027)

| Months | Milestone | Exit criterion |
|---|---|---|
| Jul 2026 | Rust bootcamp via VM skeleton | hand-written WASM subset executes |
| Aug–Oct 2026 | full interpreter + ABI + desktop shell | spec suite passing; program draws to framebuffer; **Pi 60fps validated** |
| Nov–Dec 2026 | snapshot/restore, cross-ISA | equivalence test passes PC↔Pi |
| Jan–Feb 2027 | daemon + discovery + protocol + simulation harness | LAN migration survives simulated & real power-pulls |
| Mar 2027 | Android shell + cloud peer + stream re-attach | 4-device end-to-end works |
| Apr 2027 | guardian flagship + evacuation + benchmarks | demo script clean; numbers collected |
| May 2027 | video, writeup, polish | resume-ready; one month slip absorbed |

## 11. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Interpreter too slow for 60fps demo on Pi | validate at M1 with a rendering microbenchmark; levers: resolution scaling, dirty-rect blitting, opcode dispatch optimization; worst case the guardian viz (low fps needs) leads and the game demotes |
| Cross-ISA float/NaN divergence | NaN canonicalization at snapshot time; CI equivalence suite from day one |
| Android NDK friction | shell is thin (SurfaceView + socket to daemon); daemon is pure Rust (cross-compiled); schedule it in Mar with the whole month |
| Scope creep via ABI growth | ABI is frozen at §4's list for v1; every addition requires a spec amendment |
| Adjacent work ships mid-project | claims soften per §1 honesty notes; system + benchmarks retain value |
| Solo-motivation decay | monthly exit criteria are demos, not documents; each month ends with something that visibly works |

## 12. Future Work (explicitly out of v1)

JIT/AOT execution tiers (protocol unchanged) · general WASI compatibility · multi-device process
groups · policy engine (follow-me via BLE proximity, load-based placement) · encrypted snapshots +
attestation · WASM threads/SIMD · iOS shell.
