# ImageDraw.text glyph-slot reuse candidate, ARM64, 2026-10-17

This focused candidate avoids loading each ordinary outline glyph a second
time for grayscale text-mask rendering. It preserves the existing reload path
for non-outline slots and non-default load flags. The source change also
touches `FreeTypeFont.getmask` and `getmask2`; their parity guards are included
below, but this evidence makes no timing claim for those APIs.

## Frozen scope

- Public benchmark workloads: `pil-imagedraw-imagedraw.text.standard` and
  `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768`.
- Backend: serial CPU only, warm cache, `call` plus `observe-receiver`, 5
  warmups, 20 iterations × 5 samples, concurrency 1 (100 measurements per
  subject and workload).
- Host: macOS 15.7.7 ARM64, Pillow 12.2.0, CPython 3.12.13.
- Workload fixture SHA-256:
  `e3d8a7907575eb4225cfa57de75323c4d9768aaef5daa3ca0f59991ee89d3e20`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty).
- Changed Rust source SHA-256:
  `96aaff1881f8c9d42f9d88e57675c17945f51c63edb89dbe9772441d86a1958a`.
- Candidate extension SHA-256:
  `3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff`.

## Change and parity

For `FT_LOAD_TARGET_NORMAL`, layout now retains the owned glyph-slot values
while collecting advances and CBoxes. If every slot is an outline, the render
pass calls `FT_Render_Glyph` on those captured slots after layout has finished.
Mixed/non-outline runs use the original render-time reload loop. Other load
flags remain on the original path. This retains Pillow's layout-before-render
ordering and avoids a second glyph load on the selected ordinary outline path.

The release core check and `make build-parity` passed. Exact parity passed on
the candidate extension in two focused reports:

- `parity.json`: 6/6 cases, including both benchmark cases and the four
  existing draw guards for RGBA, negative placement, and embedded BGRA.
- `font-parity.json`: 11/11 cases covering `getmask`/`getmask2`, spaces,
  fractional and negative starts, monochrome mode, stroke width, SBIT, and
  embedded color fallback.

The Kata correctness audit confirmed that this pinned `fontdone` wrapper
returns owned slot snapshots and that its normal `FT_LOAD_RENDER` path uses the
same deferred outline renderer. Keep the eligibility check narrow. Kata:
`pillow-rs#8f4r`.

## End-to-end measurements

The pre-change baseline is run
`migration-benchmark-f15c53df69354886b369d016cf105757`, using extension
`3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`. Candidate
runs B1 and B2 use the same candidate extension and the exact same workload
fixture, sample policy, backend, and timing boundary. Every candidate target
receipt is terminal-complete, records actual CPU, one operation, zero
dispatches, and no fallback.

| Workload | Baseline Pillow / CPU | Baseline ratio | Candidate B1 Pillow / CPU | B1 ratio | Candidate B2 Pillow / CPU | B2 ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `pil-imagedraw-imagedraw.text.standard` | 0.869438 / 0.550271 ms | 1.580× | 0.851333 / 0.531646 ms | 1.601× | 0.850187 / 0.536000 ms | 1.586× |
| `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768` | 0.830874 / 0.515854 ms | 1.611× | 0.785645 / 0.486063 ms | 1.616× | 0.808478 / 0.458584 ms | 1.763× |

The measured public-call phase medians were 0.434250/0.403271 ms at baseline,
0.394521/0.370167 ms in B1, and 0.403709/0.349791 ms in B2 (standard/RGBA
context). Both candidate runs improved that phase for both workloads. The full
call-plus-receiver medians were lower than the baseline in all four candidate
rows, though the standard workload's speedup ratio stayed close to baseline.
These are dirty-tree measurements with one baseline run and two candidate
runs, not an interleaved A-B-B-A trial or published matrix credit.

Both workloads remain below the 2× CPU checkpoint. This candidate does not
complete `ImageDraw.text`; other modes, sizes, fonts, and applicable public
font operations still need their own evidence. SIMD and GPU are not declared
for direct text drawing and receive no accelerated credit.

## Artifacts

- `parity.json`, `font-parity.json`, and their attestations: selected exact
  parity proofs.
- `benchmark.json` and `benchmark-b2.json`: candidate runs B1 and B2.
- `benchmark-attestation.json`, `benchmark-b2-attestation.json`, producer
  attestations, and children: execution and build identity evidence.
- `raw-samples.json` and `raw-samples-b2.json`: ordered timing samples and
  phase telemetry.
- `SHA256SUMS.txt`: checksums for captured evidence, excluding this README and
  the checksum file itself.
