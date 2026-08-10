# CPU drawing coverage audit — 2026-08-11

## Scope and evidence

This is an audit-only change in the isolated worktree
`/Users/lazytrot/work/pillow-rs/.worktrees/coverage-drawing-audit-20260811`.

- Baseline/main HEAD: `bf3b284844171e81fff58f8e00b41428b908610f`
- CPU coverage snapshot: `7b33de24-fda1-4a8f-999f-7ca9a82a54d3`
- Corpus: 3,101 cases
- Files audited:
  - `pillow-rs/src/draw/mod.rs`
  - `pillow-rs/src/compute/pool_cpu/ops/draw.rs`
- Branch/line counts below use `covered/total` notation.
- No source, test, manifest, input-generator, expected-output, GPU, TIFF, or
  crash-lane files were changed.

The managed aggregate snapshot reports:

| File | Lines | Branches | Functions | Regions |
| --- | ---: | ---: | ---: | ---: |
| `draw/mod.rs` | 1661/1741 (95.4049%) | 451/486 (92.7984%) | 89/102 (87.2549%) | 2653/2762 (96.0536%) |
| `pool_cpu/ops/draw.rs` | 1084/1134 (95.5908%) | 191/246 (77.6423%) | 70/73 (95.8904%) | 1805/1882 (95.9086%) |
| Combined arithmetic | 2745/2875 (95.4783%) | 642/732 (87.7049%) | 159/175 (90.8571%) | 4458/4644 (95.9948%) |

The combined row is arithmetic over the two file records; it is not a new
coverage denominator. Coverage MCP managed queries were used for the snapshot
and line ranges; the interactive `coverage show` path was not used.

## Highest-value reachable gaps

### 1. Arc clipping: non-axis, even-half-plane branch

`pillow-rs/src/compute/pool_cpu/ops/draw.rs:722-798`, function
`arc_clip_state`, has the largest clear public gap.

The current corpus demonstrates the surrounding cases:

- `arc.nuanced.tall-ellipse-axis-transpose` reaches the `a < b` transpose
  recursion at lines 723-729.
- `arc.nuanced.full-sweep` reaches the `end == start + 360` no-root path at
  lines 733-734.
- `arc.nuanced.axis-boundary-sweep` reaches the axis-boundary path at lines
  749-754.
- `arc.nuanced.wrapped-negative-angles` reaches the odd half-plane path at
  lines 755-774.
- `arc.nuanced.empty-sweep`, `fill-width`, and the standard/parameter cases
  cover the ordinary and empty public paths.

No current arc input reaches the final `else` at lines 776-795. In the
aggregate snapshot, line 777 has 0/2 branch outcomes covered and line 787 has
0/4. The operation-scoped run confirmed the same result: lines 776-790 were
not entered while the surrounding transpose, full-sweep, axis, and odd
half-plane paths were entered.

Proposed follow-up batch, not added in this audit:

| Case | Candidate public input | Intended coverage |
| --- | --- | --- |
| `arc-even-short` | box `[0, 0, 12, 10]`, `start=15`, `end=75`, red fill, `width=2` | line 777 true branch and line 787 true branch |
| `arc-even-wrapped` | same box/fill, `start=15`, `end=0`, `width=2` | normalized 15→360 path; line 777 false and line 787 false |

These must be run through the source/target parity harness before being
promoted to the shared input set. They are valid public `ImageDraw.arc`
arguments; the audit does not assert their output bytes.

Managed evidence:

- Command: `PYTHON=/Users/lazytrot/work/pillow-rs/.venv/bin/python make migration-parity-operation-coverage MIGRATION_COVERAGE_OPERATION=PIL.ImageDraw.ImageDraw.arc`
- Managed run: `601ba483-8dfa-45bd-98d3-ce3a23893b99`
- Rust operation snapshot: `5ec57e98-af9b-47f0-bc0b-a584dd414423`
- Python operation snapshot: `4cad4ee5-e91b-4366-bf15-beb8f6c09092`
- Result: passed, exit 0, 61.445 seconds, no infrastructure errors.

### 2. Independent bounds outcomes in `plot`

`pillow-rs/src/draw/mod.rs:2212-2227`, function `plot`, is reached by point,
line, polygon, ellipse, and arc-family operations. The compound bounds guard
at line 2221 has only 5/16 branch outcomes covered in the aggregate snapshot;
it accounts for 11 missing branch outcomes. Existing negative/clipped line,
polygon, point, and fully off-canvas shape cases exercise clipping, but they do
not isolate every ordering of negative x/y and right/bottom overflow.

Proposed follow-up point batch on a 16×16 canvas, not added here:

