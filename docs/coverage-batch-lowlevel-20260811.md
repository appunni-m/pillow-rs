# Low-level coverage batch classification — 2026-08-11

This is a coverage classification record for the managed low-level worker
worktree. It does not add parity inputs and does not change runtime code.

## Provenance and scope

| Item | Value |
| --- | --- |
| Managed worktree id | f606b0a6-89dc-4e05-952d-3c27e5b5682d |
| Worktree | /Users/lazytrot/work/pillow-rs/.worktrees/coverage-batch-lowlevel-20260811 |
| Branch | codex/coverage-batch-lowlevel-20260811 |
| HEAD | bec899a5e38902642ca6347bd13e347ef28686bc |
| CPU snapshot | f14bdccb-debc-4c65-826d-36277c72d319, suite migration-parity-rust |
| SIMD snapshot | 135b0592-60b6-410f-811e-87d83261952b, suite migration-parity-rust-simd |
| Auto-selected operation snapshot | Unrelated and excluded, as directed |

The user-supplied active-scope baselines are CPU 23,429/26,611 lines and
SIMD 24,942/30,558 lines. The explicit snapshots above are the current
coverage sources. Their full LLVM project summaries are CPU 39,252/68,614
lines (57.2069839974%) and SIMD 40,759/68,614 (59.4033287667%); those full
project totals are not substitutes for the active-scope baselines.

Coverage MCP schema revision is 7. The snapshots carry the MCP warning that
LLVM JSON segments are normalized to segment start lines while aggregate
region coverage is preserved. All file metrics and ranges below came from
explicit snapshot reads followed by bounded source_context reads at the same
HEAD.

Gap notation below is exact source coverage notation:

- A plain range is an uncovered executable line range.
- Pn means a partial-branch line with n missed branches; it can also be an
  uncovered line.
- A range is a classification of the current snapshot, not a claim that the
  code is defective.

## Summary decision

No runtime implementation change is justified by this audit. The gaps split
into four classes:

1. generic Rust APIs and conversion kernels that are not attributed by the
   current PIL parity scope;
2. valid public behavior that would require an existing public-contract case
   or a separately approved fixture, not a synthetic malformed input;
3. internal invariant, mismatch, overflow, and panic guards that must not be
   reached by fake parity inputs; and
4. GPU-only or backend-excluded code, for which this worker was explicitly not
   authorized to execute a lane.

Direct Rust tests are safe only for the non-public-contract invariants called
out below. Such tests must stay Rust unit tests and must not be counted in the
PIL parity denominator.

## 1. ops/utils.rs

File-local metrics are 0/48 lines, 0/14 branches, 0/2 functions, and 0/65
regions in both explicit snapshots. The exact gap ranges are:

~~~text
26-28, 30[P2], 31, 34, 36, 37[P2], 38-39, 41[P2], 42-43, 45,
46[P2], 47, 50, 52-53, 55-59, 62-63,
72, 73[P2], 74, 77-79, 80[P4], 81, 85-86, 88-89
~~~

The first group is align_row_to_32. Its zero-byte, zero-row, padding, and
copy paths are externally reachable through the Rust export and the Python
binding wrapper, but they are not exercised by the current migration parity
scope. Existing direct Rust tests cover normal alignment, padding, 1-bit
alignment, empty input, and the flatten helper; those tests are not reflected
in either parity snapshot.

The second group is flatten_pixel_list. The function is exported from the
Rust crate but has no binding or call site in the audited public migration
path. There is no safe reason to create a parity input for it.

Safe follow-up: retain or extend direct Rust unit tests for utility invariants
only if maintainers want Rust-API coverage. Do not add a parity fixture solely
to drive the zero-row, malformed-shape, or utility branches. No defect was
found.

## 2. raster/traits/primitive.rs

File-local metrics are 0/132 lines, 0/48 branches, 0/14 functions, and 0/225
regions in both explicit snapshots. Exact gaps:

