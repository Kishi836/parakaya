# Task 2 — Snapshot format (typing guide)

> Open beside the code with `Ctrl+K V` in VS Code. Hints are visible; **full solutions are
> folded** — click ▶ only after you've tried. Full plan: `docs/2026-10-12-parakaya-snapshot-plan.md`
> (Task 2) · format spec: `docs/2026-10-12-parakaya-snapshot-design.md` §2.

**Goal:** turn an `Interp` (plus the program's `.wasm` bytes) into a `Vec<u8>` you could write to
a file — and back again — so that bytes written on your x86 PC load identically on an ARM Pi.

**Files you'll touch:** `src\decode.rs`, `src\error.rs`, `src\lib.rs`, new `src\snapshot.rs`
(all under `crates\parakaya-vm\`).

**Progress checklist**

- [ ] 1. `decode.rs`: make `Reader` shareable + overflow-safe
- [ ] 2. `error.rs`: `SnapshotError` + `From<DecodeError>`
- [ ] 3. `lib.rs`: register the module
- [ ] 4. `snapshot.rs`: stubs + 6 tests → run → red
- [ ] 5. `fnv1a` → its test goes green
- [ ] 6. writers: `put_u32`, `put_u64`, `put_bytes`, `put_values`
- [ ] 7. `save`
- [ ] 8. readers: `arr`, `get_u32`, `get_u64`, `get_usize`, `get_bytes`, `get_values`
- [ ] 9. `load` → all green
- [ ] 10. fmt + clippy + test → "Task 2 done" → review → commit

---

## Concepts first

**1. A snapshot is a flat list of fields, written in a fixed order.** `save` writes them; `load`
reads them back **in exactly the same order**. If the two ever disagree on order or width,
everything after that point is garbage. Keep `save` and `load` side by side and make them
mirror images.

**2. Little-endian, fixed width.** A `u32` is 4 bytes; which byte comes first depends on the
CPU unless you pin it. `0x01020304u32.to_le_bytes()` is always `[04, 03, 02, 01]` (low byte
first), on every machine. `u32::from_le_bytes([04, 03, 02, 01])` gives `0x01020304` back.

| Rust | C equivalent |
|---|---|
| `x.to_le_bytes()` | `buf[0] = x; buf[1] = x >> 8; buf[2] = x >> 16; buf[3] = x >> 24;` |
| `fwrite(&x, 4, 1, f)` — **don't** | writes in the *host's* byte order: breaks across CPUs |

**3. Why every `usize` becomes `u64`.** `usize` is 8 bytes on your 64-bit PC and 4 on a 32-bit
Pi OS. Write it raw and the file layout depends on who wrote it. So:

- **writing:** `x as u64` — can never lose bits (`usize` is at most 64 bits anywhere).
- **reading:** `usize::try_from(n)` — *can* fail on a 32-bit machine if `n` is huge, and we turn
  that into `SnapshotError::TooLarge`. C's `(size_t)n` would silently chop the top bits off.

**4. The program travels inside the snapshot.** One file to copy to another device; no need to
also copy the right `.wasm` and hope it matches.

**5. Checksum first, parse second.** The last 8 bytes are an FNV-1a hash of everything before
them. `load` checks it **before reading anything else** — a file damaged in transfer is
rejected whole, never half-interpreted.

**The file layout** (all little-endian):

```
magic      "PKSN"                                        4 bytes
version    u32 = 1                                       4 bytes
wasm       len u64, then the bytes
values     count u64, then each value:  tag u8 + payload
             tag 0 = I32 (4 B) · 1 = I64 (8 B) · 2 = F32 (4 B) · 3 = F64 (8 B)
ctrls      count u64, then each:  opener_pc u64, end_pc u64, is_loop u8, height u64
frames     count u64, then each:  func_idx u32, pc u64, locals (count u64 + values),
                                  value_base u64, ctrl_base u64
memory     len u64, then the bytes
checksum   u64  FNV-1a of every byte above
```

---

## Step 1 — `decode.rs`: share the `Reader` (type exactly)

`load` needs the same bounds-checked byte cursor the `.wasm` decoder already has. Replace the
`struct Reader` definition **and its first two methods** (`u8`, `bytes`) with:

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

Leave `u32_leb`, `i32_leb` and `name` below it untouched.

- **`pub(crate)`** = usable anywhere inside this crate, invisible to anyone using the library.
  Somewhere between C's file-`static` and a header export.
- **`checked_add`** returns `None` instead of overflowing. Today a `.wasm` length is at most a
  `u32`, so `pos + n` can't overflow on a 64-bit PC — but a snapshot length is a `u64`, and a
  corrupt one near `u64::MAX` would make `pos + n` wrap around (debug build: panic). Now it's
  just `UnexpectedEof`.

**Check:** `cargo test` — still 20 + 3 green (nothing uses the change yet).

## Step 2 — `error.rs`: the error type (type exactly)

Append at the end of the file:

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

**Why the `From` impl:** `Reader` methods return `Result<_, DecodeError>`, but `load` returns
`Result<_, SnapshotError>`. When you write `r.bytes(4)?` inside `load`, `?` quietly calls
`SnapshotError::from(the_decode_error)`. Without this impl, every `?` on a `Reader` call
would be a type error. (Python analogy: catching one exception type and re-raising another —
done once, for every call site.)

## Step 3 — `lib.rs`: register the module (type exactly)

Add `pub mod snapshot;` to the module list (alphabetical, between `module` and `value`), and add
`SnapshotError` to the error re-export:

```rust
pub mod module;
pub mod snapshot;
pub mod value;

pub use error::{DecodeError, SnapshotError, Trap};
```

(It won't compile until Step 4 creates the file.)

## Step 4 — `snapshot.rs`: stubs + tests (type exactly)

Create `crates\parakaya-vm\src\snapshot.rs`:

```rust
//! `Interp` ⇄ bytes. Every field is fixed-width little-endian and every `usize` is written
//! as `u64`, so a snapshot written on one CPU loads on any other.
//! Layout: `docs/2026-10-12-parakaya-snapshot-design.md` §2.

use crate::decode::Reader;
use crate::error::SnapshotError;
use crate::exec::{CtrlFrame, Frame, Interp};
use crate::value::Value;

/// Serialize `it`, embedding the program `wasm` so the file is self-contained.
pub fn save(wasm: &[u8], it: &Interp) -> Vec<u8> {
    todo!()
}

/// Parse a snapshot back into `(wasm, Interp)`. The checksum is verified before anything
/// else is read, so corrupted bytes are never interpreted.
pub fn load(bytes: &[u8]) -> Result<(Vec<u8>, Interp), SnapshotError> {
    todo!()
}

/// FNV-1a, 64-bit: a few lines, no dependency, and identical on every platform
/// (unlike std's `DefaultHasher`, which may change between Rust releases).
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

What the tests pin:

| Test | Proves |
|---|---|
| `fnv1a_matches_reference_values` | your hash is *standard* FNV-1a, not a look-alike — so another implementation (another language, another device) computes the same checksum |
| `round_trip_is_lossless` | save → load gives back exactly what went in; `sample()` uses all 4 `Value` tags and every field |
| `flipped_byte_fails_checksum` | one bit of damage is caught |
| `cut_short_fails` | a truncated transfer is caught; under 8 bytes there's no checksum to check |
| `wrong_magic_…` | `tamper` re-seals the checksum, so we reach the parser's own checks |
| `huge_length_does_not_panic` | a crafted length near `u64::MAX` → an error, not a crash (Step 1's `checked_add`) |

`b"wasm"` is a **byte-string literal**: a `&[u8; 4]`, not a `&str`. The "program" here is
just 4 placeholder bytes — `save`/`load` never decode it.

## Step 4b — Run: expect red

```powershell
cargo test -p parakaya-vm snapshot
```

**Predict first:** will it compile? What will the 6 tests say?

<details><summary>Expected output</summary>

It **compiles**, with warnings (`unused variable: wasm`, `unused import: Reader`, …) — the
stubs don't use their inputs yet. All 6 tests FAIL with `not yet implemented`: `todo!()`
compiles fine and panics when it runs. This red is "wrong behaviour", unlike Task 1's
"doesn't compile" red. The warnings are a progress bar: each disappears as you fill in code.
</details>

## Step 5 — `fnv1a`

**The algorithm** (it's the whole spec):

```
hash = 0xcbf29ce484222325                 (the "offset basis")
for each byte b:
    hash = hash XOR b
    hash = hash × 0x100000001b3            (the "FNV prime"), wrapping on overflow
```

**Hints:**
- `u64::from(b)` widens a `u8` to `u64` (lossless, so `from`, not `as`).
- `^=` is XOR-assign, same as C.
- In Rust, `*` **panics on overflow** in debug builds. Hashing *wants* overflow (it's
  arithmetic mod 2⁶⁴, like C's unsigned math) — so use `wrapping_mul`, the same thing you did
  for `i32.add` in M1.
- `for &b in bytes` — the `&b` pattern copies each byte out of the slice.
- Underscores in number literals are just for reading: `0xcbf2_9ce4_8422_2325`.

<details><summary>Solution</summary>

```rust
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
```
</details>

**Check:** `cargo test -p parakaya-vm fnv1a` → `fnv1a_matches_reference_values` passes.

## Step 6 — The writers

Below `fnv1a`, add four small helpers. Each **appends** to `out: &mut Vec<u8>`. Signatures
(type exactly):

```rust
fn put_u32(out: &mut Vec<u8>, x: u32) {}

fn put_u64(out: &mut Vec<u8>, x: u64) {}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {}

fn put_values(out: &mut Vec<u8>, values: &[Value]) {}
```

**Hints:**
- `put_u32` / `put_u64`: one line each — `out.extend_from_slice(&…to_le_bytes())`.
  (`extend_from_slice` = Python's `list.extend`.)
- `put_bytes`: the **length first** (as a `u64` — `bytes.len() as u64`), then the bytes.
  The reader needs the length to know where the bytes end.
- `put_values`: the count first (same as `put_bytes`), then for each value a **tag byte** then
  its payload. `match v { Value::I32(x) => …, Value::I64(x) => …, … }`. `out.push(0)` appends
  the tag. Tags: I32 = 0, I64 = 1, F32 = 2, F64 = 3.
- Floats: `f32` has `to_le_bytes()` too. It writes the **raw bit pattern** — even NaN payloads
  survive exactly (writing the number as text would not).

<details><summary>Solution</summary>

```rust
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
```

`for v in values` over a `&[Value]` gives `v: &Value`; `match v` on a reference still works,
and `x` binds as a reference too — `x.to_le_bytes()` auto-dereferences.
</details>

## Step 7 — `save`

Replace `save`'s `todo!()`. Above it, add the two constants (type exactly):

```rust
const MAGIC: &[u8; 4] = b"PKSN";
const VERSION: u32 = 1;
```

**Hints** — walk the layout table top to bottom:

1. `let mut out = Vec::new();`
2. magic (`extend_from_slice(MAGIC)`), version (`put_u32`), wasm (`put_bytes`)
3. the value stack: `put_values(&mut out, &it.values)`
4. ctrls: count, then for each `c in &it.ctrls`: `opener_pc`, `end_pc` (both `as u64`),
   `is_loop` as **one byte** (`c.is_loop as u8` → 0 or 1), `height`
5. frames: count, then for each `f`: `func_idx` (already a `u32` — `put_u32`), `pc`, locals
   (`put_values`), `value_base`, `ctrl_base`
6. memory (`put_bytes`)
7. **last:** `let sum = fnv1a(&out);` then `put_u64(&mut out, sum);` and return `out`

Why compute `sum` on its own line instead of `put_u64(&mut out, fnv1a(&out))`? Try the one-liner
and read rustc's error: `E0502 cannot borrow 'out' as immutable because it is also borrowed as
mutable`. The `&mut out` borrow starts first, and `&out` would read it at the same time.
Splitting it into two lines ends the read before the write begins.

<details><summary>Solution</summary>

```rust
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
```
</details>

**Check:** `cargo build` compiles. (Tests still red — `load` is next.)

## Step 8 — The readers

Mirror images of the writers. One new concept first — **const generics** (type this one
exactly):

```rust
/// Read exactly `N` bytes as an array; `N` is inferred from the caller
/// (e.g. `i32::from_le_bytes` wants `[u8; 4]`).
fn arr<const N: usize>(r: &mut Reader) -> Result<[u8; N], SnapshotError> {
    Ok(r.bytes(N)?.try_into().unwrap()) // bytes(N) returned exactly N bytes
}
```

`N` is a number baked in at compile time, like a C array size — but **inferred**. In
`u32::from_le_bytes(arr(r)?)`, `from_le_bytes` needs `[u8; 4]`, so Rust picks `N = 4`.
In `u64::from_le_bytes(arr(r)?)`, `N = 8`. One function, every width.
`try_into().unwrap()` turns the `&[u8]` slice into a fixed `[u8; N]` array; the `unwrap` can't
fail because `bytes(N)` returned exactly `N` bytes. And the `?` on `r.bytes(N)?` works thanks
to Step 2's `From` impl.

Now write these five. Type the signatures exactly, then replace each `todo!()` with your body:

```rust
fn get_u32(r: &mut Reader) -> Result<u32, SnapshotError> {
    todo!()
}

fn get_u64(r: &mut Reader) -> Result<u64, SnapshotError> {
    todo!()
}

/// A `u64` from the file, as this machine's `usize`. Fails instead of truncating on
/// a 32-bit host.
fn get_usize(r: &mut Reader) -> Result<usize, SnapshotError> {
    todo!()
}

fn get_bytes<'a>(r: &mut Reader<'a>) -> Result<&'a [u8], SnapshotError> {
    todo!()
}

fn get_values(r: &mut Reader) -> Result<Vec<Value>, SnapshotError> {
    todo!()
}
```

(An empty `{}` body wouldn't compile here: it returns `()`, not a `Result`. `todo!()` stands in
for "any type" until you write the real body.)

**Hints:**
- `get_u32` / `get_u64`: one line each — `Ok(u32::from_le_bytes(arr(r)?))`.
- `get_usize`: `usize::try_from(get_u64(r)?)` gives a `Result` with the *wrong* error type;
  `.map_err(|_| SnapshotError::TooLarge)` swaps it. (Concept 3 above.)
- `get_bytes`: read the length with `get_usize`, then `Ok(r.bytes(n)?)`.
  The `'a` says: the slice returned borrows from the **original input**, not from the `Reader`
  — so it stays valid after the `Reader` is gone. Zero copying.
- `get_values`: read the count (`get_u64`), then loop that many times: read the tag byte
  (`r.u8()?`), `match` it — `0 => Value::I32(i32::from_le_bytes(arr(r)?))`, … — and any other
  tag → `return Err(SnapshotError::BadTag(tag))`.
- **Don't** use `Vec::with_capacity(count)`. The count comes from the file: a corrupt count of
  2⁶⁰ would try to allocate exabytes before reading a single value. Plain `Vec::new()` + `push`
  fails cleanly at end-of-file instead.

<details><summary>Solution</summary>

```rust
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

`tag => …` is a catch-all arm that also **names** the value it caught.
</details>

## Step 9 — `load`

Replace `load`'s `todo!()`. Two phases:

**Phase A — checksum, before anything else:**
1. The body is everything except the last 8 bytes. `bytes.len().checked_sub(8)` — `None` if
   the file is shorter than 8 bytes → `SnapshotError::UnexpectedEof` (use `.ok_or(…)?`).
2. `let (body, sum) = bytes.split_at(body_len);`
3. `fnv1a(body)` must equal `u64::from_le_bytes(sum.try_into().unwrap())` — else
   `return Err(SnapshotError::BadChecksum);`

**Phase B — parse `body` with a `Reader`, mirroring `save` line by line:**
1. `let mut r = Reader { bytes: body, pos: 0 };`
2. `r.bytes(4)?` must equal `MAGIC` (else `BadMagic`); `get_u32` must equal `VERSION`
   (else `BadVersion`)
3. `let wasm = get_bytes(&mut r)?.to_vec();` — `to_vec()` copies the borrowed slice into an
   owned `Vec` we can return.
4. The value stack, then start the `Interp`:
   `let mut it = Interp { values: get_values(&mut r)?, ..Interp::default() };`
   — `..Interp::default()` = "all other fields from the default" (Python:
   `{**defaults, "values": …}`).
5. ctrls: `for _ in 0..get_u64(&mut r)? { it.ctrls.push(CtrlFrame { … }) }` — `is_loop` comes
   back as `r.u8()? != 0`.
6. frames: same shape, `Frame { … }`.
7. `it.memory = get_bytes(&mut r)?.to_vec();`
8. Every byte of `body` must be used: `r.pos != body.len()` → `TrailingBytes`.
9. `Ok((wasm, it))`

**The subtle one:** inside `CtrlFrame { opener_pc: get_usize(&mut r)?, end_pc: …, … }`, every
field reads from the file. Rust evaluates struct-literal fields **top to bottom as written**,
so the field order in your literal *is* the read order. Write them in exactly the order `save`
wrote them — swap two lines and `round_trip_is_lossless` fails.

<details><summary>Solution</summary>

```rust
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
```
</details>

**Check:** `cargo test -p parakaya-vm` → **26 passed** (20 from Task 1 + 6 new), and the
3 end-to-end tests still pass.

**Try breaking it (then undo):** in `load`, swap the `opener_pc` and `end_pc` lines. Which test
fails, and what do `left`/`right` show? That's concept 1 — order is the whole format.

## Step 10 — Finish

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

All clean → **tell me "Task 2 done" and I'll review the diff.** Then commit:

```powershell
git add crates/parakaya-vm/src/decode.rs crates/parakaya-vm/src/error.rs crates/parakaya-vm/src/lib.rs crates/parakaya-vm/src/snapshot.rs
git commit -m "feat(vm): versioned little-endian snapshot format with checksum"
```

**Rustc habits:** fix the **first** error only, re-run, repeat. Trust the `help:` lines. Paste
anything you're stuck on for more than a few minutes.