- `xy=[[-1, 4]]`
- `xy=[[4, -1]]`
- `xy=[[16, 4]]`
- `xy=[[4, 16]]`
- in-bounds control `xy=[[4, 4]]`

Use one point per case so the four comparisons can be attributed to a public
operation. The existing line case `line.nuanced.out-of-bounds` and polygon
case `polygon.nuanced.out-of-bounds` are useful controls but are not evidence
that all four independent `plot` bounds outcomes are covered.

### 3. Bresenham direction and zero-length wide-line paths

The standard line implementation in
`pillow-rs/src/draw/mod.rs:2141-2199`, function `bresenham_line`, executes
every direction family but remains partial in each directional decision:

- line 2146 (`step_x`) is 2/4;
- line 2153 (`step_y`) is 2/4;
- lines 2160, 2165, and 2170 (vertical, horizontal, and shallow/steep split)
  are each 2/4;
- lines 2177 and 2191 (`error >= 0` in the two loop forms) are each 2/4.

The current manifest has `reversed-slope-directions`, `reverse-axes`,
`vertical`, `horizontal`, `steep-negative-direction`, `shallow-negative-y`,
`shallow-low-slope`, `steep-high-slope`, `partially-clipped-negative-line`,
`wide`, `width-three`, and `wide-joint-curve`. Those cases cover the useful
diagonal and positive-axis classes, but not a descending pure vertical or
descending pure horizontal segment.

`pillow-rs/src/compute/pool_cpu/ops/draw.rs:110-165`, function
`draw_line_on_canvas`, has an additional high-value public gap. The wide-line
zero-length guard at line 127 is only 2/8 covered; lines 128-129 have zero
hits. The current `flat-points` case is not a width-greater-than-one,
same-endpoint case.

Proposed follow-up line batch, not added here:

| Case | Candidate input | Intended coverage |
| --- | --- | --- |
| `line-descending-vertical` | `[4, 12, 4, 0]`, width 1 | negative `step_y` in the vertical path |
| `line-descending-horizontal` | `[12, 4, 0, 4]`, width 1 | negative `step_x` in the horizontal path |
| `line-wide-zero-length` | `[4, 4, 4, 4]`, width 3 | lines 127-129, including the wide-line point fallback |

Validate each candidate against Pillow before adding it to the corpus.

### 4. Polygon scanline edge/coalescing and off-canvas interval paths

The largest aggregate `draw/mod.rs` branch gaps are concentrated in the
scanline fill at lines 2374-2503. The aggregate missed-branch ranking includes
line 2376 (13 missed), line 2381 (8 missed), line 2469 (8 missed), line 2462
(7 missed), line 2467 (7 missed), line 2404 (5 missed), and line 2492 (5
missed).

The selected polygon operation run executed 28 cases and passed all 28 with no
infrastructure errors. It reaches the major ordinary and clipping behavior:

- `paired-points`, `concave-collinear-clipped`, `two-points-line`, and
  `two-points-outline` reach normal edge construction and outline behavior.
- `horizontal-runs`, `horizontal-only`, and the two
  `coalesced-horizontal-*` cases exercise horizontal-edge handling and both
  monotonic coalescing directions.
- `out-of-bounds` and `above-canvas` exercise clipping controls.

The polygon operation snapshot still shows the rare paths are missing:

- line 2376: 1/6 branch outcomes;
- line 2377: 3/12;
- line 2381: 3/12;
- line 2385 (fallthrough after a non-monotonic horizontal run): 0 hits;
- line 2492: 6/12, with continuation lines 2493-2494 at 0 hits in that
  operation run.

Proposed follow-up polygon batch, not added here:

| Case | Candidate points | Intended coverage |
| --- | --- | --- |
| `polygon-horizontal-backtrack` | `[(2,2),(10,2),(6,2),(10,8),(2,8)]` | non-monotonic consecutive horizontal run and line 2385 fallthrough |
| `polygon-off-left-interval` | `[(-12,2),(-4,2),(-4,8),(-12,8)]` | interval entirely left of the image in lines 2492-2494 |
| `polygon-off-right-interval` | `[(20,2),(28,2),(28,8),(20,8)]` on a 16×16 canvas | interval entirely right of the image in lines 2492-2494 |

The candidates are deliberately separate so each clipping outcome can be
verified. Their exact parity must be checked before promotion.

Several apparent polygon gaps are not public-input gaps:

- `draw/mod.rs:2337-2338` is guarded by `Draw::polygon` at lines 699-700 and
  by Python input validation for empty/single-point malformed inputs.
- `draw/mod.rs:2395-2396` cannot occur after the `n >= 2` loop has built at
  least one edge.
