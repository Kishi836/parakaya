# Next-Project Brainstorm — Decision History

> Purpose: restore points for the brainstorming session on the "game-breaking solo project"
> (resume-defining, placement deadline **June 1, 2027**). If a session is lost, resume from the
> latest restore point below.

## Fixed decisions (settled, not in question)

| # | Question | Answer |
|---|----------|--------|
| 1 | Artifact type | **Working system** (not research result, not adopted tool) |
| 2 | Stack layers of interest | OS/hardware boundary · Data/storage engines · Networking/distributed |
| 3 | Timeline | ~11 months; hard deadline **1 June 2027** (placements, 6th→7th sem) |
| 4 | Hardware/budget | Gaming-class NVIDIA GPU · SBCs/phones/microcontrollers (₹5–15k ok) · cloud credits ok |

## ⭐ RESTORE POINT 1 — "Novelty type" (the 4-option category choice)

**The question:** Which flavor of "game-breaking"?
Options: (a) Problem nobody noticed · (b) Problem people gave up on · (c) Known-hard flex · (d) Pitch a mix

**Revision history:**
1. **First choice: (c) Known-hard flex** → led to 3 system pitches (Distributed SQL DB / GPU query engine / Micro-hypervisor) → picked **Distributed SQL DB** → architecture section presented (Rust, DST-from-day-one, Raft, LSM, pgwire).
2. User realized the options weren't fully understood → asked what problem it solves / whether these already exist → explanation given (known-hard flex = execution game; the things exist, the wow is a solo student building one with industry-grade proof).
3. **Rolled back to this question and re-chose: (b) Problem people gave up on** ← **CURRENT PATH**

**Shelved (not dead) if we ever restore to the old branch:** the full Distributed SQL DB pitch —
Postgres-wire-compatible, Rust, LSM storage engine, Raft on a 5-node Pi cluster, FoundationDB-style
deterministic simulation testing, Jepsen validation.

## ⭐ RESTORE POINT 2 — "The corpse" (which abandoned problem to revive)

**The question:** Which abandoned problem to revive?
Options pitched: (a) **WinFS revival** — semantic/database filesystem, canceled by Microsoft 2006 ·
(b) Process teleportation — live WASM process migration across architectures (Sprite/MOSIX corpse) ·
(c) Offline internet — terrestrial DTN revival (Bundle Protocol corpse)

**Choice: (a) WinFS revival** — a local, automatic, semantic, queryable filesystem layer for Windows
(OS layer + storage engine layer; local AI = the missing 2006 ingredient).
Shelved but viable: (b) and (c) — full pitches recoverable from this session's context above.

## Novelty check results — WinFS revival (run 2026-07-02)

**Verdict: the naive slot is CLOSED.** Microsoft un-gave-up in 2026:
- **Windows 11 26H2** ships AI File Explorer: natural-language semantic file search on-device (NPU,
  Phi Silica), reworked Recall. The consumer demo ("ask your files a question") now gets the reply
  "doesn't Windows already do that?"
- **LSFS** (arXiv 2410.11843, AIOS): academic LLM-based semantic file system already published.
- Crowded hobby/tool space: LlamaFS, ODF, AIFS, File Brain, SpotMyFile.
- **SemanticDrive** (GitHub: IstakAm/semantic-storage-engine-using-WINFSP-and-Gemma): a WinFsp+LLM
  virtual drive with natural-language smart folders — squats on the exact pitched combo.
- Remnant slot (open but weakened): the TRUE WinFS — typed schema + cross-file RELATIONSHIPS +
  live query-folders as real folders for every app (relational FS layer, not a search box).
  Defensible architecturally, but the layperson wow is eaten by Windows 26H2.

**Fallback candidate re-scanned — process teleportation (WASM live migration):**
- Academic area is WARM, not dead: Wanco (POPL 2025 SRC) instruments AOT-compiled WASM for
  platform-neutral snapshots; 2025-26 papers on edge-cloud WASM migration call compiled-module
  heterogeneous migration "still an open challenge."
- **No shipped SYSTEM exists** — nobody has built the consumer-visible "app follows you across
  devices" runtime with a real demo. Papers validate the problem; the system slot is open.
- Strong revival narrative: 90s gave up (Sprite/MOSIX) → WASM removed the wall → academia circling
  → the student ships the system the papers point toward.

**Caveat logged for corpse (c) DTN:** Meshtastic + Reticulum are ACTIVE communities — offline mesh
is not truly abandoned; that slot needs careful carving if ever revisited.