~~~text
34-36, 39-42, 69-71, 99-101, 130-132, 144-146,
198, 199[P4], 200-201, 202[P2], 203-204, 206-207,
208[P2], 209-213, 214[P2], 215-217, 218[P2], 219, 221, 223,
225, 226[P4], 227-228, 229[P2], 230-231, 233-234, 235[P2],
236-240, 241[P2], 242-243, 244[P2], 245-246, 248, 249[P2],
250, 252, 254,
256-260, 262-263[P4], 264, 266-267, 269-270, 271[P2],
272-273, 275-277, 278[P2], 279-280, 281[P2], 282-283,
285, 286[P2], 287, 288[P2], 289-290, 291[P2], 292-298,
300-304,
306-310, 311[P6], 312, 314, 316,
322-324, 340-342, 346-348, 352-354
~~~

The zero line rate is attribution, not reachability proof. Primitive is
generic and heavily inlined through raster color and pixel conversions. The
safe non-public-contract test options are direct Rust tests for:

- integer, f32, and f64 Primitive conversions at endpoints;
- saturating_trunc_f32_to_u128 and saturating_trunc_f64_to_u128 for NaN,
  infinity, negative values, subnormal values, exponent overflow, and
  truncation;
- f64_to_f32 for NaN/infinity, subnormal, underflow, overflow, and rounding
  ties; and
- round_shift_right tie behavior and the concrete EncodableLayout
  implementations.

These tests would exercise implementation invariants and must not inflate the
PIL parity denominator. No public Pillow endpoint in the active scope provides
an evidence-backed input for these generic branches.

Review note, not a runtime change: the Primitive trait documentation describes
from_f32 as clamped, while the f64 implementation is a direct cast. The
active Pillow surface has no f64 image mode or exact parity observation for
this contract, so this audit records it without changing behavior.

## 3. raster/buffer.rs

Both snapshots report 184/202 lines, 8/14 branches, 38/39 functions, and
272/290 regions. Exact gaps:

~~~text
162[P4], 163, 168[P4], 169, 421[P10], 624[P15], 632,
709, 711-712, 749[P30], 750, 822, 824-825,
872[P15], 873, 1007-1011
~~~

Reachability classification:

- 162-163 is the zero-width row path in Rows::with_image. It is reachable
  from a valid zero-width Rust ImageBuffer, but it is not a PIL parity
  contract. A direct Rust unit test using a zero-width image is safe.
- 168-169 is the short-row-buffer invariant panic. Safe ImageBuffer
  constructors establish the required length, so a malformed private row
  slice must not be manufactured as a public parity case.
- 421 is the normal row transition in EnumeratePixelsMut::next. A direct
  Rust test over a small valid image can cover it; use an existing public
  operation only if a real parity case already needs mutable enumeration.
- 624 and 632 are from_raw validation for insufficient storage. A direct
  Rust test can assert rejection of a short buffer without changing the PIL
  denominator.
- 709, 711-712, 822, and 824-825 are out-of-bounds get_pixel and
  get_pixel_mut panic paths. The checked accessors are the safe alternatives;
  do not add invalid-coordinate parity inputs merely for coverage.
- 749-750 is the internal pixel_indices bounds guard. It is an invariant
  guard for valid image dimensions, not evidence of a public defect.
- 872-873 is the allocation-size overflow guard in ImageBuffer::new. A
  direct Rust catch_unwind test with dimensions that overflow the checked
  length is safe because the guard runs before allocation; it is not a PIL
  fixture.
- 1007-1011 is Clone::clone_from. A direct Rust trait test can check that
  dimensions and pixels are replaced correctly; this is Rust API coverage,
  not a parity input.

No runtime defect was found. The only safe coverage additions here are
Rust-only tests for the listed Rust API and invariant behavior.

## 4. checked dimensions

File-local metrics are 37/63 lines, 4/8 branches, 6/8 functions, and 45/56
regions in both snapshots. Exact gaps:

~~~text
106[P2], 107, 113[P1], 114, 122, 126, 130[P1], 131, 139, 143
~~~

Existing direct Rust tests already cover valid dimensions, zero width/height,
zero channels, pixel-count overflow, the configured maximum, and successful
allocation. They are intentionally outside the parity denominator.