- `draw/mod.rs:2472-2474` has an `xx.push(x)` immediately before the
  `last_mut()` check; the `None` outcome is invariant-protected.
- `pool_cpu/ops/draw.rs:503-509`, `BinaryMask::width` and `height`, are
  internal helpers with zero hits; the polygon path passes image dimensions
  directly and does not call them. Adding a public case would not cover these
  methods.

### 5. Rectangle and horizontal-line clipping guards

`pillow-rs/src/compute/pool_cpu/ops/draw.rs:167-209`,
`draw_rect_on_canvas`, has partial guards at line 179 (2/4) and line 187
(5/8). Existing `rectangle.nuanced.negative-clipped` and
`rectangle.nuanced.fully-off-canvas` cover ordinary clipping, but no current
rectangle case uses a reversed box. Unlike ellipse, the public
`Draw::rectangle` method at `draw/mod.rs:610-631` pushes the supplied
coordinates without rejecting reversed order, so this is a reachable public
candidate.

`draw_hline` at `pool_cpu/ops/draw.rs:399-413` has line 402 at 5/8. Its
zero-height and out-of-range-y outcomes are mostly backend defensive paths
for ellipse segment generation; they are lower priority than the rectangle
reversal because the public ellipse wrapper rejects reversed boxes at
`draw/mod.rs:652-664`.

Proposed follow-up rectangle candidate, not added here:

- `xy=[[10,10],[2,2]]`, with a normal fill on a 16×16 canvas, to verify the
  reversed-box return at line 179 and Pillow's public result.

## Secondary findings and classification

- Ellipse backend guards at `pool_cpu/ops/draw.rs:424-431` and the analogous
  arc/chord/pieslice guards around lines 926-930, 956-960, and 996-1000 have
  zero-hit overflow/negative-dimension bodies. Public `Draw::ellipse` rejects
  reversed boxes before dispatch, so these are defensive unless a binding can
  supply an invalid internal operation.
- `draw/mod.rs:1521`, `1586`, `1644`, and `2091` are direct core convenience
  wrappers (`text`, optional-font helpers, and `into_image`) not used by the
  current Python wrapper call path. Their uncovered bodies should not be
  padded with synthetic tests; text compositor branches around lines 1682,
  1693, 1710, 1795, 1805, 1868, 1897, 2010, and 2068 belong in a separate
  text/font coverage batch because their reachability depends on font masks
  and mode-specific text behavior.
- `BinaryMask::width/height` at `pool_cpu/ops/draw.rs:503-509` are internal
  dead helpers for this path, not evidence of a missing public manifest case.

## Recommended next execution order

1. Add and parity-validate the two arc cases; they should cover the largest
   completely unentered public branch family with only two cases.
2. Run the three line cases and five point bound-isolation cases through the
   managed operation coverage target.
3. Validate the rectangle reversal and the three polygon candidates; retain
   only exact parity cases.
4. Re-run the aggregate CPU snapshot and compare only the two audited files.
5. Keep defensive/invariant-protected regions named in the report rather than
   changing thresholds or excluding them silently.

## Reproducible commands and managed metrics

Run from this isolated worktree with the repository's managed Make targets:

```text
PYTHON=/Users/lazytrot/work/pillow-rs/.venv/bin/python make migration-parity-operation-coverage MIGRATION_COVERAGE_OPERATION=PIL.ImageDraw.ImageDraw.arc
PYTHON=/Users/lazytrot/work/pillow-rs/.venv/bin/python make migration-parity-operation-coverage MIGRATION_COVERAGE_OPERATION=PIL.ImageDraw.ImageDraw.polygon
```

The arc run completed successfully as managed run
`601ba483-8dfa-45bd-98d3-ce3a23893b99` with Rust snapshot
`5ec57e98-af9b-47f0-bc0b-a584dd414423` and Python snapshot
`4cad4ee5-e91b-4366-bf15-beb8f6c09092`.

The polygon run completed successfully as managed run
`64663e13-ca81-480f-b715-42122f1621cf` with 28/28 selected cases passing and
zero infrastructure errors. Its Rust snapshot is
`635906f2-ddfc-48f2-940e-a70aee0fe1fb`; its Python snapshot is
`e8375c4c-2d6d-4935-a163-3a8021d606dc`.

`make help` also passed in the isolated worktree. No GPU, crash-inducing,
pending-TIFF, or fontdone lane was run.

## Audit result

No isolated source or test change was required. The fastest honest CPU drawing
coverage gain is a small parity-validated batch for arc clipping, wide
zero-length lines, independent point bounds, and the rare polygon clipping
shapes listed above. The remaining zero-hit helper and invalid-internal-state
regions are classified explicitly instead of being represented as fake public
coverage.