## ⭐ RESTORE POINT 3 — post-novelty-check pivot

**The question:** WinFS wow eaten by Windows 26H2; teleportation slot verifiably open. Which way?
Options: (a) pivot to process teleportation (recommended) · (b) true-WinFS relational slot anyway ·
(c) re-pitch fresh corpses.

**Choice: (a) PROCESS TELEPORTATION** — a WASM-based runtime where a running program migrates live
across heterogeneous devices (gaming PC ↔ Raspberry Pi ↔ phone ↔ cloud), mid-execution.
Revival narrative: Sprite/MOSIX (90s, dead) → WASM removed the CPU-heterogeneity wall →
2025-26 academia circling (Wanco, edge-cloud migration papers) → no shipped system exists.

## Deep-check results — process teleportation (run 2026-07-02)

**Competitor map (capstone-PS1 style):**
| Neighbor | What it does | What it is NOT (the opening) |
|---|---|---|
| **Golem Cloud** | Shipped durable-computing platform: continuous WASM worker snapshots (heap+stack+IP), transparent migration between nodes on failure. Dev Preview; GA planned Q3 2026 | **Server-side, cloud-internal, fault-recovery only.** No consumer devices, no interactive apps, no user-initiated "follow me" handoff, no P2P |
| **MVVM / Wharf** (arXiv 2410.15894) | Research framework: live migration of LLM-agent workspaces edge↔cloud on heterogeneous HW (3 devices, 18 workloads) | Research prototype for agent workloads; no consumer UX, no shipped system, no device mesh |
| **Wanco** (POPL'25 SRC) | AOT-compiler instrumentation for platform-neutral WASM snapshots; beats CRIU on migration smoothness | A *mechanism* prototype by one student — no system around it |
| **wasmtime** | Issue #4002 requests snapshot-migrate — **unimplemented** since 2022 | Mainstream runtimes ship nothing |
| **WAMR** | Research-grade checkpoint/restore instrumentation exists | Not a product; no cross-device system |

**The unoccupied slot (precise):** a user-facing, cross-device SYSTEM in which running programs
migrate live among heterogeneous consumer devices (x86 PC ↔ ARM Pi ↔ Android phone ↔ cloud VM),
via user-initiated or policy-driven handoff over LAN/P2P, with visible sub-second continuity —
including device discovery, transactional handoff protocol, and open-resource (WASI) state
virtualization. The papers prove the MECHANISM is feasible (lowers technical risk); nobody has
built the SYSTEM. Golem explicitly occupies only the cloud-failover corner.

**Candidate verifiable markers (the "Jepsen equivalents"):**
- Live migration across 4 device classes / 2 ISAs (x86-64 ↔ aarch64) demonstrated
- Measured handoff latency (target: sub-second on LAN for real state sizes)
- Transactional handoff: kill -9 / power-pull mid-migration loses nothing (exactly-once resume)
- Real workloads (game loop / LLM chat session / long computation), not just microbenchmarks
- Reproducible public benchmark + demo video

## Kill-vector research — round 2 (user-demanded, run 2026-07-02)

**KV1 — Big tech ships OS-level app migration? NO.** Apple Handoff, Google "Task Continuity"
(Android 17, not ready yet), Microsoft "Cross-Device Resume" (Spotify/M365/browser-tabs only) are
ALL app-cooperative state sync: the app developer implements save/restore, whitelisted apps only.
None migrate a running process. Their investment PROVES demand while leaving the slot open.
Framing must answer "isn't that Handoff?" → Handoff resumes bookmarks; this moves the living program.

**KV2 — WASM cloud platforms expand to devices? Not on any visible roadmap.** wasmCloud actors
historically stateless (new wash-runtime is stateless wasmtime wrapper); Fermyon Spin = stateless
serverless; Golem = cloud failover, GA Q3 2026, server business.

**KV3 — Academic productization? Partial overlap found, contained.** MVVM (github.com/Multi-V-VM/MVVM,
~15k LOC C++ on WAMR, AOT checkpoint instrumentation ~500 LOC) is OPEN SOURCE and migrates across
ARM/RISC-V/x86 — but is a research codebase for LLM-agent workspaces: no device discovery, no
consumer handoff UX, no phone app, no transactional guarantees, no polished system. Mechanism
exists in research form → the project's claim must be the SYSTEM, not the raw mechanism.

**KV4 — Cloud streaming substitute? Real but orthogonal.** GeForce Now/Windows 365 give continuity
by keeping the app in the cloud — requires constant network, latency, subscription, no privacy/
offline. Local-first LAN teleportation serves the offline/private/edge niche streaming cannot.

**Honest verdict given to user:** mechanism = not novel (research exists, cite it); SYSTEM =
first-of-kind, verifiably unoccupied; ~85% confidence slot stays open through June 2027; ~99%
confidence resume value survives even if someone ships adjacent work mid-project (unlike WinFS,
where the wow was already dead before starting). No 100% guarantee exists; claims must be phrased
precisely (capstone-style honesty notes).

## ⭐ RESTORE POINT 4 — TOPIC LOCKED (2026-07-02)

**THE PROJECT: Process teleportation.** A working system in which running programs (WASM) migrate
live across heterogeneous consumer devices — gaming PC (x86-64) ↔ Raspberry Pi (aarch64) ↔ Android
phone ↔ cloud VM — via user-initiated handoff over LAN/P2P, with sub-second continuity and a
transactional protocol (power-pull mid-hop loses nothing).

Claim to defend: **first working cross-device live-teleportation SYSTEM for ordinary programs**
(mechanism credit goes to MVVM/Wanco — cite, don't claim). Locked with eyes open: ~85% slot
survival to June 2027, ~99% resume-value survival.

## Current state (as of 2026-07-02)

- Phase: DESIGN (brainstorming skill).

### Design decisions locked
1. **Workloads: graphical flagship** — playable 2D game hops devices mid-frame. Trick: game draws
   into a framebuffer inside WASM linear memory → screen migrates with the process for free; each
   device runs a thin shell (display buffer + forward input). Plus compute workloads (raytracer /
   chess engine / encoder) and a terminal session for sober benchmarks.
2. **Runtime: own WASM interpreter in Rust.** Execution state is plain data → snapshot at ANY
   instruction boundary, deterministic, cross-ISA by construction (capability edge over MVVM/Wanco
   safepoint checkpointing). Cost accepted: ~5-15x slower than AOT (fine for 2D game; wasm3-class
   interpreters run DOOM). JIT tiering explicitly deferred to future work. Interview answer for
   "why not wasmtime/WAMR?" scripted in session 2026-07-02: requirements-driven tradeoff —
   migration is the product; compiled runtimes bury state in native registers.

3. **Triggers: manual summon + one auto policy** — tap a device to summon the process, plus
   host-dying auto-evacuation (battery critical / shutdown signal → flee to nearest peer).
4. **Name: PARAKAYA** — from parakaya pravesha (yogic transmigration: soul enters another body).
   Process = soul, device = body.
   **Tagline (revised 2026-07-02, user's pick from anime candidates): "Return by Death"** (Re:Zero's
   checkpoint-restore power — literally what Parakaya does). Usage rule: GitHub/README/demo video =
   "Return by Death"; resume bullet uses a descriptive line instead (recruiter-safe).
   Demo video title candidate: "That Time My Process Got Reincarnated as a Raspberry Pi".
   README fun-fact: parakaya pravesha = Orochimaru's Living Corpse Reincarnation jutsu.
   (Previous tagline "The program that refuses to die" retired but usable as body copy.)
5. **Flagship workload: THE MEDICAL GUARDIAN** (user choice, 2026-07-02) — patient-monitoring
   anomaly detector on a simulated ECG stream; follows the patient across devices; never drops a
   beat mid-migration; all-local/private. Ties into MedInnTech minor + FHIR project → coherent
   resume arc. Adds one ABI surface: sensor-stream subscribe/re-attach on resume. Framed as
   SIMULATED demo, never a medical-device claim.
   Demo lineup: Act 1 = medical guardian (flagship) · Act 2 = immortal molecular computation
   (kept per user — zero extra scope) · Act 3 = playable game hop (fun, free once SDK exists).
   Design §1 (architecture & components) approved with this amendment.
### Design status
- §1 Architecture & components — APPROVED (with medical-guardian flagship amendment)
- §2 Migration protocol & failure model (INV-1 "at most one soul", INV-2 "soul never lost",
  two-phase WAL handoff, epoch fencing, urgent evacuation, 10s local durability) — APPROVED
- §3 Testing, verifiable markers, 11-month milestones — APPROVED ("this looks gud")
- **Spec APPROVED by user: `2026-07-02-parakaya-design.md`.**
- **M1 implementation plan written: `2026-07-02-parakaya-m1-vm-skeleton-plan.md`** — 8 TDD tasks:
  repo/CI scaffold → i32 arithmetic exec → locals/comparisons → control flow (jump table + ctrl
  stack) → functions/frames (no host recursion) → linear memory → binary decoder → public
  instantiate/invoke API + WAT end-to-end tests. One plan per milestone; M2 plan written when M1
  ships.
- **Repo location (user override 2026-07-02): `Projects\Parakaya` (inside OneDrive)** with a
  git-ignored `.cargo\config.toml` redirecting target-dir to `C:\Users\Admin\.cargo-target\parakaya`
  so build artifacts never sync. `/setup-matt-pocock-skills` runs inside the repo at Task 1.
- **Execution mode (user, 2026-07-02): Task 1 = subagent-driven; Tasks 2–8 = USER DRIVES, AI
  reviews/unblocks** (project doubles as the user's Rust bootcamp — ownership matters for
  interviews). M1 execution begins.

### M1 execution log
- **Task 1 COMPLETE (2026-07-02).** Commit `c9317ed` (scaffold) + `bbdc005` (agent-skills config).
  Review: spec ✅, quality Approved, no Critical/Important. Toolchain saga: rustup MSVC installed;
  GNU toolchain tried and reverted (Windows blocked its rustc); VS Build Tools installed → linker
  verified. `/setup-matt-pocock-skills` done: GitHub tracker (gh), default 5 triage labels,
  single-context CONTEXT.md seeded with the Parakaya glossary. Repo: Projects\Parakaya, branch main.
  SDD ledger: `.superpowers/sdd/progress.md` in the repo.
- **Task 2 COMPLETE (2026-10-08, user-driven).** Commit `a588cd4`. Review found a plan bug:
  `i32.div_s` used `wrapping_div`, so `i32::MIN / -1` returned `MIN` where the WASM spec requires a
  trap. **Fix (user approved):** new `Trap::IntegerOverflow` variant + explicit `(i32::MIN, -1)` arm;
  plan Task 2 updated with `div_by_zero_traps` / `div_overflow_traps` tests. Also fixed in plan:
  Tasks 3–7 committed with `git commit -am`, which skips the new files Tasks 4/5/7 create (would
  commit `lib.rs` declaring a module whose file is missing → CI break); now `git add
  crates/parakaya-vm/src; git commit -m`.
- **Task 3 COMPLETE (2026-10-08, user-driven)** — locals + comparisons (`712ba2b`).
- **Tasks 4–8 COMPLETE locally (2026-10-08, AI-executed at user request — user in class).**
  `d1e058d` control flow · `6ccf8bd` call frames · `f4917a8` memory · `561950d` decoder ·
  `28595f9` public `Vm` API. **M1 exit criterion green locally:** 18 unit + 3 end-to-end tests
  (WAT factorial, memory counter, unknown export); clippy `-D warnings` + fmt clean.
  Plan bugs found and fixed, each with a test that fails on the plan's code:
  1. `If`/`Else` leaked control frames (then-arm reaching `Else`, and `if(0)` with no else, both
     skipped `End` without popping) → later `br` depths resolved against stale frames.
  2. `br` out of a function walked into the **caller's** control frames; now bounded by the
     frame's `ctrl_base`, and `br` to the function body = return (WASM semantics).
  3. `run` silently zero-padded a wrong argument count; now traps `TypeMismatch`.
  4. Decoder accepted a section claiming bytes past EOF; now `UnexpectedEof`.
  Also: jump tables built once per `run` (plan cloned a HashMap on every instruction).
  Deferred: malformed modules panic rather than error — documented M1 "no validation pass".
- **M1 SHIPPED (2026-10-12)** — pushed to github.com/Kishi836/parakaya, CI green, README status
  updated. User walked through the AI-written Tasks 4–8 code the same day.
- **Rescope (2026-10-06):** instead of the full M2, ship a file-based snapshot hop first —
  D11–12 `--pause-after N` / `resume <file>` with "interrupted run == uninterrupted run" test,
  D13 resume on a second device, D14 README + video. Daemon/WAL/mDNS stay future work.
- **NEXT:** D11 snapshot/restore plan (`docs/2026-10-12-parakaya-snapshot-plan.md`).

## Process rules in force

- Brainstorming skill (superpowers): one question at a time, 2–3 approaches with trade-offs,
  design presented section-by-section for approval, then spec → plan. No implementation until
  design approved.
- User wants restore points recorded in this file at each major fork.
