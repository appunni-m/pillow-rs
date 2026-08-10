# Image and pixel API coverage audit

Baseline commit: `4f0fe721c`

This isolated bucket reviewed `pillow-rs/src/image.rs` and the thin Python
delegations for `getpixel`, `putpixel`, `getdata`, `putdata`, `load`, `mode`,
`size`, `info`, and `palette`. The current managed CPU snapshot remains the
authoritative aggregate: `23,496/26,647` active-scope lines (88.175029%),
`3,570/4,198` branches (85.040495%), `1,982/2,468` functions (80.307942%),
and `38,368/44,382` regions (86.449461%).

## Evidence

- The existing corpus already contains nuanced pixel/data coverage, including
  scalar and multiband `getpixel`/`putpixel`, `getdata` bands, clipped and
  packed `putdata`, opened-image metadata, palette metadata, and the recent
  `I;16` public pixel cases. These cases are listed in the maintained parity
  review and are not duplicated here.
- A fresh managed `PIL.Image.Image.getpixel` operation probe on this branch
  selected 20 tests and passed all 20 (`run_id`
  `26e13028-17ea-45f9-947c-e3fe88b368b8`). Its ingested LLVM snapshot
  `c7f4a4a2-0bb4-4fa4-9070-e991daf64a29` reports zero covered lines for the
  whole operation artifact, so it is invalid evidence for a targeted delta.
- Three sibling probes (`putpixel`, `getdata`, and `putdata`) were submitted
  concurrently against the same worktree and failed while copying the shared
  instrumented extension. The retained error is a missing
  `target/llvm-cov-target/maturin/lib_core.dylib`; this is an artifact-build
  race, not a parity failure. They must be rerun serially if fresh operation
  snapshots are needed.

## Classification

No input or runtime change is justified by this audit. The remaining image
coverage is distributed across shared constructors, format/materialization
fallbacks, internal normalization helpers, error exits, and direct core
variants that the Python wrapper cannot reach as public operations. Adding
cases without a valid nonzero targeted snapshot would either duplicate the
existing contract or claim coverage that the managed artifact did not record.

The next safe action is to add a serialized operation-coverage target (or run
these probes one at a time) and then inspect exact LLVM line ranges before
selecting any new public input. GPU, crash-inducing, 16-bit TIFF, and fontdone
lanes remain intentionally outside this bucket.
