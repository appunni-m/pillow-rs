# FreeType Parity Debugging

Use this guide when a generated fixture lane differs from the pinned FreeType C
oracle. The goal is to find the first divergent value, not to inspect thousands
of failing rows at once.

## Standard Loop

1. Pick one font, one glyph, one size, one endpoint, and one fixture family.
2. Reproduce only that lane through Make:

```bash
make test-parity PARITY_ARGS='test_metrics_only_matrix_exact_parity -- --nocapture'
```

3. Capture the failing row ID from `/tmp/freetype_failure_ids.txt`.
4. Build the C oracle helper:

```bash
make fixture-ref-bin
```

5. Dump C and Rust values at the same pipeline boundary.
6. Compare stage by stage until the first divergence is clear.
7. Fix the Rust root cause, remove temporary prints, and rerun the lane plus
   the broad gate.

## Useful Boundaries

- raw glyph points and contours
- scaled outline before hinting
- bytecode or autohint state
- phantom points and advances
- final hinted outline
- bbox/cbox
- rasterizer cells, spans, and bitmap bytes
- public bitmap placement and metrics

## Trace Rules

Temporary `println!`, `eprintln!`, and `fprintf` traces are acceptable only in
local debugging. Do not commit them. Permanent Rust traces must use guarded
`log::trace!` calls.

When a C nuance explains a fix, add a short comment at the Rust implementation
site with the C file/function reference and the observed behavior. Do not store
that nuance only in a status report.

## Live Oracle Helpers

The generated fixtures are the default correctness path. For interactive
diagnosis, these Make targets are available:

```bash
make test-direct-live
PIPE_FONT=DejaVuSerif-Bold PIPE_SIZE=10 PIPE_CHAR='$' \
  RUST_LOG=autohint::pipeline=trace make test-pipe-trace
```

The C source under `freetype/` is ignored and fetched by `make oracle-fetch`.
Local trace patches to that tree are diagnostic only and must not be committed.
