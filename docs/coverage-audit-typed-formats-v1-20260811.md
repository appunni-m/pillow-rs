# Typed image-format coverage audit

Date: 2026-08-11

Worktree: `coverage-typed-formats-v1-20260811`
Baseline commit: `bf3b284844171e81fff58f8e00b41428b908610f`

## Scope and safety boundary

This is a read-only coverage audit of public PNG, raw, `F`, `I`, and `I;16*`
paths implemented through `pillow-rs/src/image.rs` and the public format API.
The active corpus contains 3,101 cases. Pending 16-bit TIFF cases, crash
quarantine cases, GPU execution, fixture output, the shared input generator,
and the manifest are outside this audit.

The isolated worktree was registered with Coverage MCP as
`1e474b25-220a-48e4-8765-66366323c250`. The supplied snapshots are:

| lane | snapshot | project lines | `image.rs` lines | `format.rs` lines |
| --- | --- | ---: | ---: | ---: |
| CPU | `7b33de24-fda1-4a8f-999f-7ca9a82a54d3` | 39,320/68,650 (57.2760%) | 3,083/3,373 | 5/5 |
| SIMD | `b4faf868-62fc-4edf-8c32-5ba0db2c8f95` | 40,792/68,650 (59.4202%) | 3,084/3,373 | 5/5 |

`format.rs` has no remaining executable-line gap in either snapshot. The
relevant SIMD and CPU `image.rs` gaps are effectively the same; the one-line
SIMD difference is outside the public typed-format candidates below.

## Evidence gathered

The maintained input generator reproduced the active corpus counts:

```text
benchmark_suites=24
benchmark_workloads=208
coverage_plans=24
duplicate_parity_cases=619
nuanced_parity_cases=1919
parity_cases=3101
```

`make migration-parity-inputs-check` passed after restoring the repository's
ignored legacy v0 input JSONs into the isolated worktree. No tracked input,
manifest, or fixture file was changed.

A managed, safe CPU probe selected 96 active public PNG/raw/F/I;16 cases,
excluding every case whose ID contains `tiff` or `crash`:

- command: `typed-formats-safe-parity-cpu-v1-20260811`
- run: `b5677804-ffcd-4728-89bc-1e7c05b1005f`
- result: 96 selected, 96 executed, 96 passed, 0 failed, 0 infrastructure
  errors, 0 not run

The run was parity-only and did not ingest a new coverage snapshot. This is
intentional: it confirms that the candidate inputs are safe and declarative;
the coordinator should attach them to the next managed CPU/SIMD coverage run.

Direct public source/target probes used a 2x2 `I;16` PNG. Both implementations
returned the same result for out-of-range `getpixel` and `putpixel`:

```text
getpixel((2, 0)) -> IndexError("image index out of range")
putpixel((2, 0), 1) -> IndexError("image index out of range")
```

The same probe exposed a target/Pillow contract mismatch for `I;16` bands:

```text
Pillow getdata(band=0) -> ValueError("image has wrong mode")
target getdata(band=0) -> [4660, 22136, 4660, 22136]
Pillow getdata(band=1) -> ValueError("image has wrong mode")
target getdata(band=1) -> ValueError("band index out of range")
```

Therefore band cases are not ready to be added as parity coverage until the
target's public error contract is corrected by the coordinator.

## Reachable uncovered public cases

These are the only high-confidence uncovered paths found that are reachable
through the existing public binding and already have exact source/target
behavior.

| Rust location | missing behavior | proposed declarative case |
| --- | --- | --- |
| `image.rs:1904-1905` (`getpixel_formatted`) | `I;16` coordinate bounds error | `PIL.Image.Image.getpixel.nuanced.l16-png-out-of-bounds`; open the existing 2x2 `I;16` PNG, call `getpixel([2, 0])`, expect `IndexError: image index out of range` |
| `image.rs:2100-2101` (`putpixel_l16`) | `I;16` coordinate bounds error before materialization | `PIL.Image.Image.putpixel.nuanced.l16-png-out-of-bounds`; open the same PNG, call `putpixel([2, 0], 1)`, expect `IndexError: image index out of range` |