The remaining 139-143 total-byte overflow can be covered safely by a
Rust-only test of private new_with_limit with dimensions/channels whose pixel
count fits but whose byte multiplication overflows; the constructor returns a
DimensionError before allocation. This does not justify a malformed public
Pillow input.

The comments and docs refer to a set_max_pixels relaxation API, but no such
implementation was found in this checkout. That is a separate API/documentation
follow-up, not a coverage-driven runtime change, and no exact parity evidence
was found for it.

## 5. registry dispatch

The registry file has 1,364 relevant lines. CPU coverage is 1,000/1,364 lines,
75/176 branches, 91/108 functions, and 1,535/2,219 regions. SIMD coverage is
701/1,364 lines, 19/176 branches, 30/108 functions, and 988/2,219 regions.
The low SIMD result is expected for CPU-oriented registry closures; the
SIMD-specific wiring is covered below.

### Dispatch and support queries

The exact dispatch gaps in both snapshots are:

~~~text
variant_key:
335, 378-379, 394, 402-404

gpu_supports:
426-428, 439-444
~~~

The variant_key lines are the absent Quantize, Blend/Composite, PointOp, and
LinearGradient/RadialGradient/EffectMandelbrot arms in the active scope.
They are public operation variants, but no new input should be fabricated in
this low-level batch. The gpu_supports ranges are backend capability and
rotate exclusion logic; GPU execution is out of scope.

In the CPU snapshot only, simd_supports is absent at:

~~~text
447-449, 460-465
~~~

The SIMD snapshot covers this exact simd_supports range, including the rotate
guard. This is backend selection, not a CPU defect.

### GPU parameter extraction

extract_params is GPU-only and spans 612-982. No GPU lane was run. The
explicit snapshot query returned these exact first-page ranges in that
window (CPU and SIMD agree):

~~~text
612-613, 637, 640, 643, 646, 649-651, 654, 658-663, 665, 669,
673-684, 689, 692-693, 697-698, 702-703, 707, 710-712, 715,
719-721, 723[P2], 724-726, 728-729, 734-736, 738[P2], 739-741,
743-744, 748, 751-759, 761, 765, 768-780, 782, 786-789,
793-794, 797-800, 805-808, 810, 814, 817, 820, 823, 826,
830-835, 838-844, 850-855, 858-867, 869, 871-877, 879, 883,
886, 890-894, 899, 902, 906-909, 912-914, 916, 918-920,
925-927, 930-934, 936, 938, 942, 945, 948, 951, 954,
957-958, 964-968, 970-975, 980, 982
~~~

The whole-file MCP result reports 349 uncovered lines and 86 partial-branch
lines for CPU, and 646 uncovered lines and 86 partial-branch lines for SIMD.
The compact whole-file response is capped at 100 returned gap ranges
(CPU next_start_line 1185; SIMD next_start_line 1134), so this record does
not pretend that the first-page list is a complete enumeration of the later
registration closure gaps. All of the audited extract_params gaps are
classified GPU-only and are intentionally left for a GPU-scoped worker.

### register_all and SIMD wiring

The first audited registration window has the following exact gaps:

CPU, 1025-1177:

~~~text
1033[P1], 1036, 1049[P1], 1058, 1071[P1], 1084,
1097[P1], 1100, 1113[P1], 1116, 1129[P1], 1132,
1147[P1], 1155, 1167, 1168[P2], 1169-1170, 1172, 1174
~~~

SIMD, 1025-1177:

~~~text
1033[P1], 1036, 1048, 1049[P2], 1050-1054, 1056, 1058, 1060,
1071[P1], 1084, 1097[P1], 1100, 1112, 1113[P2], 1114, 1116,
1118, 1128, 1129[P2], 1130, 1132
~~~

These are closure/key mismatch and error paths reached when a registration
record does not match the PipelineOp key selected by variant_key. Normal
registry dispatch supplies matching keys, so a wrong-key operation is not a
valid parity input. The selected SIMD wiring range 2626-2734 has no uncovered
or partial executable lines in either snapshot; simd_set and its registered
records are therefore not a missing-test bucket.

## 6. SIMD defensive branches

### pool_simd/mod.rs

