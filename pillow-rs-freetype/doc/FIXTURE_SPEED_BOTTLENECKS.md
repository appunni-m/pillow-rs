# FreeType Fixture Speed Bottlenecks

This note tracks the expensive paths that make FreeType fixture tests feel slow.
Keep fixture tests fast by separating normal parity checks from reference
generation and live-oracle comparison.

## Measured Baseline

Command:

```sh
cargo test -p pillow-rs-freetype --test coverage_matrix_tests -- --nocapture
```

Before caching fonts in the test harness, the run was interrupted after:

```text
real 134.91
user 120.10
sys 8.95
```

The test had compiled and was still inside `test_coverage_matrix_pil`. After
caching `Font` by `(font, size_pt)` in `tests/coverage_matrix_tests.rs`, the
same test reached assertion reporting in:

```text
test runtime: 1.43s
real 2.22
```

The current matrix still has pixel parity failures; this document is only about
the speed bottlenecks.

## Bottleneck 1: Rebuilding `Font` Per Fixture Row

`coverage_matrix.json` currently contains:

```text
rows: 7,640
fonts: 8
font-size pairs: 40
operations: 3,760 getmask, 3,760 getbbox, 120 metadata rows
```

The old runner cached raw font bytes, then called `Font::truetype` for every
row. That repeated table parsing, `loca`/`glyf` copies, bytecode table setup,
and `FaceGlobals::new` thousands of times for only 40 unique font-size pairs.

Status: fixed in `tests/coverage_matrix_tests.rs` by caching constructed
`Font` values by `(font, size_pt.to_bits())`.

Rule: fixture tests should cache at the highest reusable level. For PIL parity,
that means a loaded `Font`, not font bytes.

## Bottleneck 2: `FaceGlobals::new` Is Eager

`Font::truetype` always constructs `FaceGlobals`, even when the selected backend
is `BitmapBackend::PIL`. The PIL rendering path skips autohint metrics in
`getmask` and `getbbox`, so eager face-global coverage work is unnecessary for
PIL-only fixture rows.

The harness-level cache makes this cost acceptable for normal fixture tests, but
library construction is still heavier than it needs to be.

Recommended follow-up: make autohint globals lazy or backend-scoped so PIL fonts
do not run the full script coverage scan unless a FreeType/autohint operation
requests it.

## Bottleneck 3: Live FreeType Oracle Spawns One Process Per Glyph

`tests/direct_ft_compare.rs` compares against `/tmp/gen_refs_v4` by spawning the
C reference binary per `(font, codepoint, size)` tuple.

`font_inventory.json` currently describes:

```text
fonts: 100
script assignments: 305
codepoint assignments: 5,543
sizes in direct test: 2
maximum live oracle calls: 11,086
```

The Rust rendering work is not the only cost here. Process startup, dynamic
linker setup, stdout capture, hex decoding, and SHA-256 all happen for every
glyph. The in-test cache does not help much because the direct matrix has few
duplicate tuples.

Rule: do not put this path on the snappy fixture-test path.

Recommended follow-ups:

- Keep `direct_ft_compare` as a deliberate deep parity check, not a default
  quick fixture test.
- Prefer precomputed static references for normal CI.
- If live comparison is required, replace per-glyph process spawning with a
  batch mode or a long-lived helper process.

## Bottleneck 4: Fixture Generation Is Offline Work

`scripts/build_fixtures.py` also calls `/tmp/gen_refs_v4` repeatedly:

- `--inventory` probes every font/script/codepoint through FreeType.
- normal fixture generation calls FreeType once per row reference.

That is appropriate for regenerating fixtures, but it should not be part of
normal test execution. Generated JSON matrices are the fast test input.

## Practical Test Split

Use these tiers:

- Snappy: static JSON matrix, cached `Font` values, no subprocess oracle.
- Focused debug: a single glyph through `examples/debug_glyph.rs` or
  `examples/trace_glyph.rs`.
- Deep parity: live FreeType oracle over `font_inventory.json`.
- Offline generation: `scripts/build_fixtures.py` and Pillow reference rebuilds.

The normal fixture suite should stay in the first tier.