These should be added as normal public parity inputs by the coordinator, not
as direct Rust-core tests. They exercise the PyO3 delegation path and do not
require TIFF, GPU, crash, or fixture-output changes.

## Candidate blocked by a real parity mismatch

`image.rs:3795-3796` (`getdata_formatted`) contains the nonzero-band error
path for `I;16`. The natural declarative additions would be:

- `PIL.Image.Image.getdata.nuanced.l16-png-band-0`
- `PIL.Image.Image.getdata.nuanced.l16-png-band-1`

They must wait for a public semantic fix. Pillow rejects both band values for
`I;16` with `ValueError("image has wrong mode")`; the target currently accepts
band zero and emits a different message for band one. Adding expected outputs
before fixing that mismatch would weaken parity accounting.

## Already covered typed paths

No new cases are justified for these paths because the active corpus already
reaches them:

- `Image.frombytes` for valid native, little-endian, big-endian, and
  native-order 16-bit modes, including zero-size input.
- PNG lazy decode/materialization and `I;16` pixel reads.
- `I;16`, `I;16B`, `I;16L`, and `I;16N` byte serialization.
- 16-bit byte and numeric `putdata`, including offset/scale and short odd-byte
  inputs.
- `F` and `I` scalar storage, formatted reads, extrema, transforms, and raw
  serialization.
- 16-bit resize, crop, rotate, transpose, convert, histogram, colors, bands,
  and extrema paths.

## Uncovered but not valid public typed-format targets

The following gaps were inspected and deliberately excluded from the proposed
case list. They are defensive, internal routing, invalid-state, or wrapper-
precluded branches rather than reachable public PNG/raw/F/I;16 behavior:

| location(s) | reason for exclusion |
| --- | --- |
| `image.rs:476`, `514` | invalid scalar storage or non-`I;16` mode fallback |
| `image.rs:1570-1571`, `1594-1595`, `1625`, `1662` | internal backend/pipeline routing branches |
| `image.rs:1987-1988` | palette pipeline invariant after operation routing |
| `image.rs:2056-2057`, `2136-2137` | direct-core mode helpers; the public wrapper calls `putpixel_value` and does not route these fallback variants |
| `image.rs:2114-2121` | impossible offsets/materialization state after prior bounds and load checks |
| `image.rs:2459` | unknown-mode `getbands` fallback |
| `image.rs:2638` | encoded `tobytes` mode override; the Python wrapper always passes `self.mode` |
| `image.rs:2670-2739` | backend locking/recursive backend control, not typed format behavior |
| `image.rs:2920-2931`, `2981` | internal indexed-color table and validated palette invariant |
| `image.rs:3615`, `3637`, `3672`, `3681`, `3714` | decode/palette/materialization/verify invariants not exposed by valid public typed inputs |
| `image.rs:3999`, `4012-4015` | post-load and storage invariants for 16-bit byte writes |
| `image.rs:4046`, `4056-4070` | oversized index is rejected by the Python wrapper before `putdata_value_at`; remaining branches are storage/unsupported-mode defenses |
| `image.rs:4542` | invalid scalar storage |
| `image.rs:4973` | fallback error mapping; existing malformed PNG inputs cover the public malformed branch, and no reproducible safe typed-format input reaches another codec error |

No test should be invented solely to execute these lines. Doing so would
either test a private core variant, manufacture an impossible state, or turn a
defensive branch into a misleading public-coverage goal.

## Coordinator handoff

Add the two out-of-bounds `I;16` cases above to the declarative corpus, then
run the normal managed CPU and SIMD coverage lanes and compare against the
supplied snapshots. Fix `I;16` `getdata(band=...)` semantics before adding the
two band cases. Keep TIFF-pending, crash-quarantine, and GPU lanes excluded.

This audit changes only this document; it does not change runtime code,
bindings, the generator, the manifest, expected outputs, or fixture files.