CPU coverage is 6/70 lines, 0/2 branches, 2/13 functions, and 6/109 regions;
this module is excluded by the CPU backend. SIMD coverage is 62/70 lines,
2/2 branches, 9/13 functions, and 97/109 regions. CPU-only gaps:

~~~text
28, 32-41, 43-54, 56, 58, 69-71, 73, 79-80, 82-84,
88-94, 96-97, 103[P2], 104, 106, 108
~~~

SIMD-only gaps are:

~~~text
40-41, 53-54
~~~

The SIMD gaps are the P and PA mode shape-mismatch errors in
normalize_palette_result. A valid DynamicImage result has a pixel count that
matches its dimensions, so no safe public input should reach these errors.
They must not be driven by a fake parity input. Direct malformed-result tests
would be internal invariant tests only and are not needed for this batch.

### pool_simd/ops/adapters.rs

The CPU snapshot reports 0/1,182 lines, 0/156 branches, and 0/82 functions
because this is the SIMD adapter module. SIMD reports 1,125/1,182 lines,
110/156 branches, and 77/82 functions. The exact SIMD gaps are:

~~~text
45, 79, 174-175,
290[P1], 292, 304[P1], 306, 318[P1], 320, 332[P1], 334,
346[P1], 348, 360[P1], 362, 374[P1], 392, 407[P1], 411,
425[P1], 427, 479[P1], 481, 497[P1], 503, 515[P1], 521,
533[P1], 539, 551[P1], 557, 569[P1], 580, 592[P1], 603,
615[P1], 617, 626[P1], 632-633, 653, 685[P1], 699,
711[P1], 725, 737[P1], 740, 752[P1], 785-786, 812,
832[P1], 856, 866[P1], 888, 899[P1], 920, 931[P1], 950,
961[P1], 994, 1005[P1], 1018, 1030[P1], 1067, 1079[P1],
1084, 1096[P1], 1100, 1112[P1], 1123, 1133[P1], 1159,
1171[P1], 1176, 1188[P1], 1226, 1238[P1], 1246, 1256[P1],
1316, 1322, 1338[P1], 1341, 1356[P1], 1359-1360,
1370[P1], 1373-1374, 1385[P1], 1393, 1416[P1], 1450,
1465[P1], 1482, 1497[P1], 1517, 1538, 1540-1541[P2], 1543,
1550-1551
~~~

Classification:

- 45 and 79 are fallback arms in mode-name conversion helpers; 174-175 is
  RGB raw-buffer shape validation. These are not missing PIL contracts.
- 290-427, 479-740, and 752-786 are operation/key matching adapters. The
  partial if-let branches and wrong-operation error arms are defensive
  dispatch checks; normal SimdPool dispatch provides the matching variant.
- 497-603 include valid filter mode alternatives. A real public filter case
  for F or another supported mode may cover them, but no synthetic mode or
  malformed operation should be added just for coverage.
- 832-1176 contains geometry adapters. Native scalar-mode and unsupported-filter
  fallbacks are valid public behavior only when an existing public operation
  supplies those modes or filters; the wrong-op errors remain internal.
- 1188-1322 contains convert, palette, and transform fallbacks. HSV, YCbCr,
  I, F, P, Mode1, RGBa, non-affine, and optional-fill behavior can be tested
  only through real public cases already represented by the manifest.
- 1338-1551 contains mutating adapters. The merge mode/band validation at
  1517, 1538, 1540-1543, and 1550-1551 must not be reached with invented
  invalid images. The line 1316 fill default is a valid optional transform
  input only if the public contract already includes a no-fill case.

No adapter runtime change is justified. Any future public-mode additions
must be exact parity cases and must be reviewed for denominator impact.

### pool_simd/ops/scalar.rs

The explicit SIMD snapshot reports 2,670/2,695 lines, 766/842 branches,
109/109 functions, and 5,112/5,169 regions. Exact gaps:

~~~text
18[P1], 29[P1], 64[P1], 75[P1], 89[P1], 102[P1],
534[P1], 549-551[P3], 552, 567[P1], 582-584[P3], 585,
600[P1], 615-617[P3], 618,
676[P1], 677, 741[P2], 743,
1872, 2041[P2], 2179[P3], 2231[P4], 2232,
2276[P3], 2279[P4], 2280, 2282[P1], 2285,
2338[P1], 2373[P3], 2426[P1], 2427, 2431[P1], 2432,
3267[P1], 3278-3280[P3], 3301[P2], 3306[P1], 3333[P1],
3350[P1], 3460[P1], 3483[P1],
3615[P1], 3616, 3619[P1], 3620,
3968[P2], 3969, 3989[P1], 3990, 3994[P1], 3995,
4025[P1], 4028, 4131, 4152[P1], 4177, 4199[P1],
4254[P1], 4325[P2], 4326, 4370[P1],
4488[P4], 4489, 4553[P4], 4554, 4556, 4558
~~~

Classification:

- 18-102, 534-677, 3267-3483, 3989-4199, and 4488-4558 are primarily
  mode/alpha alternatives. They are valid only for real L, LA, RGB, RGBA,
  and other modes already accepted by the public operation. Existing public
  mode cases may be extended later; no artificial mode values are allowed.
- 741 and 743 are bounds checks inside offset. Valid image shape invariants
  make the failed bounds path internal; do not shorten a pixel slice to make
  it run.
- 1872 is the unknown transpose-method no-op. The public enum maps valid
  methods and does not provide an invalid discriminant.
- 2041, 2179, 2231, 2276, and related zero-dimension geometry branches are
  defensive kernel guards. A real public zero-dimension contract case may be
  considered separately; a coverage-only malformed input is not safe.
- 2282 and 2285 are valid bleed-boundary behavior and may be covered only by
  a real public bleed case.
- 2426-2432 are pad-loop bounds guards. Treat them as invariants unless a
  valid public geometry case demonstrates a contract path.
- 3615-3616 is the public put_pixel out-of-bounds no-op candidate; it may be
  tested only through an actual public parity case. 3619-3620 is the
  internal pixel-length guard and must not be faked.
- 3968-4028, 4152-4199, 4254, 4325-4370 are valid box, alpha, histogram,
  empty-destination, and factor-boundary alternatives where the public API
  permits them. They are not automatically unreachable, but this batch has
  no evidence authorizing new parity inputs.
- 4131 and 4553-4558 are fallback arms for invalid or otherwise impossible
  mode combinations after public validation. Do not create invalid mode
  values.

Direct Rust tests for scalar kernels are permissible only when they assert
non-public-contract math or invariant behavior and are kept out of the parity
denominator. No such test was added here.

## Safe testing options and non-actions

| Area | Safe option | Do not do |
| --- | --- | --- |
| Utility and primitive math | Direct Rust unit tests for documented Rust invariants and numeric boundaries | Add PIL inputs solely to call generic helpers |
| Buffer and checked dimensions | Direct Rust tests for valid zero-size images, rejected short buffers, checked accessors, and overflow-before-allocation | Manufacture malformed internal slices or use invalid coordinates as parity fixtures |
| Registry | Cover an operation only when its public manifest contract is being expanded with exact parity evidence | Pass a wrong PipelineOp to a registry key, or add GPU cases |
| SIMD adapters/scalar | Use existing public mode/filter/geometry cases where they are contract-valid | Use invalid modes, wrong adapter variants, short pixel buffers, or synthetic shape mismatches |
| GPU, crash, pending TIFF, fontdone | No execution in this batch | Run any excluded lane |

No test command was run after the existing snapshots. This was intentional:
the task requested classification from current CPU/SIMD snapshots and forbade
fake parity inputs. No direct Rust unit test was needed to establish the
reachability decisions.

## Commands and evidence

Shell commands run in the managed worktree included:

~~~text
pwd
git branch --show-current
git rev-parse HEAD
git status --short --branch
rg --files docs | rg 'coverage|parity' | head -40
make help
~~~

Coverage MCP reads used the schema-7 operations project_context,
coverage_query, and source_context. coverage_query used the explicit CPU and
SIMD snapshot IDs above with view=file and bounded line selections for the
requested source ranges; source_context was bounded to coverage-identified
windows in each audited file. No run_test call was made.

The worktree was clean at the start at the stated HEAD. This document is the
only intended change for this batch.
