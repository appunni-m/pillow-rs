# ImageShow PNG save options — focused parity, 2026-10-20

## Defect

The controlled `show_file` case exercised the actual save and temporary-file
materialization path, but its capture viewer had no `Viewer.options`. It
therefore never reproduced Pillow's built-in PNG viewer configuration
`{"compress_level": 1, "save_all": True}`. In addition, the Python `Image.save`
wrapper discarded `**options`, so the old decoded-pixel assertion could pass
while the target silently ignored the requested compression level.

## Change

`Image.save` now forwards PNG `compress_level` through the PyO3 binding to the
core encoder's typed compression option. Values outside 0–9 are rejected.
`save_all` remains inert for this single-frame Image API. The controlled PNG
viewer now uses the built-in viewer options, and file parity includes the PNG
zlib FLEVEL hint in addition to decoded format, mode, size, and pixels.

## Focused evidence

Only these cases ran under `python-cpu`; no display process launched:

- `PIL.Image.Image.show.capture.la-file-PNG`
- `PIL.ImageShow.show.capture.la-file-PNG`

Run `migration-parity-fe84cd5944574dc885ffaa899602dfa5` passed 2/2, with no
diffs or infrastructure errors. Both source and target reported PNG, LA,
2×1, matching pixels, save options `compress_level=1` and `save_all=true`, and
`png_zlib_flevel=0`. The captured files were decoded after the timed operation.

The focused `scripts.test_migration_parity_display` module passed 17/17 tests,
including PNG viewer options, temporary-file capture and cleanup, and PNG
FLEVEL extraction. `CARGO_INCREMENTAL=0 make build-parity`
completed successfully. The build emitted the repository's existing seven
binding missing-doc warnings; no full test, parity, benchmark, or coverage
campaign ran.

Raw selected parity and producer/identity sidecars are in this directory. The
remaining `save_all` contract for multi-frame inputs is open; this target's
Image API currently represents one still image. No performance result or
operation completion is claimed.
