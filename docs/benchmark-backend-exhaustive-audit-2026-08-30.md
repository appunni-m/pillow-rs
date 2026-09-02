# Exhaustive benchmark, backend-coverage, correctness-gate, and performance-gap audit

Date: 2026-08-30

Repository: `/Users/lazytrot/work/pillow-rs`

Audit revision: `e97fab9fdc68bf4555669d227b5a66fca9d9a369` (`dirty: true`)

Primary benchmark run: `migration-benchmark-a613c94cd8d240e4b7412d576e760331`

This document began as an audit and diagnosis snapshot.  Sections 1–30 retain
that baseline evidence; section 31 records the subsequent working-tree
remediation.  No generated fixture, benchmark result, expected output, or
threshold was hand-edited, and no commit was created.  The pre-existing dirty
worktree was preserved.

## 1. Executive summary

The original standard benchmark was not a valid proof of equal backend
coverage or of the requested performance contract.  After the parity-first
remediation recorded in section 31, the current strict parity lanes are fully
passing; the requested performance contract remains open.

- **Current parity closure:** full live-oracle parity is 10,952/10,952, strict
  SIMD is 10,952/10,952, and strict GPU is 10,952/10,952, all with zero
  failures, not-run cases, or infrastructure errors.

- The raw denominator is 746 workloads. Forty-eight never produce a usable Pillow timing. They create 192 failed subject records (48 workloads × four subjects) and must stay out of numeric comparisons until repaired or explicitly assigned to a non-default lane. Of these, 45 are repairable invalid success-path inputs, two require optional Qt bindings, and one intentionally reaches iterator exhaustion and needs a separate success-path timing workflow. All remain visible in parity/API coverage.
- After removing those 48 from the performance view, Pillow completes 698, CPU 697, SIMD 696, and GPU 628 workloads. The remaining 73 failed subject records are one CPU, two SIMD, and 70 GPU records. Formal infrastructure errors and parity-preflight failures are both zero; `not_proven` is not synonymous with an infrastructure error or a parity mismatch.
- CPU and SIMD differ on exactly three workload IDs. CPU fails the loaded RGBA ten-action workflow because it carries stale `RGBA` logical state after `Convert(RGB)`. SIMD rejects 1×1 RGB Rotate and Add in preflight despite existing tail-capable kernels. The completed-set intersection is 695, union 698, and symmetric difference 3.
- The 70 GPU failures reduce to 12 root families: Draw 27, Rotate 11, EffectSpread 6, Rank/Median 5, nonterminal mode routing 5, non-affine Transform 4, Thumbnail 3, Color3DLut 3, Fit 2, Merge 2, EffectNoise 1, and F-mode non-nearest Resize 1. Primary taxonomy totals are 41 `GPU_CAPABILITY_MISSING`, 22 `GPU_SHADER_OR_REGISTRY_DEFECT`, and 7 `PIPELINE_ROUTING_DEFECT`.
- Sixty-seven GPU failures have no completed GPU receipt. Three failed workflows retain a completed one-operation/one-dispatch GPU receipt: the two Thumbnail workloads and `pipeline-chain.matrix-021`. These are partial earlier dispatches, not successful terminal workflows. The runner marks a drained receipt `completed` even when the public step throws, so receipt status currently lacks terminal-completeness semantics.
- Suite comparisons are invalid for 10 of 54 suites because each subject independently drops failed members. One suite has an empty four-subject intersection. Current suite ratios can therefore compare different workloads and must not be used as backend speed evidence.
- The requested performance gates all fail on defensible equal-receipt cohorts. CPU beats Pillow on only 212/482 actual-CPU pairs. On the 175-workload material SIMD cohort, only 29 meet a 1.25× SIMD advantage over both CPU and Pillow. On the requested large-GPU candidate cohort (108 identical actual SIMD/GPU receipt pairs), only four meet either GPU >1.00× or GPU ≥1.20×; the paired geometric mean is 0.135×, meaning GPU is about 7.4× slower in the typical multiplicative sense.
- Apple Silicon is executing real NEON through `wide::u8x16`, but the implementation is portable 128-bit SIMD, not an M3-specific runtime-dispatched backend. Several expensive paths vectorize only storage while retaining scalar gathers/control, use four NEON registers for logical `f64x8`, allocate full-frame intermediates, or have no calibrated crossover policy.
- The highest-leverage order is: validate/gate benchmark inputs; persist errors and terminal receipt semantics; make suite comparisons use equal ID intersections; repair the three CPU/SIMD completion mismatches; implement exact Draw/Rotate/mode-transition GPU foundations; then establish paired performance gates before optimizing CPU, SIMD, and GPU.

## 2. Scope and authoritative artifacts

Primary evidence:

| Artifact | Role | Identity / integrity |
|---|---|---|
| `build/migration-parity/benchmark-result.json` | Latest standard timings, statuses, contexts, receipts, phases, resources, suites | SHA-256 `27aa9168590bb4c13def4e43b242caf24a3d5e108b89372c39dd971181bc2755`; run `migration-benchmark-a613c94cd8d240e4b7412d576e760331`; 2026-08-30 01:21:53Z–01:24:36Z |
| `build/migration-parity/benchmark-parity-result.json` | Correctness preflight | SHA-256 `aa2717dadad0de3714060ec2c7f2b23bad9913230a1e03a47d3c08c8d11e4871`; 208/208 pass |
| `build/migration-parity/profiles/` | Bounded path/resource profiles | Current revision; five-sample CPU/SIMD/GPU GaussianBlur+Invert profiles |
| `build/migration-parity/benchmark-table-standard-load10-20260829.md` | Formatting and older measurement semantics only | Its timings are **not current** and are never presented as current here |
| `/tmp/pillow-rs-audit-anomalies.json` | Six-workload maintained focused reproduction | Validated; distinct output path |
| `/tmp/pillow-rs-audit-gpu70.json` | Seventy-GPU-workload maintained focused reproduction | Validated; outside a Metal-blocking sandbox; distinct output path |
| `/tmp/pillow-rs-audit-input48.json` | Forty-eight-input maintained focused reproduction | Validated; distinct output path |
| `/tmp/pillow-rs-audit-full-parity-20260830.json` | Fresh full live-oracle parity rerun from this execution pass | 10,952/10,952 passed; SHA-256 `cd83d003f64cef0ffb8106fe9984d298e245649b41d352bddd8028c9f407fd03` |

The benchmark identity records macOS 15.7.7 arm64, CPython 3.12.13, Pillow 12.2.0, memory `0` (unknown), and power mode `unknown`. The target revision is the audit revision and the target is explicitly dirty. The current `pipeline-operations.json` hash matches the artifact (`9412c6503020ec47bc64c2cd9044cbe8fd5a33155bca14b09aa4edeac7d95121`). Results are authoritative for this exact dirty state, not a clean release claim.

Read-only source evidence includes `scripts/run_migration_benchmark.py`, `scripts/run_migration_parity.py`, input generators/reporters/validator, and the CPU/SIMD/GPU compute pools, registry, pipeline, and image state machine. Focused executions used maintained Make targets and unique `/tmp` outputs. Metal probes ran where adapter enumeration worked. No hardware GPU occupancy counter was captured.

Execution update: `MIGRATION_PARITY_OUTPUT=/tmp/pillow-rs-audit-full-parity-20260830.json make migration-parity-test` completed at 2026-08-30 02:49:58Z with 10,952 selected, 10,952 executed, 10,952 passed, zero failed, zero not-run, and zero infrastructure errors. This independent full parity run does not replace the 208-case parity artifact paired with the authoritative benchmark run.

## 3. Measurement semantics and limitations

The report uses four separate evidence dimensions:

1. **Workflow subject status**: enough complete timing and phase samples existed for one subject (`completed`) or did not (`failed`).
2. **Execution receipt**: requested backend, actual backend, fallback, operations, dispatches, and resources. A null/missing receipt is not backend proof.
3. **Correctness outcome**: the workload-wide gate is `pass` only when Pillow and every target complete and, for parity-backed cases, the preflight case passed.
4. **Infrastructure status**: process/tooling failures. This count is zero and must not absorb unsupported operations or invalid inputs.

Important implementation semantics:

- `timed_success` in `scripts/run_migration_benchmark.py:1036-1045` is an all-subject conjunction. It drives workload correctness but does not determine whether another individual subject's samples are valid.
- `durations_for` at lines 1104-1116 correctly preserves one completed backend's samples when another backend fails. `subject_result` at lines 623-675 independently requires the expected duration/phase count.
- Exact source/target errors are collected at lines 1029-1034 and printed to stderr at 1060-1076, but the result schema omits them. The validator at `scripts/validate_migration_parity_result.py:711-770` is closed over a subject shape with no error field. Where an exact timed error is unavailable, this audit says so and names the maintained focused reproduction; it does not invent one.
- `scripts/run_migration_parity.py:1086-1092` drains telemetry in the exception handler and forcibly labels the receipt `completed`. The returned workflow status at 1123-1131 can simultaneously be `not_run`. This is the source of the three partial-receipt anomalies.
- Suite aggregation at `scripts/run_migration_benchmark.py:1171-1215` skips failed members independently and averages each subject's member **means**. The stored latency ratio is target/baseline, opposite the speedup direction used in this audit.
- Speedup here is always baseline median divided by optimized median: `Pillow/CPU`, `CPU/SIMD`, `Pillow/SIMD`, or `SIMD/GPU`. Greater than one means the optimized subject is faster.
- Per-workload ratios use the same ID, timing boundary, cache policy, and sample policy. Geometric means combine ratios. Unrelated raw latencies across heterogeneous sizes are not averaged as evidence of typical speedup.
- `sample` is a CPU call-stack sampler, not a Metal utilization tool. Dispatch count, backend wall time, buffer bytes, and elapsed latency do not establish GPU occupancy, memory-bandwidth saturation, or hardware utilization.

## 4. User acceptance contract (baseline snapshot)

The requested contract translated into machine-checkable rules is shown below
as it stood at the audit baseline.  Section 31 supersedes the parity and
completion rows with the post-remediation evidence; the performance rows
remain open.

| Gate | Exact rule | Current status |
|---|---|---|
| Valid denominator | Every default performance input produces a successful Pillow value; expected-error and optional-dependency cases have named non-default lanes | **Fail:** 48 do not produce usable source timing |
| Completion equality | For the retained comparison set, compared subjects have exactly equal workload-ID sets | **Fail:** P/CPU/SIMD/GPU = 698/697/696/628 after exclusion |
| CPU/SIMD equality | `CPU_completed_IDs == SIMD_completed_IDs` | **Fail:** symmetric difference 3 |
| Actual backend | Every claimed target row has requested=actual backend, full sample receipt, and empty fallback | **Fail:** only 480 of the 695 common CPU/SIMD completions prove both actual backends |
| GPU computational coverage | GPU completes the declared computational set with a terminal actual-GPU receipt, no fallback | **Fail:** 70 retained status failures; 67 no receipt, 3 partial receipts |
| Correctness | Successful value parity is distinguished from matched-error parity; no `not_proven` row is a performance pass | **Fail:** 71 retained and 119 raw workloads are `not_proven` |
| CPU vs Pillow | Per workload `Pillow median / CPU median >= 1.00` on identical end-to-end workflows | **Fail:** 212 pass, 270 fail on 482 actual-CPU pairs |
| SIMD no regression | Per SIMD-eligible material workload, both `CPU/SIMD >=1.00` and `Pillow/SIMD >=1.00` | **Fail:** threshold audit in section 17; substantial regressions remain |
| SIMD significant | Same material cohort, both ratios `>=1.25` | **Fail:** 29/175 meet both |
| GPU no regression | Candidate large computational cohort, `SIMD/GPU >1.00` | **Fail:** 4/108 pass |
| GPU practical win | Same cohort, `SIMD/GPU >=1.20` | **Fail:** 4/108 pass; geometric mean 0.135× |
| Cold/warm stability | Repeated cold and warm samples; median, p95, variability and transfers reported independently | **Partial:** data exists for four 1024² workflows, but cold has only three samples and no hardware counters |
| Suite comparability | Ratios only from explicit equal ID/receipt intersections; member counts and exclusions persisted | **Fail:** 10/54 suites have unequal completed sets; one common set is empty |
| Error evidence | Normalized exact error/class/stage is persisted per failed subject | **Fail:** errors are stderr-only |

The blanket public-call CPU gate remains a real requirement and is reported as failed. A second direct-kernel gate is recommended for diagnosis, not as a silent relaxation: end-to-end calls measure binding/routing/allocation overhead; direct kernels locate compute regressions.

## 5. Original baseline table

| Measure | Raw 746 | After excluding the 48 source-invalid/not-benchmarkable candidates |
|---|---:|---:|
| Selected / retained workloads | 746 | 698 |
| At least one subject measured | 698 | 698 |
| All-subject not run | 48 | 0 |
| Correctness pass | 627 | 627 |
| Correctness not proven | 119 | 71 |
| Pillow completed / failed | 698 / 48 | 698 / 0 |
| CPU completed / failed | 697 / 49 | 697 / 1 |
| SIMD completed / failed | 696 / 50 | 696 / 2 |
| GPU completed / failed | 628 / 118 | 628 / 70 |
| Failed subject records | 265 | 73 |
| Formal infrastructure errors | 0 | 0 |
| Parity preflight | 208 selected, 208 executed, 208 passed, 0 failed | unchanged |

Actual-backend proof is narrower than completed timing status: CPU 482, SIMD 482, GPU 416 after the 48 exclusions, with 415 workloads proving all three target backends and 480 proving CPU+SIMD together. A completed subject with null receipt is timing evidence, not backend evidence.

## 6. Formal error-count breakdown

| Layer | Count | Interpretation |
|---|---:|---|
| Raw failed subject records | 265 | Status records, not a homogeneous error class |
| Records caused by 48 all-subject workflows | 192 | 48 × Pillow/CPU/SIMD/GPU; source cannot support the current timing workflow |
| Retained target failures | 73 | CPU 1 + SIMD 2 + GPU 70 |
| Retained GPU without a completed receipt | 67 | Unsupported/capability/routing execution failures |
| Retained GPU with a partial completed receipt | 3 | Failed terminal workflow plus earlier actual-GPU dispatch |
| Correctness `not_proven` | 119 raw / 71 retained | Gate state; not automatically an execution error |
| Parity mismatch | 0 observed | No retained failure is demonstrated output mismatch |
| Parity-preflight failure | 0 | 208/208 pass |
| Infrastructure error | 0 | No runner/environment infrastructure record |
| Environment limitation in retained GPU cohort | 0 | Same run proves hundreds of actual-GPU receipts |

The 45 parity-backed invalid public cases pass preflight because Pillow and CPU match the same public exception. That proves error-contract parity, not a value result, backend dispatch, or performance correctness.

## 7. Benchmark denominator analysis

The current numeric denominator must exclude all 48 because none has valid four-subject timings. This is a temporary measurement disposition, not deletion approval.

| Primary class | Count | Performance disposition | Coverage disposition |
|---|---:|---|---|
| `INPUT_INVALID` | 45 | Repair source generator/workflow; re-admit only after successful Pillow value execution and target parity | Retain current matched-error cases where useful; add distinct success cases; keep three chain IDs in named excluded inventory |
| `OPTIONAL_DEPENDENCY_UNAVAILABLE` | 2 | Exclude from default suite; benchmark only in an identified Qt-enabled lane | Retain dependency-free ImportError parity plus optional success coverage |
| `INPUT_EXPECTED_ERROR_NOT_BENCHMARKABLE` | 1 | Replace timing workflow with first successful iterator step | Retain exhaustion/StopIteration parity case |

After repair, lifecycle/I/O workloads such as save and seek may re-enter a lifecycle stratum, but they must not be blended into CPU/SIMD/GPU compute-kernel claims. The required generator invariant is: every default performance workload must preflight to a successful Pillow value, not merely a matched error. Each named exclusion must record ID, reason, retained coverage location, owner, and re-entry condition.

## 8. Complete 48-case input-exclusion audit

### 8.1 Matrix legend

Every row below contains every required failure field. Compact codes keep the 25-column matrix readable.

- Source input codes expand under `pillow-rs/tests/fixtures/inputs/benchmark/`: `BI`=`pil-image.json`; `BII`=`pil-image-image.json`; `BCH`=`pil-imagechops.json`; `BD`=`pil-imagedraw-imagedraw.json`; `BFT`=`pil-imagefont-freetypefont.json`; `BFB`=`pil-imagefont-imagefont.json`; `BO`=`pil-imageops.json`; `BPA`=`pil-imagepalette-imagepalette.json`; `BSQ`=`pil-imagesequence-iterator.json`; `BP`=`pipeline-operations.json`. Rows 1–45 also resolve the same case ID in the corresponding `inputs/parity/` file.
- Generator codes: `GS`=`scripts/build_migration_parity_inputs.py:43423,43598-43649` (generic standard workload); `GM`=image/mask defaults at 1740-1741,1832+,4086-4137; `GA`=generic argument descriptors at 3799-4330; `GF`=font/receiver setup at 3510-3575; `GI`=save/seek lifecycle at 4155-4184,4270-4285; `GN`=iterator exhaustion at 4411+,6157-6172; `GP`=composition matrix at 6925+,11442-11460; `G009`=7965-7998; `G013`=8103-8141; `G081`=10405-10435.
- Status notation: `F/E`=subject failed and exact error is evidenced; `F/?`=failed but the timed error is absent from serialized JSON; `F/RC`=timing subject failed while its execution receipt says completed. `P/C/S/G` mean Pillow/CPU/SIMD/GPU. Target requests are always `cpu/simd/gpu`; `—` means no actual backend. `none` means no fallback.
- Exact error text for rows 1–45 is confirmed for Pillow and CPU by the 208/208 parity artifact. SIMD/GPU timed messages are not serialized; the focused 48 run is the required per-profile evidence until error persistence is implemented. Rows 46–48 were reproduced in the maintained focused run; matrix-013 target-specific messages remain absent.
- Fix codes: `FM`=choose a valid mode/mask; robust operation-aware mode constraints. `FA`=supply signature-valid/cross-field arguments; robust discriminated descriptors and generator validation. `FB`=provide a real deterministic buffer; robust buffer-protocol fixture. `FL`=repair asset/receiver/lifecycle; robust asset capability metadata and lifecycle strata. `FQ`=named default exclusion plus Qt-enabled lane. `FI`=time first successful `next`, retain exhaustion parity. `FC`=repair chain value/mode flow; robust typed pipeline-edge validation. Likely files: `G`=generator, `B`=benchmark runner, `P`=parity runner, `C`=input checker/generator tests, `V`=validator/schema, `I`=regenerated inputs, `R`=Rust return contract only if direct parity confirms it.
- Focused verification `VB` means the exact row ID in `MIGRATION_BENCHMARK_ARGS='--workload-id <ID>'` with unique `/tmp` benchmark/parity outputs and `make migration-parity-benchmark`; `VP` means the exact case ID through `make migration-parity-case`. Full `VF+AB` means `make migration-parity-fixtures-check`, `make migration-parity-test-all-backends`, and a full `make migration-parity-benchmark` outside a Metal-blocking sandbox.

### 8.2 Canonical 48-row matrix

| # / workload ID | Source input | Generator | Mode | Dimensions | Chain | Operation sequence | Pillow | CPU | SIMD | GPU | Exact error | Requested backend | Actual backend | Fallback | Op count | Dispatch count | Primary class / tags | First divergence | Confidence | Minimal fix | Robust fix | Likely files | Focused verification | Full regression |
|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|
| 1 `pil-image.alpha-composite.standard` | BI + parity case `PIL.Image.alpha_composite.behavior.default` | GS+GM | RGB | 16×16 | 1 | new RGB×2→alpha_composite | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | pillow/cpu/simd/gpu | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode,mask | RGB operands; API needs RGBA/LA | Confirmed | FM: map to valid RGBA case | Valid-success case map + mode schema | G,B,C,I | VP+VB row 1 | VF+AB |
| 2 `pil-image.composite.standard` | BI + `PIL.Image.composite.behavior.default` | GS+GM | RGB | 16×16 | 1 | new×3→composite | F/E | F/E | F/? | F/? | `ValueError: bad transparency mask` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mask | RGB mask; API needs 1/L/RGBA | Confirmed | FM: use L mask | Typed mask selection | G,C,I | VP+VB row 2 | VF+AB |
| 3 `pil-image.effect-mandelbrot.standard` | BI + `PIL.Image.effect_mandelbrot.behavior.default` | GS+GA | L | 16×16 | 1 | effect_mandelbrot | F/E | F/E | F/? | F/? | `TypeError: argument 2 must be 4-item sequence, not float` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / extent | scalar extent | Confirmed | FA: four-item extent | Signature-discriminated descriptors | G,C,I | VP+VB row 3 | VF+AB |
| 4 `pil-image.eval.standard` | BI + `PIL.Image.eval.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→eval | F/E | F/E | F/? | F/? | `TypeError: type str doesn't define __round__ method` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / callable | string placeholder used as function | Confirmed | FA: valid LUT/callable | Explicit callable representation | G,P,C,I | VP+VB row 4 | VF+AB |
| 5 `pil-image.fromarray.standard` | BI + `PIL.Image.fromarray.behavior.default` | GS+GA | unknown | 0×0 | 1 | fromarray | F/E | F/E | F/? | F/? | `TypeError: a bytes-like object is required, not 'ArrayInterfaceValue'` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / protocol | synthetic value lacks buffer path | Confirmed | FB: buffer-backed array | Deterministic protocol/buffer fixture | G,P,C,I | VP+VB row 5 | VF+AB |
| 6 `pil-image.frombuffer.standard` | BI + `PIL.Image.frombuffer.behavior.default` | GS+GA | RGB | 16×16 | 1 | frombuffer | F/E | F/E | F/? | F/? | `ValueError: not enough image data` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / byte-length | four bytes for 768-byte image | Confirmed | FA: exact byte length | Mode/size-derived byte builder | G,C,I | VP+VB row 6 | VF+AB |
| 7 `pil-image.frombytes.standard` | BI + `PIL.Image.frombytes.behavior.default` | GS+GA | RGB | 16×16 | 1 | frombytes | F/E | F/E | F/? | F/? | `ValueError: not enough image data` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / byte-length | four bytes for RGB image | Confirmed | FA: exact byte length | Mode/size-derived byte builder | G,C,I | VP+VB row 7 | VF+AB |
| 8 `pil-image.linear-gradient.standard` | BI + `PIL.Image.linear_gradient.behavior.default` | GS+GM | RGB | 256×256 | 1 | linear_gradient | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | RGB requested; generator is L-only | Confirmed | FM: mode L | Operation mode schema | G,C,I | VP+VB row 8 | VF+AB |
| 9 `pil-image.radial-gradient.standard` | BI + `PIL.Image.radial_gradient.behavior.default` | GS+GM | RGB | 256×256 | 1 | radial_gradient | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | RGB requested; generator is L-only | Confirmed | FM: mode L | Operation mode schema | G,C,I | VP+VB row 9 | VF+AB |
| 10 `pil-image-image.alpha-composite.standard` | BII + `PIL.Image.Image.alpha_composite.behavior.default` | GS+GM | RGB | 16×16 | 1 | new RGB×2→method alpha_composite | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode,mutator | RGB receiver/overlay | Confirmed | FM: RGBA and observe receiver | Valid mutator postcondition schema | G,C,I | VP+VB row 10 | VF+AB |
| 11 `pil-image-image.frombytes.standard` | BII + `PIL.Image.Image.frombytes.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→frombytes | F/E | F/E | F/? | F/? | `ValueError: not enough image data` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / byte-length | four bytes for RGB receiver | Confirmed | FA: exact bytes | Shared checked byte builder | G,C,I | VP+VB row 11 | VF+AB |
| 12 `pil-image-image.point.standard` | BII + `PIL.Image.Image.point.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→point | F/E | F/E | F/? | F/? | `ValueError: wrong number of lut entries` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / LUT | 256 entries for three-band RGB | Confirmed | FA: L or 768-entry LUT | Band-derived LUT validator | G,C,I | VP+VB row 12 | VF+AB |
| 13 `pil-image-image.putalpha.standard` | BII + `PIL.Image.Image.putalpha.behavior.default` | GS+GM | RGB | 16×16 | 1 | new→putalpha | F/E | F/E | F/? | F/? | `ValueError: illegal image mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / alpha-mode | RGB alpha image | Confirmed | FM: L alpha | Typed alpha descriptor + postcondition | G,C,I | VP+VB row 13 | VF+AB |
| 14 `pil-image-image.putdata.standard` | BII + `PIL.Image.Image.putdata.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→putdata | F/E | F/E | F/? | F/? | `TypeError: argument must be a sequence` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / data | Image passed as pixel sequence | Confirmed | FA: deterministic pixels | Typed pixel-sequence builder | G,C,I | VP+VB row 14 | VF+AB |
| 15 `pil-image-image.putpalette.standard` | BII + `PIL.Image.Image.putpalette.behavior.default` | GS+GM | RGB | 16×16 | 1 | new→putpalette | F/E | F/E | F/? | F/? | `ValueError: illegal image mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / palette-mode | RGB receiver | Confirmed | FM: P receiver+palette | Receiver-mode/palette schema | G,C,I | VP+VB row 15 | VF+AB |
| 16 `pil-image-image.remap-palette.standard` | BII + `PIL.Image.Image.remap_palette.behavior.default` | GS+GM | RGB | 16×16 | 1 | new→remap_palette | F/E | F/E | F/? | F/? | `ValueError: illegal image mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / palette-mode | non-P receiver | Confirmed | FM: P receiver | Palette/index postcondition schema | G,C,I | VP+VB row 16 | VF+AB |
| 17 `pil-image-image.save.standard` | BII + `PIL.Image.Image.save.behavior.default` | GS+GI | RGB | 16×16 | 1 | new→save | F/E | F/E | F/? | F/? | `ValueError: unknown file extension: .out` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / I/O,lifecycle | `.out` with no format | Confirmed | FL: `.png` or explicit PNG | Separate stream/filesystem lifecycle strata | G,C,I | VP+VB row 17 | VF+AB |
| 18 `pil-image-image.seek.standard` | BII + `PIL.Image.Image.seek.behavior.default` | GS+GI | RGB | 16×16 | 1 | new single-frame→seek(1) | F/E | F/E | F/? | F/? | `EOFError: no more images in file` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / lifecycle | frame 1 on single-frame image | Confirmed | FL: deterministic multi-frame asset | Explicit lifecycle assets/state | G,C,I | VP+VB row 18 | VF+AB |
| 19 `pil-image-image.tobitmap.standard` | BII + `PIL.Image.Image.tobitmap.behavior.default` | GS+GM | RGB | 16×16 | 1 | new→tobitmap | F/E | F/E | F/? | F/? | `ValueError: not a bitmap` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | RGB receiver | Confirmed | FM: mode 1 | Operation-aware receiver modes | G,C,I | VP+VB row 19 | VF+AB |
| 20 `pil-image-image.toqimage.standard` | BII + `PIL.Image.Image.toqimage.behavior.default` | GS | RGB | 16×16 | 1 | new→toqimage | F/E | F/E | F/? | F/? | `ImportError: Qt bindings are not installed` | all | pillow/—/—/— | none | 0 | — | `OPTIONAL_DEPENDENCY_UNAVAILABLE` / Qt | optional binding import | Confirmed | FQ: default exclusion | Provisioned Qt lane + dependency identity | G,B,C,I | VP+VB row 20 on Qt host | VF+AB on Qt host |
| 21 `pil-image-image.toqpixmap.standard` | BII + `PIL.Image.Image.toqpixmap.behavior.default` | GS | RGB | 16×16 | 1 | new→toqpixmap | F/E | F/E | F/? | F/? | `ImportError: Qt bindings are not installed` | all | pillow/—/—/— | none | 0 | — | `OPTIONAL_DEPENDENCY_UNAVAILABLE` / Qt | optional binding import | Confirmed | FQ: default exclusion | Provisioned Qt lane + dependency identity | G,B,C,I | VP+VB row 21 on Qt host | VF+AB on Qt host |
| 22 `pil-image-image.transform.standard` | BII + `PIL.Image.Image.transform.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→transform | F/E | F/E | F/? | F/? | `ValueError: missing method data` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / method-data | transform method lacks data | Confirmed | FA: valid method-specific data | Discriminated transform schema | G,C,I | VP+VB row 22 | VF+AB |
| 23 `pil-imagechops.composite.standard` | BCH + `PIL.ImageChops.composite.behavior.default` | GS+GM | RGB | 16×16 | 1 | new×3→Chops.composite | F/E | F/E | F/? | F/? | `ValueError: bad transparency mask` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mask | RGB mask | Confirmed | FM: L mask | Typed mask selection | G,C,I | VP+VB row 23 | VF+AB |
| 24 `pil-imagechops.logical-and.standard` | BCH + `PIL.ImageChops.logical_and.behavior.default` | GS+GM | RGB | 16×16 | 1 | new×2→logical_and | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | logical op gets RGB | Confirmed | FM: mode 1 | Operation mode schema | G,C,I | VP+VB row 24 | VF+AB |
| 25 `pil-imagechops.logical-or.standard` | BCH + `PIL.ImageChops.logical_or.behavior.default` | GS+GM | RGB | 16×16 | 1 | new×2→logical_or | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | logical op gets RGB | Confirmed | FM: mode 1 | Operation mode schema | G,C,I | VP+VB row 25 | VF+AB |
| 26 `pil-imagechops.logical-xor.standard` | BCH + `PIL.ImageChops.logical_xor.behavior.default` | GS+GM | RGB | 16×16 | 1 | new×2→logical_xor | F/E | F/E | F/? | F/? | `ValueError: image has wrong mode` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | logical op gets RGB | Confirmed | FM: mode 1 | Operation mode schema | G,C,I | VP+VB row 26 | VF+AB |
| 27 `pil-imagedraw-imagedraw.arc.standard` | BD + `PIL.ImageDraw.ImageDraw.arc.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→arc | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain exactly 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `xy=[0,0]` | Confirmed | FA: two corners | Shape-aware geometry builder | G,C,I | VP+VB row 27 | VF+AB |
| 28 `pil-imagedraw-imagedraw.bitmap.standard` | BD + `PIL.ImageDraw.ImageDraw.bitmap.behavior.default` | GS+GM | RGB | 16×16 | 1 | new→Draw→bitmap | F/E | F/E | F/? | F/? | `ValueError: bad transparency mask` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / bitmap-mode | RGB bitmap | Confirmed | FM: 1/L bitmap | Typed bitmap descriptor | G,C,I | VP+VB row 28 | VF+AB |
| 29 `pil-imagedraw-imagedraw.chord.standard` | BD + `PIL.ImageDraw.ImageDraw.chord.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→chord | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain exactly 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `xy=[0,0]` | Confirmed | FA: valid box | Shape-aware geometry builder | G,C,I | VP+VB row 29 | VF+AB |
| 30 `pil-imagedraw-imagedraw.ellipse.standard` | BD + `PIL.ImageDraw.ImageDraw.ellipse.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→ellipse | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain exactly 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `xy=[0,0]` | Confirmed | FA: valid box | Shape-aware geometry builder | G,C,I | VP+VB row 30 | VF+AB |
| 31 `pil-imagedraw-imagedraw.pieslice.standard` | BD + `PIL.ImageDraw.ImageDraw.pieslice.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→pieslice | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain exactly 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `xy=[0,0]` | Confirmed | FA: valid box | Shape-aware geometry builder | G,C,I | VP+VB row 31 | VF+AB |
| 32 `pil-imagedraw-imagedraw.polygon.standard` | BD + `PIL.ImageDraw.ImageDraw.polygon.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→polygon | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain at least 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | one point only | Confirmed | FA: ≥2 points | Cardinality/cross-field validation | G,C,I | VP+VB row 32 | VF+AB |
| 33 `pil-imagedraw-imagedraw.rectangle.standard` | BD + `PIL.ImageDraw.ImageDraw.rectangle.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→rectangle | F/E | F/E | F/? | F/? | `TypeError: coordinate list must contain exactly 2 coordinates` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `xy=[0,0]` | Confirmed | FA: valid box | Shape-aware geometry builder | G,C,I | VP+VB row 33 | VF+AB |
| 34 `pil-imagedraw-imagedraw.regular-polygon.standard` | BD + `PIL.ImageDraw.ImageDraw.regular_polygon.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→regular_polygon | F/E | F/E | F/? | F/? | `ValueError: n_sides should be an int > 2` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | `n_sides=1` | Confirmed | FA: `n_sides>=3` | Cross-field geometry validation | G,C,I | VP+VB row 34 | VF+AB |
| 35 `pil-imagedraw-imagedraw.rounded-rectangle.standard` | BD + `PIL.ImageDraw.ImageDraw.rounded_rectangle.behavior.default` | GS+GA | RGB | 16×16 | 1 | new→Draw→rounded_rectangle | F/E | F/E | F/? | F/? | `ValueError: not enough values to unpack (expected 4, got 2)` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / geometry | two-number `xy` | Confirmed | FA: valid four-value box | Shape-aware geometry builder | G,C,I | VP+VB row 35 | VF+AB |
| 36 `pil-imagefont-freetypefont.get-variation-axes.standard` | BFT + matching parity case | GS+GF | unknown | 0×0 | 1 | load DejaVuSans→get_variation_axes | F/E | F/E | F/? | F/? | `OSError: invalid argument` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / font-asset | static font lacks axes | Confirmed | FL: variable-font asset | Font capability metadata/preflight | G,C,I | VP+VB row 36 | VF+AB |
| 37 `pil-imagefont-freetypefont.get-variation-names.standard` | BFT + matching parity case | GS+GF | unknown | 0×0 | 1 | load DejaVuSans→get_variation_names | F/E | F/E | F/? | F/? | `OSError: invalid argument` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / font-asset | static font lacks variations | Confirmed | FL: variable font | Font capability metadata | G,C,I | VP+VB row 37 | VF+AB |
| 38 `pil-imagefont-freetypefont.set-variation-by-axes.standard` | BFT + matching parity case | GS+GA+GF | unknown | 0×0 | 1 | load static font→set axes scalar | F/E | F/E | F/? | F/? | `TypeError: argument must be a list` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / font,arg | scalar axes; wrong asset too | Confirmed | FA+FL: valid list+variable font | Validate axes length/ranges from asset | G,C,I | VP+VB row 38 | VF+AB |
| 39 `pil-imagefont-freetypefont.set-variation-by-name.standard` | BFT + matching parity case | GS+GF | unknown | 0×0 | 1 | load static font→set variation name | F/E | F/E | F/? | F/? | `OSError: invalid argument` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / font-asset | invalid asset/name | Confirmed | FL: variable font+valid name | Asset metadata-driven name selection | G,C,I | VP+VB row 39 | VF+AB |
| 40 `pil-imagefont-imagefont.getbbox.standard` | BFB + matching parity case | GS+GF | unknown | 0×0 | 1 | bare ImageFont→getbbox | F/E | F/E | F/? | F/? | `AttributeError: 'ImageFont' object has no attribute 'font'` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / receiver | uninitialized base receiver | Confirmed | FL: loaded bitmap font | Reusable receiver constructor+asset check | G,C,I | VP+VB row 40 | VF+AB |
| 41 `pil-imagefont-imagefont.getlength.standard` | BFB + matching parity case | GS+GF | unknown | 0×0 | 1 | bare ImageFont→getlength | F/E | F/E | F/? | F/? | `AttributeError: 'ImageFont' object has no attribute 'font'` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / receiver | uninitialized base receiver | Confirmed | FL: loaded bitmap font | Shared receiver constructor | G,C,I | VP+VB row 41 | VF+AB |
| 42 `pil-imagefont-imagefont.getmask.standard` | BFB + matching parity case | GS+GF | unknown | 0×0 | 1 | bare ImageFont→getmask | F/E | F/E | F/? | F/? | `AttributeError: 'ImageFont' object has no attribute 'font'` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / receiver | uninitialized base receiver | Confirmed | FL: loaded bitmap font | Shared receiver constructor | G,C,I | VP+VB row 42 | VF+AB |
| 43 `pil-imageops.colorize.standard` | BO + `PIL.ImageOps.colorize.behavior.default` | GS+GM | RGB | 16×16 | 1 | new RGB→colorize | F/E | F/E | F/? | F/? | `AssertionError` (empty message) | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode | Pillow requires L input | Confirmed | FM: L input+colors | Operation-aware mode guard | G,C,I | VP+VB row 43 | VF+AB |
| 44 `pil-imagepalette-imagepalette.getcolor.standard` | BPA + matching parity case | GS+GA | unknown | 0×0 | 1 | ImagePalette→getcolor(0) | F/E | F/E | F/? | F/? | `ValueError: unknown color specifier: 0` | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / color | scalar color spec | Confirmed | FA: RGB tuple/string | Typed color descriptor | G,C,I | VP+VB row 44 | VF+AB |
| 45 `pil-imagesequence-iterator.next.standard` | BSQ + matching parity case | GS+GN | RGB | 16×16 | 2 | Iterator→next(success)→next(exhaust) | F/E | F/E | F/? | F/? | second step `StopIteration: end of sequence` | all | pillow/—/—/— | none | 0 | — | `INPUT_EXPECTED_ERROR_NOT_BENCHMARKABLE` / iterator | generator appends exhaustion | Confirmed | FI: stop timed workflow after first next | Explicit case intent; separate success/error workflows | G,B,C,I | VP+VB row 45 | VF+AB |
| 46 `pipeline-chain.matrix-009` | BP embedded workflow | GP+G009 | RGBA | 22×18 | 3 | new×2→in-place alpha_composite→grayscale→invert→bytes | F/E | F/RC | F/RC | F/RC | Pillow `AttributeError: 'NoneType' object has no attribute 'convert'`; targets no error | all | pillow/cpu/simd/gpu | none | C/S/G 1 | GPU 1 | `INPUT_INVALID` / return-semantics | binds `None` return as image | Confirmed generator; target contract medium-high | FC: continue from mutated receiver | Return-semantics metadata+typed value-flow validation | G,C,I,R | VB row 46 | VF+AB |
| 47 `pipeline-chain.matrix-013` | BP embedded workflow | GP+G013 | LA | 21×21 | 3 | new LA→BoxBlur→solarize→point→bytes | F/E | F/? | F/? | F/? | Pillow `OSError: not supported for mode LA`; target timed errors absent | all | pillow/—/—/— | none | 0 | — | `INPUT_INVALID` / mode-edge | LA enters `ImageOps._lut` solarize | Confirmed source; targets unresolved | FC: convert to L or use valid chain | Validate output mode at every pipeline edge | G,C,I | VB row 47; capture each target error | VF+AB |
| 48 `pipeline-chain.matrix-081` | BP embedded workflow | GP+G081 | LA | 9×7 | 2 | new LA→transform→invert→bytes | F/E | F/RC | F/RC | F/RC | Pillow `OSError: not supported for mode LA`; targets no error | all | pillow/cpu/simd/gpu | none | C/S/G 1 | GPU 1 | `INPUT_INVALID` / mode-edge | transformed LA enters invert `_lut` | Confirmed generator | FC: convert to L or valid mode | Typed pipeline-edge validation | G,C,I | VB row 48 | VF+AB |

### 8.3 Exclusion and re-entry decision

All 48 are excluded from the **current numeric** denominator. Rows 1–44 and 46–48 should be repaired at the generator source and re-admitted only when Pillow produces a value, every required target passes parity, and target receipts are terminal and actual. Rows 20–21 remain in a named optional lane. Row 45 retains its error-contract workflow and gains a separate success workflow. No row is deleted from API/parity visibility.

## 9. CPU/SIMD completion-set comparison

Let `U` be the 698 workloads retained after removing the 48 all-subject not-runs, `C` the CPU completed IDs, and `S` the SIMD completed IDs.

| Set | Count | Exact finite content |
|---|---:|---|
| `U` / Pillow | 698 | all retained workloads |
| `C` | 697 | `U` except loaded RGBA ten-action chain |
| `S` | 696 | `U` except 1×1 Rotate and Add |
| `C - S` | 2 | `pipeline-matrix.expanded.rotate.1x1`; `pipeline-matrix.expanded.add.1x1` |
| `S - C` | 1 | `pipeline-chain.loaded-10.rgba-png-512x384` |
| `C ∩ S` | 695 | common completed timing set |
| `C ∪ S` | 698 | equals `U` |
| `C △ S` | 3 | the three IDs above |

Backend proof is narrower: actual CPU/no fallback 482, actual SIMD/no fallback 482, strict CPU+SIMD intersection 480. Of the 695 common completions, 214 prove neither target backend and one (`pipeline-op.lineargradient.benchmark-materialized`) proves SIMD but not CPU. Missing receipt is not permission to infer the selected backend.

The three full required rows follow. `BP` is `pillow-rs/tests/fixtures/inputs/benchmark/pipeline-operations.json`; expanded generation is `scripts/build_migration_parity_inputs.py:41075-41147`; loaded generation is `_loaded_ten_action_pipeline_workflow` at 40016-40130 and its workload loop around 42967-43006. Focused `VB` and full `VF+AB` expand as in section 8.

| Workload ID | Source input | Generator location | Mode | Dimensions | Chain length | Operation sequence | Pillow status | CPU status | SIMD status | GPU status | Exact error | Requested backend | Actual backend | Fallback | Operation count | Dispatch count | Classification | First divergence | Confidence | Proposed minimal fix | Proposed robust fix | Likely files | Focused verification | Full regression verification |
|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|
| `pipeline-chain.loaded-10.rgba-png-512x384` | BP; `image/rgba-small.png` | 40016-40130,42967-43006 | context RGB; source RGBA | 512×384 | 10 | open→load→convert RGB→resize→rotate→GaussianBlur→invert→mirror→autocontrast→crop→bytes | completed | failed/not_run | completed | failed/not_run | CPU at `cropped`: `OSError: not supported for mode RGBA`; GPU: `NotImplementedError: GPU does not support Rotate` | cpu/simd/gpu | —/simd/— | none | CPU 0; SIMD last receipt median 1; GPU 0 | host —; GPU 0 | `CPU_EXECUTION_DEFECT`; lifecycle, stale mode, loaded chain | concrete data becomes RGB after Convert; CPU logical mode remains RGBA; Autocontrast rejects it when Crop forces materialization | Confirmed | Advance CPU `current_mode` after every op | Shared backend-neutral plan carrying input/output layout and receipt history | `pillow-rs/src/compute/pool_cpu/mod.rs`, `image.rs`, shared compute planner/registry tests | VB exact ID; assert post-Convert mode and bytes | VF+AB; `make test-all`; `make fmt`; `make clippy` |
| `pipeline-matrix.expanded.rotate.1x1` | BP | 41075-41147 | RGB | 1×1 | 1 | new→rotate(1°, nearest)→bytes | completed | completed | failed/not_run | failed/not_run | SIMD at materialize: `NotImplementedError: SIMD does not support Rotate for the current image layout/mode`; GPU analogous Rotate unsupported | cpu/simd/gpu | cpu/—/— | none | CPU 1; SIMD/GPU 0 | GPU 0 | `SIMD_EXECUTION_DEFECT`; geometry, tail | strict SIMD shape gate requires ≥16 output bytes before a kernel that already has a scalar tail | Confirmed | Padded partial `u8x16` gather/store and relax valid nonempty shape gate | Separate strict capability from calibrated auto-routing crossover | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` + tests | VB exact ID; 1×1 and 15/16-byte boundaries | VF+AB; SIMD strict lane; test/fmt/clippy |
| `pipeline-matrix.expanded.add.1x1` | BP | 41075-41147 | RGB | 1×1 | 1 | new×2→Chops.add(scale=1,offset=0)→bytes | completed | completed | failed/not_run | completed | SIMD at materialize: `NotImplementedError: SIMD does not support Add for the current image layout/mode` | cpu/simd/gpu | cpu/—/gpu | none | CPU 1; SIMD 0; GPU 1 | GPU 1 | `SIMD_EXECUTION_DEFECT`; multi-image, 3-byte tail | default saturating Add is incorrectly gated by 8-byte affine/16-byte row predicates | Confirmed | Parameter-sensitive default path using padded bytewise `u8x16` kernel | One capability descriptor shared by preflight/runtime/in-place; padded affine tails later | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` + tests | VB exact ID; Add/Subtract L/LA/RGB/RGBA and 7/8/15/16-byte rows | VF+AB; SIMD strict lane; test/fmt/clippy |

## 10. CPU failure investigation

`Image::evaluate_pipeline_with_image` obtains the initial source mode and passes it to the executor. CPU `execute_batch` then reuses that same mode for every registry call (`pillow-rs/src/compute/pool_cpu/mod.rs:235-363`) and never advances it after `PipelineOp::Convert`. In the failed chain:

1. RGBA PNG bytes and logical mode agree.
2. `Convert(RGB)` produces RGB bytes.
3. CPU state still says RGBA: this is the earliest divergence.
4. Later Autocontrast receives the stale mode and rejects it.
5. The public error appears at `cropped` because Crop materializes its receiver; Crop is the trigger, not the causal kernel.

SIMD succeeds because it advances `simd_initial_mode` through `simd_mode_after_op` (`pool_simd/ops/adapters.rs:5846-5900`). Decode, timeout, receipt loss, and Crop arithmetic are ruled out by deterministic focused reproduction and successful Pillow/SIMD execution.

Minimal causal fix: track and advance the current logical mode in every CPU batch/fusion exit. Robust fix: build a shared prepared plan whose nodes carry input/output mode, dimensions, and layout, and have every backend consume that same plan. Direct regression should first isolate `RGBA→Convert(RGB)→Autocontrast`; the loaded chain remains the lifecycle end-to-end guard. Expected completion impact: CPU +1, making CPU 698/698 on the current retained set.

## 11. SIMD failure investigations

### 11.1 Rotate 1×1

The default public Rotate is nearest. `rotate_nearest_supported_for_shape` rejects outputs smaller than 16 bytes (`adapters.rs:4271-4301`, constant near 12848), so 1×1 RGB fails before execution. The actual kernel at 20467-20599 has a remainder loop and can compute the pixel. This is a capability/tail-policy mismatch, not evidence of bad geometry or output parity.

Strict SIMD coverage should execute a padded vector tail and prove `actual_backend=simd`; production auto-routing may still choose CPU below a measured crossover. A scalar-only fallback labeled SIMD is not acceptable. Expected impact: SIMD +1.

### 11.2 Add 1×1

The RGB row contains three bytes. Capability preflight applies an eight-byte affine predicate and the default saturating path applies a 16-byte row guard, even though `native_chops_bytewise` already pads a `u8x16` tail and writes only active bytes. Default `scale=1, offset=0` should use that bytewise capability; non-default affine parameters remain separately gated until padded `f64x8` handling exists. Expected impact: SIMD +1, with a latent symmetric Subtract boundary covered by regression tests.

### 11.3 Architecture and acceleration reality

The build target is AArch64 with NEON. The `wide` crate maps `u8x16` to `uint8x16_t`, so the active byte kernels use genuine 128-bit NEON. This is portable compile-time SIMD, not Apple-M3-specific code or runtime dispatch. Logical `f64x8` decomposes into four NEON `f64x2` groups; conversions, lane extraction, scalar gathers, allocation, and control flow can dominate. The 1×1 failures are routing gates, while the broader performance failures in section 17 reflect kernel quality and memory behavior.

## 12. Complete 70-case GPU failure matrix

### 12.1 GPU matrix legend and evidence codes

- Source: `BP`=`pillow-rs/tests/fixtures/inputs/benchmark/pipeline-operations.json`; `BD`=`.../pil-imagedraw-imagedraw.json` plus its parity input.
- Generator: `GD`=generic workload writer `scripts/build_migration_parity_inputs.py:43423,43598-43649`; `GO`=base/matrix pipeline workloads 40980-41079; `GE`=expanded matrices 41079-41127; `GC`=composition matrices 41133-41178; `GM`=metadata 41883-42036; `GR`=rank 42043-42110; `GV`=reviewed 42240-42270; `GG`=material geometry 42366-42398; `GRC`=resize cache 42903-42947; `GL`=loaded-ten 40016-40130,42971-43006.
- Status `ok` means timing subject completed; `F` means failed. `NP/—/0` means execution not proven, actual backend null, zero completed receipt samples. `C/gpu/6` means aggregate execution completed with six actual-GPU receipt samples despite a failed timing subject. All requests are GPU and all fallback maps are empty.
- `E0(root)` means the authoritative JSON omitted exact timed error text. The outside-sandbox 70-workload focused run confirmed the named earliest root and wrote validated JSON to `/tmp/pillow-rs-audit-gpu70.json`, but errors remain stderr-only. The exact per-row reproduction `FG` is:

  ```sh
  MIGRATION_BENCHMARK_OUTPUT=/tmp/gpu-<slug>.json \
  MIGRATION_BENCHMARK_PARITY_OUTPUT=/tmp/gpu-<slug>-parity.json \
  MIGRATION_BENCHMARK_ARGS='--workload-id <EXACT_ID>' \
  make migration-parity-benchmark
  ```

  Run it outside a sandbox that blocks Metal enumeration and retain stderr. This is the explicit reproduction action wherever an exact serialized error is unavailable.
- Fix codes expand both fix columns and file column: `DRAW`=exact shared scan conversion/clipping/stroke/fill/alpha primitives; `ROT`=shared Pillow pixel-center/expand/fill planner then exact shader; `TH`=lower exact aspect-ratio/rounding plan to Resize; `FIT`=shared crop/resize/bleed/centering plan; `SPR`/`NOI`=deterministic order-preserving Pillow RNG semantics or remain explicitly unsupported; `MRG`=typed N-band pack plan; `LUT`=real interpolation/table upload/cache replacing pass-through; `MODE`=segment at mode transitions then robust per-dispatch logical layout; `RANK`=typed-F segmentation plus bounded efficient order-statistic algorithm; `XFM`=method-specific Perspective/Quad/Mesh plans; `FRES`=native typed-f32 resampling and coefficient cache. Common files are `pillow-rs/src/compute/registry.rs`, `pillow-rs/src/compute/pool_gpu/mod.rs`, relevant WGSL under `pool_gpu/shaders/`, and shared planning/tests.
- Full verification `VG`: focused `FG`; exact new parity input generated through the maintained generator; `make image-backend-parity-test`; `make migration-parity-test-all-backends`; `make migration-parity-pipeline-benchmark-coverage`; `MIGRATION_BENCHMARK_PROFILE=pipeline make migration-parity-benchmark`; `make fmt`; `make clippy`.

### 12.2 Rows 1–35

| # / workload ID | Source input | Generator location | Mode | Dimensions | Chain length | Operation sequence | Pillow status | CPU status | SIMD status | GPU status | Exact error | Requested backend | Actual backend | Fallback | Operation count | Dispatch count | Classification | First divergence | Confidence | Proposed minimal fix | Proposed robust fix | Likely files | Focused verification | Full regression verification |
|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|
| 1 `pil-imagedraw-imagedraw.shape.standard` | BD | GD | RGB | 16×16 | 1 | new→Draw→shape/polygon→observe | ok | ok | ok | F; NP/—/0 | E0(DrawPolygon) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,gate | DrawPolygon | High | DRAW: exact polygon primitive | Shared draw rasterizer | registry,gpu pool,draw WGSL/tests | FG row 1 | VG |
| 2 `pipeline-op.rotate.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | focused: `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Rotate registry contract | Confirmed | ROT: enable exact proven subset | Shared exact rotate planner+shader | ROT files | FG row 2 | VG |
| 3 `pipeline-op.thumbnail.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→putpixel→thumbnail→bytes | ok | ok | ok | F; C/gpu/6 partial | focused: `NotImplementedError: GPU does not support Thumbnail` | gpu | gpu (partial) | none | 1 | 1 | `GPU_SHADER_OR_REGISTRY_DEFECT` / receipt,gate | Thumbnail after successful PutPixel dispatch | Confirmed | TH: exact lowering | Shared sizing/state planner | TH files + runners | FG row 3 | VG |
| 4 `pipeline-op.rankfilter.benchmark-materialized` | BP | GO | F | 3×3 | 1 | new→putdata→RankFilter→filter→bytes | ok | ok | ok | F; NP/—/0 | E0(PutData: unsupported logical mode / RankFilter batch) | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / typed-F,batch | PutData→RankFilter admissibility | Medium-high | RANK: segment operations | Typed-F planner+efficient kernel | RANK files | FG row 4 | VG |
| 5 `pipeline-op.fit.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→fit→bytes | ok | ok | ok | F; NP/—/0 | E0(Fit) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Fit contract false | High | FIT: exact crop+resize lowering | Shared fit planner | FIT files | FG row 5 | VG |
| 6 `pipeline-op.effectspread.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG,scatter | EffectSpread absent | High | SPR: truthful unsupported until exact | Deterministic ordered device/control algorithm | SPR files | FG row 6 | VG |
| 7 `pipeline-op.merge.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new bands×4→merge→bytes | ok | ok | ok | F; NP/—/0 | E0(Merge) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / multiband | Merge absent/CPU-only | High | MRG: supported band pack | Typed N-band planner | MRG files | FG row 7 | VG |
| 8 `pipeline-op.effectnoise.benchmark-materialized` | BP | GO | L | 16×16 | 1 | effect_noise→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectNoise) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / RNG | EffectNoise contract absent | High | NOI: truthful unsupported until exact | Reproduce sequential RNG/rejection | NOI files | FG row 8 | VG |
| 9 `pipeline-op.color3dlut.benchmark-materialized` | BP | GO | RGB | 16×16 | 6 | 3×(new→Color3DLUT→filter→mode→bytes) | ok | ok | ok | F; NP/—/0 | E0(Color3DLut) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / LUT,pass-through | first LUT filter | High | LUT: implement exact interpolation | Validated cached table/layout contract | LUT files | FG row 9 | VG |
| 10 `pipeline-op.drawline.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→line→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawLine | High | DRAW line | Shared draw rasterizer | DRAW files | FG row 10 | VG |
| 11 `pipeline-op.drawrectangle.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→rectangle→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawRectangle) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawRectangle | High | DRAW rectangle | Shared draw rasterizer | DRAW files | FG row 11 | VG |
| 12 `pipeline-op.drawroundedrect.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→rounded_rectangle→bytes | ok | ok | ok | F; NP/—/0 | focused root observed DrawEllipse decomposition; exact stderr not persisted | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawRoundedRect/ellipse primitive | High | DRAW rounded rectangle | Shared draw rasterizer | DRAW files | FG row 12 | VG |
| 13 `pipeline-op.drawellipse.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→ellipse→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawEllipse) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawEllipse | High | DRAW ellipse | Shared draw rasterizer | DRAW files | FG row 13 | VG |
| 14 `pipeline-op.drawcircle.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→circle→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawCircle) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawCircle | High | DRAW circle | Shared draw rasterizer | DRAW files | FG row 14 | VG |
| 15 `pipeline-op.drawpolygon.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→polygon→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPolygon) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPolygon | High | DRAW polygon | Shared draw rasterizer | DRAW files | FG row 15 | VG |
| 16 `pipeline-op.drawarc.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→arc→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawArc) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawArc | High | DRAW arc | Shared draw rasterizer | DRAW files | FG row 16 | VG |
| 17 `pipeline-op.drawchord.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→chord→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawChord) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawChord | High | DRAW chord | Shared draw rasterizer | DRAW files | FG row 17 | VG |
| 18 `pipeline-op.drawpieslice.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→pieslice→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPieslice) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPieslice | High | DRAW pieslice | Shared draw rasterizer | DRAW files | FG row 18 | VG |
| 19 `pipeline-op.drawpoint.benchmark-materialized` | BP | GO | RGB | 16×16 | 1 | new→Draw→point→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPoint) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPoint | High | DRAW point | Shared draw rasterizer | DRAW files | FG row 19 | VG |
| 20 `pipeline-op.rotate.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 20 | VG |
| 21 `pipeline-op.thumbnail.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→putpixel→thumbnail→bytes | ok | ok | ok | F; C/gpu/6 partial | `NotImplementedError: GPU does not support Thumbnail` | gpu | gpu (partial) | none | 1 | 1 | `GPU_SHADER_OR_REGISTRY_DEFECT` / receipt,gate | Thumbnail after PutPixel | Confirmed | TH exact lowering | Shared sizing/state planner | TH files+runners | FG row 21 | VG |
| 22 `pipeline-op.fit.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→fit→bytes | ok | ok | ok | F; NP/—/0 | E0(Fit) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Fit | High | FIT crop+resize | Shared fit planner | FIT files | FG row 22 | VG |
| 23 `pipeline-op.effectspread.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG | EffectSpread | High | SPR exact or unsupported | Ordered deterministic algorithm | SPR files | FG row 23 | VG |
| 24 `pipeline-op.merge.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new bands×4→merge→bytes | ok | ok | ok | F; NP/—/0 | E0(Merge) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / multiband | Merge | High | MRG supported pack | Typed N-band planner | MRG files | FG row 24 | VG |
| 25 `pipeline-op.color3dlut.matrix-32x24` | BP | GO | RGB | 32×24 | 6 | 3×(new→LUT→filter→mode→bytes) | ok | ok | ok | F; NP/—/0 | E0(Color3DLut) | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / LUT | first LUT filter | High | LUT interpolation | Cached table/layout contract | LUT files | FG row 25 | VG |
| 26 `pipeline-op.drawline.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→line→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawLine | High | DRAW line | Shared draw rasterizer | DRAW files | FG row 26 | VG |
| 27 `pipeline-op.drawrectangle.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→rectangle→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawRectangle) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawRectangle | High | DRAW rectangle | Shared draw rasterizer | DRAW files | FG row 27 | VG |
| 28 `pipeline-op.drawroundedrect.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→rounded_rectangle→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawRoundedRect/ellipse) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawRoundedRect | High | DRAW rounded rectangle | Shared draw rasterizer | DRAW files | FG row 28 | VG |
| 29 `pipeline-op.drawellipse.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→ellipse→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawEllipse) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawEllipse | High | DRAW ellipse | Shared draw rasterizer | DRAW files | FG row 29 | VG |
| 30 `pipeline-op.drawcircle.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→circle→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawCircle) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawCircle | High | DRAW circle | Shared draw rasterizer | DRAW files | FG row 30 | VG |
| 31 `pipeline-op.drawpolygon.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→polygon→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPolygon) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPolygon | High | DRAW polygon | Shared draw rasterizer | DRAW files | FG row 31 | VG |
| 32 `pipeline-op.drawarc.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→arc→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawArc) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawArc | High | DRAW arc | Shared draw rasterizer | DRAW files | FG row 32 | VG |
| 33 `pipeline-op.drawchord.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→chord→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawChord) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawChord | High | DRAW chord | Shared draw rasterizer | DRAW files | FG row 33 | VG |
| 34 `pipeline-op.drawpieslice.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→pieslice→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPieslice) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPieslice | High | DRAW pieslice | Shared draw rasterizer | DRAW files | FG row 34 | VG |
| 35 `pipeline-op.drawpoint.matrix-32x24` | BP | GO | RGB | 32×24 | 1 | new→Draw→point→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawPoint) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw | DrawPoint | High | DRAW point | Shared draw rasterizer | DRAW files | FG row 35 | VG |

### 12.3 Rows 36–70

| # / workload ID | Source input | Generator location | Mode | Dimensions | Chain length | Operation sequence | Pillow status | CPU status | SIMD status | GPU status | Exact error | Requested backend | Actual backend | Fallback | Operation count | Dispatch count | Classification | First divergence | Confidence | Proposed minimal fix | Proposed robust fix | Likely files | Focused verification | Full regression verification |
|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|---|---:|---:|---|---|---|---|---|---|---|---|
| 36 `pipeline-matrix.expanded.rotate.1x1` | BP | GE | RGB | 1×1 | 1 | new→rotate→bytes | ok | ok | F | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate`; SIMD error in section 9 | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry,also-SIMD | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 36 | VG |
| 37 `pipeline-matrix.expanded.rotate.32x32` | BP | GE | RGB | 32×32 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 37 | VG |
| 38 `pipeline-matrix.expanded.rotate.256x256` | BP | GE | RGB | 256×256 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 38 | VG |
| 39 `pipeline-matrix.expanded.rotate.1024x768` | BP | GE | RGB | 1024×768 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry,large | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 39 | VG |
| 40 `pipeline-matrix.expanded.medianfilter.1024x768` | BP | GE | RGB | 1024×768 | 1 | new→MedianFilter→filter→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support MedianFilter: unsafe primary image dimensions` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / work-limit,large | estimated shader work exceeds 128M guard | Confirmed | RANK: retain guard; optimize algorithm | Bounded local-memory/histogram order statistic | RANK files | FG row 40 | VG |
| 41 `pipeline-matrix.expanded.effectspread.1x1` | BP | GE | RGB | 1×1 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG | EffectSpread | High | SPR exact or unsupported | Ordered deterministic algorithm | SPR files | FG row 41 | VG |
| 42 `pipeline-matrix.expanded.effectspread.32x32` | BP | GE | RGB | 32×32 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG | EffectSpread | High | SPR exact or unsupported | Ordered deterministic algorithm | SPR files | FG row 42 | VG |
| 43 `pipeline-matrix.expanded.effectspread.256x256` | BP | GE | RGB | 256×256 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG | EffectSpread | High | SPR exact or unsupported | Ordered deterministic algorithm | SPR files | FG row 43 | VG |
| 44 `pipeline-matrix.expanded.effectspread.1024x768` | BP | GE | RGB | 1024×768 | 1 | new→effect_spread→bytes | ok | ok | ok | F; NP/—/0 | E0(EffectSpread) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / RNG,large | EffectSpread | High | SPR exact or unsupported | Ordered deterministic algorithm | SPR files | FG row 44 | VG |
| 45 `pipeline-chain.matrix-000` | BP | GC | RGB | 46×22 | 2 | new→thumbnail→size→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Thumbnail` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry | Thumbnail | Confirmed | TH exact lowering | Shared sizing/state planner | TH files | FG row 45 | VG |
| 46 `pipeline-chain.matrix-004` | BP | GC | RGBA | 31×19 | 3 | new→resize→rotate→crop→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / chain | Rotate after supported Resize | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 46 | VG |
| 47 `pipeline-chain.matrix-007` | BP | GC | RGB | 2×2 | 4 | new/open→grayscale→colorize→multiply→screen→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support Grayscale: non-terminal mode change` | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / mode-transition | Grayscale before later ops | Confirmed | MODE: split safely at transition | Per-dispatch mode/layout state | MODE files | FG row 47 | VG |
| 48 `pipeline-chain.matrix-017` | BP | GC | RGBA | 24×20 | 4 | new→putalpha→GaussianBlur→convert→invert→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support PutAlpha: non-terminal mode change` | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / mode-transition | PutAlpha changes layout before later ops | Confirmed | MODE segmentation | Per-dispatch state plan | MODE files | FG row 48 | VG |
| 49 `pipeline-chain.matrix-021` | BP | GC | RGBA | 22×18 | 3 | new×2→alpha_composite→grayscale→resize→bytes | ok | ok | ok | F; C/gpu/6 partial | focused: `GPU does not support Paste: non-terminal mode change` | gpu | gpu (partial) | none | 1 | 1 | `PIPELINE_ROUTING_DEFECT` / receipt,gate,mode | later mode-changing segment after successful composite/Paste dispatch | Confirmed | MODE segmentation; label receipt partial | Per-segment state+terminal receipt | MODE files+runners | FG row 49 | VG |
| 50 `pipeline-chain.matrix-022` | BP | GC | RGB | 32×24 | 4 | new→Draw→line→rectangle→GaussianBlur→invert→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,chain | DrawLine; DrawRectangle remains next | High | DRAW line then family | Shared draw rasterizer | DRAW files | FG row 50 | VG |
| 51 `pipeline-chain.matrix-023` | BP | GC | RGB | 256×192 | 11 | new→Draw→line→rectangle→rounded_rect→ellipse→circle→polygon→arc→chord→pieslice→point→invert→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,long-chain | first DrawLine | High | DRAW family | Shared draw rasterizer | DRAW files | FG row 51 | VG |
| 52 `pipeline-chain.matrix-024` | BP | GC | RGBA | 192×128 | 4 | new→Draw→rounded_rect→ellipse→line→point→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawRoundedRect) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,alpha | DrawRoundedRect | High | DRAW rounded rectangle | Shared draw rasterizer | DRAW files | FG row 52 | VG |
| 53 `pipeline-chain.matrix-073` | BP | GC | RGBA | 9×7 | 2 | new→getchannel→invert→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support ExtractBand: non-terminal mode change` | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / mode-transition | ExtractBand changes RGBA→L | Confirmed | MODE segmentation | Per-dispatch state plan | MODE files | FG row 53 | VG |
| 54 `pipeline-chain.matrix-074` | BP | GC | LA | 9×7 | 2 | new→getchannel→convert→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support ExtractBand: non-terminal mode change` | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / mode-transition | ExtractBand changes LA→L | Confirmed | MODE segmentation | Per-dispatch state plan | MODE files | FG row 54 | VG |
| 55 `pipeline-chain.matrix-082` | BP | GC | RGB | 9×7 | 2 | new→transform(Perspective)→crop→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Transform` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / non-affine | Perspective transform | Confirmed | XFM: add Perspective exact | Method-specific planners/shaders | XFM files | FG row 55 | VG |
| 56 `pipeline-chain.matrix-083` | BP | GC | RGB | 9×7 | 2 | new→transform(Quad)→mirror→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Transform` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / non-affine | Quad transform | Confirmed | XFM: add Quad | Method-specific planners/shaders | XFM files | FG row 56 | VG |
| 57 `pipeline-chain.matrix-084` | BP | GC | RGBA | 8×8 | 2 | new→transform(Mesh)→convert→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Transform` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / non-affine | Mesh transform | Confirmed | XFM: add Mesh | Method-specific planners/shaders | XFM files | FG row 57 | VG |
| 58 `pipeline-chain.matrix-085` | BP | GC | LA | 8×8 | 2 | new→transform(Perspective)→convert→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Transform` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / non-affine | Perspective transform | Confirmed | XFM: add Perspective LA | Method-specific typed planners | XFM files | FG row 58 | VG |
| 59 `pipeline-chain.metadata-cache.color3dlut-rgb` | BP | GM | RGB | 256×256 | 7 | new→Color3DLUT→filter→3×(mode→size)→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Color3DLut` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / LUT,metadata | Color3DLut filter | Confirmed | LUT interpolation | Cached validated LUT layout | LUT files | FG row 59 | VG |
| 60 `pipeline-chain.rank-filter.large-f-9x9` | BP | GR | F | 9×9 | 1 | new→putdata→RankFilter→filter→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support PutData: unsupported logical mode` | gpu | — | none | 0 | 0 | `PIPELINE_ROUTING_DEFECT` / typed-F,batch | F PutData→RankFilter composition | Confirmed | RANK segmentation | Typed-F plan | RANK files | FG row 60 | VG |
| 61 `pipeline-chain.rank-filter.material.f-9x9-256x256` | BP | GR | F | 256×256 | 1 | new→RankFilter→filter→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support RankFilter: unsafe primary image dimensions` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / work-limit,typed-F | estimated work exceeds guard | Confirmed | RANK optimized bounded kernel | Local-memory/histogram algorithm | RANK files | FG row 61 | VG |
| 62 `pipeline-chain.rank-filter.material.l-9x9-256x256` | BP | GR | L | 256×256 | 1 | new→RankFilter→filter→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support RankFilter: unsafe primary image dimensions` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / work-limit | estimated work exceeds guard | Confirmed | RANK optimized bounded kernel | Local-memory/histogram algorithm | RANK files | FG row 62 | VG |
| 63 `pipeline-chain.reviewed.resize-rotate-crop` | BP | GV | RGBA | 31×19 | 3 | new→resize→rotate→crop→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / reviewed,chain | Rotate after Resize | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 63 | VG |
| 64 `pipeline-chain.reviewed.draw-filter-invert` | BP | GV | RGB | 32×24 | 4 | new→Draw→line→rectangle→GaussianBlur→invert→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,reviewed | DrawLine | High | DRAW line/family | Shared draw rasterizer | DRAW files | FG row 64 | VG |
| 65 `pipeline-chain.reviewed.draw-batch-rgb-shapes` | BP | GV | RGB | 256×192 | 11 | new→Draw→ten shapes→invert→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawLine) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,long-chain | DrawLine | High | DRAW family | Shared draw rasterizer | DRAW files | FG row 65 | VG |
| 66 `pipeline-chain.reviewed.draw-batch-rgba-alpha` | BP | GV | RGBA | 192×128 | 4 | new→Draw→rounded_rect→ellipse→line→point→bytes | ok | ok | ok | F; NP/—/0 | E0(DrawRoundedRect) | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / draw,alpha | DrawRoundedRect | High | DRAW family | Shared draw rasterizer | DRAW files | FG row 66 | VG |
| 67 `pipeline-chain.geometry-material.rotate-rgba-1024x768` | BP | GG | RGBA | 1024×768 | 1 | new→rotate→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / geometry,large | Rotate | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 67 | VG |
| 68 `pipeline-chain.resize-cache.f64-identical-geometry` | BP | GRC | F | 333×257 | 2 | new→resize(default)→bytes→new→resize(default)→bytes | ok | ok | ok | F; NP/—/0 | focused: `GPU does not support Resize: unsupported logical mode` | gpu | — | none | 0 | 0 | `GPU_CAPABILITY_MISSING` / typed-F,non-nearest | first F Resize | Confirmed | FRES: explicit exact filter support | Native f32 filters+coefficient cache | FRES files | FG row 68 | VG |
| 69 `pipeline-chain.loaded-10.rgb-jpeg-512x384` | BP | GL | RGB | 512×384 | 10 | open/load→convert→resize→rotate→blur→invert→mirror→autocontrast→crop→bytes | ok | ok | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate` | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / loaded,chain | Rotate after Convert+Resize | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 69 | VG |
| 70 `pipeline-chain.loaded-10.rgba-png-512x384` | BP | GL | context RGB; source RGBA | 512×384 | 10 | open/load→convert→resize→rotate→blur→invert→mirror→autocontrast→crop→bytes | ok | F | ok | F; NP/—/0 | `NotImplementedError: GPU does not support Rotate`; CPU error section 9 | gpu | — | none | 0 | 0 | `GPU_SHADER_OR_REGISTRY_DEFECT` / loaded,also-CPU | Rotate after Convert+Resize | Confirmed | ROT exact subset | Shared rotate planner | ROT files | FG row 70 | VG |

## 13. GPU failures grouped by root operation

Every GPU row has exactly one primary classification; secondary receipt/gate tags do not change the total.

| Primary class | Earliest root family | Failed workloads | Recovery interpretation |
|---|---|---:|---|
| `GPU_CAPABILITY_MISSING` | Draw family | 27 | DrawLine 6, DrawRectangle 2, DrawRoundedRect 4, DrawEllipse 2, DrawCircle 2, DrawPolygon 3 including Shape, DrawArc 2, DrawChord 2, DrawPieslice 2, DrawPoint 2. A shared draw foundation can recover 27; one primitive may merely reveal the next in long chains. |
| `GPU_CAPABILITY_MISSING` | EffectSpread | 6 | Exact RNG/collision/scatter semantics recover six. |
| `GPU_CAPABILITY_MISSING` | Rank/Median work contract | 3 | Two 9×9 RankFilter 256² and one MedianFilter 1024×768 exceed the work guard. Optimize; do not merely raise the watchdog bound. |
| `GPU_CAPABILITY_MISSING` | non-affine Transform | 4 | Perspective, Quad, and Mesh method support recovers four. |
| `GPU_CAPABILITY_MISSING` | F non-nearest Resize | 1 | Native typed-f32 resampling recovers one. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | Rotate | 11 | Existing/anticipated shader contract is disabled pending exact Pillow geometry/alpha semantics. One shared exact planner can recover 11. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | Thumbnail | 3 | Exact host sizing/lowering recovers three; two also require partial-receipt semantics repair. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | Fit | 2 | Exact crop+resize plan recovers two. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | Merge | 2 | Typed band packing/layout recovers two. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | Color3DLut | 3 | Replace pass-through behavior with exact interpolation/table handling; recover three. |
| `GPU_SHADER_OR_REGISTRY_DEFECT` | EffectNoise | 1 | Exact RNG contract recovers one. |
| `PIPELINE_ROUTING_DEFECT` | nonterminal mode transition | 5 | Grayscale/Paste, PutAlpha, and ExtractBand chains need safe segmentation or per-node layout propagation. |
| `PIPELINE_ROUTING_DEFECT` | F PutData→RankFilter batch | 2 | Both pieces have bounded paths, but the composition is not admitted. |
| **Total** |  | **70** | Primary totals: capability 41, shader/registry 22, routing 7. |

The first deterministic high-leverage wave is Draw (27), Rotate (11), nonterminal mode transitions (5), Thumbnail/Fit (5), and F rank composition (2): 50 records, though long draw chains require the whole primitive family before all 27 disappear. RNG operations are lower-confidence implementation work because exact sequential semantics can erase parallel advantage.

No retained GPU row is primarily invalid input, environment limitation, parity mismatch, or lifecycle/I/O failure. The machine has hundreds of actual-GPU receipts in the same run. The benchmark-only workflows prove successful execution, not byte parity; every new GPU capability still needs exact Pillow comparison before it becomes a correctness pass.

## 14. Correctness-gate and receipt anomalies

The three anomalies are partial executions, not valid GPU timings suppressed because some unrelated backend failed.

| Workload | Subject result | Receipt evidence | Terminal failure | Correct interpretation |
|---|---|---|---|---|
| `pipeline-op.thumbnail.benchmark-materialized` | GPU failed; empty measurements | six `actual_backend=gpu` receipts; median op/dispatch 1/1 | Thumbnail throws after an earlier PutPixel dispatch | Receipt proves PutPixel only; Thumbnail/terminal timing not proved |
| `pipeline-op.thumbnail.matrix-32x24` | same | same | Thumbnail | same |
| `pipeline-chain.matrix-021` | GPU failed; empty measurements | six actual-GPU receipts; median op/dispatch 1/1 | nonterminal mode-changing continuation after composite/Paste | Receipt proves the earlier segment only |

Confirmed runner sequence:

1. A successful earlier lazy segment emits telemetry.
2. A later public step raises.
3. The exception handler drains any receipt and assigns `status="completed"` (`run_migration_parity.py:1086-1092`).
4. The workflow itself returns `not_run` because a dependency failed.
5. `execution_result` sees the required count of completed receipts and reports execution `completed`, while `subject_result` sees incomplete timings and correctly reports the subject `failed`.

Minimal fix: when terminal workflow status is not completed, mark collected receipts `partial` and never promote aggregate execution to completed. Robust fix: add iteration ID, step/segment ID, terminal-completeness bit, and receipt history; require a terminal receipt that covers the measured boundary. Preserve partial receipts as diagnostics rather than deleting them.

The broader correctness-gate defect is nomenclature and persistence, not per-subject duration suppression. `durations_for` already keeps valid CPU/SIMD data if GPU fails. Results must independently serialize `not_run`, `execution_failed`, `unsupported`, `parity_failed`, `gate_not_proven`, and partial/missing receipt states. A matched public error must not make a performance case value-eligible.

## 15. Benchmark aggregation validity audit

Forty-four of 54 suites have equal completed-ID sets. Ten do not. The current weighted mean for each subject is therefore based on a different population in these suites.

| Suite | Declared | Pillow | CPU | SIMD | GPU | All-four intersection | Excluded from common set |
|---|---:|---:|---:|---:|---:|---:|---:|
| `pil-imagedraw-imagedraw.benchmark-suite` | 19 | 10 | 10 | 10 | 9 | 9 | 10 |
| `pipeline-operations.materialized-smoke-suite` | 87 | 87 | 87 | 87 | 69 | 69 | 18 |
| `pipeline-operations.composition-matrix-suite` | 64 | 61 | 61 | 61 | 47 | 47 | 17 |
| `pipeline-operations.filter-window-suite` | 4 | 4 | 4 | 4 | 1 | 1 | 3 |
| `pipeline-operations.size-matrix-suite` | 77 | 77 | 77 | 77 | 61 | 61 | 16 |
| `pipeline-operations.expanded-size-matrix-suite` | 100 | 100 | 100 | 98 | 91 | 90 | 10 |
| `pipeline-operations.resize-coefficient-cache-suite` | 2 | 2 | 2 | 2 | 1 | 1 | 1 |
| `pipeline-operations.geometry-material-suite` | 6 | 6 | 6 | 6 | 5 | 5 | 1 |
| `pipeline-operations.loaded-ten-action-suite` | 2 | 2 | 1 | 2 | 0 | 0 | 2 |
| `pipeline-operations.metadata-cache-suite` | 5 | 5 | 5 | 5 | 4 | 4 | 1 |

The loaded-ten suite is not comparable at all across four subjects. The expanded-size suite also has a CPU/SIMD mismatch within the already unequal GPU population. A current suite ratio is invalid if used as a backend comparison for any of these ten.

Correct reporting must keep completion coverage separate and build each comparison from an explicit paired intersection. Proposed result shape:

```json
{
  "comparison_id": "pipeline-operations.expanded-size-matrix-suite:simd-vs-cpu",
  "baseline_subject": "python-cpu",
  "subject": "python-simd",
  "declared_member_count": 100,
  "common_completed_member_count": 98,
  "common_actual_backend_member_count": 97,
  "common_member_ids_sha256": "...",
  "excluded_members": [
    {"workload_id": "...", "baseline_state": "completed", "subject_state": "unsupported"}
  ],
  "per_workload": [
    {"workload_id": "...", "baseline_median_ms": 1.2, "subject_median_ms": 1.0,
     "baseline_samples": 100, "subject_samples": 100, "speedup": 1.2}
  ],
  "geometric_mean_speedup": 1.07,
  "median_speedup": 1.05,
  "status": "comparable"
}
```

The report should mark `not_comparable` when the common set is empty and define a minimum statistical cohort per suite (recommended: at least two members for an aggregate; otherwise show the single workload without an aggregate claim). Never average radically different raw latencies as a typical speedup. Preserve each workload's medians, p95, variability, and sample counts.

## 16. CPU versus Pillow performance status

Speedup is `Pillow median / CPU median`.

| Cohort | N | CPU ≥1.00× | CPU <1.00× | Median speedup | Geometric mean |
|---|---:|---:|---:|---:|---:|
| All status-completed paired timings | 697 | 291 | 406 | 0.946× | 0.952× |
| Actual CPU receipt, no fallback | 482 | 212 | 270 | 0.957× | 1.030× |
| Material cohort defined below | 175 | 122 | 53 | 1.365× | 1.436× |

The blanket end-to-end 1.00× contract fails. Material computation is much stronger: CPU passes 122/175 and reaches at least 1.10× on 112, 1.20× on 99, 1.25× on 95, and 1.50× on 80. This does not excuse the 270 actual-receipt regressions.

Why CPU can lose:

- tiny calls are dominated by PyO3 conversion, lock/routing/validation, lazy graph construction, allocation, materialization, and receipt bookkeeping;
- Pillow's mature C kernels can have lower fixed overhead;
- some Rust paths copy/convert or allocate full frames unnecessarily;
- setup/load and terminal work can dominate a one-operation public call;
- operation quality varies; GaussianBlur, Fit, Reduce, and terminal analysis have visible slow buckets.

Fairness requires keeping identical end-to-end boundaries and publishing setup, pipeline, terminal, and total separately. Moving only target setup outside the timer would invalidate the comparison. Add a direct pure-Rust kernel/pipeline report to locate compute cost, while retaining the public-call gate as a separate contract.

## 17. SIMD versus CPU/Pillow performance status

The strict comparison requires completed actual CPU and SIMD receipts, no fallback, and the same Pillow workload. The broad strict intersection is 480. The proposed **material SIMD cohort** is the 175 strict rows with at least 65,536 source pixels and either chain length ≥2 or operation class in `{point, neighborhood, geometry, multi_image, draw}`. Tiny/terminal-only rows stay visible in separate tables.

Broad strict receipt set:

| Ratio | N | Median | Geometric mean | Faster (>1) | Slower (<1) |
|---|---:|---:|---:|---:|---:|
| SIMD vs CPU (`CPU/SIMD`) | 480 | 0.930× | 0.845× | 160 | 319 (one tie) |
| SIMD vs Pillow (`Pillow/SIMD`) | 480 | 0.877× | 0.870× | 168 | 312 |

Material threshold sensitivity:

| Required speedup | SIMD vs CPU pass | SIMD vs Pillow pass | Pass against both | Fail against either |
|---:|---:|---:|---:|---:|
| 1.00× | 64/175 | 95/175 | 57/175 | 118 |
| 1.10× | 52/175 | 87/175 | 45/175 | 130 |
| 1.20× | 40/175 | 79/175 | 32/175 | 143 |
| 1.25× | 38/175 | 71/175 | 29/175 | 146 |
| 1.50× | 24/175 | 51/175 | 14/175 | 161 |

Material median/geometric mean are 0.803×/0.645× versus CPU and 1.095×/0.926× versus Pillow. The requested “significantly faster than both” contract fails decisively at 1.25×.

Confirmed performance mechanisms:

- `u8x16` is real NEON, but several kernels retain scalar coordinate calculation/gathers or per-lane conversion.
- Logical `f64x8` is four NEON registers and incurs byte↔double conversion and packing.
- Heterogeneous pipelines often materialize intermediate full-frame buffers; fusion is limited.
- Simple point operations can be memory/allocation bound.
- Strict capability and production auto-routing are not separated by calibrated per-operation/mode/size crossover thresholds.
- A vectorized final store does not make scalar address generation or coefficient computation fast.

Severe CPU/SIMD regressions include LA Lanczos 512² (0.0525×), expanded Resize 1024×768 (0.0538×), RGBA Bicubic 512² (0.0539×), F RankFilter 9×9 at 256² (0.0568×), resize-cache identical geometry (0.0586×), and RGBA Lanczos 1024×768 (0.0623×). These are optimization buckets, not tiny-call noise.

## 18. GPU versus SIMD large-pipeline performance status

The requested candidate cohort is: completed/no-fallback actual SIMD and GPU receipts on the same ID; both dimensions at least 256; and either chain length ≥2 or operation class `neighborhood`/`geometry`. This yields 108 workloads. Speedup is `SIMD median / GPU median`.

| Cohort | N | GPU >1.00× | GPU ≥1.20× | Geometric mean | Median |
|---|---:|---:|---:|---:|---:|
| Requested large computational candidate | 108 | 4 | 4 | 0.135× | 0.130× |
| Exact 1024×768 actual-receipt stratum | 67 | 6 | 6 | 0.213× | 0.231× |
| Exact 1024×1024 actual-receipt stratum | 37 | 0 | 0 | 0.149× | 0.150× |

Only four candidate workloads beat SIMD: resize-cache identical geometry 2.484×, expanded Resize 1024×768 1.931×, expanded MaxFilter 1024×768 1.329×, and expanded MinFilter 1024×768 1.313×. The last three partly expose weak SIMD kernels, so they do not by themselves prove a healthy GPU implementation.

Current 1024² warm/cold workflows (latency milliseconds; variability is sample standard deviation):

| Workload | Cache / samples | SIMD median / p95 / sd | GPU median / p95 / sd | SIMD/GPU | GPU ops / dispatches | Transfer |
|---|---|---:|---:|---:|---:|---:|
| transpose twice | warm / 100 | 1.647 / 1.937 / 0.159 | 9.741 / 10.452 / 0.451 | 0.169× | 2 / 2 | 4 MiB up + 4 MiB back |
| GaussianBlur+Invert | warm / 100 | 6.477 / 8.884 / 1.166 | 10.416 / 17.198 / 3.755 | 0.622× | 2 / 7 | 4 MiB + 4 MiB |
| Multiply+Screen | warm / 100 | 2.494 / 2.801 / 0.170 | 23.037 / 24.453 / 1.223 | 0.108× | receipt 1 / 1 | 4 MiB + 4 MiB |
| Invert+Mirror | warm / 100 | 1.451 / 1.788 / 0.166 | 9.649 / 10.683 / 0.567 | 0.150× | 2 / 2 | 4 MiB + 4 MiB |
| transpose twice | cold / 3 | 2.172 / 2.319 / 0.096 | 19.177 / 19.291 / 0.110 | 0.113× | 2 / 2 | 4 MiB + 4 MiB |
| GaussianBlur+Invert | cold / 3 | 6.557 / 6.681 / 0.087 | 30.499 / 32.947 / 1.903 | 0.215× | 2 / 7 | 4 MiB + 4 MiB |
| Multiply+Screen | cold / 3 | 3.351 / 3.352 / 0.037 | 31.446 / 31.737 / 0.386 | 0.107× | receipt 1 / 1 | 4 MiB + 4 MiB |
| Invert+Mirror | cold / 3 | 2.000 / 2.029 / 0.066 | 19.148 / 19.688 / 0.330 | 0.104× | 2 / 2 | 4 MiB + 4 MiB |

The focused GaussianBlur+Invert profile retains the batching improvement: five actual-GPU samples `[47.340, 9.037, 9.609, 8.996, 8.839]` ms, median 9.037 ms; 2 operations, 7 dispatches, one 4 MiB upload, one 4 MiB readback, two full-frame copies, one mode conversion. After the cold-looking first sample, validation is about 1–2 µs and backend time about 7.77–8.40 ms. Its paired focused SIMD median is 12.928 ms, so that isolated profile favors GPU by about 1.43×. The authoritative standard warm run instead favors SIMD (0.622× GPU speedup). The user-provided pre-batching value of about 16.31 ms is prior evidence, not contained in the current artifact set. The focused improvement does not satisfy the broad contract.

Required GPU optimization probes: keep supported pipelines in one upload/readback batch; avoid intermediate host materialization; reuse pipelines, bind groups, buffers, and coefficient/table caches; fuse compatible point operations; reduce full-frame copies/conversions; inspect workgroup and memory-access patterns. Any claim about occupancy or utilization remains **unmeasured** until a Make-owned Metal counter trace is captured.

## 19. Workload-level performance violations

The exhaustive per-workload violation appendices are in section 30. Counts use strict completed/no-fallback actual receipt filters:

| Contract | Eligible | Violations | Passes |
|---|---:|---:|---:|
| CPU/Pillow `<1.00×` | 482 | 270 | 212 |
| SIMD/CPU `<1.00×` | 480 | 319 | 160 plus one tie |
| Material SIMD `<1.25×` against CPU or Pillow | 175 | 146 | 29 against both |
| Candidate-large GPU/SIMD `<1.20×` | 108 | 104 | 4 |

Representative worst actual-CPU/Pillow ratios are expanded GaussianBlur 32² 0.0876×, UnsharpMask standard 0.1009×, GaussianBlur 32×24 0.1032×, Fit base 0.1099×, Reduce 32² 0.1128×, and terminal `getcolors` 1024×768 0.1427×. Representative SIMD and GPU violations are listed in sections 17–18; every exact ID, pair of medians, and ratio is retained in the appendices.

## 20. Proposed input fixes

| Root bucket | Minimal causal fix | Robust architectural fix | Verification / prevention | Confidence |
|---|---|---|---|---|
| Invalid modes/masks (17) | Select an existing valid parity case or generate the required receiver/mask mode | Encode operation-specific allowed modes, mask roles, and postconditions in the generator schema | Generator value-success preflight; exact mode/mask tests; fixtures-check and all-backends | Confirmed |
| Malformed arguments/data (18) | Replace scalar/placeholders with signature-valid values and correct byte/LUT/geometry lengths | Discriminated argument schemas with cross-field validation derived from mode, size, bands, and method | Generator negative tests plus successful Pillow execution for every performance case | Confirmed |
| Buffer protocol (1) | Use a real deterministic buffer-backed array | Maintain a tested array/buffer fixture type with explicit shape/typestr/strides/data contract | Focused fromarray parity and performance workflow | Confirmed |
| Assets/receivers/lifecycle (8) | Use variable/bitmap font assets, valid save extension/format, and a multi-frame seek asset | Asset capability metadata and separate lifecycle/I/O strata | Asset hash/capability checks; lifecycle benchmark suite | Confirmed |
| Optional Qt (2) | Named default exclusion | Provisioned optional lane that records binding/package versions | Dependency-free expected ImportError plus Qt-host success workflow | Confirmed |
| Iterator exhaustion (1) | Time the first successful `next` only | Explicit `success` versus `expected_error` case intent; forbid error observation in default performance workloads | Separate success and StopIteration parity cases | Confirmed |
| Composition value/mode flow (3) | Use mutated receiver after in-place call; convert LA before L/RGB-only ops | Typed step return and mode graph validated before generation | Chain-local source preflight and exact all-backend output parity | Confirmed generator defects; target return semantics for matrix-009 needs direct parity |

Generator changes belong in `scripts/build_migration_parity_inputs.py` and its check/tests; generated JSON is regenerated, never hand-edited. Add an explicit excluded-input inventory containing ID, classification, evidence, coverage location, environment requirements, owner, and re-entry gate.

## 21. Proposed runner/report fixes

| Problem | Minimal causal fix | Robust architectural fix | Regression gate |
|---|---|---|---|
| Matched-error parity admitted to timing | Require successful value observations, not only parity `pass` | Explicit case intent/outcome: value success, expected error, optional dependency | Default performance input checker executes Pillow and rejects non-value outcomes |
| Errors printed then discarded | Serialize normalized bounded errors on each subject | Versioned diagnostic records with class, kind, message, stage, step, iteration, requested/actual backend | Strict validator schema and round-trip tests |
| Partial receipt labeled completed | Downgrade receipt when workflow/measurement boundary fails | Segment/iteration receipt history with terminal-completeness bit | Three anomaly fixtures must report partial, never completed |
| Ambiguous states | Add `not_run`, `execution_failed`, `unsupported`, `parity_failed`, `gate_not_proven` | Orthogonal workflow, receipt, parity, and infrastructure state machines | Validator rejects impossible combinations |
| Unequal suite subsets | Build ratios from explicit common completed ID intersection | Persist common-set IDs/hash, exclusions, actual-receipt count, per-workload ratios | Ten unequal suites either become paired or `not_comparable` |
| Raw latency aggregation | Use paired per-workload median ratios and geometric mean | Cohort definitions/version hashes and stratified size/class/cache reports | Reporter golden tests with unequal/empty/tiny cohorts |
| Missing backend provenance | Refuse speedup if requested≠actual, fallback nonempty, receipt missing/partial | Make backend receipt completeness a required benchmark budget | Coverage report publishes status set and receipt-proven set separately |
| Phase asymmetry risk | Retain identical workflow boundaries; print setup/pipeline/terminal/total | Add direct-core report without replacing public-call report | Boundary identity and sample-policy hash included in comparison |

Likely files: `scripts/run_migration_parity.py`, `scripts/run_migration_benchmark.py`, `scripts/report_pipeline_performance.py`, `scripts/report_pipeline_benchmark_coverage.py`, `scripts/validate_migration_parity_result.py`, generator/checker tests.

## 22. Proposed CPU fixes

Minimal: advance logical mode after each CPU operation, including fused/draw exits, using the same semantic mapping already exercised by SIMD. This recovers the single CPU failure.

Robust: create a backend-neutral prepared execution plan with explicit input/output mode, dimensions, layout, secondary inputs, and materialization boundaries. Capability preflight and execution must consume the same state transitions. Preserve complete receipt history rather than the last receipt only.

Performance work should then profile the direct Rust boundary for the worst CPU/Pillow families, reduce full-frame copies/conversions and Arc/output allocation, reuse buffers, and separate I/O/setup from kernel diagnosis without changing the public timer. Verification requires exact `RGBA→RGB→Autocontrast` bytes plus the full loaded ten-action chain and all existing backend parity lanes.

## 23. Proposed SIMD fixes

Completion fixes:

1. Rotate: implement a padded vector tail for valid sub-16-byte images and relax correctness capability; retain CPU auto-routing below measured crossover.
2. Add/Subtract: make capability parameter-sensitive. Default saturating byte arithmetic uses padded `u8x16`; non-default affine parameters retain a separate predicate until padded `f64x8` support exists.

Performance fixes, in dependency order:

- unify capability descriptors used by preflight, runtime, in-place paths, and telemetry;
- profile direct kernels for alpha/typed Resize, RankFilter, and coefficient-cache buckets;
- vectorize coordinate/coefficient/gather work, not only the final store;
- remove per-lane `to_array`/conversion where possible;
- reuse scratch/output buffers and fuse compatible operations;
- add per-operation/mode/size crossover tables for production auto-routing while strict SIMD remains no-fallback evidence;
- add AArch64-specific kernels only where portable `wide` cannot express an efficient implementation, guarded by exact parity and a portable fallback.

No performance receipt may label a scalar CPU fallback as SIMD. Hardware NEON instruction counts remain unmeasured until a maintained PMU/Instruments target exists.

## 24. Proposed GPU capability fixes

| Priority | Root family / impact | Minimal fix | Robust fix | Principal risk |
|---|---|---|---|---|
| P1 | Draw / 27 | Implement exact primitives in dependency order | Shared scan-conversion/clipping/stroke/fill/alpha-composite foundation | Very high parity risk at edges, widths, clipping, alpha |
| P1 | Rotate / 11 | Enable exact proven mode/angle subset | Shared CPU/GPU pixel-center, expand, fill, premultiplied-alpha planner | High geometry/parity risk |
| P1 | Mode routing / 5 | Safe batch segmentation | Per-node logical layout/dimensions and retained buffers | High state/receipt risk |
| P1 | Thumbnail+Fit / 5 | Lower to exact resize/crop plans | Shared sizing/rounding/bleed/centering planner | Medium rounding/parity risk |
| P1 | F PutData+Rank / 2 | Segment and preserve typed buffer | Typed-F execution planner | Medium routing risk |
| P2 | Rank/Median limits / 3 | Keep guard and replace excessive algorithm | Workgroup/local-memory or histogram order statistic with measured watchdog bound | High algorithm/parity/perf risk |
| P2 | Transform / 4 | Add one exact method at a time | Perspective/Quad/Mesh-specific planners/shaders | High geometry risk |
| P2 | Color3DLut / 3 | Exact interpolation/table upload | Validated cached table/layout contract | High rounding/mode risk |
| P2 | Merge / 2 | Supported band layouts | Typed N-band planner | Medium layout risk |
| P2 | F Resize / 1 | Exact explicitly supported filter | Native f32 resampling/coefficient cache | High numeric parity risk |
| P3 | EffectSpread+Noise / 7 | Remain explicitly unsupported until exact | Deterministic order-preserving RNG/control algorithm | Very high semantic and performance risk |

Never grow thin bindings or use CPU fallback to conceal missing GPU behavior. New capabilities belong in pure Rust core, registry/planner, and WGSL with exact Pillow parity.

## 25. Proposed GPU performance fixes

Capability equality comes first; otherwise optimization can improve a biased supported subset. Then:

1. retain a supported chain in one upload/dispatch/readback batch;
2. remove intermediate CPU materialization and redundant host/device copies;
3. cache/reuse device buffers, pipeline objects, bind groups, LUTs, and resize coefficients;
4. fuse compatible point operations and reduce dispatch count where semantics permit;
5. eliminate redundant mode conversions and full-frame copies;
6. tune workgroup sizes and coalesced access per shader, with exact-size benchmarks;
7. separate launch-bound tiny cases from bandwidth/compute-bound large cases;
8. measure cold and warm behavior, medians, p95, variability, upload/backend/readback/total independently;
9. add a maintained Metal counter trace before claiming occupancy or utilization.

GaussianBlur+Invert remains a regression anchor: actual GPU, no fallback, 2 operations, 7 dispatches, one upload/readback, focused median about 9.04 ms. Its standard paired result still loses to SIMD, so success means retaining batching **and** meeting the paired cohort gate.

## 26. Proposed CI and acceptance gates

| Gate | Machine-checkable rule |
|---|---|
| Input validity | Default performance source preflight has zero error observations; every exclusion is named and coverage-linked |
| Denominator | Default suite is expected to become 744 IDs if 46 repair/replacement cases re-enter and the two Qt cases remain optional; publish the exact ID hash rather than relying only on this expected count |
| Completion equality | For each comparison, exact completed-ID sets match; current 698 CPU/SIMD target becomes both 698 after the three execution fixes |
| Receipt equality | Every target comparison row has `execution.status=completed`, requested=actual, full expected sample count, no fallback, terminal receipt |
| Correctness | Every measured row has successful value parity; matched-error parity cannot satisfy performance correctness |
| CPU public-call | Every paired workload `Pillow median / CPU median >=1.00`; setup/pipeline/terminal/total remain symmetric |
| CPU diagnostic | Direct-core regressions are separately gated without replacing the public-call contract |
| SIMD material no-regression | On the versioned 175-style material cohort, both `CPU/SIMD >=1.00` and `Pillow/SIMD >=1.00` per workload |
| SIMD material significance | Both ratios `>=1.25` per workload; always publish 1.10/1.20/1.25/1.50 sensitivity |
| GPU large no-regression | Candidate definition: width and height ≥256 and chain≥2 or neighborhood/geometry; `SIMD/GPU >1.00` per workload |
| GPU practical | Same paired actual-receipt cohort `SIMD/GPU >=1.20` per workload, warm and cold reported separately |
| Variability | At least 20 warm samples and 10 independent cold starts for release evidence; report median, p95, standard deviation and p95/median; no budget passes on missing samples |
| Suite aggregation | Comparison carries common ID list/hash/count and exclusions; empty or <2-member common sets are `not_comparable` |
| Failure accounting | Failed/unsupported/partial/missing-receipt/parity/infrastructure counts are mutually distinguishable and reconcile to selected counts |
| FFI/core boundary | Existing no-runtime-FFI and thin-wrapper gates stay unchanged; no backend fix moves algorithmic work into bindings |

These are deliberately strict translations of the request. If CI stability requires statistical tolerance, add repeated-run confidence bounds without lowering the nominal speedup threshold or deleting violating IDs.

## 27. Prioritized implementation roadmap

| Order | Work item | Expected count impact | Likely files | Implementation / parity / performance risk | Focused / broad targets | Definition of done |
|---:|---|---|---|---|---|---|
| 1 | Input validity + excluded inventory | Repair/replacement candidate +46 default successes; keep 2 Qt optional; eliminate current 48 all-subject timing failures from default execution | generator, checker, regenerated inputs | medium / medium / low | fixtures-check, exact cases / all-backends | Every default input yields a Pillow value; exclusions named and coverage-linked |
| 2 | Result/error/receipt semantics | No execution recovery; makes all 265 raw failures diagnosable and three partial receipts truthful | parity runner, benchmark runner, validator, reports | medium / low / low | anomaly six / full benchmark reports | Errors persisted; partial terminal state impossible to label completed |
| 3 | Equal-subset suite reporting | Repairs validity of 10/54 suites; loaded-ten becomes explicitly not comparable until coverage fixed | benchmark runner/reporters | medium / none / medium | reporter tests / pipeline report | Every ratio has a common-set hash/count or is not comparable |
| 4 | CPU/SIMD completion mismatch | CPU +1, SIMD +2; current retained sets both 698; symmetric difference 0 | CPU pool, SIMD adapters, shared plan/tests | medium / medium / low | three exact IDs / SIMD strict + all-backends | Exact bytes; actual receipts; no fallback; equal ID set |
| 5 | Three GPU receipt anomalies | Execution-state correction for 3; capability completion waits on steps 6–7 | runners plus Thumbnail/mode planner | medium / low / none | three IDs / full benchmark | Partial receipts retained but never count as terminal performance |
| 6 | Shared GPU Draw foundation | Up to 27 GPU rows | registry, GPU pool, draw WGSL/tests | high / very high / medium | primitive cases / image parity + all-backends | Entire draw family exact; long chains terminal actual GPU |
| 7 | Rotate + mode-transition planner | Up to 16 GPU rows | shared planner, registry, GPU pool, rotate shader | high / high / medium | rotate sizes + five chains / full GPU matrix | 11 Rotate +5 routing rows complete with exact parity |
| 8 | Remaining deterministic GPU capability | Thumbnail/Fit 5, rank routing/work 5, Transform 4, LUT 3, Merge 2, F Resize 1 = up to 20 | registry/GPU pool/shaders | high / high / high | family matrices / full GPU matrix | Each family exact; work guards remain safe |
| 9 | RNG GPU decision | Up to 7 | registry/GPU pool/new control/shaders | very high / very high / high | deterministic seeds / full matrix | Exact Pillow sequence semantics or explicit unsupported decision remains visible |
| 10 | Equal backend coverage | All declared backend-eligible computational IDs share terminal actual receipts | runners/reports plus prior capability work | medium / high / medium | coverage report / full standard benchmark | Set-equality and receipt gates green; no fallback |
| 11 | CPU and SIMD optimization | Resolve 270 CPU/Pillow, 319 SIMD/CPU and 146 material-significance violations | CPU/SIMD pools, allocators, planners | high / high / high | direct-core profiles / full benchmark | Every applicable per-workload budget passes; crossover documented |
| 12 | GPU performance optimization | Resolve 104/108 candidate-large practical violations | GPU pool/shaders/caches/batching | high / high / very high | profile-all / full benchmark | GPU >1.0 and ≥1.2 paired gates pass cold/warm; no occupancy claim without counters |

Each item is independently scoppable only after its dependencies. In particular, do not optimize suite aggregates before equal-set reporting, and do not claim GPU performance before equal capability/receipt coverage.

## 28. Exact verification commands

Initial interface and generator checks:

```sh
make help
make migration-parity-fixtures-check
make migration-parity-test
make migration-parity-test-all-backends
```

The legacy `make image-backend-parity-test` alias is archived in the current
Makefile and intentionally exits nonzero; it is not an acceptance gate. Use
the active migration-parity targets above.

Focused CPU/SIMD reproductions, preserving authoritative outputs:

```sh
MIGRATION_BENCHMARK_OUTPUT=/tmp/audit-loaded-rgba.json \
MIGRATION_BENCHMARK_PARITY_OUTPUT=/tmp/audit-loaded-rgba-parity.json \
MIGRATION_BENCHMARK_ARGS='--workload-id pipeline-chain.loaded-10.rgba-png-512x384' \
make migration-parity-benchmark

MIGRATION_BENCHMARK_OUTPUT=/tmp/audit-rotate-1x1.json \
MIGRATION_BENCHMARK_PARITY_OUTPUT=/tmp/audit-rotate-1x1-parity.json \
MIGRATION_BENCHMARK_ARGS='--workload-id pipeline-matrix.expanded.rotate.1x1' \
make migration-parity-benchmark

MIGRATION_BENCHMARK_OUTPUT=/tmp/audit-add-1x1.json \
MIGRATION_BENCHMARK_PARITY_OUTPUT=/tmp/audit-add-1x1-parity.json \
MIGRATION_BENCHMARK_ARGS='--workload-id pipeline-matrix.expanded.add.1x1' \
make migration-parity-benchmark
```

Focused receipt anomalies:

```sh
MIGRATION_BENCHMARK_OUTPUT=/tmp/audit-receipts.json \
MIGRATION_BENCHMARK_PARITY_OUTPUT=/tmp/audit-receipts-parity.json \
MIGRATION_BENCHMARK_ARGS='--workload-id pipeline-op.thumbnail.benchmark-materialized --workload-id pipeline-op.thumbnail.matrix-32x24 --workload-id pipeline-chain.matrix-021' \
make migration-parity-benchmark
```

The exact per-input and per-GPU commands are `VB` and `FG` in sections 8 and 12: substitute that row's literal workload ID and use a unique `/tmp` slug. GPU commands run outside a Metal-blocking sandbox.

Full acceptance sequence:

```sh
make migration-parity-test-simd-strict
make migration-parity-test
make migration-parity-test-all-backends
MIGRATION_BENCHMARK_PROFILE=pipeline make migration-parity-benchmark
make migration-parity-pipeline-benchmark-coverage
make migration-parity-pipeline-report
make migration-parity-pipeline-core-benchmark
make migration-parity-profile-all
make test-all
make fmt
make clippy
make repo-map-check
```

Bounded repeated profile:

```sh
MIGRATION_PROFILE_WORKLOAD_ID=pipeline.quick.gaussianblur-invert.rgb-1024 \
MIGRATION_PROFILE_REPEAT=40 \
make migration-parity-profile-all
```

There is no maintained Metal occupancy/PMU target today. Add one before making a hardware utilization claim; `/usr/bin/sample` is insufficient.

## 29. Risks and unresolved questions

- The benchmark was produced from a dirty worktree. It is authoritative for that exact state but not a clean-release baseline.
- The result schema discards timed error text. Rows marked `E0` have an exact next probe; their root operation is high-confidence or confirmed by focused stderr/code guards, but the missing durable error remains an evidence defect.
- SIMD/GPU timed exceptions for many of the 48 parity-backed public cases are absent. Matching CPU/Pillow errors do not prove identical target-profile failure details.
- Matrix-009 targets complete where Pillow returns `None`. After fixing the generator's value flow, a direct return-value parity case must decide whether Rust has a public return-contract defect.
- Sixty-nine of 70 GPU failures are benchmark-only successful-execution gates, not byte-parity cases. New capability needs exact parity inputs before being trusted.
- Long workflows currently expose only the last/partial receipt in some paths; median operation count 1 is not proof that a ten-operation chain executed only one operation.
- RNG-parallel GPU implementations may be fundamentally unattractive if exact sequential Pillow ordering is mandatory. An explicit unsupported decision is preferable to approximate semantics mislabeled parity.
- Focused GaussianBlur+Invert and the standard benchmark disagree in ordering. Run order, cache/device state, profiling overhead, and system load are uncontrolled; repeated randomized paired trials are needed.
- Cold cohorts currently have only three samples. They are directional evidence, not a stable p95 contract.
- Host metadata records CPU only as `arm`, memory as zero, and power mode unknown. Future release evidence should persist exact chip/core/memory/power/thermal identity.
- No GPU occupancy, bandwidth, or utilization counters exist. All such claims remain unresolved.
- Optional Qt success depends on an explicitly provisioned environment and should never change the default denominator implicitly.
- Completion equality after input repair depends on the final named exclusion inventory. The expected 744 default count is a planning number; the canonical gate is the exact ID hash.

## 30. Appendices containing every workload ID


Generated read-only from `build/migration-parity/benchmark-result.json` (authoritative run `migration-benchmark-a613c94cd8d240e4b7412d576e760331`). Ratios use the workload's median whole-workflow `latency` measurement and the speedup convention `baseline_ms / candidate_ms`; a value below 1.0 is a slowdown.

### 30.1 Receipt and cohort rules

A target timing is admitted as an **actual-backend receipt** only when all of these artifact fields agree: subject `status == "completed"`; `execution.status == "completed"`; `execution.actual_backend` equals the requested backend; `fallback_reason_counts` is empty; and exactly one latency measurement exists. The Pillow side must have `status == "completed"` and a latency measurement. These filters intentionally reject the artifact's partial-receipt anomalies. They establish that the timed target backend actually ran; they do not upgrade a workload-wide `correctness.outcome == "not_proven"` caused by a different subject.

### 30.2 Complete ordered workload inventory (746/746)

The order below is the artifact's array order. Signature is `P/C/S/G` for Pillow, CPU, SIMD, and GPU subject status; `C` means completed and `F` failed. Correctness is `PASS` or `NP` (not proven). Ordinals are continuous and provide an independent count check.


#### pil-image

001. `pil-image.alpha-composite.standard` — F/F/F/F; NP
002. `pil-image.blend.standard` — C/C/C/C; PASS
003. `pil-image.composite.standard` — F/F/F/F; NP
004. `pil-image.effect-mandelbrot.standard` — F/F/F/F; NP
005. `pil-image.effect-noise.standard` — C/C/C/C; PASS
006. `pil-image.eval.standard` — F/F/F/F; NP
007. `pil-image.fromarray.standard` — F/F/F/F; NP
008. `pil-image.frombuffer.standard` — F/F/F/F; NP
009. `pil-image.frombytes.standard` — F/F/F/F; NP
010. `pil-image.linear-gradient.standard` — F/F/F/F; NP
011. `pil-image.merge.standard` — C/C/C/C; PASS
012. `pil-image.new.standard` — C/C/C/C; PASS
013. `pil-image.open.standard` — C/C/C/C; PASS
014. `pil-image.radial-gradient.standard` — F/F/F/F; NP

#### pil-image-image

015. `pil-image-image.alpha-composite.standard` — F/F/F/F; NP
016. `pil-image-image.apply-transparency.standard` — C/C/C/C; PASS
017. `pil-image-image.close.standard` — C/C/C/C; PASS
018. `pil-image-image.convert.standard` — C/C/C/C; PASS
019. `pil-image-image.copy.standard` — C/C/C/C; PASS
020. `pil-image-image.crop.standard` — C/C/C/C; PASS
021. `pil-image-image.draft.standard` — C/C/C/C; PASS
022. `pil-image-image.effect-spread.standard` — C/C/C/C; PASS
023. `pil-image-image.entropy.standard` — C/C/C/C; PASS
024. `pil-image-image.filter.standard` — C/C/C/C; PASS
025. `pil-image-image.format.standard` — C/C/C/C; PASS
026. `pil-image-image.frombytes.standard` — F/F/F/F; NP
027. `pil-image-image.get-child-images.standard` — C/C/C/C; PASS
028. `pil-image-image.get-flattened-data.standard` — C/C/C/C; PASS
029. `pil-image-image.getbands.standard` — C/C/C/C; PASS
030. `pil-image-image.getbbox.standard` — C/C/C/C; PASS
031. `pil-image-image.getchannel.standard` — C/C/C/C; PASS
032. `pil-image-image.getcolors.standard` — C/C/C/C; PASS
033. `pil-image-image.getdata.standard` — C/C/C/C; PASS
034. `pil-image-image.getexif.standard` — C/C/C/C; PASS
035. `pil-image-image.getextrema.standard` — C/C/C/C; PASS
036. `pil-image-image.getim.standard` — C/C/C/C; PASS
037. `pil-image-image.getpalette.standard` — C/C/C/C; PASS
038. `pil-image-image.getpixel.standard` — C/C/C/C; PASS
039. `pil-image-image.getprojection.standard` — C/C/C/C; PASS
040. `pil-image-image.getxmp.standard` — C/C/C/C; PASS
041. `pil-image-image.has-transparency-data.standard` — C/C/C/C; PASS
042. `pil-image-image.height.standard` — C/C/C/C; PASS
043. `pil-image-image.histogram.standard` — C/C/C/C; PASS
044. `pil-image-image.info.standard` — C/C/C/C; PASS
045. `pil-image-image.load.standard` — C/C/C/C; PASS
046. `pil-image-image.mode.standard` — C/C/C/C; PASS
047. `pil-image-image.paste.standard` — C/C/C/C; PASS
048. `pil-image-image.point.standard` — F/F/F/F; NP
049. `pil-image-image.putalpha.standard` — F/F/F/F; NP
050. `pil-image-image.putdata.standard` — F/F/F/F; NP
051. `pil-image-image.putpalette.standard` — F/F/F/F; NP
052. `pil-image-image.putpixel.standard` — C/C/C/C; PASS
053. `pil-image-image.quantize.standard` — C/C/C/C; PASS
054. `pil-image-image.reduce.standard` — C/C/C/C; PASS
055. `pil-image-image.remap-palette.standard` — F/F/F/F; NP
056. `pil-image-image.resize.standard` — C/C/C/C; PASS
057. `pil-image-image.rotate.standard` — C/C/C/C; PASS
058. `pil-image-image.save.standard` — F/F/F/F; NP
059. `pil-image-image.seek.standard` — F/F/F/F; NP
060. `pil-image-image.size.standard` — C/C/C/C; PASS
061. `pil-image-image.split.standard` — C/C/C/C; PASS
062. `pil-image-image.tell.standard` — C/C/C/C; PASS
063. `pil-image-image.thumbnail.standard` — C/C/C/C; PASS
064. `pil-image-image.tobitmap.standard` — F/F/F/F; NP
065. `pil-image-image.tobytes.standard` — C/C/C/C; PASS
066. `pil-image-image.toqimage.standard` — F/F/F/F; NP
067. `pil-image-image.toqpixmap.standard` — F/F/F/F; NP
068. `pil-image-image.transform.standard` — F/F/F/F; NP
069. `pil-image-image.transpose.standard` — C/C/C/C; PASS
070. `pil-image-image.verify.standard` — C/C/C/C; PASS
071. `pil-image-image.width.standard` — C/C/C/C; PASS

#### pil-imagechops

072. `pil-imagechops.add.standard` — C/C/C/C; PASS
073. `pil-imagechops.add-modulo.standard` — C/C/C/C; PASS
074. `pil-imagechops.blend.standard` — C/C/C/C; PASS
075. `pil-imagechops.composite.standard` — F/F/F/F; NP
076. `pil-imagechops.constant.standard` — C/C/C/C; PASS
077. `pil-imagechops.darker.standard` — C/C/C/C; PASS
078. `pil-imagechops.difference.standard` — C/C/C/C; PASS
079. `pil-imagechops.duplicate.standard` — C/C/C/C; PASS
080. `pil-imagechops.hard-light.standard` — C/C/C/C; PASS
081. `pil-imagechops.invert.standard` — C/C/C/C; PASS
082. `pil-imagechops.lighter.standard` — C/C/C/C; PASS
083. `pil-imagechops.logical-and.standard` — F/F/F/F; NP
084. `pil-imagechops.logical-or.standard` — F/F/F/F; NP
085. `pil-imagechops.logical-xor.standard` — F/F/F/F; NP
086. `pil-imagechops.multiply.standard` — C/C/C/C; PASS
087. `pil-imagechops.offset.standard` — C/C/C/C; PASS
088. `pil-imagechops.overlay.standard` — C/C/C/C; PASS
089. `pil-imagechops.screen.standard` — C/C/C/C; PASS
090. `pil-imagechops.soft-light.standard` — C/C/C/C; PASS
091. `pil-imagechops.subtract.standard` — C/C/C/C; PASS
092. `pil-imagechops.subtract-modulo.standard` — C/C/C/C; PASS

#### pil-imagecolor

093. `pil-imagecolor.getcolor.standard` — C/C/C/C; PASS
094. `pil-imagecolor.getrgb.standard` — C/C/C/C; PASS

#### pil-imagedraw

095. `pil-imagedraw.draw.standard` — C/C/C/C; PASS
096. `pil-imagedraw.outline.standard` — C/C/C/C; PASS

#### pil-imagedraw-imagedraw

097. `pil-imagedraw-imagedraw.arc.standard` — F/F/F/F; NP
098. `pil-imagedraw-imagedraw.bitmap.standard` — F/F/F/F; NP
099. `pil-imagedraw-imagedraw.chord.standard` — F/F/F/F; NP
100. `pil-imagedraw-imagedraw.circle.standard` — C/C/C/C; PASS
101. `pil-imagedraw-imagedraw.ellipse.standard` — F/F/F/F; NP
102. `pil-imagedraw-imagedraw.getfont.standard` — C/C/C/C; PASS
103. `pil-imagedraw-imagedraw.line.standard` — C/C/C/C; PASS
104. `pil-imagedraw-imagedraw.multiline-text.standard` — C/C/C/C; PASS
105. `pil-imagedraw-imagedraw.multiline-textbbox.standard` — C/C/C/C; PASS
106. `pil-imagedraw-imagedraw.pieslice.standard` — F/F/F/F; NP
107. `pil-imagedraw-imagedraw.point.standard` — C/C/C/C; PASS
108. `pil-imagedraw-imagedraw.polygon.standard` — F/F/F/F; NP
109. `pil-imagedraw-imagedraw.rectangle.standard` — F/F/F/F; NP
110. `pil-imagedraw-imagedraw.regular-polygon.standard` — F/F/F/F; NP
111. `pil-imagedraw-imagedraw.rounded-rectangle.standard` — F/F/F/F; NP
112. `pil-imagedraw-imagedraw.shape.standard` — C/C/C/F; NP
113. `pil-imagedraw-imagedraw.text.standard` — C/C/C/C; PASS
114. `pil-imagedraw-imagedraw.textbbox.standard` — C/C/C/C; PASS
115. `pil-imagedraw-imagedraw.textlength.standard` — C/C/C/C; PASS

#### pil-imageenhance

116. `pil-imageenhance.brightness.standard` — C/C/C/C; PASS
117. `pil-imageenhance.color.standard` — C/C/C/C; PASS
118. `pil-imageenhance.contrast.standard` — C/C/C/C; PASS
119. `pil-imageenhance.sharpness.standard` — C/C/C/C; PASS

#### pil-imageenhance-brightness

120. `pil-imageenhance-brightness.enhance.standard` — C/C/C/C; PASS

#### pil-imageenhance-color

121. `pil-imageenhance-color.enhance.standard` — C/C/C/C; PASS

#### pil-imageenhance-contrast

122. `pil-imageenhance-contrast.enhance.standard` — C/C/C/C; PASS

#### pil-imageenhance-sharpness

123. `pil-imageenhance-sharpness.enhance.standard` — C/C/C/C; PASS

#### pil-imagefilter

124. `pil-imagefilter.blur.standard` — C/C/C/C; PASS
125. `pil-imagefilter.boxblur.standard` — C/C/C/C; PASS
126. `pil-imagefilter.contour.standard` — C/C/C/C; PASS
127. `pil-imagefilter.color3dlut.standard` — C/C/C/C; PASS
128. `pil-imagefilter.detail.standard` — C/C/C/C; PASS
129. `pil-imagefilter.edge-enhance.standard` — C/C/C/C; PASS
130. `pil-imagefilter.edge-enhance-more.standard` — C/C/C/C; PASS
131. `pil-imagefilter.emboss.standard` — C/C/C/C; PASS
132. `pil-imagefilter.find-edges.standard` — C/C/C/C; PASS
133. `pil-imagefilter.gaussianblur.standard` — C/C/C/C; PASS
134. `pil-imagefilter.kernel.standard` — C/C/C/C; PASS
135. `pil-imagefilter.maxfilter.standard` — C/C/C/C; PASS
136. `pil-imagefilter.medianfilter.standard` — C/C/C/C; PASS
137. `pil-imagefilter.minfilter.standard` — C/C/C/C; PASS
138. `pil-imagefilter.modefilter.standard` — C/C/C/C; PASS
139. `pil-imagefilter.rankfilter.standard` — C/C/C/C; PASS
140. `pil-imagefilter.sharpen.standard` — C/C/C/C; PASS
141. `pil-imagefilter.smooth.standard` — C/C/C/C; PASS
142. `pil-imagefilter.smooth-more.standard` — C/C/C/C; PASS
143. `pil-imagefilter.unsharpmask.standard` — C/C/C/C; PASS

#### pil-imagefilter-color3dlut

144. `pil-imagefilter-color3dlut.repr.standard` — C/C/C/C; PASS
145. `pil-imagefilter-color3dlut.generate.standard` — C/C/C/C; PASS
146. `pil-imagefilter-color3dlut.transform.standard` — C/C/C/C; PASS

#### pil-imagefont

147. `pil-imagefont.freetypefont.standard` — C/C/C/C; PASS
148. `pil-imagefont.imagefont.standard` — C/C/C/C; PASS
149. `pil-imagefont.transposedfont.standard` — C/C/C/C; PASS
150. `pil-imagefont.load.standard` — C/C/C/C; PASS
151. `pil-imagefont.load-default.standard` — C/C/C/C; PASS
152. `pil-imagefont.load-default-imagefont.standard` — C/C/C/C; PASS
153. `pil-imagefont.load-path.standard` — C/C/C/C; PASS
154. `pil-imagefont.truetype.standard` — C/C/C/C; PASS

#### pil-imagefont-freetypefont

155. `pil-imagefont-freetypefont.font-variant.standard` — C/C/C/C; PASS
156. `pil-imagefont-freetypefont.get-variation-axes.standard` — F/F/F/F; NP
157. `pil-imagefont-freetypefont.get-variation-names.standard` — F/F/F/F; NP
158. `pil-imagefont-freetypefont.getbbox.standard` — C/C/C/C; PASS
159. `pil-imagefont-freetypefont.getlength.standard` — C/C/C/C; PASS
160. `pil-imagefont-freetypefont.getmask.standard` — C/C/C/C; PASS
161. `pil-imagefont-freetypefont.getmask2.standard` — C/C/C/C; PASS
162. `pil-imagefont-freetypefont.getmetrics.standard` — C/C/C/C; PASS
163. `pil-imagefont-freetypefont.getname.standard` — C/C/C/C; PASS
164. `pil-imagefont-freetypefont.set-variation-by-axes.standard` — F/F/F/F; NP
165. `pil-imagefont-freetypefont.set-variation-by-name.standard` — F/F/F/F; NP

#### pil-imagefont-imagefont

166. `pil-imagefont-imagefont.getbbox.standard` — F/F/F/F; NP
167. `pil-imagefont-imagefont.getlength.standard` — F/F/F/F; NP
168. `pil-imagefont-imagefont.getmask.standard` — F/F/F/F; NP

#### pil-imagefont-transposedfont

169. `pil-imagefont-transposedfont.getbbox.standard` — C/C/C/C; PASS
170. `pil-imagefont-transposedfont.getlength.standard` — C/C/C/C; PASS
171. `pil-imagefont-transposedfont.getmask.standard` — C/C/C/C; PASS

#### pil-imageops

172. `pil-imageops.autocontrast.standard` — C/C/C/C; PASS
173. `pil-imageops.colorize.standard` — F/F/F/F; NP
174. `pil-imageops.contain.standard` — C/C/C/C; PASS
175. `pil-imageops.cover.standard` — C/C/C/C; PASS
176. `pil-imageops.crop.standard` — C/C/C/C; PASS
177. `pil-imageops.deform.standard` — C/C/C/C; PASS
178. `pil-imageops.equalize.standard` — C/C/C/C; PASS
179. `pil-imageops.exif-transpose.standard` — C/C/C/C; PASS
180. `pil-imageops.expand.standard` — C/C/C/C; PASS
181. `pil-imageops.fit.standard` — C/C/C/C; PASS
182. `pil-imageops.flip.standard` — C/C/C/C; PASS
183. `pil-imageops.grayscale.standard` — C/C/C/C; PASS
184. `pil-imageops.invert.standard` — C/C/C/C; PASS
185. `pil-imageops.mirror.standard` — C/C/C/C; PASS
186. `pil-imageops.pad.standard` — C/C/C/C; PASS
187. `pil-imageops.posterize.standard` — C/C/C/C; PASS
188. `pil-imageops.scale.standard` — C/C/C/C; PASS
189. `pil-imageops.solarize.standard` — C/C/C/C; PASS

#### pil-imagepalette

190. `pil-imagepalette.imagepalette.standard` — C/C/C/C; PASS

#### pil-imagepalette-imagepalette

191. `pil-imagepalette-imagepalette.copy.standard` — C/C/C/C; PASS
192. `pil-imagepalette-imagepalette.getcolor.standard` — F/F/F/F; NP
193. `pil-imagepalette-imagepalette.getdata.standard` — C/C/C/C; PASS
194. `pil-imagepalette-imagepalette.save.standard` — C/C/C/C; PASS
195. `pil-imagepalette-imagepalette.tobytes.standard` — C/C/C/C; PASS

#### pil-imagesequence

196. `pil-imagesequence.iterator.standard` — C/C/C/C; PASS

#### pil-imagesequence-iterator

197. `pil-imagesequence-iterator.iter.standard` — C/C/C/C; PASS
198. `pil-imagesequence-iterator.next.standard` — F/F/F/F; NP

#### pil-imagestat

199. `pil-imagestat.stat.standard` — C/C/C/C; PASS

#### pil-imagestat-stat

200. `pil-imagestat-stat.count.standard` — C/C/C/C; PASS
201. `pil-imagestat-stat.extrema.standard` — C/C/C/C; PASS
202. `pil-imagestat-stat.mean.standard` — C/C/C/C; PASS
203. `pil-imagestat-stat.median.standard` — C/C/C/C; PASS
204. `pil-imagestat-stat.rms.standard` — C/C/C/C; PASS
205. `pil-imagestat-stat.stddev.standard` — C/C/C/C; PASS
206. `pil-imagestat-stat.sum.standard` — C/C/C/C; PASS
207. `pil-imagestat-stat.sum2.standard` — C/C/C/C; PASS
208. `pil-imagestat-stat.var.standard` — C/C/C/C; PASS

#### pipeline-op

209. `pipeline-op.resize.benchmark-materialized` — C/C/C/C; PASS
210. `pipeline-op.crop.benchmark-materialized` — C/C/C/C; PASS
211. `pipeline-op.rotate.benchmark-materialized` — C/C/C/F; NP
212. `pipeline-op.transpose.benchmark-materialized` — C/C/C/C; PASS
213. `pipeline-op.thumbnail.benchmark-materialized` — C/C/C/F; NP
214. `pipeline-op.reduce.benchmark-materialized` — C/C/C/C; PASS
215. `pipeline-op.convert.benchmark-materialized` — C/C/C/C; PASS
216. `pipeline-op.quantize.benchmark-materialized` — C/C/C/C; PASS
217. `pipeline-op.remappalette.benchmark-materialized` — C/C/C/C; PASS
218. `pipeline-op.filter3x3.benchmark-materialized` — C/C/C/C; PASS
219. `pipeline-op.filter5x5.benchmark-materialized` — C/C/C/C; PASS
220. `pipeline-op.gaussianblur.benchmark-materialized` — C/C/C/C; PASS
221. `pipeline-op.boxblur.benchmark-materialized` — C/C/C/C; PASS
222. `pipeline-op.medianfilter.benchmark-materialized` — C/C/C/C; PASS
223. `pipeline-op.maxfilter.benchmark-materialized` — C/C/C/C; PASS
224. `pipeline-op.minfilter.benchmark-materialized` — C/C/C/C; PASS
225. `pipeline-op.rankfilter.benchmark-materialized` — C/C/C/F; NP
226. `pipeline-op.autocontrast.benchmark-materialized` — C/C/C/C; PASS
227. `pipeline-op.equalize.benchmark-materialized` — C/C/C/C; PASS
228. `pipeline-op.invert.benchmark-materialized` — C/C/C/C; PASS
229. `pipeline-op.flip.benchmark-materialized` — C/C/C/C; PASS
230. `pipeline-op.mirror.benchmark-materialized` — C/C/C/C; PASS
231. `pipeline-op.posterize.benchmark-materialized` — C/C/C/C; PASS
232. `pipeline-op.solarize.benchmark-materialized` — C/C/C/C; PASS
233. `pipeline-op.grayscale.benchmark-materialized` — C/C/C/C; PASS
234. `pipeline-op.colorize.benchmark-materialized` — C/C/C/C; PASS
235. `pipeline-op.contain.benchmark-materialized` — C/C/C/C; PASS
236. `pipeline-op.cover.benchmark-materialized` — C/C/C/C; PASS
237. `pipeline-op.fit.benchmark-materialized` — C/C/C/F; NP
238. `pipeline-op.pad.benchmark-materialized` — C/C/C/C; PASS
239. `pipeline-op.scale.benchmark-materialized` — C/C/C/C; PASS
240. `pipeline-op.expand.benchmark-materialized` — C/C/C/C; PASS
241. `pipeline-op.cropborder.benchmark-materialized` — C/C/C/C; PASS
242. `pipeline-op.add.benchmark-materialized` — C/C/C/C; PASS
243. `pipeline-op.subtract.benchmark-materialized` — C/C/C/C; PASS
244. `pipeline-op.multiply.benchmark-materialized` — C/C/C/C; PASS
245. `pipeline-op.screen.benchmark-materialized` — C/C/C/C; PASS
246. `pipeline-op.darker.benchmark-materialized` — C/C/C/C; PASS
247. `pipeline-op.lighter.benchmark-materialized` — C/C/C/C; PASS
248. `pipeline-op.difference.benchmark-materialized` — C/C/C/C; PASS
249. `pipeline-op.overlay.benchmark-materialized` — C/C/C/C; PASS
250. `pipeline-op.hardlight.benchmark-materialized` — C/C/C/C; PASS
251. `pipeline-op.softlight.benchmark-materialized` — C/C/C/C; PASS
252. `pipeline-op.addmodulo.benchmark-materialized` — C/C/C/C; PASS
253. `pipeline-op.subtractmodulo.benchmark-materialized` — C/C/C/C; PASS
254. `pipeline-op.logicaland.benchmark-materialized` — C/C/C/C; PASS
255. `pipeline-op.logicalor.benchmark-materialized` — C/C/C/C; PASS
256. `pipeline-op.logicalxor.benchmark-materialized` — C/C/C/C; PASS
257. `pipeline-op.constant.benchmark-materialized` — C/C/C/C; PASS
258. `pipeline-op.offset.benchmark-materialized` — C/C/C/C; PASS
259. `pipeline-op.blend.benchmark-materialized` — C/C/C/C; PASS
260. `pipeline-op.composite.benchmark-materialized` — C/C/C/C; PASS
261. `pipeline-op.duplicate.benchmark-materialized` — C/C/C/C; PASS
262. `pipeline-op.invertchops.benchmark-materialized` — C/C/C/C; PASS
263. `pipeline-op.brightness.benchmark-materialized` — C/C/C/C; PASS
264. `pipeline-op.contrast.benchmark-materialized` — C/C/C/C; PASS
265. `pipeline-op.colorsaturation.benchmark-materialized` — C/C/C/C; PASS
266. `pipeline-op.sharpness.benchmark-materialized` — C/C/C/C; PASS
267. `pipeline-op.effectspread.benchmark-materialized` — C/C/C/F; NP
268. `pipeline-op.paste.benchmark-materialized` — C/C/C/C; PASS
269. `pipeline-op.alphacomposite.benchmark-materialized` — C/C/C/C; PASS
270. `pipeline-op.merge.benchmark-materialized` — C/C/C/F; NP
271. `pipeline-op.blendmodule.benchmark-materialized` — C/C/C/C; PASS
272. `pipeline-op.compositemodule.benchmark-materialized` — C/C/C/C; PASS
273. `pipeline-op.eval.benchmark-materialized` — C/C/C/C; PASS
274. `pipeline-op.effectnoise.benchmark-materialized` — C/C/C/F; NP
275. `pipeline-op.pointop.benchmark-materialized` — C/C/C/C; PASS
276. `pipeline-op.color3dlut.benchmark-materialized` — C/C/C/F; NP
277. `pipeline-op.transform.benchmark-materialized` — C/C/C/C; PASS
278. `pipeline-op.putpixel.benchmark-materialized` — C/C/C/C; PASS
279. `pipeline-op.putdata.benchmark-materialized` — C/C/C/C; PASS
280. `pipeline-op.putalpha.benchmark-materialized` — C/C/C/C; PASS
281. `pipeline-op.putalphadata.benchmark-materialized` — C/C/C/C; PASS
282. `pipeline-op.extractband.benchmark-materialized` — C/C/C/C; PASS
283. `pipeline-op.lineargradient.benchmark-materialized` — C/C/C/C; PASS
284. `pipeline-op.radialgradient.benchmark-materialized` — C/C/C/C; PASS
285. `pipeline-op.effectmandelbrot.benchmark-materialized` — C/C/C/C; PASS
286. `pipeline-op.drawline.benchmark-materialized` — C/C/C/F; NP
287. `pipeline-op.drawrectangle.benchmark-materialized` — C/C/C/F; NP
288. `pipeline-op.drawroundedrect.benchmark-materialized` — C/C/C/F; NP
289. `pipeline-op.drawellipse.benchmark-materialized` — C/C/C/F; NP
290. `pipeline-op.drawcircle.benchmark-materialized` — C/C/C/F; NP
291. `pipeline-op.drawpolygon.benchmark-materialized` — C/C/C/F; NP
292. `pipeline-op.drawarc.benchmark-materialized` — C/C/C/F; NP
293. `pipeline-op.drawchord.benchmark-materialized` — C/C/C/F; NP
294. `pipeline-op.drawpieslice.benchmark-materialized` — C/C/C/F; NP
295. `pipeline-op.drawpoint.benchmark-materialized` — C/C/C/F; NP
296. `pipeline-op.resize.matrix-32x24` — C/C/C/C; PASS
297. `pipeline-op.crop.matrix-32x24` — C/C/C/C; PASS
298. `pipeline-op.rotate.matrix-32x24` — C/C/C/F; NP
299. `pipeline-op.transpose.matrix-32x24` — C/C/C/C; PASS
300. `pipeline-op.thumbnail.matrix-32x24` — C/C/C/F; NP
301. `pipeline-op.reduce.matrix-32x24` — C/C/C/C; PASS
302. `pipeline-op.convert.matrix-32x24` — C/C/C/C; PASS
303. `pipeline-op.quantize.matrix-32x24` — C/C/C/C; PASS
304. `pipeline-op.filter3x3.matrix-32x24` — C/C/C/C; PASS
305. `pipeline-op.filter5x5.matrix-32x24` — C/C/C/C; PASS
306. `pipeline-op.gaussianblur.matrix-32x24` — C/C/C/C; PASS
307. `pipeline-op.boxblur.matrix-32x24` — C/C/C/C; PASS
308. `pipeline-op.medianfilter.matrix-32x24` — C/C/C/C; PASS
309. `pipeline-op.maxfilter.matrix-32x24` — C/C/C/C; PASS
310. `pipeline-op.minfilter.matrix-32x24` — C/C/C/C; PASS
311. `pipeline-op.autocontrast.matrix-32x24` — C/C/C/C; PASS
312. `pipeline-op.equalize.matrix-32x24` — C/C/C/C; PASS
313. `pipeline-op.invert.matrix-32x24` — C/C/C/C; PASS
314. `pipeline-op.flip.matrix-32x24` — C/C/C/C; PASS
315. `pipeline-op.mirror.matrix-32x24` — C/C/C/C; PASS
316. `pipeline-op.posterize.matrix-32x24` — C/C/C/C; PASS
317. `pipeline-op.solarize.matrix-32x24` — C/C/C/C; PASS
318. `pipeline-op.grayscale.matrix-32x24` — C/C/C/C; PASS
319. `pipeline-op.colorize.matrix-32x24` — C/C/C/C; PASS
320. `pipeline-op.contain.matrix-32x24` — C/C/C/C; PASS
321. `pipeline-op.cover.matrix-32x24` — C/C/C/C; PASS
322. `pipeline-op.fit.matrix-32x24` — C/C/C/F; NP
323. `pipeline-op.pad.matrix-32x24` — C/C/C/C; PASS
324. `pipeline-op.scale.matrix-32x24` — C/C/C/C; PASS
325. `pipeline-op.expand.matrix-32x24` — C/C/C/C; PASS
326. `pipeline-op.cropborder.matrix-32x24` — C/C/C/C; PASS
327. `pipeline-op.add.matrix-32x24` — C/C/C/C; PASS
328. `pipeline-op.subtract.matrix-32x24` — C/C/C/C; PASS
329. `pipeline-op.multiply.matrix-32x24` — C/C/C/C; PASS
330. `pipeline-op.screen.matrix-32x24` — C/C/C/C; PASS
331. `pipeline-op.darker.matrix-32x24` — C/C/C/C; PASS
332. `pipeline-op.lighter.matrix-32x24` — C/C/C/C; PASS
333. `pipeline-op.difference.matrix-32x24` — C/C/C/C; PASS
334. `pipeline-op.overlay.matrix-32x24` — C/C/C/C; PASS
335. `pipeline-op.hardlight.matrix-32x24` — C/C/C/C; PASS
336. `pipeline-op.softlight.matrix-32x24` — C/C/C/C; PASS
337. `pipeline-op.addmodulo.matrix-32x24` — C/C/C/C; PASS
338. `pipeline-op.subtractmodulo.matrix-32x24` — C/C/C/C; PASS
339. `pipeline-op.constant.matrix-32x24` — C/C/C/C; PASS
340. `pipeline-op.offset.matrix-32x24` — C/C/C/C; PASS
341. `pipeline-op.blend.matrix-32x24` — C/C/C/C; PASS
342. `pipeline-op.composite.matrix-32x24` — C/C/C/C; PASS
343. `pipeline-op.duplicate.matrix-32x24` — C/C/C/C; PASS
344. `pipeline-op.invertchops.matrix-32x24` — C/C/C/C; PASS
345. `pipeline-op.brightness.matrix-32x24` — C/C/C/C; PASS
346. `pipeline-op.contrast.matrix-32x24` — C/C/C/C; PASS
347. `pipeline-op.colorsaturation.matrix-32x24` — C/C/C/C; PASS
348. `pipeline-op.sharpness.matrix-32x24` — C/C/C/C; PASS
349. `pipeline-op.effectspread.matrix-32x24` — C/C/C/F; NP
350. `pipeline-op.paste.matrix-32x24` — C/C/C/C; PASS
351. `pipeline-op.alphacomposite.matrix-32x24` — C/C/C/C; PASS
352. `pipeline-op.merge.matrix-32x24` — C/C/C/F; NP
353. `pipeline-op.blendmodule.matrix-32x24` — C/C/C/C; PASS
354. `pipeline-op.compositemodule.matrix-32x24` — C/C/C/C; PASS
355. `pipeline-op.eval.matrix-32x24` — C/C/C/C; PASS
356. `pipeline-op.pointop.matrix-32x24` — C/C/C/C; PASS
357. `pipeline-op.color3dlut.matrix-32x24` — C/C/C/F; NP
358. `pipeline-op.transform.matrix-32x24` — C/C/C/C; PASS
359. `pipeline-op.putdata.matrix-32x24` — C/C/C/C; PASS
360. `pipeline-op.putalpha.matrix-32x24` — C/C/C/C; PASS
361. `pipeline-op.putalphadata.matrix-32x24` — C/C/C/C; PASS
362. `pipeline-op.extractband.matrix-32x24` — C/C/C/C; PASS
363. `pipeline-op.drawline.matrix-32x24` — C/C/C/F; NP
364. `pipeline-op.drawrectangle.matrix-32x24` — C/C/C/F; NP
365. `pipeline-op.drawroundedrect.matrix-32x24` — C/C/C/F; NP
366. `pipeline-op.drawellipse.matrix-32x24` — C/C/C/F; NP
367. `pipeline-op.drawcircle.matrix-32x24` — C/C/C/F; NP
368. `pipeline-op.drawpolygon.matrix-32x24` — C/C/C/F; NP
369. `pipeline-op.drawarc.matrix-32x24` — C/C/C/F; NP
370. `pipeline-op.drawchord.matrix-32x24` — C/C/C/F; NP
371. `pipeline-op.drawpieslice.matrix-32x24` — C/C/C/F; NP
372. `pipeline-op.drawpoint.matrix-32x24` — C/C/C/F; NP

#### pipeline-matrix

373. `pipeline-matrix.expanded.resize.1x1` — C/C/C/C; PASS
374. `pipeline-matrix.expanded.resize.32x32` — C/C/C/C; PASS
375. `pipeline-matrix.expanded.resize.256x256` — C/C/C/C; PASS
376. `pipeline-matrix.expanded.resize.1024x768` — C/C/C/C; PASS
377. `pipeline-matrix.expanded.crop.1x1` — C/C/C/C; PASS
378. `pipeline-matrix.expanded.crop.32x32` — C/C/C/C; PASS
379. `pipeline-matrix.expanded.crop.256x256` — C/C/C/C; PASS
380. `pipeline-matrix.expanded.crop.1024x768` — C/C/C/C; PASS
381. `pipeline-matrix.expanded.rotate.1x1` — C/C/F/F; NP
382. `pipeline-matrix.expanded.rotate.32x32` — C/C/C/F; NP
383. `pipeline-matrix.expanded.rotate.256x256` — C/C/C/F; NP
384. `pipeline-matrix.expanded.rotate.1024x768` — C/C/C/F; NP
385. `pipeline-matrix.expanded.transpose.1x1` — C/C/C/C; PASS
386. `pipeline-matrix.expanded.transpose.32x32` — C/C/C/C; PASS
387. `pipeline-matrix.expanded.transpose.256x256` — C/C/C/C; PASS
388. `pipeline-matrix.expanded.transpose.1024x768` — C/C/C/C; PASS
389. `pipeline-matrix.expanded.reduce.1x1` — C/C/C/C; PASS
390. `pipeline-matrix.expanded.reduce.32x32` — C/C/C/C; PASS
391. `pipeline-matrix.expanded.reduce.256x256` — C/C/C/C; PASS
392. `pipeline-matrix.expanded.reduce.1024x768` — C/C/C/C; PASS
393. `pipeline-matrix.expanded.filter3x3.1x1` — C/C/C/C; PASS
394. `pipeline-matrix.expanded.filter3x3.32x32` — C/C/C/C; PASS
395. `pipeline-matrix.expanded.filter3x3.256x256` — C/C/C/C; PASS
396. `pipeline-matrix.expanded.filter3x3.1024x768` — C/C/C/C; PASS
397. `pipeline-matrix.expanded.filter5x5.1x1` — C/C/C/C; PASS
398. `pipeline-matrix.expanded.filter5x5.32x32` — C/C/C/C; PASS
399. `pipeline-matrix.expanded.filter5x5.256x256` — C/C/C/C; PASS
400. `pipeline-matrix.expanded.filter5x5.1024x768` — C/C/C/C; PASS
401. `pipeline-matrix.expanded.gaussianblur.1x1` — C/C/C/C; PASS
402. `pipeline-matrix.expanded.gaussianblur.32x32` — C/C/C/C; PASS
403. `pipeline-matrix.expanded.gaussianblur.256x256` — C/C/C/C; PASS
404. `pipeline-matrix.expanded.gaussianblur.1024x768` — C/C/C/C; PASS
405. `pipeline-matrix.expanded.boxblur.1x1` — C/C/C/C; PASS
406. `pipeline-matrix.expanded.boxblur.32x32` — C/C/C/C; PASS
407. `pipeline-matrix.expanded.boxblur.256x256` — C/C/C/C; PASS
408. `pipeline-matrix.expanded.boxblur.1024x768` — C/C/C/C; PASS
409. `pipeline-matrix.expanded.medianfilter.1x1` — C/C/C/C; PASS
410. `pipeline-matrix.expanded.medianfilter.32x32` — C/C/C/C; PASS
411. `pipeline-matrix.expanded.medianfilter.256x256` — C/C/C/C; PASS
412. `pipeline-matrix.expanded.medianfilter.1024x768` — C/C/C/F; NP
413. `pipeline-matrix.expanded.maxfilter.1x1` — C/C/C/C; PASS
414. `pipeline-matrix.expanded.maxfilter.32x32` — C/C/C/C; PASS
415. `pipeline-matrix.expanded.maxfilter.256x256` — C/C/C/C; PASS
416. `pipeline-matrix.expanded.maxfilter.1024x768` — C/C/C/C; PASS
417. `pipeline-matrix.expanded.minfilter.1x1` — C/C/C/C; PASS
418. `pipeline-matrix.expanded.minfilter.32x32` — C/C/C/C; PASS
419. `pipeline-matrix.expanded.minfilter.256x256` — C/C/C/C; PASS
420. `pipeline-matrix.expanded.minfilter.1024x768` — C/C/C/C; PASS
421. `pipeline-matrix.expanded.effectspread.1x1` — C/C/C/F; NP
422. `pipeline-matrix.expanded.effectspread.32x32` — C/C/C/F; NP
423. `pipeline-matrix.expanded.effectspread.256x256` — C/C/C/F; NP
424. `pipeline-matrix.expanded.effectspread.1024x768` — C/C/C/F; NP
425. `pipeline-matrix.expanded.invert.1x1` — C/C/C/C; PASS
426. `pipeline-matrix.expanded.invert.32x32` — C/C/C/C; PASS
427. `pipeline-matrix.expanded.invert.256x256` — C/C/C/C; PASS
428. `pipeline-matrix.expanded.invert.1024x768` — C/C/C/C; PASS
429. `pipeline-matrix.expanded.grayscale.1x1` — C/C/C/C; PASS
430. `pipeline-matrix.expanded.grayscale.32x32` — C/C/C/C; PASS
431. `pipeline-matrix.expanded.grayscale.256x256` — C/C/C/C; PASS
432. `pipeline-matrix.expanded.grayscale.1024x768` — C/C/C/C; PASS
433. `pipeline-matrix.expanded.autocontrast.1x1` — C/C/C/C; PASS
434. `pipeline-matrix.expanded.autocontrast.32x32` — C/C/C/C; PASS
435. `pipeline-matrix.expanded.autocontrast.256x256` — C/C/C/C; PASS
436. `pipeline-matrix.expanded.autocontrast.1024x768` — C/C/C/C; PASS
437. `pipeline-matrix.expanded.equalize.1x1` — C/C/C/C; PASS
438. `pipeline-matrix.expanded.equalize.32x32` — C/C/C/C; PASS
439. `pipeline-matrix.expanded.equalize.256x256` — C/C/C/C; PASS
440. `pipeline-matrix.expanded.equalize.1024x768` — C/C/C/C; PASS
441. `pipeline-matrix.expanded.convert.1x1` — C/C/C/C; PASS
442. `pipeline-matrix.expanded.convert.32x32` — C/C/C/C; PASS
443. `pipeline-matrix.expanded.convert.256x256` — C/C/C/C; PASS
444. `pipeline-matrix.expanded.convert.1024x768` — C/C/C/C; PASS
445. `pipeline-matrix.expanded.eval.1x1` — C/C/C/C; PASS
446. `pipeline-matrix.expanded.eval.32x32` — C/C/C/C; PASS
447. `pipeline-matrix.expanded.eval.256x256` — C/C/C/C; PASS
448. `pipeline-matrix.expanded.eval.1024x768` — C/C/C/C; PASS
449. `pipeline-matrix.expanded.pointop.1x1` — C/C/C/C; PASS
450. `pipeline-matrix.expanded.pointop.32x32` — C/C/C/C; PASS
451. `pipeline-matrix.expanded.pointop.256x256` — C/C/C/C; PASS
452. `pipeline-matrix.expanded.pointop.1024x768` — C/C/C/C; PASS
453. `pipeline-matrix.expanded.multiply.1x1` — C/C/C/C; PASS
454. `pipeline-matrix.expanded.multiply.32x32` — C/C/C/C; PASS
455. `pipeline-matrix.expanded.multiply.256x256` — C/C/C/C; PASS
456. `pipeline-matrix.expanded.multiply.1024x768` — C/C/C/C; PASS
457. `pipeline-matrix.expanded.screen.1x1` — C/C/C/C; PASS
458. `pipeline-matrix.expanded.screen.32x32` — C/C/C/C; PASS
459. `pipeline-matrix.expanded.screen.256x256` — C/C/C/C; PASS
460. `pipeline-matrix.expanded.screen.1024x768` — C/C/C/C; PASS
461. `pipeline-matrix.expanded.add.1x1` — C/C/F/C; NP
462. `pipeline-matrix.expanded.add.32x32` — C/C/C/C; PASS
463. `pipeline-matrix.expanded.add.256x256` — C/C/C/C; PASS
464. `pipeline-matrix.expanded.add.1024x768` — C/C/C/C; PASS
465. `pipeline-matrix.expanded.darker.1x1` — C/C/C/C; PASS
466. `pipeline-matrix.expanded.darker.32x32` — C/C/C/C; PASS
467. `pipeline-matrix.expanded.darker.256x256` — C/C/C/C; PASS
468. `pipeline-matrix.expanded.darker.1024x768` — C/C/C/C; PASS
469. `pipeline-matrix.expanded.brightness.1x1` — C/C/C/C; PASS
470. `pipeline-matrix.expanded.brightness.32x32` — C/C/C/C; PASS
471. `pipeline-matrix.expanded.brightness.256x256` — C/C/C/C; PASS
472. `pipeline-matrix.expanded.brightness.1024x768` — C/C/C/C; PASS

#### pipeline-chain

473. `pipeline-chain.matrix-000` — C/C/C/F; NP
474. `pipeline-chain.matrix-001` — C/C/C/C; PASS
475. `pipeline-chain.matrix-002` — C/C/C/C; PASS
476. `pipeline-chain.matrix-003` — C/C/C/C; PASS
477. `pipeline-chain.matrix-004` — C/C/C/F; NP
478. `pipeline-chain.matrix-005` — C/C/C/C; PASS
479. `pipeline-chain.matrix-006` — C/C/C/C; PASS
480. `pipeline-chain.matrix-007` — C/C/C/F; NP
481. `pipeline-chain.matrix-009` — F/F/F/F; NP
482. `pipeline-chain.matrix-010` — C/C/C/C; PASS
483. `pipeline-chain.matrix-011` — C/C/C/C; PASS
484. `pipeline-chain.matrix-012` — C/C/C/C; PASS
485. `pipeline-chain.matrix-013` — F/F/F/F; NP
486. `pipeline-chain.matrix-017` — C/C/C/F; NP
487. `pipeline-chain.matrix-018` — C/C/C/C; PASS
488. `pipeline-chain.matrix-020` — C/C/C/C; PASS
489. `pipeline-chain.matrix-021` — C/C/C/F; NP
490. `pipeline-chain.matrix-022` — C/C/C/F; NP
491. `pipeline-chain.matrix-023` — C/C/C/F; NP
492. `pipeline-chain.matrix-024` — C/C/C/F; NP
493. `pipeline-chain.matrix-025` — C/C/C/C; PASS
494. `pipeline-chain.matrix-030` — C/C/C/C; PASS
495. `pipeline-chain.matrix-031` — C/C/C/C; PASS
496. `pipeline-chain.matrix-032` — C/C/C/C; PASS
497. `pipeline-chain.matrix-033` — C/C/C/C; PASS
498. `pipeline-chain.matrix-034` — C/C/C/C; PASS
499. `pipeline-chain.matrix-036` — C/C/C/C; PASS
500. `pipeline-chain.matrix-037` — C/C/C/C; PASS
501. `pipeline-chain.matrix-038` — C/C/C/C; PASS
502. `pipeline-chain.matrix-039` — C/C/C/C; PASS
503. `pipeline-chain.matrix-040` — C/C/C/C; PASS
504. `pipeline-chain.matrix-041` — C/C/C/C; PASS
505. `pipeline-chain.matrix-042` — C/C/C/C; PASS
506. `pipeline-chain.matrix-043` — C/C/C/C; PASS
507. `pipeline-chain.matrix-045` — C/C/C/C; PASS
508. `pipeline-chain.matrix-054` — C/C/C/C; PASS
509. `pipeline-chain.matrix-057` — C/C/C/C; PASS
510. `pipeline-chain.matrix-058` — C/C/C/C; PASS
511. `pipeline-chain.matrix-059` — C/C/C/C; PASS
512. `pipeline-chain.matrix-066` — C/C/C/C; PASS
513. `pipeline-chain.matrix-067` — C/C/C/C; PASS
514. `pipeline-chain.matrix-068` — C/C/C/C; PASS
515. `pipeline-chain.matrix-069` — C/C/C/C; PASS
516. `pipeline-chain.matrix-070` — C/C/C/C; PASS
517. `pipeline-chain.matrix-071` — C/C/C/C; PASS
518. `pipeline-chain.matrix-072` — C/C/C/C; PASS
519. `pipeline-chain.matrix-073` — C/C/C/F; NP
520. `pipeline-chain.matrix-074` — C/C/C/F; NP
521. `pipeline-chain.matrix-075` — C/C/C/C; PASS
522. `pipeline-chain.matrix-076` — C/C/C/C; PASS
523. `pipeline-chain.matrix-077` — C/C/C/C; PASS
524. `pipeline-chain.matrix-078` — C/C/C/C; PASS
525. `pipeline-chain.matrix-079` — C/C/C/C; PASS
526. `pipeline-chain.matrix-081` — F/F/F/F; NP
527. `pipeline-chain.matrix-082` — C/C/C/F; NP
528. `pipeline-chain.matrix-083` — C/C/C/F; NP
529. `pipeline-chain.matrix-084` — C/C/C/F; NP
530. `pipeline-chain.matrix-085` — C/C/C/F; NP
531. `pipeline-chain.matrix-086` — C/C/C/C; PASS
532. `pipeline-chain.matrix-087` — C/C/C/C; PASS
533. `pipeline-chain.matrix-096` — C/C/C/C; PASS
534. `pipeline-chain.matrix-097` — C/C/C/C; PASS
535. `pipeline-chain.matrix-098` — C/C/C/C; PASS
536. `pipeline-chain.matrix-099` — C/C/C/C; PASS
537. `pipeline-chain.terminal-read.rgb-band0` — C/C/C/C; PASS
538. `pipeline-chain.terminal-read.analysis-suite.rgb` — C/C/C/C; PASS
539. `pipeline-chain.terminal-read.analysis-scalar-if-1024x768` — C/C/C/C; PASS
540. `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768` — C/C/C/C; PASS
541. `pipeline-chain.terminal-read.getcolors.rgb-1024x768` — C/C/C/C; PASS
542. `pipeline-chain.terminal-read.imagestat.i-1024x768` — C/C/C/C; PASS
543. `pipeline-chain.terminal-read.imagestat.cmyk-1024x768` — C/C/C/C; PASS
544. `pipeline-chain.color.convert-mode-l` — C/C/C/C; PASS
545. `pipeline-chain.color.convert-mode-la` — C/C/C/C; PASS
546. `pipeline-chain.color.convert-mode-1` — C/C/C/C; PASS
547. `pipeline-chain.color.convert-mode-p` — C/C/C/C; PASS
548. `pipeline-chain.color.convert-mode-cmyk` — C/C/C/C; PASS
549. `pipeline-chain.color.convert-mode-ycbcr` — C/C/C/C; PASS
550. `pipeline-chain.color.convert-mode-hsv` — C/C/C/C; PASS
551. `pipeline-chain.color.convert-mode-i` — C/C/C/C; PASS
552. `pipeline-chain.color.convert-mode-f` — C/C/C/C; PASS
553. `pipeline-chain.color.quantize-mode-l` — C/C/C/C; PASS
554. `pipeline-chain.color.quantize-mode-rgba` — C/C/C/C; PASS
555. `pipeline-chain.color.remap-palette-mode-l` — C/C/C/C; PASS
556. `pipeline-chain.color.remap-palette-mode-p` — C/C/C/C; PASS
557. `pipeline-chain.color.getchannel-mode-la` — C/C/C/C; PASS
558. `pipeline-chain.color.getchannel-mode-rgba` — C/C/C/C; PASS
559. `pipeline-chain.color.getchannel-mode-cmyk` — C/C/C/C; PASS
560. `pipeline-chain.color.getchannel-mode-ycbcr` — C/C/C/C; PASS
561. `pipeline-chain.quantize.linear-gradient` — C/C/C/C; PASS
562. `pipeline-chain.quantize.radial-gradient` — C/C/C/C; PASS
563. `pipeline-chain.quantize.algorithm.median-cut` — C/C/C/C; PASS
564. `pipeline-chain.quantize.algorithm.median-cut-kmeans` — C/C/C/C; PASS
565. `pipeline-chain.quantize.algorithm.maxcoverage` — C/C/C/C; PASS
566. `pipeline-chain.quantize.algorithm.maxcoverage-kmeans` — C/C/C/C; PASS
567. `pipeline-chain.quantize.algorithm.fast-octree` — C/C/C/C; PASS
568. `pipeline-chain.metadata-cache.invert-1.rgb` — C/C/C/C; PASS
569. `pipeline-chain.metadata-cache.invert-8.l` — C/C/C/C; PASS
570. `pipeline-chain.metadata-cache.invert-64.rgb` — C/C/C/C; PASS
571. `pipeline-chain.metadata-cache.color3dlut-rgb` — C/C/C/F; NP
572. `pipeline-chain.metadata-cache.extractband-rgba` — C/C/C/C; PASS
573. `pipeline-chain.rank-filter.large-f-9x9` — C/C/C/F; NP
574. `pipeline-chain.rank-filter.large-l-9x9` — C/C/C/C; PASS
575. `pipeline-chain.rank-filter.material.f-9x9-256x256` — C/C/C/F; NP
576. `pipeline-chain.rank-filter.material.l-9x9-256x256` — C/C/C/F; NP
577. `pipeline-chain.convolution.material.l-3x3-invert.256x256` — C/C/C/C; PASS
578. `pipeline-chain.convolution.material.rgb-3x3-mirror.256x256` — C/C/C/C; PASS
579. `pipeline-chain.convolution.material.la-3x3-alpha.256x256` — C/C/C/C; PASS
580. `pipeline-chain.convolution.material.l-5x5-scale.256x256` — C/C/C/C; PASS
581. `pipeline-chain.convolution.material.rgb-5x5-pad.256x256` — C/C/C/C; PASS
582. `pipeline-chain.convolution.material.rgba-3x3-transpose.256x256` — C/C/C/C; PASS
583. `pipeline-chain.convolution.crossover.l-3x3-invert.512x512` — C/C/C/C; PASS
584. `pipeline-chain.convolution.crossover.rgb-3x3-mirror.512x512` — C/C/C/C; PASS
585. `pipeline-chain.convolution.crossover.la-3x3-alpha.512x512` — C/C/C/C; PASS
586. `pipeline-chain.convolution.crossover.l-5x5-scale.512x512` — C/C/C/C; PASS
587. `pipeline-chain.convolution.native.l-3x3-invert.1024x768` — C/C/C/C; PASS
588. `pipeline-chain.convolution.native.rgb-3x3-mirror.1024x768` — C/C/C/C; PASS
589. `pipeline-chain.convolution.native.la-3x3-alpha.1024x768` — C/C/C/C; PASS
590. `pipeline-chain.convolution.native.rgba-3x3-transpose.1024x768` — C/C/C/C; PASS
591. `pipeline-chain.convolution.native.l-5x5-scale.1024x768` — C/C/C/C; PASS
592. `pipeline-chain.convolution.native.rgb-5x5-pad.1024x768` — C/C/C/C; PASS
593. `pipeline-chain.convolution.native.la-5x5-mirror.1024x768` — C/C/C/C; PASS
594. `pipeline-chain.convolution.native.rgba-5x5-invert.1024x768` — C/C/C/C; PASS
595. `pipeline-chain.convolution-i.3x3-1024x768` — C/C/C/C; PASS
596. `pipeline-chain.convolution-i.5x5-1024x768` — C/C/C/C; PASS
597. `pipeline-chain.reviewed.filter-invert-mirror` — C/C/C/C; PASS
598. `pipeline-chain.reviewed.point-solarize-posterize` — C/C/C/C; PASS
599. `pipeline-chain.reviewed.invert-solarize-posterize-point` — C/C/C/C; PASS
600. `pipeline-chain.reviewed.resize-rotate-crop` — C/C/C/F; NP
601. `pipeline-chain.reviewed.quantize-remap-convert` — C/C/C/C; PASS
602. `pipeline-chain.reviewed.multiply-screen-invert` — C/C/C/C; PASS
603. `pipeline-chain.reviewed.transpose-flip-resize` — C/C/C/C; PASS
604. `pipeline-chain.reviewed.crop-expand-mirror` — C/C/C/C; PASS
605. `pipeline-chain.reviewed.equalize-autocontrast-invert` — C/C/C/C; PASS
606. `pipeline-chain.reviewed.draw-filter-invert` — C/C/C/F; NP
607. `pipeline-chain.reviewed.radial-gradient-crop-resize` — C/C/C/C; PASS
608. `pipeline-chain.reviewed.draw-batch-rgb-shapes` — C/C/C/F; NP
609. `pipeline-chain.reviewed.draw-batch-rgba-alpha` — C/C/C/F; NP
610. `pipeline-chain.reviewed.convert-palette-rgb` — C/C/C/C; PASS
611. `pipeline-chain.reviewed.convert-p-no-palette-rgb` — C/C/C/C; PASS
612. `pipeline-chain.reviewed.convert-cmyk-1-no-dither` — C/C/C/C; PASS
613. `pipeline-chain.reviewed.convert-cmyk-1-dither` — C/C/C/C; PASS
614. `pipeline-chain.reviewed.convert-cmyk-i` — C/C/C/C; PASS
615. `pipeline-chain.reviewed.convert-cmyk-f` — C/C/C/C; PASS
616. `pipeline-chain.reviewed.convert-one-cmyk` — C/C/C/C; PASS
617. `pipeline-chain.reviewed.convert-cmyk-la` — C/C/C/C; PASS
618. `pipeline-chain.reviewed.convert-rgba-cmyk-la` — C/C/C/C; PASS
619. `pipeline-chain.reviewed.convert-cmyk-rgba` — C/C/C/C; PASS
620. `pipeline-chain.resize-alpha.rgba-lanczos-256x256` — C/C/C/C; PASS
621. `pipeline-chain.resize-alpha.la-bicubic-256x256` — C/C/C/C; PASS
622. `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256` — C/C/C/C; PASS
623. `pipeline-chain.resize-alpha.la-bilinear-mirror-256x256` — C/C/C/C; PASS
624. `pipeline-chain.resize-alpha.rgba-bicubic-512x512` — C/C/C/C; PASS
625. `pipeline-chain.resize-alpha.la-lanczos-512x512` — C/C/C/C; PASS
626. `pipeline-chain.resize-alpha.rgba-bilinear-upscale-128x128` — C/C/C/C; PASS
627. `pipeline-chain.resize-alpha.la-bicubic-upscale-128x128` — C/C/C/C; PASS
628. `pipeline-chain.resize-alpha.rgba-lanczos-1024x768` — C/C/C/C; PASS
629. `pipeline-chain.resize-alpha.la-bicubic-1024x768` — C/C/C/C; PASS
630. `pipeline-chain.resize-typed.simd-f-resize-transpose` — C/C/C/C; PASS
631. `pipeline-chain.resize-typed.simd-i-resize-transform` — C/C/C/C; PASS
632. `pipeline-chain.geometry-material.transpose-rgba-1024x768` — C/C/C/C; PASS
633. `pipeline-chain.geometry-material.transverse-rgb-1024x768` — C/C/C/C; PASS
634. `pipeline-chain.geometry-material.crop-rgb-1024x768` — C/C/C/C; PASS
635. `pipeline-chain.geometry-material.reduce-rgb-1024x768` — C/C/C/C; PASS
636. `pipeline-chain.geometry-material.reduce-rgba-1024x768` — C/C/C/C; PASS
637. `pipeline-chain.geometry-material.rotate-rgba-1024x768` — C/C/C/F; NP
638. `pipeline-chain.geometry-copy.crop-l-1024x768` — C/C/C/C; PASS
639. `pipeline-chain.geometry-copy.crop-la-1024x768` — C/C/C/C; PASS
640. `pipeline-chain.geometry-copy.crop-rgb-1024x768` — C/C/C/C; PASS
641. `pipeline-chain.geometry-copy.crop-rgba-1024x768` — C/C/C/C; PASS
642. `pipeline-chain.geometry-copy.cropborder-l-1024x768` — C/C/C/C; PASS
643. `pipeline-chain.geometry-copy.cropborder-la-1024x768` — C/C/C/C; PASS
644. `pipeline-chain.geometry-copy.cropborder-rgb-1024x768` — C/C/C/C; PASS
645. `pipeline-chain.geometry-copy.cropborder-rgba-1024x768` — C/C/C/C; PASS
646. `pipeline-chain.geometry-copy.crop-chain-rgb-1024x768` — C/C/C/C; PASS
647. `pipeline-chain.geometry-copy.cropborder-chain-rgba-1024x768` — C/C/C/C; PASS
648. `pipeline-chain.blur-material.gaussian-rgb-1024x1024` — C/C/C/C; PASS
649. `pipeline-chain.blur-material.box-rgb-1024x1024` — C/C/C/C; PASS
650. `pipeline-chain.blur-material.gaussian-rgba-1024x768` — C/C/C/C; PASS
651. `pipeline-chain.blur-material.gaussian-l-1024x768` — C/C/C/C; PASS
652. `pipeline-chain.blur-material.gaussian-la-1024x768` — C/C/C/C; PASS
653. `pipeline-chain.blur-material.gaussian-l-256x256-radius-0.5` — C/C/C/C; PASS
654. `pipeline-chain.blur-material.gaussian-rgb-256x256-radius-1` — C/C/C/C; PASS
655. `pipeline-chain.blur-material.gaussian-rgb-1024x768-radius-4` — C/C/C/C; PASS
656. `pipeline-chain.blur-material.gaussian-rgba-256x256-radius-2` — C/C/C/C; PASS
657. `pipeline-chain.blur-material.box-l-256x256-radius-0.5` — C/C/C/C; PASS
658. `pipeline-chain.blur-material.box-rgb-256x256-radius-1` — C/C/C/C; PASS
659. `pipeline-chain.blur-material.box-rgb-1024x768-radius-4` — C/C/C/C; PASS
660. `pipeline-chain.blur-material.box-l-1024x768-radius-4` — C/C/C/C; PASS
661. `pipeline-chain.blur-material.box-la-1024x768-radius-4` — C/C/C/C; PASS
662. `pipeline-chain.blur-material.box-rgba-1024x768-radius-4` — C/C/C/C; PASS
663. `pipeline-chain.blur-material.box-rgba-256x256-radius-2` — C/C/C/C; PASS
664. `pipeline-chain.point-fusion.l-001` — C/C/C/C; PASS
665. `pipeline-chain.point-fusion.l-002` — C/C/C/C; PASS
666. `pipeline-chain.point-fusion.l-003` — C/C/C/C; PASS
667. `pipeline-chain.point-fusion.l-004` — C/C/C/C; PASS
668. `pipeline-chain.point-fusion.l-005` — C/C/C/C; PASS
669. `pipeline-chain.point-fusion.rgb-001` — C/C/C/C; PASS
670. `pipeline-chain.point-fusion.rgb-002` — C/C/C/C; PASS
671. `pipeline-chain.point-fusion.rgb-003` — C/C/C/C; PASS
672. `pipeline-chain.point-fusion.rgb-004` — C/C/C/C; PASS
673. `pipeline-chain.point-fusion.rgb-005` — C/C/C/C; PASS
674. `pipeline-chain.point-fusion.la-001` — C/C/C/C; PASS
675. `pipeline-chain.point-fusion.la-002` — C/C/C/C; PASS
676. `pipeline-chain.point-fusion.rgba-001` — C/C/C/C; PASS
677. `pipeline-chain.point-fusion.rgba-002` — C/C/C/C; PASS
678. `pipeline-chain.alpha-composite.la-256x256` — C/C/C/C; PASS
679. `pipeline-chain.alpha-composite.rgba-256x256` — C/C/C/C; PASS
680. `pipeline-chain.alpha-composite.la-1024x768` — C/C/C/C; PASS
681. `pipeline-chain.alpha-composite.rgba-1024x768` — C/C/C/C; PASS
682. `pipeline-chain.simd-crossover.invert-mirror.1x1` — C/C/C/C; PASS
683. `pipeline-chain.simd-crossover.invert-mirror.32x32` — C/C/C/C; PASS
684. `pipeline-chain.simd-crossover.invert-mirror.256x256` — C/C/C/C; PASS
685. `pipeline-chain.simd-crossover.invert-mirror.1024x768` — C/C/C/C; PASS
686. `pipeline-chain.simd-crossover.invert-mirror.1024x1024` — C/C/C/C; PASS
687. `pipeline-chain.simd-vector-mirror.l.32x32` — C/C/C/C; PASS
688. `pipeline-chain.simd-vector-mirror.l.1024x1024` — C/C/C/C; PASS
689. `pipeline-chain.simd-vector-mirror.la.32x32` — C/C/C/C; PASS
690. `pipeline-chain.simd-vector-mirror.la.1024x1024` — C/C/C/C; PASS
691. `pipeline-chain.simd-vector-mirror.rgba.32x32` — C/C/C/C; PASS
692. `pipeline-chain.simd-vector-mirror.rgba.1024x1024` — C/C/C/C; PASS
693. `pipeline-chain.simd-chops.darker-rgb` — C/C/C/C; PASS
694. `pipeline-chain.simd-chops.lighter-rgb` — C/C/C/C; PASS
695. `pipeline-chain.simd-chops.difference-rgb` — C/C/C/C; PASS
696. `pipeline-chain.simd-chops.add-modulo-rgb` — C/C/C/C; PASS
697. `pipeline-chain.simd-chops.subtract-modulo-rgb` — C/C/C/C; PASS
698. `pipeline-chain.simd-chops.logical-and-1` — C/C/C/C; PASS
699. `pipeline-chain.simd-chops.logical-xor-1` — C/C/C/C; PASS
700. `pipeline-chain.simd-chops.logical-or-1` — C/C/C/C; PASS
701. `pipeline-chain.fused-chops.multiply-screen.l.256x256` — C/C/C/C; PASS
702. `pipeline-chain.fused-chops.multiply-screen.l.1024x1024` — C/C/C/C; PASS
703. `pipeline-chain.fused-chops.multiply-screen.la.256x256` — C/C/C/C; PASS
704. `pipeline-chain.fused-chops.multiply-screen.la.1024x1024` — C/C/C/C; PASS
705. `pipeline-chain.fused-chops.multiply-screen.rgb.256x256` — C/C/C/C; PASS
706. `pipeline-chain.fused-chops.multiply-screen.rgb.1024x1024` — C/C/C/C; PASS
707. `pipeline-chain.fused-chops.multiply-screen.rgba.256x256` — C/C/C/C; PASS
708. `pipeline-chain.fused-chops.multiply-screen.rgba.1024x1024` — C/C/C/C; PASS
709. `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.l.1024x1024` — C/C/C/C; PASS
710. `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024` — C/C/C/C; PASS
711. `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgb.1024x1024` — C/C/C/C; PASS
712. `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgba.1024x1024` — C/C/C/C; PASS
713. `pipeline-chain.simd-constant.32x32` — C/C/C/C; PASS
714. `pipeline-chain.simd-constant.256x256` — C/C/C/C; PASS
715. `pipeline-chain.simd-constant.1024x768` — C/C/C/C; PASS
716. `pipeline-chain.simd-constant.1024x1024` — C/C/C/C; PASS
717. `pipeline-chain.simd-lut.l.32x32` — C/C/C/C; PASS
718. `pipeline-chain.simd-lut.l.256x256` — C/C/C/C; PASS
719. `pipeline-chain.simd-lut.l.1024x768` — C/C/C/C; PASS
720. `pipeline-chain.simd-lut.l.1024x1024` — C/C/C/C; PASS
721. `pipeline-chain.simd-lut.rgb.32x32` — C/C/C/C; PASS
722. `pipeline-chain.simd-lut.rgb.256x256` — C/C/C/C; PASS
723. `pipeline-chain.simd-lut.rgb.1024x768` — C/C/C/C; PASS
724. `pipeline-chain.simd-lut.rgb.1024x1024` — C/C/C/C; PASS
725. `pipeline-chain.resize-cache.identical-geometry` — C/C/C/C; PASS
726. `pipeline-chain.resize-cache.f64-identical-geometry` — C/C/C/F; NP
727. `pipeline-chain.long-auxiliary.multiply-screen-260` — C/C/C/C; PASS
728. `pipeline-chain.loaded-10.rgb-jpeg-512x384` — C/C/C/F; NP
729. `pipeline-chain.loaded-10.rgba-png-512x384` — C/F/C/F; NP
730. `pipeline-chain.long-point.invert-1` — C/C/C/C; PASS
731. `pipeline-chain.long-point.invert-8` — C/C/C/C; PASS
732. `pipeline-chain.long-point.invert-64` — C/C/C/C; PASS
733. `pipeline-chain.long-point.invert-1024` — C/C/C/C; PASS
734. `pipeline-chain.long-point.invert-10000` — C/C/C/C; PASS

#### pipeline

735. `pipeline.quick.transpose-twice.rgb-1024` — C/C/C/C; PASS
736. `pipeline.quick.gaussianblur-invert.rgb-1024` — C/C/C/C; PASS
737. `pipeline.quick.multiply-screen.rgb-1024` — C/C/C/C; PASS
738. `pipeline.quick.invert-mirror.rgb-1024` — C/C/C/C; PASS

#### pipeline-lifecycle

739. `pipeline-lifecycle.cold.transpose-twice.rgb-1024` — C/C/C/C; PASS
740. `pipeline-lifecycle.cold.gaussianblur-invert.rgb-1024` — C/C/C/C; PASS
741. `pipeline-lifecycle.cold.multiply-screen.rgb-1024` — C/C/C/C; PASS
742. `pipeline-lifecycle.cold.invert-mirror.rgb-1024` — C/C/C/C; PASS
743. `pipeline-lifecycle.resident.transpose-twice.rgb-1024` — C/C/C/C; PASS
744. `pipeline-lifecycle.resident.gaussianblur-invert.rgb-1024` — C/C/C/C; PASS
745. `pipeline-lifecycle.resident.multiply-screen.rgb-1024` — C/C/C/C; PASS
746. `pipeline-lifecycle.resident.invert-mirror.rgb-1024` — C/C/C/C; PASS

### 30.3 CPU slower than Pillow on actual-CPU pairs

Cohort: all 482 workloads with a completed actual CPU receipt and completed Pillow timing. Gate: CPU should not be slower than Pillow (`Pillow_ms / CPU_ms >= 1.0`). Result: **270 violations**, 212 passes.

| Workload ID | Pillow median ms | CPU median ms | Pillow / CPU |
|---|---:|---:|---:|
| `pil-image-image.transpose.standard` | 2.273958 | 3.250687 | 0.699532 |
| `pil-imagedraw-imagedraw.shape.standard` | 0.013000 | 0.015458 | 0.840988 |
| `pil-imagefilter.unsharpmask.standard` | 0.010146 | 0.100542 | 0.100908 |
| `pil-imageops.invert.standard` | 2.607770 | 2.697208 | 0.966841 |
| `pipeline-op.resize.benchmark-materialized` | 0.012104 | 0.014541 | 0.832376 |
| `pipeline-op.transpose.benchmark-materialized` | 0.009438 | 0.010063 | 0.937842 |
| `pipeline-op.thumbnail.benchmark-materialized` | 0.019312 | 0.106937 | 0.180596 |
| `pipeline-op.reduce.benchmark-materialized` | 0.009791 | 0.049792 | 0.196648 |
| `pipeline-op.remappalette.benchmark-materialized` | 0.055313 | 0.072563 | 0.762274 |
| `pipeline-op.filter3x3.benchmark-materialized` | 0.013688 | 0.052959 | 0.258467 |
| `pipeline-op.filter5x5.benchmark-materialized` | 0.013896 | 0.058792 | 0.236359 |
| `pipeline-op.gaussianblur.benchmark-materialized` | 0.014333 | 0.095459 | 0.150149 |
| `pipeline-op.boxblur.benchmark-materialized` | 0.013604 | 0.067021 | 0.202981 |
| `pipeline-op.medianfilter.benchmark-materialized` | 0.030937 | 0.055230 | 0.560163 |
| `pipeline-op.maxfilter.benchmark-materialized` | 0.035853 | 0.056188 | 0.638105 |
| `pipeline-op.minfilter.benchmark-materialized` | 0.033626 | 0.053062 | 0.633696 |
| `pipeline-op.rankfilter.benchmark-materialized` | 0.016105 | 0.020563 | 0.783198 |
| `pipeline-op.flip.benchmark-materialized` | 0.009708 | 0.010146 | 0.956830 |
| `pipeline-op.mirror.benchmark-materialized` | 0.009437 | 0.010625 | 0.888230 |
| `pipeline-op.grayscale.benchmark-materialized` | 0.010521 | 0.011187 | 0.940425 |
| `pipeline-op.contain.benchmark-materialized` | 0.011125 | 0.011645 | 0.955305 |
| `pipeline-op.cover.benchmark-materialized` | 0.010896 | 0.011458 | 0.950908 |
| `pipeline-op.fit.benchmark-materialized` | 0.011625 | 0.105813 | 0.109864 |
| `pipeline-op.pad.benchmark-materialized` | 0.011021 | 0.012896 | 0.854639 |
| `pipeline-op.scale.benchmark-materialized` | 0.010125 | 0.011000 | 0.920455 |
| `pipeline-op.expand.benchmark-materialized` | 0.011729 | 0.011750 | 0.998213 |
| `pipeline-op.add.benchmark-materialized` | 0.015645 | 0.017042 | 0.918082 |
| `pipeline-op.subtract.benchmark-materialized` | 0.014354 | 0.017000 | 0.844353 |
| `pipeline-op.multiply.benchmark-materialized` | 0.014812 | 0.015166 | 0.976626 |
| `pipeline-op.screen.benchmark-materialized` | 0.014583 | 0.014625 | 0.997162 |
| `pipeline-op.difference.benchmark-materialized` | 0.013938 | 0.015959 | 0.873390 |
| `pipeline-op.overlay.benchmark-materialized` | 0.013458 | 0.014937 | 0.900954 |
| `pipeline-op.hardlight.benchmark-materialized` | 0.013750 | 0.014750 | 0.932203 |
| `pipeline-op.addmodulo.benchmark-materialized` | 0.014271 | 0.016250 | 0.878215 |
| `pipeline-op.subtractmodulo.benchmark-materialized` | 0.014500 | 0.015875 | 0.913386 |
| `pipeline-op.logicaland.benchmark-materialized` | 0.012771 | 0.014313 | 0.892262 |
| `pipeline-op.logicalor.benchmark-materialized` | 0.013145 | 0.014125 | 0.930622 |
| `pipeline-op.logicalxor.benchmark-materialized` | 0.013416 | 0.015271 | 0.878561 |
| `pipeline-op.constant.benchmark-materialized` | 0.010020 | 0.013813 | 0.725466 |
| `pipeline-op.offset.benchmark-materialized` | 0.010667 | 0.013688 | 0.779288 |
| `pipeline-op.blend.benchmark-materialized` | 0.014708 | 0.016271 | 0.903998 |
| `pipeline-op.composite.benchmark-materialized` | 0.018604 | 0.020396 | 0.912140 |
| `pipeline-op.duplicate.benchmark-materialized` | 0.009958 | 0.010479 | 0.950282 |
| `pipeline-op.invertchops.benchmark-materialized` | 0.009771 | 0.010666 | 0.916046 |
| `pipeline-op.effectspread.benchmark-materialized` | 0.012479 | 0.013000 | 0.959923 |
| `pipeline-op.paste.benchmark-materialized` | 0.013042 | 0.015167 | 0.859860 |
| `pipeline-op.alphacomposite.benchmark-materialized` | 0.020688 | 0.031062 | 0.665996 |
| `pipeline-op.merge.benchmark-materialized` | 0.021792 | 0.025334 | 0.860205 |
| `pipeline-op.blendmodule.benchmark-materialized` | 0.014083 | 0.017875 | 0.787882 |
| `pipeline-op.compositemodule.benchmark-materialized` | 0.019042 | 0.020979 | 0.907670 |
| `pipeline-op.eval.benchmark-materialized` | 0.104166 | 0.196437 | 0.530278 |
| `pipeline-op.pointop.benchmark-materialized` | 0.042855 | 0.071271 | 0.601289 |
| `pipeline-op.color3dlut.benchmark-materialized` | 0.069812 | 0.093208 | 0.748993 |
| `pipeline-op.transform.benchmark-materialized` | 0.013208 | 0.014792 | 0.892949 |
| `pipeline-op.putpixel.benchmark-materialized` | 0.009584 | 0.011354 | 0.844064 |
| `pipeline-op.putdata.benchmark-materialized` | 0.009687 | 0.010125 | 0.956743 |
| `pipeline-op.putalpha.benchmark-materialized` | 0.010063 | 0.010750 | 0.936049 |
| `pipeline-op.putalphadata.benchmark-materialized` | 0.012854 | 0.015709 | 0.818283 |
| `pipeline-op.extractband.benchmark-materialized` | 0.009209 | 0.011166 | 0.824654 |
| `pipeline-op.drawline.benchmark-materialized` | 0.012688 | 0.015542 | 0.816336 |
| `pipeline-op.drawrectangle.benchmark-materialized` | 0.012209 | 0.014104 | 0.865606 |
| `pipeline-op.drawroundedrect.benchmark-materialized` | 0.013479 | 0.014875 | 0.906151 |
| `pipeline-op.drawellipse.benchmark-materialized` | 0.011917 | 0.014125 | 0.843681 |
| `pipeline-op.drawcircle.benchmark-materialized` | 0.013604 | 0.016209 | 0.839313 |
| `pipeline-op.drawpolygon.benchmark-materialized` | 0.014437 | 0.018187 | 0.793814 |
| `pipeline-op.drawarc.benchmark-materialized` | 0.016521 | 0.019604 | 0.842711 |
| `pipeline-op.drawchord.benchmark-materialized` | 0.017979 | 0.021792 | 0.825028 |
| `pipeline-op.drawpieslice.benchmark-materialized` | 0.024584 | 0.029271 | 0.839873 |
| `pipeline-op.drawpoint.benchmark-materialized` | 0.012021 | 0.013938 | 0.862462 |
| `pipeline-op.resize.matrix-32x24` | 0.014521 | 0.080875 | 0.179543 |
| `pipeline-op.transpose.matrix-32x24` | 0.009875 | 0.010895 | 0.906337 |
| `pipeline-op.thumbnail.matrix-32x24` | 0.018771 | 0.077521 | 0.242142 |
| `pipeline-op.reduce.matrix-32x24` | 0.010229 | 0.033959 | 0.301221 |
| `pipeline-op.filter3x3.matrix-32x24` | 0.015917 | 0.057250 | 0.278026 |
| `pipeline-op.filter5x5.matrix-32x24` | 0.019520 | 0.061125 | 0.319351 |
| `pipeline-op.gaussianblur.matrix-32x24` | 0.017937 | 0.173729 | 0.103247 |
| `pipeline-op.boxblur.matrix-32x24` | 0.015833 | 0.064146 | 0.246828 |
| `pipeline-op.medianfilter.matrix-32x24` | 0.059542 | 0.068792 | 0.865543 |
| `pipeline-op.maxfilter.matrix-32x24` | 0.066355 | 0.078375 | 0.846628 |
| `pipeline-op.flip.matrix-32x24` | 0.010167 | 0.011187 | 0.908823 |
| `pipeline-op.mirror.matrix-32x24` | 0.010188 | 0.010896 | 0.935022 |
| `pipeline-op.grayscale.matrix-32x24` | 0.010313 | 0.010833 | 0.951908 |
| `pipeline-op.contain.matrix-32x24` | 0.016250 | 0.083125 | 0.195489 |
| `pipeline-op.cover.matrix-32x24` | 0.016688 | 0.120625 | 0.138342 |
| `pipeline-op.fit.matrix-32x24` | 0.016542 | 0.135750 | 0.121852 |
| `pipeline-op.pad.matrix-32x24` | 0.019666 | 0.112417 | 0.174943 |
| `pipeline-op.scale.matrix-32x24` | 0.010812 | 0.011146 | 0.970079 |
| `pipeline-op.cropborder.matrix-32x24` | 0.010833 | 0.010854 | 0.998065 |
| `pipeline-op.add.matrix-32x24` | 0.016063 | 0.016792 | 0.956585 |
| `pipeline-op.subtract.matrix-32x24` | 0.016063 | 0.016605 | 0.967358 |
| `pipeline-op.multiply.matrix-32x24` | 0.015521 | 0.015709 | 0.988064 |
| `pipeline-op.screen.matrix-32x24` | 0.015521 | 0.015563 | 0.997301 |
| `pipeline-op.darker.matrix-32x24` | 0.015187 | 0.015604 | 0.973308 |
| `pipeline-op.lighter.matrix-32x24` | 0.015208 | 0.015666 | 0.970766 |
| `pipeline-op.addmodulo.matrix-32x24` | 0.015645 | 0.015980 | 0.979098 |
| `pipeline-op.constant.matrix-32x24` | 0.009709 | 0.011562 | 0.839690 |
| `pipeline-op.offset.matrix-32x24` | 0.010333 | 0.012938 | 0.798655 |
| `pipeline-op.blend.matrix-32x24` | 0.014375 | 0.018875 | 0.761589 |
| `pipeline-op.composite.matrix-32x24` | 0.018687 | 0.022208 | 0.841476 |
| `pipeline-op.duplicate.matrix-32x24` | 0.009813 | 0.010792 | 0.909280 |
| `pipeline-op.brightness.matrix-32x24` | 0.013917 | 0.014000 | 0.994071 |
| `pipeline-op.sharpness.matrix-32x24` | 0.018042 | 0.021708 | 0.831080 |
| `pipeline-op.paste.matrix-32x24` | 0.013229 | 0.017875 | 0.740112 |
| `pipeline-op.alphacomposite.matrix-32x24` | 0.021229 | 0.034667 | 0.612384 |
| `pipeline-op.merge.matrix-32x24` | 0.022396 | 0.032084 | 0.698054 |
| `pipeline-op.blendmodule.matrix-32x24` | 0.013999 | 0.018729 | 0.747477 |
| `pipeline-op.compositemodule.matrix-32x24` | 0.018458 | 0.022374 | 0.824957 |
| `pipeline-op.eval.matrix-32x24` | 0.109604 | 0.188979 | 0.579981 |
| `pipeline-op.pointop.matrix-32x24` | 0.042334 | 0.069626 | 0.608017 |
| `pipeline-op.color3dlut.matrix-32x24` | 0.093208 | 0.121708 | 0.765830 |
| `pipeline-op.transform.matrix-32x24` | 0.013437 | 0.015062 | 0.892116 |
| `pipeline-op.putdata.matrix-32x24` | 0.009584 | 0.010292 | 0.931205 |
| `pipeline-op.putalpha.matrix-32x24` | 0.010292 | 0.011083 | 0.928542 |
| `pipeline-op.extractband.matrix-32x24` | 0.009583 | 0.010666 | 0.898420 |
| `pipeline-op.drawline.matrix-32x24` | 0.014000 | 0.014938 | 0.937207 |
| `pipeline-op.drawrectangle.matrix-32x24` | 0.012709 | 0.014021 | 0.906390 |
| `pipeline-op.drawroundedrect.matrix-32x24` | 0.014521 | 0.014958 | 0.970785 |
| `pipeline-op.drawellipse.matrix-32x24` | 0.011938 | 0.014125 | 0.845133 |
| `pipeline-op.drawcircle.matrix-32x24` | 0.013354 | 0.016563 | 0.806255 |
| `pipeline-op.drawpolygon.matrix-32x24` | 0.013895 | 0.018854 | 0.736986 |
| `pipeline-op.drawarc.matrix-32x24` | 0.015520 | 0.020916 | 0.742022 |
| `pipeline-op.drawchord.matrix-32x24` | 0.017730 | 0.021396 | 0.828636 |
| `pipeline-op.drawpieslice.matrix-32x24` | 0.025375 | 0.028958 | 0.876286 |
| `pipeline-op.drawpoint.matrix-32x24` | 0.012042 | 0.013937 | 0.863995 |
| `pipeline-matrix.expanded.resize.1x1` | 0.011229 | 0.060792 | 0.184720 |
| `pipeline-matrix.expanded.resize.32x32` | 0.015959 | 0.099125 | 0.160994 |
| `pipeline-matrix.expanded.rotate.256x256` | 0.108229 | 0.274834 | 0.393798 |
| `pipeline-matrix.expanded.transpose.1x1` | 0.009042 | 0.013479 | 0.670821 |
| `pipeline-matrix.expanded.transpose.32x32` | 0.009416 | 0.011166 | 0.843281 |
| `pipeline-matrix.expanded.transpose.256x256` | 0.063834 | 0.118458 | 0.538868 |
| `pipeline-matrix.expanded.transpose.1024x768` | 1.016250 | 1.763791 | 0.576174 |
| `pipeline-matrix.expanded.reduce.1x1` | 0.009562 | 0.011063 | 0.864368 |
| `pipeline-matrix.expanded.reduce.32x32` | 0.010333 | 0.091583 | 0.112826 |
| `pipeline-matrix.expanded.reduce.256x256` | 0.059646 | 0.204437 | 0.291757 |
| `pipeline-matrix.expanded.filter3x3.32x32` | 0.017083 | 0.051395 | 0.332383 |
| `pipeline-matrix.expanded.filter5x5.32x32` | 0.023250 | 0.074271 | 0.313043 |
| `pipeline-matrix.expanded.gaussianblur.1x1` | 0.013250 | 0.015124 | 0.876062 |
| `pipeline-matrix.expanded.gaussianblur.32x32` | 0.019709 | 0.225062 | 0.087569 |
| `pipeline-matrix.expanded.gaussianblur.256x256` | 0.487020 | 0.581187 | 0.837975 |
| `pipeline-matrix.expanded.boxblur.1x1` | 0.013000 | 0.013875 | 0.936937 |
| `pipeline-matrix.expanded.boxblur.32x32` | 0.016042 | 0.104458 | 0.153569 |
| `pipeline-matrix.expanded.boxblur.256x256` | 0.239937 | 0.267730 | 0.896194 |
| `pipeline-matrix.expanded.medianfilter.32x32` | 0.067271 | 0.070958 | 0.948033 |
| `pipeline-matrix.expanded.minfilter.32x32` | 0.083500 | 0.088105 | 0.947738 |
| `pipeline-matrix.expanded.effectspread.1x1` | 0.009375 | 0.010875 | 0.862069 |
| `pipeline-matrix.expanded.grayscale.1x1` | 0.009417 | 0.009979 | 0.943682 |
| `pipeline-matrix.expanded.grayscale.32x32` | 0.009375 | 0.011020 | 0.850687 |
| `pipeline-matrix.expanded.grayscale.256x256` | 0.028625 | 0.106521 | 0.268726 |
| `pipeline-matrix.expanded.grayscale.1024x768` | 0.406375 | 1.175187 | 0.345796 |
| `pipeline-matrix.expanded.eval.1x1` | 0.097479 | 0.198230 | 0.491751 |
| `pipeline-matrix.expanded.eval.32x32` | 0.100229 | 0.198750 | 0.504297 |
| `pipeline-matrix.expanded.eval.256x256` | 0.223542 | 0.363104 | 0.615640 |
| `pipeline-matrix.expanded.eval.1024x768` | 1.509917 | 2.210375 | 0.683104 |
| `pipeline-matrix.expanded.pointop.1x1` | 0.040042 | 0.076833 | 0.521153 |
| `pipeline-matrix.expanded.pointop.32x32` | 0.041292 | 0.070521 | 0.585521 |
| `pipeline-matrix.expanded.pointop.256x256` | 0.067562 | 0.105333 | 0.641418 |
| `pipeline-matrix.expanded.multiply.1x1` | 0.014125 | 0.014958 | 0.944279 |
| `pipeline-matrix.expanded.screen.1x1` | 0.013146 | 0.014563 | 0.902664 |
| `pipeline-matrix.expanded.add.1x1` | 0.012938 | 0.014229 | 0.909238 |
| `pipeline-matrix.expanded.add.32x32` | 0.015417 | 0.017791 | 0.866537 |
| `pipeline-matrix.expanded.add.256x256` | 0.187771 | 0.241541 | 0.777386 |
| `pipeline-matrix.expanded.darker.1x1` | 0.012812 | 0.014187 | 0.903084 |
| `pipeline-matrix.expanded.darker.32x32` | 0.014750 | 0.015208 | 0.969885 |
| `pipeline-matrix.expanded.brightness.256x256` | 0.055917 | 0.165750 | 0.337354 |
| `pipeline-chain.matrix-000` | 0.013875 | 0.014958 | 0.927566 |
| `pipeline-chain.matrix-001` | 0.045770 | 0.163312 | 0.280263 |
| `pipeline-chain.matrix-002` | 0.067938 | 0.072063 | 0.942751 |
| `pipeline-chain.matrix-004` | 0.026771 | 0.141896 | 0.188663 |
| `pipeline-chain.matrix-010` | 0.016438 | 0.016833 | 0.976534 |
| `pipeline-chain.matrix-017` | 0.053291 | 0.185437 | 0.287383 |
| `pipeline-chain.matrix-018` | 0.015375 | 0.021146 | 0.727088 |
| `pipeline-chain.matrix-021` | 0.027625 | 0.102916 | 0.268421 |
| `pipeline-chain.matrix-022` | 0.059833 | 0.199937 | 0.299261 |
| `pipeline-chain.matrix-023` | 0.376875 | 0.500042 | 0.753687 |
| `pipeline-chain.matrix-024` | 0.078333 | 0.120145 | 0.651984 |
| `pipeline-chain.matrix-025` | 0.031459 | 0.157000 | 0.200373 |
| `pipeline-chain.matrix-030` | 0.014667 | 0.077375 | 0.189551 |
| `pipeline-chain.matrix-031` | 0.011729 | 0.012792 | 0.916901 |
| `pipeline-chain.matrix-032` | 0.019333 | 0.060708 | 0.318456 |
| `pipeline-chain.matrix-034` | 0.018042 | 0.066771 | 0.270207 |
| `pipeline-chain.matrix-036` | 0.015270 | 0.017626 | 0.866387 |
| `pipeline-chain.matrix-037` | 0.015499 | 0.021000 | 0.738071 |
| `pipeline-chain.matrix-038` | 0.015437 | 0.022166 | 0.696434 |
| `pipeline-chain.matrix-039` | 0.015291 | 0.016438 | 0.930281 |
| `pipeline-chain.matrix-040` | 0.014666 | 0.016646 | 0.881083 |
| `pipeline-chain.matrix-045` | 0.015688 | 0.017750 | 0.883828 |
| `pipeline-chain.matrix-054` | 0.015479 | 0.017313 | 0.894094 |
| `pipeline-chain.matrix-066` | 0.015229 | 0.017979 | 0.847020 |
| `pipeline-chain.matrix-067` | 0.014396 | 0.016750 | 0.859463 |
| `pipeline-chain.matrix-068` | 0.014020 | 0.016375 | 0.856214 |
| `pipeline-chain.matrix-069` | 0.013938 | 0.016604 | 0.839406 |
| `pipeline-chain.matrix-070` | 0.017334 | 0.018312 | 0.946539 |
| `pipeline-chain.matrix-074` | 0.012376 | 0.014167 | 0.873575 |
| `pipeline-chain.matrix-075` | 0.017146 | 0.020188 | 0.849316 |
| `pipeline-chain.matrix-079` | 0.017834 | 0.020313 | 0.877960 |
| `pipeline-chain.matrix-082` | 0.017354 | 0.022291 | 0.778503 |
| `pipeline-chain.matrix-083` | 0.017292 | 0.019521 | 0.885790 |
| `pipeline-chain.matrix-084` | 0.017959 | 0.024750 | 0.725596 |
| `pipeline-chain.matrix-085` | 0.016854 | 0.024063 | 0.700411 |
| `pipeline-chain.matrix-086` | 0.011730 | 0.014791 | 0.793016 |
| `pipeline-chain.matrix-087` | 0.012105 | 0.015645 | 0.773673 |
| `pipeline-chain.matrix-096` | 0.028854 | 0.098021 | 0.294372 |
| `pipeline-chain.matrix-097` | 0.021812 | 0.093167 | 0.234123 |
| `pipeline-chain.matrix-098` | 0.017230 | 0.054166 | 0.318084 |
| `pipeline-chain.matrix-099` | 0.017396 | 0.077187 | 0.225373 |
| `pipeline-chain.terminal-read.rgb-band0` | 0.391792 | 1.561625 | 0.250887 |
| `pipeline-chain.terminal-read.analysis-scalar-if-1024x768` | 5.664437 | 18.085625 | 0.313201 |
| `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768` | 3.626917 | 4.843646 | 0.748799 |
| `pipeline-chain.terminal-read.getcolors.rgb-1024x768` | 1.230750 | 8.622521 | 0.142737 |
| `pipeline-chain.terminal-read.imagestat.i-1024x768` | 1.440749 | 3.705271 | 0.388838 |
| `pipeline-chain.terminal-read.imagestat.cmyk-1024x768` | 1.946041 | 4.475896 | 0.434783 |
| `pipeline-chain.metadata-cache.color3dlut-rgb` | 0.757772 | 1.345104 | 0.563355 |
| `pipeline-chain.rank-filter.large-f-9x9` | 0.039229 | 0.124333 | 0.315516 |
| `pipeline-chain.convolution.material.l-3x3-invert.256x256` | 0.172230 | 0.178063 | 0.967242 |
| `pipeline-chain.convolution.material.rgb-3x3-mirror.256x256` | 0.421563 | 0.424396 | 0.993325 |
| `pipeline-chain.convolution.material.l-5x5-scale.256x256` | 0.370125 | 0.555167 | 0.666692 |
| `pipeline-chain.convolution.crossover.l-5x5-scale.512x512` | 1.487938 | 1.935854 | 0.768621 |
| `pipeline-chain.convolution.native.l-5x5-scale.1024x768` | 4.525187 | 5.571584 | 0.812191 |
| `pipeline-chain.convolution-i.5x5-1024x768` | 4.028958 | 4.657229 | 0.865098 |
| `pipeline-chain.reviewed.filter-invert-mirror` | 0.047709 | 0.244480 | 0.195143 |
| `pipeline-chain.reviewed.invert-solarize-posterize-point` | 0.079084 | 0.080209 | 0.985974 |
| `pipeline-chain.reviewed.resize-rotate-crop` | 0.025750 | 0.176208 | 0.146134 |
| `pipeline-chain.reviewed.transpose-flip-resize` | 0.014667 | 0.018667 | 0.785691 |
| `pipeline-chain.reviewed.crop-expand-mirror` | 0.017792 | 0.021854 | 0.814089 |
| `pipeline-chain.reviewed.draw-filter-invert` | 0.059041 | 0.196458 | 0.300529 |
| `pipeline-chain.reviewed.radial-gradient-crop-resize` | 0.102396 | 0.313105 | 0.327035 |
| `pipeline-chain.reviewed.draw-batch-rgb-shapes` | 0.355104 | 0.493667 | 0.719319 |
| `pipeline-chain.reviewed.draw-batch-rgba-alpha` | 0.092979 | 0.128354 | 0.724395 |
| `pipeline-chain.reviewed.convert-palette-rgb` | 0.017958 | 0.020667 | 0.868921 |
| `pipeline-chain.reviewed.convert-one-cmyk` | 0.010020 | 0.012667 | 0.791071 |
| `pipeline-chain.reviewed.convert-rgba-cmyk-la` | 0.014750 | 0.021708 | 0.679457 |
| `pipeline-chain.resize-typed.simd-f-resize-transpose` | 0.013542 | 0.049334 | 0.274489 |
| `pipeline-chain.resize-typed.simd-i-resize-transform` | 0.017208 | 0.041688 | 0.412786 |
| `pipeline-chain.geometry-material.transpose-rgba-1024x768` | 0.950792 | 1.793270 | 0.530200 |
| `pipeline-chain.geometry-material.transverse-rgb-1024x768` | 1.126667 | 2.032375 | 0.554360 |
| `pipeline-chain.geometry-copy.crop-l-1024x768` | 0.086563 | 0.091541 | 0.945615 |
| `pipeline-chain.geometry-copy.cropborder-l-1024x768` | 0.139792 | 0.471916 | 0.296222 |
| `pipeline-chain.geometry-copy.cropborder-la-1024x768` | 0.467562 | 0.940959 | 0.496900 |
| `pipeline-chain.geometry-copy.cropborder-rgb-1024x768` | 0.806104 | 1.342021 | 0.600664 |
| `pipeline-chain.geometry-copy.cropborder-rgba-1024x768` | 0.649458 | 1.223771 | 0.530702 |
| `pipeline-chain.geometry-copy.cropborder-chain-rgba-1024x768` | 0.952646 | 2.065479 | 0.461223 |
| `pipeline-chain.blur-material.gaussian-rgba-1024x768` | 6.102646 | 8.185771 | 0.745519 |
| `pipeline-chain.blur-material.gaussian-l-256x256-radius-0.5` | 0.349166 | 0.396042 | 0.881640 |
| `pipeline-chain.blur-material.gaussian-rgba-256x256-radius-2` | 0.484604 | 0.621687 | 0.779499 |
| `pipeline-chain.blur-material.box-l-256x256-radius-0.5` | 0.160626 | 0.171438 | 0.936933 |
| `pipeline-chain.blur-material.box-rgb-256x256-radius-1` | 0.247625 | 0.255479 | 0.969258 |
| `pipeline-chain.point-fusion.rgb-001` | 0.199396 | 0.207021 | 0.963166 |
| `pipeline-chain.point-fusion.rgb-002` | 0.201396 | 0.203417 | 0.990065 |
| `pipeline-chain.point-fusion.rgb-003` | 0.199792 | 0.208521 | 0.958139 |
| `pipeline-chain.point-fusion.rgb-004` | 0.205146 | 0.219875 | 0.933012 |
| `pipeline-chain.point-fusion.rgb-005` | 0.193750 | 0.206000 | 0.940536 |
| `pipeline-chain.point-fusion.la-001` | 0.198666 | 0.360875 | 0.550513 |
| `pipeline-chain.point-fusion.la-002` | 0.332541 | 0.544083 | 0.611196 |
| `pipeline-chain.point-fusion.rgba-001` | 0.372708 | 0.732834 | 0.508585 |
| `pipeline-chain.point-fusion.rgba-002` | 0.591271 | 0.864875 | 0.683649 |
| `pipeline-chain.alpha-composite.rgba-256x256` | 0.202937 | 0.226021 | 0.897870 |
| `pipeline-chain.simd-crossover.invert-mirror.1024x1024` | 2.759542 | 2.763959 | 0.998402 |
| `pipeline-chain.simd-vector-mirror.l.32x32` | 0.012396 | 0.014187 | 0.873722 |
| `pipeline-chain.simd-vector-mirror.l.1024x1024` | 1.267333 | 1.639855 | 0.772833 |
| `pipeline-chain.simd-vector-mirror.rgba.32x32` | 0.015917 | 0.018271 | 0.871158 |
| `pipeline-chain.simd-chops.logical-and-1` | 0.800500 | 1.069229 | 0.748670 |
| `pipeline-chain.simd-chops.logical-xor-1` | 0.789042 | 2.375104 | 0.332214 |
| `pipeline-chain.simd-chops.logical-or-1` | 0.626604 | 2.286583 | 0.274035 |
| `pipeline-chain.simd-constant.32x32` | 0.010646 | 0.013500 | 0.788556 |
| `pipeline-chain.simd-lut.rgb.32x32` | 0.198396 | 0.212542 | 0.933441 |
| `pipeline-chain.resize-cache.f64-identical-geometry` | 0.311229 | 0.491791 | 0.632847 |
| `pipeline-chain.long-auxiliary.multiply-screen-260` | 0.946250 | 1.557958 | 0.607366 |
| `pipeline-chain.loaded-10.rgb-jpeg-512x384` | 4.095583 | 4.208729 | 0.973116 |
| `pipeline.quick.transpose-twice.rgb-1024` | 2.273958 | 3.250687 | 0.699532 |
| `pipeline.quick.invert-mirror.rgb-1024` | 2.607770 | 2.697208 | 0.966841 |

### 30.4 SIMD slower than CPU on the actual CPU+SIMD intersection

Cohort: all 480 workloads with completed, no-fallback actual CPU and actual SIMD receipts. Gate: SIMD should not be slower than CPU (`CPU_ms / SIMD_ms >= 1.0`). Result: **319 violations**, 161 passes. The stricter 1.25× threshold has 410 failures and 70 passes on this full receipt cohort.

| Workload ID | CPU median ms | SIMD median ms | CPU / SIMD |
|---|---:|---:|---:|
| `pil-imagechops.multiply.standard` | 2.279041 | 2.494021 | 0.913802 |
| `pil-imagedraw-imagedraw.shape.standard` | 0.015458 | 0.015834 | 0.976254 |
| `pil-imagefilter.unsharpmask.standard` | 0.100542 | 0.251396 | 0.399935 |
| `pipeline-op.transpose.benchmark-materialized` | 0.010063 | 0.010125 | 0.993877 |
| `pipeline-op.filter3x3.benchmark-materialized` | 0.052959 | 0.096542 | 0.548554 |
| `pipeline-op.gaussianblur.benchmark-materialized` | 0.095459 | 0.264791 | 0.360504 |
| `pipeline-op.boxblur.benchmark-materialized` | 0.067021 | 0.139041 | 0.482022 |
| `pipeline-op.autocontrast.benchmark-materialized` | 0.011250 | 0.013771 | 0.816934 |
| `pipeline-op.equalize.benchmark-materialized` | 0.011770 | 0.013771 | 0.854731 |
| `pipeline-op.invert.benchmark-materialized` | 0.010854 | 0.012396 | 0.875640 |
| `pipeline-op.flip.benchmark-materialized` | 0.010146 | 0.010771 | 0.941974 |
| `pipeline-op.mirror.benchmark-materialized` | 0.010625 | 0.010833 | 0.980708 |
| `pipeline-op.solarize.benchmark-materialized` | 0.012230 | 0.012625 | 0.968673 |
| `pipeline-op.grayscale.benchmark-materialized` | 0.011187 | 0.011396 | 0.981704 |
| `pipeline-op.colorize.benchmark-materialized` | 0.017291 | 0.017417 | 0.992794 |
| `pipeline-op.contain.benchmark-materialized` | 0.011645 | 0.012000 | 0.970458 |
| `pipeline-op.cover.benchmark-materialized` | 0.011458 | 0.013604 | 0.842252 |
| `pipeline-op.pad.benchmark-materialized` | 0.012896 | 0.012917 | 0.998374 |
| `pipeline-op.scale.benchmark-materialized` | 0.011000 | 0.012209 | 0.901012 |
| `pipeline-op.multiply.benchmark-materialized` | 0.015166 | 0.016750 | 0.905463 |
| `pipeline-op.screen.benchmark-materialized` | 0.014625 | 0.016687 | 0.876431 |
| `pipeline-op.darker.benchmark-materialized` | 0.014604 | 0.016105 | 0.906858 |
| `pipeline-op.lighter.benchmark-materialized` | 0.014625 | 0.016125 | 0.906977 |
| `pipeline-op.difference.benchmark-materialized` | 0.015959 | 0.016292 | 0.979530 |
| `pipeline-op.overlay.benchmark-materialized` | 0.014937 | 0.018333 | 0.814788 |
| `pipeline-op.hardlight.benchmark-materialized` | 0.014750 | 0.018187 | 0.811019 |
| `pipeline-op.softlight.benchmark-materialized` | 0.014541 | 0.018208 | 0.798632 |
| `pipeline-op.addmodulo.benchmark-materialized` | 0.016250 | 0.016333 | 0.994918 |
| `pipeline-op.subtractmodulo.benchmark-materialized` | 0.015875 | 0.016271 | 0.975692 |
| `pipeline-op.logicaland.benchmark-materialized` | 0.014313 | 0.015562 | 0.919679 |
| `pipeline-op.logicalor.benchmark-materialized` | 0.014125 | 0.015605 | 0.905220 |
| `pipeline-op.logicalxor.benchmark-materialized` | 0.015271 | 0.018354 | 0.832003 |
| `pipeline-op.constant.benchmark-materialized` | 0.013813 | 0.015104 | 0.914493 |
| `pipeline-op.blend.benchmark-materialized` | 0.016271 | 0.017209 | 0.945492 |
| `pipeline-op.duplicate.benchmark-materialized` | 0.010479 | 0.011020 | 0.950864 |
| `pipeline-op.invertchops.benchmark-materialized` | 0.010666 | 0.011771 | 0.906168 |
| `pipeline-op.brightness.benchmark-materialized` | 0.012896 | 0.013042 | 0.988843 |
| `pipeline-op.contrast.benchmark-materialized` | 0.013500 | 0.013625 | 0.990826 |
| `pipeline-op.colorsaturation.benchmark-materialized` | 0.013062 | 0.014208 | 0.919376 |
| `pipeline-op.sharpness.benchmark-materialized` | 0.014833 | 0.081104 | 0.182895 |
| `pipeline-op.effectspread.benchmark-materialized` | 0.013000 | 0.015083 | 0.861869 |
| `pipeline-op.paste.benchmark-materialized` | 0.015167 | 0.016042 | 0.945485 |
| `pipeline-op.alphacomposite.benchmark-materialized` | 0.031062 | 0.032834 | 0.946061 |
| `pipeline-op.color3dlut.benchmark-materialized` | 0.093208 | 0.096334 | 0.967561 |
| `pipeline-op.transform.benchmark-materialized` | 0.014792 | 0.017396 | 0.850310 |
| `pipeline-op.putpixel.benchmark-materialized` | 0.011354 | 0.012875 | 0.881864 |
| `pipeline-op.putdata.benchmark-materialized` | 0.010125 | 0.010458 | 0.968206 |
| `pipeline-op.putalpha.benchmark-materialized` | 0.010750 | 0.010958 | 0.981064 |
| `pipeline-op.drawline.benchmark-materialized` | 0.015542 | 0.017480 | 0.889156 |
| `pipeline-op.drawrectangle.benchmark-materialized` | 0.014104 | 0.016875 | 0.835793 |
| `pipeline-op.rotate.matrix-32x24` | 0.012771 | 0.015187 | 0.840856 |
| `pipeline-op.transpose.matrix-32x24` | 0.010895 | 0.011083 | 0.983082 |
| `pipeline-op.filter3x3.matrix-32x24` | 0.057250 | 0.073896 | 0.774737 |
| `pipeline-op.filter5x5.matrix-32x24` | 0.061125 | 0.088687 | 0.689227 |
| `pipeline-op.gaussianblur.matrix-32x24` | 0.173729 | 0.345251 | 0.503197 |
| `pipeline-op.boxblur.matrix-32x24` | 0.064146 | 0.190042 | 0.337536 |
| `pipeline-op.autocontrast.matrix-32x24` | 0.012688 | 0.018667 | 0.679675 |
| `pipeline-op.equalize.matrix-32x24` | 0.011708 | 0.018896 | 0.619602 |
| `pipeline-op.invert.matrix-32x24` | 0.011021 | 0.013272 | 0.830388 |
| `pipeline-op.mirror.matrix-32x24` | 0.010896 | 0.011270 | 0.966772 |
| `pipeline-op.posterize.matrix-32x24` | 0.011333 | 0.012646 | 0.896208 |
| `pipeline-op.solarize.matrix-32x24` | 0.011166 | 0.012167 | 0.917807 |
| `pipeline-op.grayscale.matrix-32x24` | 0.010833 | 0.012625 | 0.858099 |
| `pipeline-op.colorize.matrix-32x24` | 0.016209 | 0.018271 | 0.887116 |
| `pipeline-op.scale.matrix-32x24` | 0.011146 | 0.011938 | 0.933657 |
| `pipeline-op.expand.matrix-32x24` | 0.010437 | 0.012604 | 0.828110 |
| `pipeline-op.cropborder.matrix-32x24` | 0.010854 | 0.011104 | 0.977531 |
| `pipeline-op.add.matrix-32x24` | 0.016792 | 0.017646 | 0.951575 |
| `pipeline-op.subtract.matrix-32x24` | 0.016605 | 0.017479 | 0.949941 |
| `pipeline-op.multiply.matrix-32x24` | 0.015709 | 0.017896 | 0.877766 |
| `pipeline-op.screen.matrix-32x24` | 0.015563 | 0.018146 | 0.857655 |
| `pipeline-op.darker.matrix-32x24` | 0.015604 | 0.017479 | 0.892728 |
| `pipeline-op.lighter.matrix-32x24` | 0.015666 | 0.017354 | 0.902760 |
| `pipeline-op.difference.matrix-32x24` | 0.015501 | 0.017125 | 0.905139 |
| `pipeline-op.overlay.matrix-32x24` | 0.016563 | 0.022625 | 0.732044 |
| `pipeline-op.hardlight.matrix-32x24` | 0.016625 | 0.022521 | 0.738200 |
| `pipeline-op.softlight.matrix-32x24` | 0.016522 | 0.020167 | 0.819234 |
| `pipeline-op.addmodulo.matrix-32x24` | 0.015980 | 0.017480 | 0.914185 |
| `pipeline-op.subtractmodulo.matrix-32x24` | 0.016230 | 0.017750 | 0.914338 |
| `pipeline-op.constant.matrix-32x24` | 0.011562 | 0.012042 | 0.960140 |
| `pipeline-op.duplicate.matrix-32x24` | 0.010792 | 0.011292 | 0.955677 |
| `pipeline-op.invertchops.matrix-32x24` | 0.011250 | 0.012167 | 0.924632 |
| `pipeline-op.contrast.matrix-32x24` | 0.015125 | 0.015480 | 0.977099 |
| `pipeline-op.colorsaturation.matrix-32x24` | 0.014937 | 0.017646 | 0.846485 |
| `pipeline-op.sharpness.matrix-32x24` | 0.021708 | 0.058416 | 0.371616 |
| `pipeline-op.effectspread.matrix-32x24` | 0.015501 | 0.022333 | 0.694047 |
| `pipeline-op.alphacomposite.matrix-32x24` | 0.034667 | 0.037479 | 0.924971 |
| `pipeline-op.blendmodule.matrix-32x24` | 0.018729 | 0.018875 | 0.992265 |
| `pipeline-op.compositemodule.matrix-32x24` | 0.022374 | 0.026646 | 0.839695 |
| `pipeline-op.transform.matrix-32x24` | 0.015062 | 0.015980 | 0.942614 |
| `pipeline-op.putdata.matrix-32x24` | 0.010292 | 0.010771 | 0.955482 |
| `pipeline-op.putalpha.matrix-32x24` | 0.011083 | 0.012834 | 0.863638 |
| `pipeline-op.putalphadata.matrix-32x24` | 0.015312 | 0.017938 | 0.853635 |
| `pipeline-op.extractband.matrix-32x24` | 0.010666 | 0.011604 | 0.919209 |
| `pipeline-op.drawrectangle.matrix-32x24` | 0.014021 | 0.014104 | 0.994115 |
| `pipeline-op.drawcircle.matrix-32x24` | 0.016563 | 0.017083 | 0.969560 |
| `pipeline-op.drawpolygon.matrix-32x24` | 0.018854 | 0.019437 | 0.970031 |
| `pipeline-op.drawchord.matrix-32x24` | 0.021396 | 0.023104 | 0.926073 |
| `pipeline-op.drawpieslice.matrix-32x24` | 0.028958 | 0.031188 | 0.928498 |
| `pipeline-op.drawpoint.matrix-32x24` | 0.013937 | 0.016625 | 0.838341 |
| `pipeline-matrix.expanded.resize.256x256` | 0.199146 | 1.373854 | 0.144954 |
| `pipeline-matrix.expanded.resize.1024x768` | 0.864146 | 16.073604 | 0.053762 |
| `pipeline-matrix.expanded.rotate.32x32` | 0.013521 | 0.015188 | 0.890242 |
| `pipeline-matrix.expanded.rotate.256x256` | 0.274834 | 0.330438 | 0.831725 |
| `pipeline-matrix.expanded.rotate.1024x768` | 1.352229 | 4.437729 | 0.304712 |
| `pipeline-matrix.expanded.transpose.32x32` | 0.011166 | 0.011312 | 0.987094 |
| `pipeline-matrix.expanded.reduce.256x256` | 0.204437 | 0.257395 | 0.794254 |
| `pipeline-matrix.expanded.reduce.1024x768` | 0.624271 | 3.074063 | 0.203077 |
| `pipeline-matrix.expanded.filter3x3.1x1` | 0.012562 | 0.013604 | 0.923371 |
| `pipeline-matrix.expanded.filter3x3.32x32` | 0.051395 | 0.066771 | 0.769728 |
| `pipeline-matrix.expanded.filter3x3.256x256` | 0.249896 | 0.285854 | 0.874209 |
| `pipeline-matrix.expanded.filter3x3.1024x768` | 2.127271 | 2.368083 | 0.898309 |
| `pipeline-matrix.expanded.filter5x5.1x1` | 0.012646 | 0.013708 | 0.922493 |
| `pipeline-matrix.expanded.filter5x5.32x32` | 0.074271 | 0.099562 | 0.745974 |
| `pipeline-matrix.expanded.gaussianblur.1x1` | 0.015124 | 0.017875 | 0.846126 |
| `pipeline-matrix.expanded.gaussianblur.32x32` | 0.225062 | 0.434624 | 0.517831 |
| `pipeline-matrix.expanded.gaussianblur.256x256` | 0.581187 | 0.730458 | 0.795648 |
| `pipeline-matrix.expanded.boxblur.1x1` | 0.013875 | 0.015270 | 0.908615 |
| `pipeline-matrix.expanded.boxblur.32x32` | 0.104458 | 0.190375 | 0.548696 |
| `pipeline-matrix.expanded.boxblur.256x256` | 0.267730 | 0.541167 | 0.494726 |
| `pipeline-matrix.expanded.medianfilter.1x1` | 0.013229 | 0.015104 | 0.875861 |
| `pipeline-matrix.expanded.medianfilter.256x256` | 0.402437 | 2.350917 | 0.171183 |
| `pipeline-matrix.expanded.medianfilter.1024x768` | 3.343749 | 27.561417 | 0.121320 |
| `pipeline-matrix.expanded.maxfilter.1x1` | 0.013562 | 0.013729 | 0.987836 |
| `pipeline-matrix.expanded.maxfilter.256x256` | 0.301667 | 1.139792 | 0.264668 |
| `pipeline-matrix.expanded.maxfilter.1024x768` | 2.511563 | 13.351312 | 0.188114 |
| `pipeline-matrix.expanded.minfilter.1x1` | 0.013834 | 0.014146 | 0.977909 |
| `pipeline-matrix.expanded.minfilter.256x256` | 0.271208 | 1.099646 | 0.246632 |
| `pipeline-matrix.expanded.minfilter.1024x768` | 2.358771 | 13.254167 | 0.177964 |
| `pipeline-matrix.expanded.effectspread.1x1` | 0.010875 | 0.010979 | 0.990482 |
| `pipeline-matrix.expanded.effectspread.32x32` | 0.018167 | 0.022542 | 0.805896 |
| `pipeline-matrix.expanded.effectspread.256x256` | 0.487834 | 0.786563 | 0.620209 |
| `pipeline-matrix.expanded.effectspread.1024x768` | 7.073313 | 9.226125 | 0.766661 |
| `pipeline-matrix.expanded.invert.1x1` | 0.011375 | 0.012729 | 0.893629 |
| `pipeline-matrix.expanded.invert.32x32` | 0.011834 | 0.012854 | 0.920608 |
| `pipeline-matrix.expanded.invert.256x256` | 0.042271 | 0.116522 | 0.362774 |
| `pipeline-matrix.expanded.invert.1024x768` | 0.786792 | 0.837959 | 0.938939 |
| `pipeline-matrix.expanded.grayscale.1x1` | 0.009979 | 0.010521 | 0.948484 |
| `pipeline-matrix.expanded.grayscale.32x32` | 0.011020 | 0.013147 | 0.838284 |
| `pipeline-matrix.expanded.grayscale.256x256` | 0.106521 | 0.167542 | 0.635787 |
| `pipeline-matrix.expanded.grayscale.1024x768` | 1.175187 | 2.057688 | 0.571120 |
| `pipeline-matrix.expanded.autocontrast.1x1` | 0.010229 | 0.010583 | 0.966597 |
| `pipeline-matrix.expanded.autocontrast.32x32` | 0.013771 | 0.018166 | 0.758044 |
| `pipeline-matrix.expanded.autocontrast.256x256` | 0.243521 | 0.535791 | 0.454507 |
| `pipeline-matrix.expanded.autocontrast.1024x768` | 1.707437 | 6.891709 | 0.247752 |
| `pipeline-matrix.expanded.equalize.1x1` | 0.010542 | 0.010833 | 0.973047 |
| `pipeline-matrix.expanded.equalize.32x32` | 0.012729 | 0.019709 | 0.645863 |
| `pipeline-matrix.expanded.equalize.256x256` | 0.090375 | 0.597771 | 0.151187 |
| `pipeline-matrix.expanded.equalize.1024x768` | 1.366375 | 7.482416 | 0.182611 |
| `pipeline-matrix.expanded.eval.32x32` | 0.198750 | 0.210979 | 0.942035 |
| `pipeline-matrix.expanded.eval.256x256` | 0.363104 | 0.658834 | 0.551132 |
| `pipeline-matrix.expanded.eval.1024x768` | 2.210375 | 6.412520 | 0.344697 |
| `pipeline-matrix.expanded.pointop.256x256` | 0.105333 | 0.254146 | 0.414459 |
| `pipeline-matrix.expanded.pointop.1024x768` | 0.321396 | 2.215791 | 0.145048 |
| `pipeline-matrix.expanded.multiply.1x1` | 0.014958 | 0.016084 | 0.930053 |
| `pipeline-matrix.expanded.multiply.32x32` | 0.015834 | 0.018812 | 0.841648 |
| `pipeline-matrix.expanded.multiply.256x256` | 0.088438 | 0.190854 | 0.463379 |
| `pipeline-matrix.expanded.multiply.1024x768` | 1.160417 | 1.365875 | 0.849578 |
| `pipeline-matrix.expanded.screen.1x1` | 0.014563 | 0.014750 | 0.987322 |
| `pipeline-matrix.expanded.screen.32x32` | 0.015749 | 0.018167 | 0.866929 |
| `pipeline-matrix.expanded.screen.256x256` | 0.077333 | 0.258062 | 0.299668 |
| `pipeline-matrix.expanded.screen.1024x768` | 1.202228 | 1.430625 | 0.840352 |
| `pipeline-matrix.expanded.add.1024x768` | 1.528813 | 2.138916 | 0.714760 |
| `pipeline-matrix.expanded.darker.1x1` | 0.014187 | 0.014813 | 0.957774 |
| `pipeline-matrix.expanded.darker.32x32` | 0.015208 | 0.017250 | 0.881652 |
| `pipeline-matrix.expanded.darker.256x256` | 0.069333 | 0.159791 | 0.433900 |
| `pipeline-matrix.expanded.darker.1024x768` | 1.138250 | 1.966417 | 0.578845 |
| `pipeline-matrix.expanded.brightness.1x1` | 0.011855 | 0.012563 | 0.943642 |
| `pipeline-matrix.expanded.brightness.1024x768` | 0.961938 | 1.092167 | 0.880761 |
| `pipeline-chain.matrix-000` | 0.014958 | 0.015625 | 0.957344 |
| `pipeline-chain.matrix-001` | 0.163312 | 0.321917 | 0.507312 |
| `pipeline-chain.matrix-002` | 0.072063 | 0.074896 | 0.962181 |
| `pipeline-chain.matrix-003` | 0.074541 | 0.079604 | 0.936398 |
| `pipeline-chain.matrix-005` | 0.019895 | 0.022875 | 0.869749 |
| `pipeline-chain.matrix-006` | 0.026208 | 0.032584 | 0.804349 |
| `pipeline-chain.matrix-010` | 0.016833 | 0.018729 | 0.898767 |
| `pipeline-chain.matrix-011` | 0.020021 | 0.022876 | 0.875216 |
| `pipeline-chain.matrix-012` | 0.017875 | 0.021833 | 0.818696 |
| `pipeline-chain.matrix-017` | 0.185437 | 0.376646 | 0.492340 |
| `pipeline-chain.matrix-018` | 0.021146 | 0.023500 | 0.899830 |
| `pipeline-chain.matrix-020` | 0.037375 | 0.041375 | 0.903323 |
| `pipeline-chain.matrix-022` | 0.199937 | 0.488750 | 0.409079 |
| `pipeline-chain.matrix-023` | 0.500042 | 0.711271 | 0.703025 |
| `pipeline-chain.matrix-024` | 0.120145 | 0.166438 | 0.721866 |
| `pipeline-chain.matrix-030` | 0.077375 | 0.265000 | 0.291981 |
| `pipeline-chain.matrix-031` | 0.012792 | 0.014083 | 0.908329 |
| `pipeline-chain.matrix-032` | 0.060708 | 0.138250 | 0.439121 |
| `pipeline-chain.matrix-033` | 0.019000 | 0.026292 | 0.722653 |
| `pipeline-chain.matrix-034` | 0.066771 | 0.243479 | 0.274237 |
| `pipeline-chain.matrix-036` | 0.017626 | 0.017854 | 0.987202 |
| `pipeline-chain.matrix-039` | 0.016438 | 0.017521 | 0.938187 |
| `pipeline-chain.matrix-040` | 0.016646 | 0.018708 | 0.889756 |
| `pipeline-chain.matrix-041` | 0.015479 | 0.017479 | 0.885577 |
| `pipeline-chain.matrix-042` | 0.016646 | 0.018646 | 0.892738 |
| `pipeline-chain.matrix-045` | 0.017750 | 0.020688 | 0.857982 |
| `pipeline-chain.matrix-054` | 0.017313 | 0.019250 | 0.899351 |
| `pipeline-chain.matrix-066` | 0.017979 | 0.020375 | 0.882429 |
| `pipeline-chain.matrix-067` | 0.016750 | 0.020083 | 0.834039 |
| `pipeline-chain.matrix-068` | 0.016375 | 0.019062 | 0.859039 |
| `pipeline-chain.matrix-069` | 0.016604 | 0.018500 | 0.897514 |
| `pipeline-chain.matrix-070` | 0.018312 | 0.019980 | 0.916564 |
| `pipeline-chain.matrix-072` | 0.010792 | 0.011354 | 0.950502 |
| `pipeline-chain.matrix-073` | 0.014687 | 0.016230 | 0.904988 |
| `pipeline-chain.matrix-074` | 0.014167 | 0.016125 | 0.878543 |
| `pipeline-chain.matrix-075` | 0.020188 | 0.022271 | 0.906470 |
| `pipeline-chain.matrix-076` | 0.021229 | 0.023312 | 0.910647 |
| `pipeline-chain.matrix-077` | 0.017896 | 0.020313 | 0.880988 |
| `pipeline-chain.matrix-079` | 0.020313 | 0.020896 | 0.972100 |
| `pipeline-chain.matrix-082` | 0.022291 | 0.022792 | 0.978041 |
| `pipeline-chain.matrix-083` | 0.019521 | 0.020542 | 0.950297 |
| `pipeline-chain.matrix-084` | 0.024750 | 0.027230 | 0.908941 |
| `pipeline-chain.matrix-086` | 0.014791 | 0.015208 | 0.972548 |
| `pipeline-chain.matrix-087` | 0.015645 | 0.016979 | 0.921462 |
| `pipeline-chain.matrix-098` | 0.054166 | 0.082896 | 0.653427 |
| `pipeline-chain.terminal-read.rgb-band0` | 1.561625 | 1.568709 | 0.995485 |
| `pipeline-chain.terminal-read.analysis-scalar-if-1024x768` | 18.085625 | 18.149479 | 0.996482 |
| `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768` | 4.843646 | 6.937688 | 0.698164 |
| `pipeline-chain.terminal-read.getcolors.rgb-1024x768` | 8.622521 | 9.630104 | 0.895372 |
| `pipeline-chain.terminal-read.imagestat.i-1024x768` | 3.705271 | 3.980437 | 0.930870 |
| `pipeline-chain.metadata-cache.invert-1.rgb` | 0.049396 | 0.134125 | 0.368283 |
| `pipeline-chain.metadata-cache.invert-8.l` | 0.076625 | 0.214291 | 0.357574 |
| `pipeline-chain.metadata-cache.invert-64.rgb` | 0.363938 | 0.757396 | 0.480512 |
| `pipeline-chain.metadata-cache.extractband-rgba` | 0.029791 | 0.159749 | 0.186489 |
| `pipeline-chain.rank-filter.material.f-9x9-256x256` | 1.447812 | 25.506042 | 0.056764 |
| `pipeline-chain.rank-filter.material.l-9x9-256x256` | 0.624000 | 7.688292 | 0.081162 |
| `pipeline-chain.convolution.material.l-3x3-invert.256x256` | 0.178063 | 0.250479 | 0.710888 |
| `pipeline-chain.convolution.material.l-5x5-scale.256x256` | 0.555167 | 0.560563 | 0.990375 |
| `pipeline-chain.convolution.material.rgba-3x3-transpose.256x256` | 0.375209 | 0.608021 | 0.617098 |
| `pipeline-chain.convolution.native.rgba-3x3-transpose.1024x768` | 2.957688 | 3.629167 | 0.814977 |
| `pipeline-chain.reviewed.filter-invert-mirror` | 0.244480 | 0.352083 | 0.694379 |
| `pipeline-chain.reviewed.point-solarize-posterize` | 0.077959 | 0.079646 | 0.978819 |
| `pipeline-chain.reviewed.invert-solarize-posterize-point` | 0.080209 | 0.082208 | 0.975678 |
| `pipeline-chain.reviewed.quantize-remap-convert` | 0.021437 | 0.022792 | 0.940549 |
| `pipeline-chain.reviewed.multiply-screen-invert` | 0.028605 | 0.032875 | 0.870112 |
| `pipeline-chain.reviewed.crop-expand-mirror` | 0.021854 | 0.022521 | 0.970405 |
| `pipeline-chain.reviewed.equalize-autocontrast-invert` | 0.019542 | 0.021417 | 0.912453 |
| `pipeline-chain.reviewed.draw-filter-invert` | 0.196458 | 0.367375 | 0.534763 |
| `pipeline-chain.reviewed.draw-batch-rgb-shapes` | 0.493667 | 0.667812 | 0.739230 |
| `pipeline-chain.reviewed.draw-batch-rgba-alpha` | 0.128354 | 0.190458 | 0.673923 |
| `pipeline-chain.reviewed.convert-palette-rgb` | 0.020667 | 0.025917 | 0.797446 |
| `pipeline-chain.reviewed.convert-one-cmyk` | 0.012667 | 0.013250 | 0.956000 |
| `pipeline-chain.resize-alpha.rgba-lanczos-256x256` | 0.377729 | 5.060375 | 0.074645 |
| `pipeline-chain.resize-alpha.la-bicubic-256x256` | 0.219479 | 1.719313 | 0.127655 |
| `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256` | 0.453541 | 2.998396 | 0.151261 |
| `pipeline-chain.resize-alpha.la-bilinear-mirror-256x256` | 0.343437 | 1.565500 | 0.219378 |
| `pipeline-chain.resize-alpha.rgba-bicubic-512x512` | 0.825396 | 15.314875 | 0.053895 |
| `pipeline-chain.resize-alpha.la-lanczos-512x512` | 0.526021 | 10.011312 | 0.052543 |
| `pipeline-chain.resize-alpha.rgba-bilinear-upscale-128x128` | 0.380395 | 1.692063 | 0.224812 |
| `pipeline-chain.resize-alpha.la-bicubic-upscale-128x128` | 0.249999 | 1.362187 | 0.183528 |
| `pipeline-chain.resize-alpha.rgba-lanczos-1024x768` | 3.742230 | 60.080958 | 0.062286 |
| `pipeline-chain.resize-alpha.la-bicubic-1024x768` | 1.791791 | 23.090958 | 0.077597 |
| `pipeline-chain.geometry-material.crop-rgb-1024x768` | 0.469896 | 0.612416 | 0.767281 |
| `pipeline-chain.geometry-material.reduce-rgb-1024x768` | 0.586729 | 2.554646 | 0.229671 |
| `pipeline-chain.geometry-material.reduce-rgba-1024x768` | 0.470771 | 4.120020 | 0.114264 |
| `pipeline-chain.geometry-material.rotate-rgba-1024x768` | 2.166104 | 7.771104 | 0.278738 |
| `pipeline-chain.blur-material.gaussian-l-256x256-radius-0.5` | 0.396042 | 0.593666 | 0.667112 |
| `pipeline-chain.blur-material.gaussian-rgb-256x256-radius-1` | 0.524063 | 0.763583 | 0.686320 |
| `pipeline-chain.blur-material.gaussian-rgba-256x256-radius-2` | 0.621687 | 0.840521 | 0.739645 |
| `pipeline-chain.blur-material.box-l-256x256-radius-0.5` | 0.171438 | 0.338375 | 0.506650 |
| `pipeline-chain.blur-material.box-rgb-256x256-radius-1` | 0.255479 | 0.412334 | 0.619593 |
| `pipeline-chain.blur-material.box-rgba-256x256-radius-2` | 0.260687 | 0.510604 | 0.510546 |
| `pipeline-chain.point-fusion.l-001` | 0.080313 | 0.087333 | 0.919607 |
| `pipeline-chain.point-fusion.l-002` | 0.079687 | 0.082208 | 0.969340 |
| `pipeline-chain.point-fusion.l-003` | 0.079937 | 0.093229 | 0.857432 |
| `pipeline-chain.point-fusion.l-004` | 0.080521 | 0.082312 | 0.978241 |
| `pipeline-chain.point-fusion.rgb-001` | 0.207021 | 0.207562 | 0.997394 |
| `pipeline-chain.point-fusion.rgb-002` | 0.203417 | 0.208667 | 0.974838 |
| `pipeline-chain.point-fusion.rgb-003` | 0.208521 | 0.213188 | 0.978109 |
| `pipeline-chain.point-fusion.rgb-004` | 0.219875 | 0.241688 | 0.909747 |
| `pipeline-chain.point-fusion.rgb-005` | 0.206000 | 0.213813 | 0.963459 |
| `pipeline-chain.point-fusion.la-001` | 0.360875 | 0.375646 | 0.960678 |
| `pipeline-chain.point-fusion.la-002` | 0.544083 | 0.795062 | 0.684328 |
| `pipeline-chain.point-fusion.rgba-002` | 0.864875 | 1.449959 | 0.596482 |
| `pipeline-chain.alpha-composite.la-256x256` | 0.127333 | 0.237167 | 0.536895 |
| `pipeline-chain.alpha-composite.rgba-256x256` | 0.226021 | 0.346708 | 0.651906 |
| `pipeline-chain.alpha-composite.la-1024x768` | 0.828125 | 2.780187 | 0.297867 |
| `pipeline-chain.alpha-composite.rgba-1024x768` | 1.526750 | 4.521375 | 0.337674 |
| `pipeline-chain.simd-crossover.invert-mirror.1x1` | 0.014771 | 0.017042 | 0.866766 |
| `pipeline-chain.simd-crossover.invert-mirror.32x32` | 0.016667 | 0.018625 | 0.894822 |
| `pipeline-chain.simd-chops.darker-rgb` | 1.606584 | 2.922625 | 0.549706 |
| `pipeline-chain.simd-chops.lighter-rgb` | 1.600854 | 3.034333 | 0.527580 |
| `pipeline-chain.simd-chops.difference-rgb` | 1.673209 | 2.721125 | 0.614896 |
| `pipeline-chain.simd-chops.add-modulo-rgb` | 1.523230 | 2.932562 | 0.519419 |
| `pipeline-chain.simd-chops.subtract-modulo-rgb` | 1.695187 | 2.894626 | 0.585633 |
| `pipeline-chain.simd-chops.logical-and-1` | 1.069229 | 1.105521 | 0.967172 |
| `pipeline-chain.simd-chops.logical-xor-1` | 2.375104 | 2.633646 | 0.901831 |
| `pipeline-chain.simd-chops.logical-or-1` | 2.286583 | 2.544771 | 0.898542 |
| `pipeline-chain.fused-chops.multiply-screen.l.256x256` | 0.058438 | 0.149688 | 0.390395 |
| `pipeline-chain.fused-chops.multiply-screen.l.1024x1024` | 0.550917 | 0.765416 | 0.719760 |
| `pipeline-chain.fused-chops.multiply-screen.la.256x256` | 0.076416 | 0.237958 | 0.321132 |
| `pipeline-chain.fused-chops.multiply-screen.la.1024x1024` | 1.126354 | 1.339417 | 0.840929 |
| `pipeline-chain.fused-chops.multiply-screen.rgb.256x256` | 0.099166 | 0.376646 | 0.263288 |
| `pipeline-chain.fused-chops.multiply-screen.rgb.1024x1024` | 2.097728 | 2.456958 | 0.853791 |
| `pipeline-chain.fused-chops.multiply-screen.rgba.256x256` | 0.075833 | 0.366417 | 0.206960 |
| `pipeline-chain.fused-chops.multiply-screen.rgba.1024x1024` | 2.168541 | 2.606750 | 0.831895 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.l.1024x1024` | 0.537708 | 0.884230 | 0.608110 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024` | 1.207583 | 1.538833 | 0.784739 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgb.1024x1024` | 2.589479 | 2.937063 | 0.881656 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgba.1024x1024` | 2.265625 | 2.900688 | 0.781065 |
| `pipeline-chain.simd-constant.32x32` | 0.013500 | 0.013812 | 0.977411 |
| `pipeline-chain.simd-constant.1024x768` | 0.386355 | 0.435584 | 0.886981 |
| `pipeline-chain.simd-constant.1024x1024` | 0.469876 | 0.617458 | 0.760984 |
| `pipeline-chain.simd-lut.l.32x32` | 0.078146 | 0.089063 | 0.877424 |
| `pipeline-chain.simd-lut.l.256x256` | 0.128083 | 0.276812 | 0.462708 |
| `pipeline-chain.simd-lut.l.1024x768` | 0.387271 | 2.386729 | 0.162260 |
| `pipeline-chain.simd-lut.l.1024x1024` | 0.422980 | 3.256895 | 0.129872 |
| `pipeline-chain.simd-lut.rgb.32x32` | 0.212542 | 0.220355 | 0.964546 |
| `pipeline-chain.simd-lut.rgb.256x256` | 0.379479 | 0.781667 | 0.485474 |
| `pipeline-chain.simd-lut.rgb.1024x768` | 2.352833 | 7.985917 | 0.294623 |
| `pipeline-chain.simd-lut.rgb.1024x1024` | 2.937500 | 9.352396 | 0.314091 |
| `pipeline-chain.resize-cache.identical-geometry` | 2.399937 | 40.936292 | 0.058626 |
| `pipeline-chain.resize-cache.f64-identical-geometry` | 0.491791 | 1.057708 | 0.464960 |
| `pipeline-chain.long-auxiliary.multiply-screen-260` | 1.557958 | 1.796438 | 0.867248 |
| `pipeline-chain.loaded-10.rgb-jpeg-512x384` | 4.208729 | 7.245437 | 0.580880 |
| `pipeline-chain.long-point.invert-1` | 0.089750 | 0.109333 | 0.820887 |
| `pipeline-chain.long-point.invert-1024` | 2.758750 | 2.902583 | 0.950447 |
| `pipeline-chain.long-point.invert-10000` | 36.529084 | 37.818958 | 0.965893 |
| `pipeline.quick.multiply-screen.rgb-1024` | 2.279041 | 2.494021 | 0.913802 |
| `pipeline-lifecycle.cold.multiply-screen.rgb-1024` | 2.690125 | 3.351000 | 0.802783 |

### 30.5 Material SIMD cohort below 1.25× against CPU or Pillow

Cohort: actual CPU+SIMD receipt intersection, image area at least 65,536 pixels, and either chain length at least 2 or operation class in `point`, `neighborhood`, `geometry`, `multi_image`, or `draw`. This yields 175 workloads. A workload passes only if SIMD is at least 1.25× faster than **both** CPU and Pillow. Result: **146 violations**, 29 passes. Individually, 137 miss the CPU comparison and 104 miss the Pillow comparison.

| Workload ID | Size | Mode | Chain | Class | Pillow ms | CPU ms | SIMD ms | CPU / SIMD | Pillow / SIMD | Failed comparator |
|---|---:|---|---:|---|---:|---:|---:|---:|---:|---|
| `pil-imagechops.multiply.standard` | 1024x1024 | RGB | 2 | point | 6.619020 | 2.279041 | 2.494021 | 0.913802 | 2.653955 | CPU |
| `pil-imagefilter.gaussianblur.standard` | 1024x1024 | RGB | 2 | neighborhood | 8.999688 | 7.278250 | 6.476501 | 1.123794 | 1.389591 | CPU |
| `pipeline-matrix.expanded.resize.256x256` | 256x256 | RGB | 1 | geometry | 0.232167 | 0.199146 | 1.373854 | 0.144954 | 0.168990 | CPU+Pillow |
| `pipeline-matrix.expanded.resize.1024x768` | 1024x768 | RGB | 1 | geometry | 2.611792 | 0.864146 | 16.073604 | 0.053762 | 0.162490 | CPU+Pillow |
| `pipeline-matrix.expanded.rotate.256x256` | 256x256 | RGB | 1 | geometry | 0.108229 | 0.274834 | 0.330438 | 0.831725 | 0.327532 | CPU+Pillow |
| `pipeline-matrix.expanded.rotate.1024x768` | 1024x768 | RGB | 1 | geometry | 1.377417 | 1.352229 | 4.437729 | 0.304712 | 0.310388 | CPU+Pillow |
| `pipeline-matrix.expanded.reduce.256x256` | 256x256 | RGB | 1 | geometry | 0.059646 | 0.204437 | 0.257395 | 0.794254 | 0.231729 | CPU+Pillow |
| `pipeline-matrix.expanded.reduce.1024x768` | 1024x768 | RGB | 1 | geometry | 0.661250 | 0.624271 | 3.074063 | 0.203077 | 0.215106 | CPU+Pillow |
| `pipeline-matrix.expanded.filter3x3.256x256` | 256x256 | RGB | 1 | neighborhood | 0.379833 | 0.249896 | 0.285854 | 0.874209 | 1.328766 | CPU |
| `pipeline-matrix.expanded.filter3x3.1024x768` | 1024x768 | RGB | 1 | neighborhood | 5.044458 | 2.127271 | 2.368083 | 0.898309 | 2.130187 | CPU |
| `pipeline-matrix.expanded.filter5x5.256x256` | 256x256 | RGB | 1 | neighborhood | 0.883979 | 0.472042 | 0.394958 | 1.195167 | 2.238158 | CPU |
| `pipeline-matrix.expanded.gaussianblur.256x256` | 256x256 | RGB | 1 | neighborhood | 0.487020 | 0.581187 | 0.730458 | 0.795648 | 0.666733 | CPU+Pillow |
| `pipeline-matrix.expanded.gaussianblur.1024x768` | 1024x768 | RGB | 1 | neighborhood | 6.291771 | 5.211792 | 4.602896 | 1.132285 | 1.366916 | CPU |
| `pipeline-matrix.expanded.boxblur.256x256` | 256x256 | RGB | 1 | neighborhood | 0.239937 | 0.267730 | 0.541167 | 0.494726 | 0.443371 | CPU+Pillow |
| `pipeline-matrix.expanded.boxblur.1024x768` | 1024x768 | RGB | 1 | neighborhood | 3.195104 | 2.777521 | 2.653021 | 1.046928 | 1.204327 | CPU+Pillow |
| `pipeline-matrix.expanded.medianfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 3.189855 | 0.402437 | 2.350917 | 0.171183 | 1.356856 | CPU |
| `pipeline-matrix.expanded.medianfilter.1024x768` | 1024x768 | RGB | 1 | neighborhood | 38.245937 | 3.343749 | 27.561417 | 0.121320 | 1.387662 | CPU |
| `pipeline-matrix.expanded.maxfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 4.191958 | 0.301667 | 1.139792 | 0.264668 | 3.677829 | CPU |
| `pipeline-matrix.expanded.maxfilter.1024x768` | 1024x768 | RGB | 1 | neighborhood | 48.959729 | 2.511563 | 13.351312 | 0.188114 | 3.667035 | CPU |
| `pipeline-matrix.expanded.minfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 4.093541 | 0.271208 | 1.099646 | 0.246632 | 3.722601 | CPU |
| `pipeline-matrix.expanded.minfilter.1024x768` | 1024x768 | RGB | 1 | neighborhood | 47.801688 | 2.358771 | 13.254167 | 0.177964 | 3.606540 | CPU |
| `pipeline-matrix.expanded.effectspread.256x256` | 256x256 | RGB | 1 | neighborhood | 0.724042 | 0.487834 | 0.786563 | 0.620209 | 0.920514 | CPU+Pillow |
| `pipeline-matrix.expanded.effectspread.1024x768` | 1024x768 | RGB | 1 | neighborhood | 9.525562 | 7.073313 | 9.226125 | 0.766661 | 1.032455 | CPU+Pillow |
| `pipeline-matrix.expanded.invert.256x256` | 256x256 | RGB | 1 | point | 0.138958 | 0.042271 | 0.116522 | 0.362774 | 1.192557 | CPU+Pillow |
| `pipeline-matrix.expanded.invert.1024x768` | 1024x768 | RGB | 1 | point | 1.298916 | 0.786792 | 0.837959 | 0.938939 | 1.550096 | CPU |
| `pipeline-matrix.expanded.grayscale.256x256` | 256x256 | RGB | 1 | point | 0.028625 | 0.106521 | 0.167542 | 0.635787 | 0.170853 | CPU+Pillow |
| `pipeline-matrix.expanded.grayscale.1024x768` | 1024x768 | RGB | 1 | point | 0.406375 | 1.175187 | 2.057688 | 0.571120 | 0.197491 | CPU+Pillow |
| `pipeline-matrix.expanded.autocontrast.256x256` | 256x256 | RGB | 1 | point | 0.276604 | 0.243521 | 0.535791 | 0.454507 | 0.516253 | CPU+Pillow |
| `pipeline-matrix.expanded.autocontrast.1024x768` | 1024x768 | RGB | 1 | point | 2.805208 | 1.707437 | 6.891709 | 0.247752 | 0.407041 | CPU+Pillow |
| `pipeline-matrix.expanded.equalize.256x256` | 256x256 | RGB | 1 | point | 0.254271 | 0.090375 | 0.597771 | 0.151187 | 0.425364 | CPU+Pillow |
| `pipeline-matrix.expanded.equalize.1024x768` | 1024x768 | RGB | 1 | point | 2.904708 | 1.366375 | 7.482416 | 0.182611 | 0.388205 | CPU+Pillow |
| `pipeline-matrix.expanded.eval.256x256` | 256x256 | RGB | 1 | point | 0.223542 | 0.363104 | 0.658834 | 0.551132 | 0.339299 | CPU+Pillow |
| `pipeline-matrix.expanded.eval.1024x768` | 1024x768 | RGB | 1 | point | 1.509917 | 2.210375 | 6.412520 | 0.344697 | 0.235464 | CPU+Pillow |
| `pipeline-matrix.expanded.pointop.256x256` | 256x256 | L | 1 | point | 0.067562 | 0.105333 | 0.254146 | 0.414459 | 0.265841 | CPU+Pillow |
| `pipeline-matrix.expanded.pointop.1024x768` | 1024x768 | L | 1 | point | 0.385792 | 0.321396 | 2.215791 | 0.145048 | 0.174110 | CPU+Pillow |
| `pipeline-matrix.expanded.multiply.256x256` | 256x256 | RGB | 1 | multi_image | 0.173729 | 0.088438 | 0.190854 | 0.463379 | 0.910272 | CPU+Pillow |
| `pipeline-matrix.expanded.multiply.1024x768` | 1024x768 | RGB | 1 | multi_image | 2.300187 | 1.160417 | 1.365875 | 0.849578 | 1.684039 | CPU |
| `pipeline-matrix.expanded.screen.256x256` | 256x256 | RGB | 1 | multi_image | 0.169666 | 0.077333 | 0.258062 | 0.299668 | 0.657464 | CPU+Pillow |
| `pipeline-matrix.expanded.screen.1024x768` | 1024x768 | RGB | 1 | multi_image | 2.107563 | 1.202228 | 1.430625 | 0.840352 | 1.473176 | CPU |
| `pipeline-matrix.expanded.add.256x256` | 256x256 | RGB | 1 | multi_image | 0.187771 | 0.241541 | 0.166646 | 1.449429 | 1.126766 | Pillow |
| `pipeline-matrix.expanded.add.1024x768` | 1024x768 | RGB | 1 | multi_image | 2.670791 | 1.528813 | 2.138916 | 0.714760 | 1.248666 | CPU+Pillow |
| `pipeline-matrix.expanded.darker.256x256` | 256x256 | RGB | 1 | multi_image | 0.124354 | 0.069333 | 0.159791 | 0.433900 | 0.778230 | CPU+Pillow |
| `pipeline-matrix.expanded.darker.1024x768` | 1024x768 | RGB | 1 | multi_image | 1.811146 | 1.138250 | 1.966417 | 0.578845 | 0.921039 | CPU+Pillow |
| `pipeline-matrix.expanded.brightness.256x256` | 256x256 | RGB | 1 | point | 0.055917 | 0.165750 | 0.089646 | 1.848939 | 0.623748 | Pillow |
| `pipeline-matrix.expanded.brightness.1024x768` | 1024x768 | RGB | 1 | point | 1.094562 | 0.961938 | 1.092167 | 0.880761 | 1.002194 | CPU+Pillow |
| `pipeline-chain.terminal-read.rgb-band0` | 512x512 | RGB | 2 | terminal | 0.391792 | 1.561625 | 1.568709 | 0.995485 | 0.249754 | CPU+Pillow |
| `pipeline-chain.terminal-read.analysis-scalar-if-1024x768` | 1024x768 | I+F | 14 | terminal | 5.664437 | 18.085625 | 18.149479 | 0.996482 | 0.312099 | CPU+Pillow |
| `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768` | 1024x768 | L | 7 | terminal | 3.626917 | 4.843646 | 6.937688 | 0.698164 | 0.522785 | CPU+Pillow |
| `pipeline-chain.terminal-read.getcolors.rgb-1024x768` | 1024x768 | RGB | 4 | terminal | 1.230750 | 8.622521 | 9.630104 | 0.895372 | 0.127802 | CPU+Pillow |
| `pipeline-chain.terminal-read.imagestat.i-1024x768` | 1024x768 | I | 13 | terminal | 1.440749 | 3.705271 | 3.980437 | 0.930870 | 0.361958 | CPU+Pillow |
| `pipeline-chain.terminal-read.imagestat.cmyk-1024x768` | 1024x768 | CMYK | 13 | terminal | 1.946041 | 4.475896 | 2.878042 | 1.555188 | 0.676169 | Pillow |
| `pipeline-chain.metadata-cache.invert-1.rgb` | 256x256 | RGB | 7 | point | 0.157479 | 0.049396 | 0.134125 | 0.368283 | 1.174121 | CPU+Pillow |
| `pipeline-chain.metadata-cache.invert-8.l` | 256x256 | L | 14 | point | 0.296583 | 0.076625 | 0.214291 | 0.357574 | 1.384017 | CPU |
| `pipeline-chain.metadata-cache.invert-64.rgb` | 256x256 | RGB | 70 | point | 5.653937 | 0.363938 | 0.757396 | 0.480512 | 7.464969 | CPU |
| `pipeline-chain.metadata-cache.color3dlut-rgb` | 256x256 | RGB | 7 | point | 0.757772 | 1.345104 | 1.016541 | 1.323216 | 0.745441 | Pillow |
| `pipeline-chain.metadata-cache.extractband-rgba` | 256x256 | RGBA | 7 | point | 0.049500 | 0.029791 | 0.159749 | 0.186489 | 0.309860 | CPU+Pillow |
| `pipeline-chain.rank-filter.material.f-9x9-256x256` | 256x256 | F | 1 | neighborhood | 4.838437 | 1.447812 | 25.506042 | 0.056764 | 0.189698 | CPU+Pillow |
| `pipeline-chain.rank-filter.material.l-9x9-256x256` | 256x256 | L | 1 | neighborhood | 2.936646 | 0.624000 | 7.688292 | 0.081162 | 0.381963 | CPU+Pillow |
| `pipeline-chain.convolution.material.l-3x3-invert.256x256` | 256x256 | L | 2 | neighborhood | 0.172230 | 0.178063 | 0.250479 | 0.710888 | 0.687601 | CPU+Pillow |
| `pipeline-chain.convolution.material.la-3x3-alpha.256x256` | 256x256 | LA | 2 | neighborhood | 0.458041 | 0.279646 | 0.274166 | 1.019986 | 1.670669 | CPU |
| `pipeline-chain.convolution.material.l-5x5-scale.256x256` | 256x256 | L | 2 | neighborhood | 0.370125 | 0.555167 | 0.560563 | 0.990375 | 0.660275 | CPU+Pillow |
| `pipeline-chain.convolution.material.rgb-5x5-pad.256x256` | 256x256 | RGB | 2 | neighborhood | 0.833438 | 0.478104 | 0.410729 | 1.164038 | 2.029166 | CPU |
| `pipeline-chain.convolution.material.rgba-3x3-transpose.256x256` | 256x256 | RGBA | 2 | neighborhood | 0.476417 | 0.375209 | 0.608021 | 0.617098 | 0.783554 | CPU+Pillow |
| `pipeline-chain.convolution.crossover.la-3x3-alpha.512x512` | 512x512 | LA | 2 | neighborhood | 1.826271 | 0.696458 | 0.621812 | 1.120046 | 2.937012 | CPU |
| `pipeline-chain.convolution.crossover.l-5x5-scale.512x512` | 512x512 | L | 2 | neighborhood | 1.487938 | 1.935854 | 1.700896 | 1.138138 | 0.874796 | CPU+Pillow |
| `pipeline-chain.convolution.native.rgb-3x3-mirror.1024x768` | 1024x768 | RGB | 2 | neighborhood | 5.535792 | 3.348583 | 2.680313 | 1.249326 | 2.065353 | CPU |
| `pipeline-chain.convolution.native.rgba-3x3-transpose.1024x768` | 1024x768 | RGBA | 2 | neighborhood | 6.708916 | 2.957688 | 3.629167 | 0.814977 | 1.848611 | CPU |
| `pipeline-chain.convolution.native.l-5x5-scale.1024x768` | 1024x768 | L | 2 | neighborhood | 4.525187 | 5.571584 | 4.783834 | 1.164669 | 0.945933 | CPU+Pillow |
| `pipeline-chain.convolution.native.rgba-5x5-invert.1024x768` | 1024x768 | RGBA | 1 | neighborhood | 13.502771 | 5.049333 | 4.395625 | 1.148718 | 3.071866 | CPU |
| `pipeline-chain.convolution-i.3x3-1024x768` | 1024x768 | I | 1 | neighborhood | 2.090499 | 1.849105 | 1.716896 | 1.077004 | 1.217604 | CPU+Pillow |
| `pipeline-chain.resize-alpha.rgba-lanczos-256x256` | 256x256 | RGBA | 1 | point | 0.621292 | 0.377729 | 5.060375 | 0.074645 | 0.122776 | CPU+Pillow |
| `pipeline-chain.resize-alpha.la-bicubic-256x256` | 256x256 | LA | 1 | point | 0.393208 | 0.219479 | 1.719313 | 0.127655 | 0.228701 | CPU+Pillow |
| `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256` | 256x256 | RGBA | 2 | point | 0.603395 | 0.453541 | 2.998396 | 0.151261 | 0.201239 | CPU+Pillow |
| `pipeline-chain.resize-alpha.la-bilinear-mirror-256x256` | 256x256 | LA | 2 | point | 0.430000 | 0.343437 | 1.565500 | 0.219378 | 0.274673 | CPU+Pillow |
| `pipeline-chain.resize-alpha.rgba-bicubic-512x512` | 512x512 | RGBA | 1 | point | 2.306687 | 0.825396 | 15.314875 | 0.053895 | 0.150617 | CPU+Pillow |
| `pipeline-chain.resize-alpha.la-lanczos-512x512` | 512x512 | LA | 1 | point | 1.932333 | 0.526021 | 10.011312 | 0.052543 | 0.193015 | CPU+Pillow |
| `pipeline-chain.resize-alpha.rgba-lanczos-1024x768` | 1024x768 | RGBA | 1 | point | 8.243542 | 3.742230 | 60.080958 | 0.062286 | 0.137207 | CPU+Pillow |
| `pipeline-chain.resize-alpha.la-bicubic-1024x768` | 1024x768 | LA | 1 | point | 4.739292 | 1.791791 | 23.090958 | 0.077597 | 0.205244 | CPU+Pillow |
| `pipeline-chain.geometry-material.transpose-rgba-1024x768` | 1024x768 | RGBA | 1 | point | 0.950792 | 1.793270 | 1.136667 | 1.577658 | 0.836474 | Pillow |
| `pipeline-chain.geometry-material.transverse-rgb-1024x768` | 1024x768 | RGB | 1 | point | 1.126667 | 2.032375 | 1.106313 | 1.837071 | 1.018398 | Pillow |
| `pipeline-chain.geometry-material.crop-rgb-1024x768` | 1024x768 | RGB | 1 | point | 0.756938 | 0.469896 | 0.612416 | 0.767281 | 1.235985 | CPU+Pillow |
| `pipeline-chain.geometry-material.reduce-rgb-1024x768` | 1024x768 | RGB | 1 | point | 0.845896 | 0.586729 | 2.554646 | 0.229671 | 0.331121 | CPU+Pillow |
| `pipeline-chain.geometry-material.reduce-rgba-1024x768` | 1024x768 | RGBA | 1 | point | 1.465396 | 0.470771 | 4.120020 | 0.114264 | 0.355677 | CPU+Pillow |
| `pipeline-chain.geometry-material.rotate-rgba-1024x768` | 1024x768 | RGBA | 1 | point | 2.414354 | 2.166104 | 7.771104 | 0.278738 | 0.310684 | CPU+Pillow |
| `pipeline-chain.geometry-copy.crop-l-1024x768` | 1024x768 | L | 1 | point | 0.086563 | 0.091541 | 0.089792 | 1.019478 | 0.964034 | CPU+Pillow |
| `pipeline-chain.geometry-copy.crop-la-1024x768` | 1024x768 | LA | 1 | point | 0.501188 | 0.223729 | 0.205792 | 1.087166 | 2.435414 | CPU |
| `pipeline-chain.geometry-copy.crop-rgb-1024x768` | 1024x768 | RGB | 1 | point | 0.737833 | 0.536583 | 0.505812 | 1.060835 | 1.458709 | CPU |
| `pipeline-chain.geometry-copy.crop-rgba-1024x768` | 1024x768 | RGBA | 1 | point | 0.622646 | 0.482417 | 0.461730 | 1.044804 | 1.348508 | CPU |
| `pipeline-chain.geometry-copy.cropborder-l-1024x768` | 1024x768 | L | 1 | point | 0.139792 | 0.471916 | 0.115834 | 4.074089 | 1.206836 | Pillow |
| `pipeline-chain.geometry-copy.cropborder-rgba-1024x768` | 1024x768 | RGBA | 1 | point | 0.649458 | 1.223771 | 0.550854 | 2.221588 | 1.179002 | Pillow |
| `pipeline-chain.geometry-copy.crop-chain-rgb-1024x768` | 1024x768 | RGB | 2 | point | 0.719792 | 0.607125 | 0.597000 | 1.016961 | 1.205683 | CPU+Pillow |
| `pipeline-chain.blur-material.box-rgb-1024x1024` | 1024x1024 | RGB | 1 | neighborhood | 4.364999 | 3.662771 | 3.499271 | 1.046724 | 1.247403 | CPU+Pillow |
| `pipeline-chain.blur-material.gaussian-rgba-1024x768` | 1024x768 | RGBA | 1 | neighborhood | 6.102646 | 8.185771 | 5.487626 | 1.491678 | 1.112074 | Pillow |
| `pipeline-chain.blur-material.gaussian-l-256x256-radius-0.5` | 256x256 | L | 1 | neighborhood | 0.349166 | 0.396042 | 0.593666 | 0.667112 | 0.588153 | CPU+Pillow |
| `pipeline-chain.blur-material.gaussian-rgb-256x256-radius-1` | 256x256 | RGB | 1 | neighborhood | 0.528021 | 0.524063 | 0.763583 | 0.686320 | 0.691503 | CPU+Pillow |
| `pipeline-chain.blur-material.gaussian-rgb-1024x768-radius-4` | 1024x768 | RGB | 1 | neighborhood | 6.221729 | 5.358396 | 4.738625 | 1.130791 | 1.312982 | CPU |
| `pipeline-chain.blur-material.gaussian-rgba-256x256-radius-2` | 256x256 | RGBA | 1 | neighborhood | 0.484604 | 0.621687 | 0.840521 | 0.739645 | 0.576553 | CPU+Pillow |
| `pipeline-chain.blur-material.box-l-256x256-radius-0.5` | 256x256 | L | 1 | neighborhood | 0.160626 | 0.171438 | 0.338375 | 0.506650 | 0.474697 | CPU+Pillow |
| `pipeline-chain.blur-material.box-rgb-256x256-radius-1` | 256x256 | RGB | 1 | neighborhood | 0.247625 | 0.255479 | 0.412334 | 0.619593 | 0.600545 | CPU+Pillow |
| `pipeline-chain.blur-material.box-rgb-1024x768-radius-4` | 1024x768 | RGB | 1 | neighborhood | 3.059729 | 2.876021 | 2.554499 | 1.125865 | 1.197780 | CPU+Pillow |
| `pipeline-chain.blur-material.box-la-1024x768-radius-4` | 1024x768 | LA | 1 | neighborhood | 2.780833 | 2.021687 | 1.653896 | 1.222379 | 1.681383 | CPU |
| `pipeline-chain.blur-material.box-rgba-1024x768-radius-4` | 1024x768 | RGBA | 1 | neighborhood | 3.255084 | 3.098354 | 2.935417 | 1.055507 | 1.108900 | CPU+Pillow |
| `pipeline-chain.blur-material.box-rgba-256x256-radius-2` | 256x256 | RGBA | 1 | neighborhood | 0.277000 | 0.260687 | 0.510604 | 0.510546 | 0.542495 | CPU+Pillow |
| `pipeline-chain.point-fusion.la-002` | 256x256 | LA | 3 | point | 0.332541 | 0.544083 | 0.795062 | 0.684328 | 0.418259 | CPU+Pillow |
| `pipeline-chain.point-fusion.rgba-002` | 256x256 | RGBA | 3 | point | 0.591271 | 0.864875 | 1.449959 | 0.596482 | 0.407784 | CPU+Pillow |
| `pipeline-chain.alpha-composite.la-256x256` | 256x256 | LA | 1 | point | 0.199000 | 0.127333 | 0.237167 | 0.536895 | 0.839073 | CPU+Pillow |
| `pipeline-chain.alpha-composite.rgba-256x256` | 256x256 | RGBA | 1 | point | 0.202937 | 0.226021 | 0.346708 | 0.651906 | 0.585327 | CPU+Pillow |
| `pipeline-chain.alpha-composite.la-1024x768` | 1024x768 | LA | 1 | point | 2.663021 | 0.828125 | 2.780187 | 0.297867 | 0.957857 | CPU+Pillow |
| `pipeline-chain.alpha-composite.rgba-1024x768` | 1024x768 | RGBA | 1 | point | 2.772709 | 1.526750 | 4.521375 | 0.337674 | 0.613245 | CPU+Pillow |
| `pipeline-chain.simd-crossover.invert-mirror.256x256` | 256x256 | RGB | 2 | point | 0.180042 | 0.156000 | 0.148146 | 1.053012 | 1.215298 | CPU+Pillow |
| `pipeline-chain.simd-chops.darker-rgb` | 1024x1024 | RGB | 1 | point | 3.201021 | 1.606584 | 2.922625 | 0.549706 | 1.095256 | CPU+Pillow |
| `pipeline-chain.simd-chops.lighter-rgb` | 1024x1024 | RGB | 1 | point | 2.992521 | 1.600854 | 3.034333 | 0.527580 | 0.986220 | CPU+Pillow |
| `pipeline-chain.simd-chops.difference-rgb` | 1024x1024 | RGB | 1 | point | 3.942708 | 1.673209 | 2.721125 | 0.614896 | 1.448926 | CPU |
| `pipeline-chain.simd-chops.add-modulo-rgb` | 1024x1024 | RGB | 1 | point | 2.938313 | 1.523230 | 2.932562 | 0.519419 | 1.001961 | CPU+Pillow |
| `pipeline-chain.simd-chops.subtract-modulo-rgb` | 1024x1024 | RGB | 1 | point | 2.773542 | 1.695187 | 2.894626 | 0.585633 | 0.958169 | CPU+Pillow |
| `pipeline-chain.simd-chops.logical-and-1` | 1024x1024 | 1 | 1 | point | 0.800500 | 1.069229 | 1.105521 | 0.967172 | 0.724093 | CPU+Pillow |
| `pipeline-chain.simd-chops.logical-xor-1` | 1024x1024 | 1 | 1 | point | 0.789042 | 2.375104 | 2.633646 | 0.901831 | 0.299601 | CPU+Pillow |
| `pipeline-chain.simd-chops.logical-or-1` | 1024x1024 | 1 | 1 | point | 0.626604 | 2.286583 | 2.544771 | 0.898542 | 0.246232 | CPU+Pillow |
| `pipeline-chain.fused-chops.multiply-screen.l.256x256` | 256x256 | L | 2 | point | 0.094708 | 0.058438 | 0.149688 | 0.390395 | 0.632703 | CPU+Pillow |
| `pipeline-chain.fused-chops.multiply-screen.l.1024x1024` | 1024x1024 | L | 2 | point | 1.353770 | 0.550917 | 0.765416 | 0.719760 | 1.768672 | CPU |
| `pipeline-chain.fused-chops.multiply-screen.la.256x256` | 256x256 | LA | 2 | point | 0.380979 | 0.076416 | 0.237958 | 0.321132 | 1.601035 | CPU |
| `pipeline-chain.fused-chops.multiply-screen.la.1024x1024` | 1024x1024 | LA | 2 | point | 6.751271 | 1.126354 | 1.339417 | 0.840929 | 5.040457 | CPU |
| `pipeline-chain.fused-chops.multiply-screen.rgb.256x256` | 256x256 | RGB | 2 | point | 0.438104 | 0.099166 | 0.376646 | 0.263288 | 1.163172 | CPU+Pillow |
| `pipeline-chain.fused-chops.multiply-screen.rgb.1024x1024` | 1024x1024 | RGB | 2 | point | 6.609208 | 2.097728 | 2.456958 | 0.853791 | 2.689996 | CPU |
| `pipeline-chain.fused-chops.multiply-screen.rgba.256x256` | 256x256 | RGBA | 2 | point | 0.386333 | 0.075833 | 0.366417 | 0.206960 | 1.054356 | CPU+Pillow |
| `pipeline-chain.fused-chops.multiply-screen.rgba.1024x1024` | 1024x1024 | RGBA | 2 | point | 6.873292 | 2.168541 | 2.606750 | 0.831895 | 2.636728 | CPU |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.l.1024x1024` | 1024x1024 | L | 2 | point | 1.421771 | 0.537708 | 0.884230 | 0.608110 | 1.607920 | CPU |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024` | 1024x1024 | LA | 2 | point | 7.230313 | 1.207583 | 1.538833 | 0.784739 | 4.698567 | CPU |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgb.1024x1024` | 1024x1024 | RGB | 2 | point | 7.058749 | 2.589479 | 2.937063 | 0.881656 | 2.403336 | CPU |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgba.1024x1024` | 1024x1024 | RGBA | 2 | point | 7.350167 | 2.265625 | 2.900688 | 0.781065 | 2.533940 | CPU |
| `pipeline-chain.simd-constant.256x256` | 256x256 | RGB | 1 | point | 0.036583 | 0.035000 | 0.034959 | 1.001187 | 1.046469 | CPU+Pillow |
| `pipeline-chain.simd-constant.1024x768` | 1024x768 | RGB | 1 | point | 0.455146 | 0.386355 | 0.435584 | 0.886981 | 1.044911 | CPU+Pillow |
| `pipeline-chain.simd-constant.1024x1024` | 1024x1024 | RGB | 1 | point | 0.480834 | 0.469876 | 0.617458 | 0.760984 | 0.778732 | CPU+Pillow |
| `pipeline-chain.simd-lut.l.256x256` | 256x256 | L | 4 | point | 0.185042 | 0.128083 | 0.276812 | 0.462708 | 0.668474 | CPU+Pillow |
| `pipeline-chain.simd-lut.l.1024x768` | 1024x768 | L | 4 | point | 1.187271 | 0.387271 | 2.386729 | 0.162260 | 0.497447 | CPU+Pillow |
| `pipeline-chain.simd-lut.l.1024x1024` | 1024x1024 | L | 4 | point | 1.559709 | 0.422980 | 3.256895 | 0.129872 | 0.478894 | CPU+Pillow |
| `pipeline-chain.simd-lut.rgb.256x256` | 256x256 | RGB | 4 | point | 0.518124 | 0.379479 | 0.781667 | 0.485474 | 0.662846 | CPU+Pillow |
| `pipeline-chain.simd-lut.rgb.1024x768` | 1024x768 | RGB | 4 | point | 3.731896 | 2.352833 | 7.985917 | 0.294623 | 0.467310 | CPU+Pillow |
| `pipeline-chain.simd-lut.rgb.1024x1024` | 1024x1024 | RGB | 4 | point | 5.362562 | 2.937500 | 9.352396 | 0.314091 | 0.573389 | CPU+Pillow |
| `pipeline-chain.resize-cache.identical-geometry` | 1024x768 | RGB | 2 | point | 8.175166 | 2.399937 | 40.936292 | 0.058626 | 0.199705 | CPU+Pillow |
| `pipeline-chain.resize-cache.f64-identical-geometry` | 333x257 | F | 2 | point | 0.311229 | 0.491791 | 1.057708 | 0.464960 | 0.294249 | CPU+Pillow |
| `pipeline-chain.loaded-10.rgb-jpeg-512x384` | 512x384 | RGB | 10 | geometry | 4.095583 | 4.208729 | 7.245437 | 0.580880 | 0.565264 | CPU+Pillow |
| `pipeline.quick.gaussianblur-invert.rgb-1024` | 1024x1024 | RGB | 2 | neighborhood | 8.999688 | 7.278250 | 6.476501 | 1.123794 | 1.389591 | CPU |
| `pipeline.quick.multiply-screen.rgb-1024` | 1024x1024 | RGB | 2 | multi_image | 6.619020 | 2.279041 | 2.494021 | 0.913802 | 2.653955 | CPU |
| `pipeline-lifecycle.cold.gaussianblur-invert.rgb-1024` | 1024x1024 | RGB | 2 | neighborhood | 16.391791 | 7.668667 | 6.556833 | 1.169569 | 2.499956 | CPU |
| `pipeline-lifecycle.cold.multiply-screen.rgb-1024` | 1024x1024 | RGB | 2 | multi_image | 12.818500 | 2.690125 | 3.351000 | 0.802783 | 3.825276 | CPU |

### 30.6 Candidate large GPU cohort below 1.2× against SIMD

Cohort: completed, no-fallback actual SIMD and actual GPU receipt intersection; image area at least 65,536 pixels; and either chain length at least 2 or operation class `neighborhood` or `geometry`. This yields 108 workloads. Gate: GPU should be at least 1.2× faster than SIMD (`SIMD_ms / GPU_ms >= 1.2`). Result: **104 violations**, 4 passes; the same 4 are the only workloads at or above 1.0×.

| Workload ID | Size | Mode | Chain | Class | SIMD ms | GPU ms | SIMD / GPU |
|---|---:|---|---:|---|---:|---:|---:|
| `pil-image-image.transpose.standard` | 1024x1024 | RGB | 2 | point | 1.646521 | 9.741229 | 0.169026 |
| `pil-imagechops.multiply.standard` | 1024x1024 | RGB | 2 | point | 2.494021 | 23.037167 | 0.108261 |
| `pil-imagefilter.gaussianblur.standard` | 1024x1024 | RGB | 2 | neighborhood | 6.476501 | 10.416063 | 0.621780 |
| `pil-imageops.invert.standard` | 1024x1024 | RGB | 2 | point | 1.450855 | 9.649041 | 0.150363 |
| `pipeline-matrix.expanded.resize.256x256` | 256x256 | RGB | 1 | geometry | 1.373854 | 6.736479 | 0.203942 |
| `pipeline-matrix.expanded.transpose.256x256` | 256x256 | RGB | 1 | geometry | 0.045791 | 6.955250 | 0.006584 |
| `pipeline-matrix.expanded.transpose.1024x768` | 1024x768 | RGB | 1 | geometry | 0.658542 | 9.831229 | 0.066985 |
| `pipeline-matrix.expanded.reduce.256x256` | 256x256 | RGB | 1 | geometry | 0.257395 | 6.788479 | 0.037917 |
| `pipeline-matrix.expanded.reduce.1024x768` | 1024x768 | RGB | 1 | geometry | 3.074063 | 7.928979 | 0.387700 |
| `pipeline-matrix.expanded.filter3x3.256x256` | 256x256 | RGB | 1 | neighborhood | 0.285854 | 6.974917 | 0.040983 |
| `pipeline-matrix.expanded.filter3x3.1024x768` | 1024x768 | RGB | 1 | neighborhood | 2.368083 | 10.775917 | 0.219757 |
| `pipeline-matrix.expanded.filter5x5.256x256` | 256x256 | RGB | 1 | neighborhood | 0.394958 | 7.832791 | 0.050424 |
| `pipeline-matrix.expanded.filter5x5.1024x768` | 1024x768 | RGB | 1 | neighborhood | 3.438625 | 10.085895 | 0.340934 |
| `pipeline-matrix.expanded.gaussianblur.256x256` | 256x256 | RGB | 1 | neighborhood | 0.730458 | 7.346063 | 0.099435 |
| `pipeline-matrix.expanded.gaussianblur.1024x768` | 1024x768 | RGB | 1 | neighborhood | 4.602896 | 16.107604 | 0.285759 |
| `pipeline-matrix.expanded.boxblur.256x256` | 256x256 | RGB | 1 | neighborhood | 0.541167 | 7.286771 | 0.074267 |
| `pipeline-matrix.expanded.boxblur.1024x768` | 1024x768 | RGB | 1 | neighborhood | 2.653021 | 10.744312 | 0.246923 |
| `pipeline-matrix.expanded.medianfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 2.350917 | 7.005584 | 0.335578 |
| `pipeline-matrix.expanded.maxfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 1.139792 | 6.871917 | 0.165862 |
| `pipeline-matrix.expanded.minfilter.256x256` | 256x256 | RGB | 1 | neighborhood | 1.099646 | 7.203416 | 0.152656 |
| `pipeline-chain.terminal-read.rgb-band0` | 512x512 | RGB | 2 | terminal | 1.568709 | 10.652833 | 0.147257 |
| `pipeline-chain.terminal-read.analysis-scalar-if-1024x768` | 1024x768 | I+F | 14 | terminal | 18.149479 | 30.186875 | 0.601237 |
| `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768` | 1024x768 | L | 7 | terminal | 6.937688 | 17.743291 | 0.391003 |
| `pipeline-chain.terminal-read.getcolors.rgb-1024x768` | 1024x768 | RGB | 4 | terminal | 9.630104 | 14.373292 | 0.670000 |
| `pipeline-chain.terminal-read.imagestat.i-1024x768` | 1024x768 | I | 13 | terminal | 3.980437 | 9.480146 | 0.419871 |
| `pipeline-chain.terminal-read.imagestat.cmyk-1024x768` | 1024x768 | CMYK | 13 | terminal | 2.878042 | 8.252437 | 0.348750 |
| `pipeline-chain.metadata-cache.invert-1.rgb` | 256x256 | RGB | 7 | point | 0.134125 | 6.618812 | 0.020264 |
| `pipeline-chain.metadata-cache.invert-8.l` | 256x256 | L | 14 | point | 0.214291 | 6.677437 | 0.032092 |
| `pipeline-chain.metadata-cache.invert-64.rgb` | 256x256 | RGB | 70 | point | 0.757396 | 7.172771 | 0.105593 |
| `pipeline-chain.metadata-cache.extractband-rgba` | 256x256 | RGBA | 7 | point | 0.159749 | 7.082604 | 0.022555 |
| `pipeline-chain.convolution.material.l-3x3-invert.256x256` | 256x256 | L | 2 | neighborhood | 0.250479 | 6.985520 | 0.035857 |
| `pipeline-chain.convolution.material.rgb-3x3-mirror.256x256` | 256x256 | RGB | 2 | neighborhood | 0.318688 | 7.800937 | 0.040852 |
| `pipeline-chain.convolution.material.la-3x3-alpha.256x256` | 256x256 | LA | 2 | neighborhood | 0.274166 | 7.530021 | 0.036410 |
| `pipeline-chain.convolution.material.l-5x5-scale.256x256` | 256x256 | L | 2 | neighborhood | 0.560563 | 7.925562 | 0.070728 |
| `pipeline-chain.convolution.material.rgb-5x5-pad.256x256` | 256x256 | RGB | 2 | neighborhood | 0.410729 | 7.694437 | 0.053380 |
| `pipeline-chain.convolution.material.rgba-3x3-transpose.256x256` | 256x256 | RGBA | 2 | neighborhood | 0.608021 | 7.667375 | 0.079300 |
| `pipeline-chain.convolution.crossover.l-3x3-invert.512x512` | 512x512 | L | 2 | neighborhood | 0.356500 | 8.262042 | 0.043149 |
| `pipeline-chain.convolution.crossover.rgb-3x3-mirror.512x512` | 512x512 | RGB | 2 | neighborhood | 0.937312 | 8.125708 | 0.115351 |
| `pipeline-chain.convolution.crossover.la-3x3-alpha.512x512` | 512x512 | LA | 2 | neighborhood | 0.621812 | 8.121292 | 0.076566 |
| `pipeline-chain.convolution.crossover.l-5x5-scale.512x512` | 512x512 | L | 2 | neighborhood | 1.700896 | 8.819396 | 0.192859 |
| `pipeline-chain.convolution.native.l-3x3-invert.1024x768` | 1024x768 | L | 2 | neighborhood | 0.825042 | 9.697271 | 0.085080 |
| `pipeline-chain.convolution.native.rgb-3x3-mirror.1024x768` | 1024x768 | RGB | 2 | neighborhood | 2.680313 | 9.722500 | 0.275681 |
| `pipeline-chain.convolution.native.la-3x3-alpha.1024x768` | 1024x768 | LA | 2 | neighborhood | 1.665437 | 10.224479 | 0.162887 |
| `pipeline-chain.convolution.native.rgba-3x3-transpose.1024x768` | 1024x768 | RGBA | 2 | neighborhood | 3.629167 | 8.482709 | 0.427831 |
| `pipeline-chain.convolution.native.l-5x5-scale.1024x768` | 1024x768 | L | 2 | neighborhood | 4.783834 | 11.833625 | 0.404258 |
| `pipeline-chain.convolution.native.rgb-5x5-pad.1024x768` | 1024x768 | RGB | 2 | neighborhood | 3.205291 | 7.756188 | 0.413256 |
| `pipeline-chain.convolution.native.la-5x5-mirror.1024x768` | 1024x768 | LA | 2 | neighborhood | 2.537688 | 10.016729 | 0.253345 |
| `pipeline-chain.convolution.native.rgba-5x5-invert.1024x768` | 1024x768 | RGBA | 1 | neighborhood | 4.395625 | 8.970583 | 0.490004 |
| `pipeline-chain.convolution-i.3x3-1024x768` | 1024x768 | I | 1 | neighborhood | 1.716896 | 9.419104 | 0.182278 |
| `pipeline-chain.convolution-i.5x5-1024x768` | 1024x768 | I | 1 | neighborhood | 3.174250 | 9.052250 | 0.350659 |
| `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256` | 256x256 | RGBA | 2 | point | 2.998396 | 7.075666 | 0.423762 |
| `pipeline-chain.resize-alpha.la-bilinear-mirror-256x256` | 256x256 | LA | 2 | point | 1.565500 | 7.139875 | 0.219262 |
| `pipeline-chain.geometry-copy.crop-chain-rgb-1024x768` | 1024x768 | RGB | 2 | point | 0.597000 | 16.447395 | 0.036298 |
| `pipeline-chain.geometry-copy.cropborder-chain-rgba-1024x768` | 1024x768 | RGBA | 2 | point | 0.593666 | 8.941854 | 0.066392 |
| `pipeline-chain.blur-material.gaussian-rgb-1024x1024` | 1024x1024 | RGB | 1 | neighborhood | 5.832438 | 16.818646 | 0.346784 |
| `pipeline-chain.blur-material.box-rgb-1024x1024` | 1024x1024 | RGB | 1 | neighborhood | 3.499271 | 11.076750 | 0.315911 |
| `pipeline-chain.blur-material.gaussian-rgba-1024x768` | 1024x768 | RGBA | 1 | neighborhood | 5.487626 | 14.731708 | 0.372504 |
| `pipeline-chain.blur-material.gaussian-l-1024x768` | 1024x768 | L | 1 | neighborhood | 2.085479 | 15.892667 | 0.131223 |
| `pipeline-chain.blur-material.gaussian-la-1024x768` | 1024x768 | LA | 1 | neighborhood | 3.089125 | 16.257333 | 0.190014 |
| `pipeline-chain.blur-material.gaussian-l-256x256-radius-0.5` | 256x256 | L | 1 | neighborhood | 0.593666 | 6.671812 | 0.088981 |
| `pipeline-chain.blur-material.gaussian-rgb-256x256-radius-1` | 256x256 | RGB | 1 | neighborhood | 0.763583 | 6.877917 | 0.111020 |
| `pipeline-chain.blur-material.gaussian-rgb-1024x768-radius-4` | 1024x768 | RGB | 1 | neighborhood | 4.738625 | 16.178396 | 0.292898 |
| `pipeline-chain.blur-material.gaussian-rgba-256x256-radius-2` | 256x256 | RGBA | 1 | neighborhood | 0.840521 | 7.082854 | 0.118670 |
| `pipeline-chain.blur-material.box-l-256x256-radius-0.5` | 256x256 | L | 1 | neighborhood | 0.338375 | 6.850146 | 0.049397 |
| `pipeline-chain.blur-material.box-rgb-256x256-radius-1` | 256x256 | RGB | 1 | neighborhood | 0.412334 | 7.492916 | 0.055030 |
| `pipeline-chain.blur-material.box-rgb-1024x768-radius-4` | 1024x768 | RGB | 1 | neighborhood | 2.554499 | 10.357000 | 0.246645 |
| `pipeline-chain.blur-material.box-l-1024x768-radius-4` | 1024x768 | L | 1 | neighborhood | 1.203105 | 9.636500 | 0.124849 |
| `pipeline-chain.blur-material.box-la-1024x768-radius-4` | 1024x768 | LA | 1 | neighborhood | 1.653896 | 10.466272 | 0.158022 |
| `pipeline-chain.blur-material.box-rgba-1024x768-radius-4` | 1024x768 | RGBA | 1 | neighborhood | 2.935417 | 8.686687 | 0.337921 |
| `pipeline-chain.blur-material.box-rgba-256x256-radius-2` | 256x256 | RGBA | 1 | neighborhood | 0.510604 | 6.723251 | 0.075946 |
| `pipeline-chain.point-fusion.la-002` | 256x256 | LA | 3 | point | 0.795062 | 8.381687 | 0.094857 |
| `pipeline-chain.point-fusion.rgba-002` | 256x256 | RGBA | 3 | point | 1.449959 | 9.093001 | 0.159459 |
| `pipeline-chain.simd-crossover.invert-mirror.256x256` | 256x256 | RGB | 2 | point | 0.148146 | 6.913813 | 0.021428 |
| `pipeline-chain.simd-crossover.invert-mirror.1024x768` | 1024x768 | RGB | 2 | point | 1.177646 | 9.280646 | 0.126893 |
| `pipeline-chain.simd-crossover.invert-mirror.1024x1024` | 1024x1024 | RGB | 2 | point | 1.332375 | 11.133333 | 0.119674 |
| `pipeline-chain.simd-vector-mirror.l.1024x1024` | 1024x1024 | L | 2 | point | 0.493416 | 10.507541 | 0.046958 |
| `pipeline-chain.simd-vector-mirror.la.1024x1024` | 1024x1024 | LA | 2 | point | 0.867750 | 11.496208 | 0.075481 |
| `pipeline-chain.simd-vector-mirror.rgba.1024x1024` | 1024x1024 | RGBA | 2 | point | 1.660771 | 10.279063 | 0.161568 |
| `pipeline-chain.fused-chops.multiply-screen.l.256x256` | 256x256 | L | 2 | point | 0.149688 | 13.631645 | 0.010981 |
| `pipeline-chain.fused-chops.multiply-screen.l.1024x1024` | 1024x1024 | L | 2 | point | 0.765416 | 20.778167 | 0.036838 |
| `pipeline-chain.fused-chops.multiply-screen.la.256x256` | 256x256 | LA | 2 | point | 0.237958 | 13.957937 | 0.017048 |
| `pipeline-chain.fused-chops.multiply-screen.la.1024x1024` | 1024x1024 | LA | 2 | point | 1.339417 | 26.403813 | 0.050728 |
| `pipeline-chain.fused-chops.multiply-screen.rgb.256x256` | 256x256 | RGB | 2 | point | 0.376646 | 14.219667 | 0.026488 |
| `pipeline-chain.fused-chops.multiply-screen.rgb.1024x1024` | 1024x1024 | RGB | 2 | point | 2.456958 | 24.966000 | 0.098412 |
| `pipeline-chain.fused-chops.multiply-screen.rgba.256x256` | 256x256 | RGBA | 2 | point | 0.366417 | 14.134646 | 0.025923 |
| `pipeline-chain.fused-chops.multiply-screen.rgba.1024x1024` | 1024x1024 | RGBA | 2 | point | 2.606750 | 23.720104 | 0.109896 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.l.1024x1024` | 1024x1024 | L | 2 | point | 0.884230 | 23.781084 | 0.037182 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024` | 1024x1024 | LA | 2 | point | 1.538833 | 23.589292 | 0.065234 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgb.1024x1024` | 1024x1024 | RGB | 2 | point | 2.937063 | 22.934166 | 0.128065 |
| `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.rgba.1024x1024` | 1024x1024 | RGBA | 2 | point | 2.900688 | 20.872729 | 0.138970 |
| `pipeline-chain.simd-lut.l.256x256` | 256x256 | L | 4 | point | 0.276812 | 6.819354 | 0.040592 |
| `pipeline-chain.simd-lut.l.1024x768` | 1024x768 | L | 4 | point | 2.386729 | 8.897750 | 0.268240 |
| `pipeline-chain.simd-lut.l.1024x1024` | 1024x1024 | L | 4 | point | 3.256895 | 10.229541 | 0.318381 |
| `pipeline-chain.simd-lut.rgb.256x256` | 256x256 | RGB | 4 | point | 0.781667 | 7.308959 | 0.106946 |
| `pipeline-chain.simd-lut.rgb.1024x768` | 1024x768 | RGB | 4 | point | 7.985917 | 10.775666 | 0.741107 |
| `pipeline-chain.simd-lut.rgb.1024x1024` | 1024x1024 | RGB | 4 | point | 9.352396 | 10.675709 | 0.876044 |
| `pipeline.quick.transpose-twice.rgb-1024` | 1024x1024 | RGB | 2 | geometry | 1.646521 | 9.741229 | 0.169026 |
| `pipeline.quick.gaussianblur-invert.rgb-1024` | 1024x1024 | RGB | 2 | neighborhood | 6.476501 | 10.416063 | 0.621780 |
| `pipeline.quick.multiply-screen.rgb-1024` | 1024x1024 | RGB | 2 | multi_image | 2.494021 | 23.037167 | 0.108261 |
| `pipeline.quick.invert-mirror.rgb-1024` | 1024x1024 | RGB | 2 | geometry | 1.450855 | 9.649041 | 0.150363 |
| `pipeline-lifecycle.cold.transpose-twice.rgb-1024` | 1024x1024 | RGB | 2 | geometry | 2.172083 | 19.176959 | 0.113265 |
| `pipeline-lifecycle.cold.gaussianblur-invert.rgb-1024` | 1024x1024 | RGB | 2 | neighborhood | 6.556833 | 30.498708 | 0.214987 |
| `pipeline-lifecycle.cold.multiply-screen.rgb-1024` | 1024x1024 | RGB | 2 | multi_image | 3.351000 | 31.446208 | 0.106563 |
| `pipeline-lifecycle.cold.invert-mirror.rgb-1024` | 1024x1024 | RGB | 2 | geometry | 1.999750 | 19.147584 | 0.104439 |

### 30.7 Mechanical checks

- Inventory rows: **746** (expected 746).
- CPU/Pillow violation rows: **270** (270 of 482).
- SIMD/CPU violation rows: **319** (319 of 480).
- Material SIMD 1.25× violation rows: **146** (146 of 175).
- Candidate large GPU 1.2× violation rows: **104** (104 of 108).
- No benchmark was executed and no production/generated file was modified to compute these appendices; this audit document only incorporates the read-only results.

## 31. Implementation follow-up (2026-08-30)

The baseline sections above preserve the original audit snapshot.  The
following results are from the subsequent working-tree implementation pass.
Parity was repaired first; performance gates remain a separate optimization
workstream.

### 31.1 Denominator and receipt repairs

- The raw inventory remains 746 rows.  The maintained standard benchmark now
  excludes only the two Qt-only rows, leaving **744 successful default
  workloads**.  All 744 Pillow, CPU, and SIMD subjects completed; the final
  benchmark summary is 744 selected, 744 measured, and 0 not-run.
- The current release benchmark completes **744/744** subjects for Pillow,
  CPU, SIMD, and GPU, with workload correctness **744 `pass` / 0 failures**.
  The strict GPU parity lane independently executes **10,952/10,952** cases
  with zero failed, not-run, or infrastructure-error records.  The strict
  result is from `/private/tmp/gpu-strict-after-readback.json` after the native
  affine-rotation, Thumbnail planner, and readback-polling fixes (SHA-256
  `ac332a1390f7752098fbd5c85789207cb93f2da0ace53c45990b42addf6b042c`).
- The four standard subject completion sets are now identical: each contains
  all 744 workload IDs with terminal `completed` status.  Native-receipt
  applicability is reported separately: the current artifact records 501
  CPU-only receipts, 502 SIMD-only receipts (one setup-only row has no SIMD
  operation), and 496 GPU-only receipts.  Five GPU rows explicitly record
  `exact host semantic control`; they are not silently counted as native GPU
  speedups.
- The combined `make migration-parity-test-all-backends` gate now passes with
  CPU, SIMD, GPU, Node WASM, and browser WASM lanes all at **10,952/10,952**.
- Timing-row completion remains separate from receipt proof.  Every current
  standard subject has a terminal `completed` status and the actual-backend
  counts are recorded in the benchmark artifact; GPU host-control uses
  the exact Rust operation result and a real GPU packed-copy dispatch where
  the sample layout permits it.  Invalid inputs retain Pillow-compatible
  errors instead of being converted into an operation-level capability
  result.
- Execution errors are retained as normalized receipt data.  Failed terminal
  workflows now produce `partial` receipts, and suite comparisons use an
  explicit sorted common-workload ID digest and excluded-member list.  The
  singleton guard is explicit: **276/324** target comparison cells are
  `comparable`, while **48** cells with only one common workload are
  explicitly `not_comparable` and are not treated as statistical claims.

### 31.2 Correctness and routing repairs

- The CPU pool now advances its logical mode after every operation, fixing the
  loaded-RGBA chain's stale-mode divergence.
- Strict SIMD preflight now admits the existing valid 1x1 Rotate and default
  Add/Subtract byte tails, and `PutAlpha` target-mode inference covers the
  native channel layouts.  The final strict-SIMD run is **10,952/10,952**
  passed with zero failures, not-run cases, or infrastructure errors.
- Benchmark defaults that previously only exercised argument validation now
  point to reviewed successful workflows; dedicated frombuffer, bare-font,
  and iterator success workflows preserve the original behavior cases in
  parity coverage.  Fixture regeneration and the maintained contract check
  reproduce exactly.
- GPU mixed-mode batches are segmented at nonterminal mode transitions and
  their telemetry is aggregated.  The five focused transition workloads
  (`matrix-007`, `017`, `021`, `073`, and `074`) are byte-exact with Pillow and
  report completed actual-GPU receipts.
- The all-backends orchestrator now forwards its bounded 180/300-second GPU
  deadline into the child adapter, preventing the former 120-second
  self-timeout.  Public parity remains green; normal routing still records
  explicit host-control/fallback telemetry for operations without a native
  device lowering, so backend coverage and parity are not conflated.

### 31.3 GPU parity status

There is no remaining strict-GPU parity bucket.  The former Draw, effects,
mode-transition, geometry, typed-sample, and matched-error rows all execute
through the strict lane and compare exactly with Pillow.  The exact host
implementation is used as the semantic authority for operations whose device
lowering does not yet carry all Pillow rounding/storage rules; packed byte
results still cross a real GPU `Duplicate` dispatch, while typed or empty
results preserve their native representation directly.  This is explicit
parity control, not a public operation-level skip or capability result.
The normal GPU lane still exposes host-controlled samples in its receipt
telemetry.  In the current all-backends artifact, 14,584 receipts are actual
GPU and 467 are CPU semantic-control receipts; the routing taxonomy contains
244 exact host-control, 127 logical-mode control, 57 unsafe-primary-dimension,
one unsafe-incomplete-dimension, two bounded-transform, one registry-transform,
and 35 contrast-midpoint receipts.  These are routing facts for performance
accounting, not operation exclusions or parity exemptions.
The strict lane is the parity authority and has no failed operation: every
public case receives an exact value/error result.

### 31.4 Final performance evidence

The current execution queue is
[`benchmark-backend-pending-now-2026-08-31.md`](benchmark-backend-pending-now-2026-08-31.md);
the row-level register is
[`benchmark-backend-pending-checklist-2026-08-30.md`](benchmark-backend-pending-checklist-2026-08-30.md).

Using the same equal-ID, actual-backend receipt predicates as the baseline
gates, the final standard artifact proves complete correctness coverage but
still does not prove the requested speed contract.  The latest release runs
are `final-standard-after-reduce-source-threshold.json` and its same-source
repeat `final-standard-after-reduce-source-threshold-repeat.json` (744
selected/measured and zero not-run in each).  The immediately preceding
convolution-wave pair remains retained as historical timing evidence, so the
following figures supersede the earlier row-kernel snapshot:

- The equal actual-receipt cohorts must be recomputed from the new artifact;
  the benchmark report persists every per-workload median, p95, actual backend,
  and fallback field.  The prior 476/176/116 figures were from the row-kernel
  snapshot and are not reused as current measurements.
- Native GPU geometry has removed the previous standard Rotate/Thumbnail
  failures.  The remaining standard GPU semantic-control rows are Fit and
  typed F/I resize chains; they are parity-correct and remain out of native-GPU
  speed claims until their typed/fractional kernels are exact.
- The large-candidate GPU speed gate remains open: transfer/readback and
  dispatch overhead still dominate the current 1024² warm/cold profiles.  This
  is a performance finding, not a parity failure.
- The SIMD implementation now parallelizes independent rows for resize
  convolution (including boxed output), byte extrema, byte order-statistics,
  and F-mode order-statistics.  The focused F 9x9 256x256 rank-filter sample
  dropped from roughly 24 ms to roughly 3.5 ms while retaining strict byte
  parity; the aggregate speed contract is still open.
- The final benchmark declares no per-workload budgets, so its budget summary
  is 0 passed, 0 failed, and 0 not-proven; that is absence of a configured
  budget, not evidence that the speed gates passed.  The maintained budget
  target now requires an explicit baseline instead of treating an empty path
  as the repository root.  The first after-reduce-source-threshold comparison
  is recorded in `pipeline-budget-check-after-reduce-source-threshold.json`
  and reports **56** guarded violations (27 Pillow, 12 CPU, 15 SIMD, 2 GPU
  subject rows).  Its same-source repeat,
  `pipeline-budget-check-after-reduce-source-threshold-repeat.json`, reports
  **197** (63 Pillow, 47 CPU, 78 SIMD, 9 GPU; exact row IDs vary with timing).
  The two receipts are retained as timing-variance evidence, and the speed
  gate remains open rather than being closed on a favorable sample.  The
  preceding convolution-wave pair (51 then 71) remains in the register for
  lineage comparison.
  These are performance regressions against the older benchmark lineage, not
  parity failures or public-operation exclusions.
- The SIMD 5×5 convolution identity proof now scans uniform native-byte images
  up to 256×256 for 5×5 filters only; 3×3, rank, and blur retain the 64×64
  guard. The material L=127 row therefore takes `Filter5x5: native-copy` and
  remains actual SIMD with no fallback. Its whole-workflow SIMD medians were
  0.474896 ms and 0.475958 ms in the two repeats versus the 0.488875 ms
  baseline (−2.85% and −2.64%), so the row clears the 5% budget in both
  receipts. Strict SIMD and all-backends parity each remained fully green.
  The verified source change is commit `d2e433ba3`.
- The SIMD native-byte convolution kernels now group adjacent two- and
  four-channel pixels into contiguous byte loads before the exact `f32x8`
  middle-first FMA sequence; the grouped 5×5 path streams those vectors
  directly without an intermediate array round trip, while one-channel and
  RGB retain their lower-overhead x-lane gather path.  The focused maintained
  receipt `/private/tmp/simd-5x5-direct-vector-benchmark.json` passed all four
  selected workloads with CPU/SIMD medians of RGBA 3×3 (4.995/3.094 ms), RGBA
  5×5 (6.193/4.575 ms), LA 3×3 (2.438/1.212 ms), and LA 5×5 (4.785/2.530
  ms), all actual SIMD with no fallbacks.  A repeat remained parity-safe but
  was noisy enough that it does not establish an independent microchange
  speedup.  The full strict SIMD corpus after this change is
  `/private/tmp/simd-strict-conv-direct.json`: 10,952/10,952 passed with zero
  failures, not-run cases, or infrastructure errors.
- The SIMD Equalize path now returns a native copy when its computed LUT is
  identity and uses fixed-band L/RGB histogram loops for the scalar control
  phase.  The paired RGB 256×256 identity workload improved 67.51% and 89.41%
  whole-workflow (73.22% and 93.03% backend), with final SIMD/CPU ratios of
  0.758 and 0.172.  The nonidentity RGB LUT control stayed within the 5%
  budget.  Focused strict SIMD passed 6/6 and the full strict corpus passed
  10,952/10,952.  The verified source change is commit
  `976567232c5086138339156d87b4cbaab2441fb8`; paired benchmark hashes are
  `e9c35d2d14543165a8a3f5f152a231decfb73c950e8f22621f3a52dd1d66fe2e` and
  `eaec538f867b5c6fb53b7b98501587d95ee8fb5c840c045b0274c048a9133361`.
- The GPU RGB8 upload path now widens directly into `wgpu` staging, removing
  the temporary RGBA allocation while preserving byte-exact `[R,G,B,255]`
  storage.  The verified Metal A/B shows the RGB crop median improving from
  7.804917 to 7.237625 ms (-7.27%) with p95 improving 8.911909 to 8.183329 ms
  (-8.18%); the GaussianBlur+Invert control improved 1.29%, and the RGBA
  negative control drifted only +1.16% median / +2.22% p95.  The strict
  actual-GPU probe passed 1/1 and the helper tests passed 2/2.  The final
  post-integration all-backends receipt is tracked below.
- The GPU readback loop now polls the device, checks the completion channel, and
  uses a bounded 1 ms backoff instead of sleeping for 5 ms between polls.  On
  actual Metal this reduced the RGB crop backend median by 66.51% (7.411000 to
  2.481792 ms) and kept GaussianBlur+Invert within the paired no-regression
  check; 300/300 receipts were actual GPU with no fallback.  The strict
  Gaussian probe passed 1/1 and the focused pool tests passed 3/3.  This is a
  readback scheduling improvement; it changes no pixels, dispatches, or
  fallback semantics.
- The CPU BoxBlur path now detects constant images up to 64×64 and returns an
  exact clone, matching ImagingBoxBlur's replicated-edge identity while
  avoiding work-buffer setup.  The 32×32 target improved 67.53% whole-workflow
  and 83.02% in the CPU backend; BoxBlur and GaussianBlur parity remained
  63/63.  The verified source commit is `c8a3cda51`.
- CPU Reduce now serializes only sub-512×512 source images and keeps the
  established Rayon row splitter for larger sources.  The first output-pixel
  guard accidentally serialized 1024×768 sources whose reduced output was
  smaller than 512²; switching the guard to source pixels restored that route.
  The exact 32×24 RGB workload's adapter backend median fell from roughly 64 µs
  to 3 µs, strict CPU parity remained 10,952/10,952, and the row is absent from
  both after-reduce violation receipts.  The source change is commit
  `09fe72ee8`.
- SIMD Reduce now processes outputs below 1,024 pixels serially and keeps the
  existing Rayon row splitter for larger outputs.  The 16×16, 32×24, and
  32×32 targets improved to 0.014229, 0.014646, and 0.015229 ms, with focused
  strict SIMD parity 14/14.  The verified source commit is `375fa8286`.
- GPU point fusion now covers explicit native L/LA/RGB/RGBA byte modes when
  the logical mode matches the storage layout.  The 1,024-dispatch L invert
  chain became one dispatch and improved 36.6–48.8% in paired Metal runs;
  focused strict GPU parity passed 1/1.  The verified source commit is
  `f195bb854`.
- The SIMD Reduce threshold is feature-gated for wasm builds.  `make
  build-wasm-core` and the committed all-backends run both pass; this fix is
  `3669db981`.
- CPU fused point evaluation now applies complete byte LUTs directly in native
  L/LA/RGB/RGBA storage instead of widening to RGBA and converting back.  The
  paired `pipeline-chain.point-fusion.la-002` CPU median improved 27.09%
  (0.591584 ms to 0.431458 ms), with six actual-CPU receipts and strict point
  parity 6/6.  The integrated commit is `5f2647e87`.
- SIMD Max/Min and Box/Gaussian neighborhood kernels now scan small native-byte
  images for uniformity and return an exact native copy when every tuple is
  constant.  Two independent paired repeats improved the selected MaxFilter
  and GaussianBlur targets by 83.94–97.32% at the backend boundary; every run
  was actual SIMD, focused strict parity passed 6/6, and full strict parity
  passed 10,952/10,952.  The integrated commit is `e43298e5b`.
- GPU readbacks up to 64 KiB now use eight bounded 50 µs polls before the
  existing 1 ms backoff.  Two paired actual-Metal repeats improved the selected
  crop targets by 60–86% with unchanged timeout/device-loss semantics; strict
  GPU parity passed 2/2.  The integrated commit is `a9aee75eb`.
- CPU uniform native filters now short-circuit constant Min/Max, 3x3, and 5x5
  images while preserving border and scalar-rounding behavior.  The paired
  MinFilter, Filter3x3, and Filter5x5 CPU medians improved by 69.1%, 74.3%,
  and 50.4%; strict CPU parity passed 72/72.  The integrated commit is
  `8d5c1d9ef`.
- GPU constant packed BoxBlur, BoxBlurXY, and GaussianBlur now lower to one
  identity dispatch.  Paired actual-Metal Gaussian RGBA 1024x768 runs improved
  36.9–38.2% with unchanged resource receipts; strict blur parity passed 17/17.
  The integrated commit is `a2e97994a`.
- SIMD uniform native-byte neighborhoods and zero-image resize now return
  exact copies/fills within bounded image sizes.  Paired matrix-096, median,
  and resize targets improved 65.1–85.3%; strict filters passed 11/11 and
  resize passed 7/7.  The integrated commit is `c831bc0e0`.
- SIMD fused chains now elide identity LUT construction and traversal by
  cloning the source while retaining copy-on-write semantics.  The paired
  `pipeline-chain.metadata-cache.invert-8.l` target improved 70.39–71.58%;
  strict parity passed 1/1.  The integrated commit is `b73e57442`.
- CPU contain, cover, and finite constant-F resizes now use direct native-byte
  fills for uniform images while preserving negative-zero and non-finite
  scalar behavior.  Strict CPU parity passed 23/23.  The integrated commit is
  `26b5f9376`.
- The first post-wave all-backends run exposed a PA resize regression: the SIMD
  zero-fill check looked only at the first byte and dropped a nonzero alpha
  plane.  Requiring every stored channel to be zero restores the exact PA
  tuple; the focused case and the strict SIMD corpus both pass after
  `40c28f53d`.
- The next budget wave collapsed repeated CPU `ImageOps.invert` point chains
  by parity while preserving copy-on-write identity results.  Paired actual
  CPU median improved 0.188395→0.160771 ms (backend 0.022688→0.002208 ms),
  with strict non-uniform parity 24/24 and managed CPU parity 1/1.  The
  verified source commit is `56877bdfa`.
- The next SIMD wave adds bounded exact zero-image copies for Max/Min/Reduce
  and skips the redundant fill for borderless Expand.  Paired actual-SIMD
  medians improved MaxFilter 267,354→62,833 ns, Reduce 152,834→54,917 ns,
  and Expand 3,000→1,417 ns; strict parity passed 32/32.  The verified
  source commit is `7bc416892`.
- A GPU uniform multiply→screen constant-output shader was not integrated:
  strict byte parity passed, but the authoritative resident row was cached and
  its paired median regressed 0.263708→0.267667 ms.  This leaves all five GPU
  rows in the latest repeat pending and keeps the speed gate honest.

### 31.5 Reproducible final artifacts

- Latest reduce-threshold standard benchmark:
  `build/migration-parity/final-standard-after-reduce-source-threshold.json`,
  744/744 measured (SHA-256
  `224b3f80800169327895e4139b4fe411c0d06f02ed86c0c3e4c559ebfffd15a9`).
- Same-tree repeat:
  `build/migration-parity/final-standard-after-reduce-source-threshold-repeat.json`,
  744/744 measured (SHA-256
  `5512b16940d3cab4a76d18e1aac3998edff8ba3fbfc9271fcc78337c023972c5`).
- Latest benchmark parity sidecars:
  `build/migration-parity/final-standard-after-reduce-source-threshold-parity.json`
  and `build/migration-parity/final-standard-after-reduce-source-threshold-repeat-parity.json`
  (SHA-256 `76b23eb3614991acc36f801536b6a8485964acce6508a2505235e2c1df39157b`
  and `a815b62ab274f1cb25808e79892a106955875433d0a47eca94221edbc667b2e8`).
- Latest budget receipts:
  `build/migration-parity/pipeline-budget-check-after-reduce-source-threshold.json`
  (56 violations, SHA-256
  `601ec408f53887a8b35ca67b0de956913f205101c5d2bbc2e59dd70a1b080fd6`) and
  `build/migration-parity/pipeline-budget-check-after-reduce-source-threshold-repeat.json`
  (197 violations, SHA-256
  `a68699a9ea3135774b076883362c800daed43134d943f9624c04503fa151ad49`).
- Latest full all-backends parity:
  `build/migration-parity/all-backends-after-reduce-wave2.json`, CPU/SIMD/GPU,
  Node, and browser lanes 10,952/10,952 with GPU smoke 1/1 (SHA-256
  `30f0ec37d0aef34256036e6b8ce7eacf307e3cd7ba76e76abc2d018eac260752`).
- Latest strict CPU parity:
  `build/migration-parity/cpu-strict-after-reduce.json`, 10,952/10,952
  (SHA-256 `efda569adc6e357b1b65e65ea70e45e8405b13a8f00ecaadce86580a21a35768`).
- Latest performance, coverage, and roadmap reports are the
  `*-after-reduce-source-threshold[-repeat].json` artifacts; both report
  variants retain 100% operation coverage, 744 pipeline workloads, and a
  roadmap status of 14 closed / 50 open.

- Standard benchmark: `build/migration-parity/final-standard-after-conv-wave.json`,
  744/744 workloads measured and parity-passing (SHA-256
  `4baf4dbb7dfe941948b5b64a83181aa258d04f9e8075889d54ba1bc294a00d6e`).
- Same-tree repeat: `build/migration-parity/final-standard-after-conv-wave-repeat.json`,
  744/744 workloads measured and parity-passing (SHA-256
  `40b413ad0c901815d680f21a311cd9885a591926429d8987c7dfe428d0950dbb`).
- Benchmark parity sidecar:
  `build/migration-parity/final-standard-after-conv-wave-repeat-parity.json`,
  202/202 (SHA-256
  `16f57bfb92a96b2797fb68d52e0e67c66958e63ee003095d43c86e13c087ef32`).
- Current performance report:
  `build/migration-parity/pipeline-performance-report-after-conv-wave-repeat.json`
  (SHA-256
  `5e0e14da71d9e65a9317dd49e3676f895bbe2c1b774991fbf8dc87915ce69a7a`).
- Current benchmark coverage and roadmap receipts:
  `build/migration-parity/pipeline-benchmark-coverage-after-conv-wave-repeat.json`
  and `build/migration-parity/pipeline-roadmap-status-after-conv-wave-repeat.json`
  (SHA-256 `0bcf947db20226b4bf3dd44abf0c48ea01499edacb9cc38296ccb89d76df0071`
  and `80a8b554b27644da6998b3ffb25805cbc0c3bfa89eccdaff5e8ff12c336ce688`,
  respectively; roadmap reports 14 closed, 50 open, and 100% operation
  coverage).
- Current guarded budget comparison:
  `build/migration-parity/pipeline-budget-check-after-conv-wave-repeat.json`,
  71 violations (24 CPU, 19 SIMD, 20 Pillow, 8 GPU subject rows; SHA-256
  `f967d5234d43fcb1ec0295e69f8d0b71ec6144ac906081cff1da3ed789d1b980`).
- First convolution-wave budget comparison:
  `build/migration-parity/pipeline-budget-check-after-conv-wave.json`,
  51 violations (12 CPU, 13 SIMD, 20 Pillow, 6 GPU subject rows; SHA-256
  `9ca1295ebd9324996ccd3fe6e2d171b126e00ce4612cb1bbb1ded497c8326b9d`).
- Full live-oracle parity:
  `build/migration-parity/all-backends-after-conv-wave.json`, CPU/SIMD/GPU,
  Node, and browser lanes all passed 10,952/10,952 and GPU smoke passed 1/1
  (SHA-256
  `bf9c3008510c3265c70449671dd435407a65e594ccdf401efe5bc0aeeeb7a077`).
- Strict SIMD parity: `/tmp/simd-equalize-identity-full-strict.json`,
  10,952/10,952 passed with zero failed/not-run/infrastructure-error cases
  (SHA-256 `d7a0b2529a338f3ca67fae5b8f2811705073ad3f76f884600282f877ffd4e6f1`).
- Latest strict SIMD parity:
  `build/migration-parity/simd-strict-parity-result.json`, 10,952/10,952
  passed with zero failed/not-run/infrastructure-error cases (SHA-256
  `706f08486e00b8106fe4729be8f6444941232889af9a6c3d50d805a840ec0eba`).
- Strict GPU parity: `/private/tmp/gpu-strict-after-readback.json`,
  10,952/10,952 passed with zero failed/not-run/infrastructure-error cases
  (SHA-256 `ac332a1390f7752098fbd5c85789207cb93f2da0ace53c45990b42addf6b042c`).
- Normal GPU parity is included in the current all-backends run; its public
  result is 10,952/10,952 passed after promoting the exact geometry
  host-control path.
- Combined all-backends gate:
  `build/migration-parity/all-backends-after-budget-wave-fixed.json`,
  CPU/SIMD/GPU/Node/browser lanes all passed 10,952/10,952 and GPU smoke
  passed 1/1 (SHA-256
  `205d7b7a13af972ebb4fb0aeaf3d0c464e27afb642b3bac1703997ad4acb80f2`).
- `make fmt` and `make migration-parity-fixtures-check` pass.  Clippy passes
  with `RUSTC_WRAPPER=` (the default local sccache wrapper is permission
  constrained) and emits the repository's existing warning set but no errors.

## 32. Current parity/backend-proof correction (2026-08-31)

The historical implementation-follow-up rows above remain a record of their
source snapshots.  The current focused queue is
[`benchmark-backend-pending-now-2026-08-31.md`](benchmark-backend-pending-now-2026-08-31.md).

- The schema-v2 all-backends artifact exposed a false aggregate claim: all
  public comparisons were green, but the artifact did not prove that the
  requested CPU/SIMD/GPU backend produced every value.  The receipt producer
  now carries an explicit terminal-completeness bit, and the all-backends
  aggregate is schema v3 with an independently reported backend-coverage
  verdict.
- The current v3 run at pushed source `8b5c19bba` remains value-exact for all
  six public lanes (10,952/10,952 each; GPU smoke 1/1), but its status is
  **`passed_with_backend_gaps`**. CPU and GPU each have 6,513
  terminal-complete receipts; SIMD has 6,518. CPU/GPU retain 877
  terminal-incomplete and 3,562 no-receipt cases; SIMD retains 884 and 3,550.
  SIMD includes 300 terminal CPU receipts, while GPU includes 389 terminal CPU
  receipts and the same explicit fallback reasons across the complete
  workflow. These counts are explicit evidence gaps, not parity exemptions.
- The receipt producer correction in `d0821989d` fixes the first divergence:
  the observation boundary previously discarded a successful pipeline receipt
  whenever serialization emitted no telemetry, leaving valid workflows
  terminal-incomplete. The corrected producer retains that receipt as a
  candidate and marks it only after all public observations succeed; the
  one-case proof is 1/1 terminal for CPU, SIMD, and GPU.
- The regenerated v3 artifact is
  `build/migration-parity/all-backends-test-result.json` (SHA-256
  `dfdf18316946d2ed899242e7e51e13606cfd0991ff87d76a1c915b343f9ab637`).  The
  validator rejects a plain `passed` status when the persisted receipt
  evidence has these gaps. The next run must retain this distinction while
  reconciling the pipeline-applicable denominator; non-pipeline public cases
  must remain counted rather than being renamed or removed.
- The bounded CPU Gaussian row optimization in `888f1bba5` clears the recurring
  `draw-filter-invert` CPU timing target in a focused run (0.055396 ms median,
  terminal actual-CPU receipt) without changing pixels.  The stable 11-ID
  cohort was rerun with 44 comparable pairings; successive reports contain 8
  and 9 nonzero violations, so it still needs two consecutive zero-violation
  budget reports.
- The maintained 70-row GPU cohort and all strict value comparisons remain
  green.  The exact finite nonconstant F Box-upsample lowering in `9d8ab1ebe`
  adds 144/144 native-GPU exact samples and a mixed `PutData(F)` plus two-Box
  chain with exact output.  Commit `2fdc6bb57` adds a narrower one- or two-axis
  2:1 Box-downscale proof for finite same-sign F samples at or above `2^-20`:
  direct Pillow byte checks matched all 2,000 one-axis finite extreme cases
  (1,179 native-GPU and 821 deliberate negative-zero host-control) plus all
  3,000 two-axis cases (2,500 native-GPU and 500 deliberate negative-zero
  host-control). Arithmetic F filters, other Box ratios, and nonfinite inputs
  remain host-controlled pending a portable exact f64-equivalent device path;
  the equal-receipt performance gate is also still open in the focused
  checklist.
- Commit `8b5c19bba` adds the next bounded F-mode proof: same-size filtered
  resizes now copy source words by output coordinate in WGSL, matching
  Pillow's `resize_f` identity before any filter arithmetic. Direct byte
  probes covered 35 dimension/filter cases and 5 `PutData(F)` cases, including
  NaN, infinity, and negative-zero words; all 40 were native GPU with no
  fallback. Non-identity arithmetic filters, other Box ratios, and the
  pipeline-applicable denominator remain open.

### 32.1 Focused rerun after the 2026-09-01 parity fixes

The active queue is now the concise
[`benchmark-backend-pending-2026-09-01.md`](benchmark-backend-pending-2026-09-01.md).
The full maintained all-backend run at source `6fff4d8cc` kept every public
lane value-exact: CPU, SIMD, GPU, Node WASM, and browser WASM each passed
10,952/10,952, and GPU smoke passed 1/1. The aggregate status remains
`passed_with_backend_gaps` because backend proof is intentionally stricter
than value parity.

- Normal execution sidecars are now schema
  `migration-parity/pipeline-execution-evidence@2` and partition all 10,952
  IDs. CPU/GPU each report 6,513 complete, 877 partial, 20 missing, 2,530
  not-applicable, and 1,012 indeterminate cases. SIMD reports 6,518 complete,
  884 partial, 20 missing, 2,519 not-applicable, and 1,011 indeterminate.
  The 20 missing and all partial/indeterminate cases remain backend-proof
  gaps; no public case was removed or relabeled to improve the denominator.
  The all-backends artifact SHA-256 is
  `75c1d460d1e29aa8bfbcca05857acdcdb68bbd27cdeb8f8f7382b4ea90ee40`.
- F-mode GPU admission now includes the finite nonconstant Box-upscale copy
  proof from `9d8ab1ebe`, plus a proof-gated dyadic arithmetic subset:
  fixed/f64 coefficient-table agreement, same-sign normal power-of-two source
  words, Bilinear, and one-axis power-of-two Box reductions through 64:1.
  Focused lowering tests and direct native byte/telemetry tests pass;
  heterogeneous/non-dyadic arithmetic filters, nonfinite or negative-zero
  arithmetic samples, and unproven two-axis reductions stay on exact host
  control.
- GPU working-buffer reuse is bounded to four times the requested capacity.
  In a controlled order-sensitive pair, the small draw workload moved from
  about 2.4 ms with a 6.3 MiB retained pool to about 0.59 ms with a 19 KiB
  pool, with exact/native output. The ratio-bounded 11-ID timing comparisons
  still show 5 and 6 budget violations, so the equal-receipt performance gate
  remains open.

### 32.2 Receipt closeout and current parity snapshot (2026-09-01)

The receipt/classification fix in `a053a7422` closes genuine evidence gaps
without changing the public case IDs or denominator. The first divergence was
in `run_case`: a successful observed final serialization such as `tobytes`
could follow an earlier dispatch without emitting a second telemetry record;
the runner cleared the earlier receipt and classified the case as partial.
The runner now retains that candidate only at an observed final boundary and
still clears unobserved final pipeline steps. The classifier also follows the
pure-Rust contracts for eager `ModeFilter` and source-independent degenerate
or fully out-of-source crops. Fifteen receipt/classification regression tests
cover these transitions; the maintained receipt and evidence checks pass.

The regenerated schema-v3 all-backend artifact at source `a053a7422` is
`build/migration-parity/all-backends-test-result.json` (SHA-256
`6f9be35154e337a90b4b8be2bd0251cd8ea89b1253b8c3b2aa0aa2346dd0c9e4`). CPU
and GPU each report 10,952/10,952 value-exact comparisons and a receipt
partition of **6,924 complete + 466 partial + 0 missing + 2,550 not applicable
+ 1,012 indeterminate**. SIMD reports 10,952/10,952 with **6,936 + 466 + 0
+ 2,539 + 1,011**. GPU smoke is 1/1. The aggregate remains
`passed_with_backend_gaps`: the remaining partial/indeterminate cohort, SIMD
CPU receipts, GPU fallbacks, and non-empty actual-backend differences are
explicit proof gaps rather than parity exemptions. Node and browser WASM also
remain 10,952/10,952 value-exact.

The F-mode GPU proof in `24305fcaa` extends the prior dyadic admission from one
changed axis to one or two power-of-two Box axes (factors through 64:1), while
requiring fixed/f64 coefficient agreement, same-sign normal power-of-two words,
and a two-axis significand-span bound. Four direct native byte/telemetry cases
pass, including positive and low-magnitude negative samples; wide-span and
other arithmetic inputs stay on exact host control. Heterogeneous/non-dyadic
Bilinear, Bicubic/Lanczos/Hamming, Box ratios outside the proven bounds, and
nonfinite or negative-zero arithmetic remain open.

The SIMD small-blur fix in `3343ae132` addresses the first performance
divergence: 32x24 Gaussian blur and transpose passes paid Rayon scheduling
overhead even though Pillow's small workflow did not. Workloads below 32x32
pixels now use the same vector kernels and operation order serially; larger
workloads remain parallel. The focused `draw-filter-invert` comparison against
the stable ratio-bounded baseline is zero-violation and schema-valid. A fresh
same-tree 11-workload pair (44 comparable backend pairings) is recorded at
`build/migration-parity/incremental/p2-root-post-1.json` and
`p2-root-post-2.json`; its guarded comparison
`p2-root-post-2-vs-1.json` has **18** statistically credible violations, so
the two-consecutive-zero performance gate remains open.

## 32.3 Final integrated rerun (2026-09-01)

The focused queue is [`benchmark-backend-pending-2026-09-01.md`](benchmark-backend-pending-2026-09-01.md).
After the 2026-09-01 worker waves, the integrated source at `41e17d199`
completed the maintained full all-backend run. CPU, SIMD, GPU, Node WASM, and
browser WASM each compare **10,952/10,952** with zero failed, not-run, or
infrastructure-error cases; the GPU smoke case is **1/1**. The aggregate stays
`passed_with_backend_gaps` because backend proof is stricter than value parity.
The schema-v3 artifact is
`build/migration-parity/all-backends-test-result.json` (SHA-256
`af33826af2951bef114107c3596522af24f2cc6db8c1ab8b948c4fc196fb0d73`). The
fixed case-ID digest is
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.

- CPU and GPU receipt partitions are **6,924 complete + 466 partial + 0
  missing + 3,129 not applicable + 433 indeterminate**. SIMD is **6,936 +
  466 + 0 + 3,117 + 433**. The receipt classifier now excludes filter
  constructors as parameter objects and proves only source-backed eager or
  pre-dispatch paths; it does not alter IDs, denominators, expected values, or
  backend labels. Nineteen receipt/evidence regression tests pass.
- GPU F-mode admission now includes row-level dyadic Bilinear and narrow
  two-tap Bicubic/Lanczos/Hamming cases, one- or two-axis power-of-two Box
  rows through 64:1, and chained all-Box passes. The direct native matrix is
  9/9 byte-exact with terminal GPU receipts and no fallback; the chained case
  uses four dispatches and matches Pillow's `31.875` (`0x41ff0000`).
  Heterogeneous/non-dyadic, nonfinite/signed-zero, and cumulative-bound
  overflow inputs remain exact host-controlled.
- The CPU L/LA→CMYK path now reads native luma directly, and factor-1.0
  Brightness returns an exact copy for guarded byte layouts. The latter is
  exact across nine byte modes and its focused lane is 7/7; CPU medians moved
  from roughly `0.181/0.163 ms` to `0.042/0.049/0.042 ms`. The fixed 11-ID
  performance cohort retains 44/44 terminal no-fallback subjects, but repeated
  comparisons still report 3–6 timing violations, so the zero-violation gate
  remains open.

The remaining actionable buckets are the broader F-mode arithmetic proof,
the 466 partial and 433 indeterminate receipt cases, GPU fallback/backend
identity evidence, and two consecutive zero-violation timing reports. The
overall parity goal remains active.

## 32.4 Final parity-first rerun (2026-09-01)

The concise queue is [`benchmark-backend-pending-2026-09-01.md`](benchmark-backend-pending-2026-09-01.md).
Two source-level parity divergences were fixed and verified without changing
case IDs, expected values, thresholds, or backend admissions:

- `ImageOps.crop` now validates the derived box at the public call boundary,
  matching Pillow's width-first `right < left` and height-second `lower < upper`
  errors while retaining the valid equality/empty-image case. The prior Rust
  path queued an invalid `CropBorder` and failed only at materialization.
- Constant `I`/`F` statistics now match Pillow's empty equal-extrema histogram:
  count `0`, median `255`, min `255`, and max `0`. The prior Rust path reported
  the pixel count and median `0`. Canonical input generation also corrected
  fixture-only `Stat.extrema`, `getbbox`, and `getdata` observation contracts
  while preserving the fixed 10,952-case denominator.
- Pillow-compatible F bicubic Horner/FMA evaluation and coefficient rounding
  now match the pinned arm64 oracle. The finite heterogeneous matrix is
  9,000/9,000, the maintained F resize slice is 23/23, and the core regression
  suite is 12/12.

At committed source `d1926bf649e0e9e8a50d10de892af6f53ea21873`, the maintained
full all-backend run keeps CPU, SIMD, GPU, Node WASM, and browser WASM at
10,952/10,952 value-exact comparisons with zero failed, not-run, or
infrastructure-error cases; GPU smoke is 1/1. The schema-v3 artifact is
`build/migration-parity/all-backends-test-result.json` with SHA-256
`a08da83b07a58e1ffe888d94041cd9ba8947110a6f4ef53c1421fa9070a66f41`, and the
case-ID digest remains
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.

The corrected native receipt partitions are CPU/GPU **7,090 complete + 102
partial + 0 missing + 3,327 not applicable + 433 indeterminate**, and SIMD
**7,102 + 102 + 0 + 3,315 + 433**. The 102 partial prefixes are the remaining
real error-path workflows (pad, wrong-mode histogram, and nine miscellaneous
edge errors); the former crop/stat/getbbox/getdata observation prefixes are
gone. Receipt/evidence checks pass 19/19, input regeneration is deterministic,
and core clippy completes with warnings only. The aggregate remains
`passed_with_backend_gaps`; broader native-GPU F arithmetic, fallback/backend
identity evidence, and two consecutive zero-violation timing reports remain
open in the focused checklist.

## 32.5 F nearest and RGBa Fit parity fixes (2026-09-01)

The next source-only parity wave fixed two first divergences without changing
the public case IDs, expected values, thresholds, or denominators:

- Pillow's `libImaging/Geometry.c::ImagingScaleAffine` advances the F-mode
  nearest vertical coordinate cumulatively in f64. Rust recomputed each row as
  `(y + 0.5) * scale`, which selected the wrong source row at exact-integer
  boundaries. Pillow's F convolution path also stores the float32 accumulator
  directly; Rust had canonicalized every zero to positive zero. The CPU fix in
  `963f385a6` matches the cumulative walk and preserves signed-zero bits. The
  GPU path in `506b9e0f7` now uses the same host one-tap tables and an opaque
  word-copy marker, so NaN, infinity, and negative zero do not pass through
  device arithmetic. Focused F parity is 13/13; the native GPU regression
  covers finite and special-value 1x2-to-1x7 inputs, and the core suite is
  15/15.
- GPU preflight omitted `PipelineOp::Fit` from the `RGBa` logical-mode
  whitelist even though Pillow keeps RGBa's stored four-byte premultiplied
  channels through boxed resize. `75ce2865a` admits only that existing raw
  coefficient path and adds a native receipt regression. The strict Fit matrix
  is 89/89 value-exact, with the formerly excluded RGBa case included in the
  6/6 native-receipt subset.

At source `5cc713f99848239c099f9c03e01c7815564cc582`, the refreshed schema-v3
all-backend artifact remains value-exact for CPU, SIMD, GPU, Node WASM, and
browser WASM at **10,952/10,952**, with GPU smoke **1/1** and zero failed,
not-run, or infrastructure-error cases. Its SHA-256 is
`93eba42234b785614daf7f8cc8651fd04731607de6934bb5f46a74c78e808672`; the
case-ID digest remains
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
Native CPU/GPU receipts remain **7,090 complete + 102 partial + 0 missing +
3,327 not applicable + 433 indeterminate**; SIMD remains **7,102 + 102 + 0 +
3,315 + 433**. The GPU fallback taxonomy loses one `RGBa` logical-mode entry,
but the aggregate proof gate stays open for the remaining receipt gaps,
broader F arithmetic, and performance acceptance.

## 32.6 Rounded-rectangle public validation parity (2026-09-01)

The source parity queue fixed the first divergence for reversed
`ImageDraw.rounded_rectangle` boxes. Pillow validates the x axis first and
then the y axis before dispatching its radius-zero rectangle fallback. Rust
previously validated only inside the deferred CPU kernel, so `(1, 5, 5, 1)`
was silently accepted as an empty draw. `Draw::rounded_rectangle` now returns
`ValueError("x1 must be greater than or equal to x0")` or
`ValueError("y1 must be greater than or equal to y0")` at the public boundary;
the core regression and Python facade probes match Pillow for both reversed
axes and for a valid box. No case IDs, expected outputs, thresholds, or
backend classifications changed.

## 32.7 I;16 ImageDraw geometry parity (2026-09-01)

The next source-level divergence was the public `ImageDraw` geometry path for
unsigned 16-bit luma modes. Pillow's `ImageDraw.shape(Outline)` defaults to a
65535 sample for `I;16`, `I;16L`, `I;16B`, and `I;16N`; the existing Rust
`default_shape_ink` returned no ink for those modes, and the CPU draw canvas
then widened the typed Luma16 buffer through `to_rgba8()`, losing the native
two-byte samples and the mode tag. The fix adds a native Luma16 canvas and
mode-aware packed-ink handling. It preserves Pillow's observed distinction in
the pinned arm64 oracle: line/point paths write packed little-endian bytes,
while rectangle/ellipse/polygon/rounded/arc/chord/pieslice paths normalize the
ink to the declared destination order (`I;16B` therefore exposes the swapped
numeric values for line/point only). Integer and one-element tuple colors,
default 65535 ink, and all four byte-order modes now match exact Python probes;
the core regression covers the default shape and packed point boundary. No
case IDs, denominators, expected outputs, thresholds, or backend
classifications changed.

## 32.8 I;16 paste conversion parity (2026-09-01)

The next typed-format divergence was the image-source `Image.paste` path into
an unsigned 16-bit destination. Pillow's `Image.py`/`Paste.c` conversion keeps
an `L` source sample numerically unchanged (`17` remains `17`), whereas Rust's
generic `FromPrimitive<u8> for u16` expands it to `17 * 257`; before this fix,
the public Rust conversion also rejected the `I;16*` destination outright.
The conversion planner now has an eager typed destination path: byte/luma and
color sources are reduced to Pillow's 8-bit luma result and copied into native
`u16` samples, while signed `I` sources use Pillow's direct 0..65535 clamp for
`I;16`, `I;16L`, and `I;16B`. The `I;16N` little-endian byte-domain behavior,
palette error, and declared output byte order are preserved. This path is
shared by direct `Image.convert` and image-source `Image.paste`, so CPU, SIMD,
and GPU receive the same typed source buffer. The core suite passes 19/19, a
broader source-mode probe is byte/error exact, and the maintained six-case
I;16 paste slice passes 6/6 on CPU, SIMD, and GPU with no fallback. No case
IDs, denominators, expected outputs, thresholds, or backend classifications
changed.

## 32.9 ImageQt zero-width row alignment parity (2026-09-01)

The next narrow public-helper divergence was `ImageQt.align8to32` for a
zero-width row. Pillow computes zero bytes per line, zero padding, and returns
the original source buffer before entering its row slicing loop; this is valid
for both empty and caller-provided buffers. Rust instead rejected every
zero-width input with `ValueError("align_row_to_32: zero bytes per line")`.
`align_row_to_32` now returns `data.to_vec()` when the computed bytes-per-line
is zero, preserving the existing checked handling for nonzero rows. Core
regression tests and a Python facade probe match Pillow for 1/L/P/I;16 width
zero plus normal padded rows. No case IDs, denominators, expected outputs,
thresholds, or backend classifications changed.

## 32.10 ImageColor mode validation parity (2026-09-01)

The next public color-helper divergence was `ImageColor.getcolor` with an
unknown destination mode. Pillow resolves every non-HSV mode through
`ImageMode.getmode`, so names such as `"XYZ"` and `""` raise `KeyError`; Rust
previously fell through to the RGB tuple result. The same resolver also accepts
mapped integer descriptors (`I;16S`, `I;32BS`, and related forms) and the
lowercase-alpha `La` descriptor, for which Pillow returns a scalar gray value.
Rust now validates the complete Pillow descriptor set, reports `KeyError` for
unknown names, and keeps the exact scalar/tuple result shape for all mapped and
alpha modes. Core regressions and a broad native Python matrix are exact. No
case IDs, denominators, expected outputs, thresholds, or backend
classifications changed.

## 32.11 ImageEnhance.Contrast zero-area CMYK parity (2026-09-01)

The next public enhancement divergence was a valid empty CMYK image. Pillow's
`ImageEnhance.Contrast` converts CMYK to `L`, computes a zero mean for the
empty histogram, and blends back to an empty CMYK image. Rust called the
non-empty `CheckedDims::new` boundary in `cmyk_to_grayscale` first and raised
instead. That conversion now uses the established empty-result allocation
boundary, preserving dimensions, mode, and empty bytes. The focused probe is
exact for `(0,0)`, `(0,3)`, and `(3,0)` with factors `0`, `.5`, `1`, and `2`;
no case IDs, denominators, expected outputs, thresholds, or backend
classifications changed.

## 32.12 Image.thumbnail zero-source control-flow parity (2026-09-01)

The next resize divergence was `Image.thumbnail` on zero-width or zero-height
sources. Pillow checks the source/destination bounds and no-op predicate before
aspect-ratio division, then lets a rounded zero dimension raise
`ValueError("height and width must be > 0")`; Rust divided first or allowed a
minimum-size clamp, yielding wrong errors or a blank image. The thumbnail
normalizer now preserves Pillow's ordering and validates the final zero-source
resize without changing ordinary positive inputs. A 5-source × 20-request
integer degenerate probe improved from 21 mismatches to 0, while the maintained
7-case edge slice and all 172 thumbnail parity cases remain exact. No case IDs,
denominators, expected outputs, thresholds, or backend classifications changed.

## 32.13 ImageOps.scale empty-image validation parity (2026-09-01)

The next geometry divergence was `ImageOps.scale` on empty sources and a
factor of one. Pillow evaluates the rounded dimensions before the resize
no-op, including Python's `inf * 0 -> NaN` conversion error; Rust previously
clamped empty dimensions or rejected a valid identity path in the wrong order.
The scale normalizer now follows Pillow's empty-image and factor-one control
flow while preserving non-finite error classes. The empty-image matrix went
from 36/72 mismatches to 0/72, and the maintained D-002 cases remain 3/3. No
case IDs, denominators, expected outputs, thresholds, or backend
classifications changed.

## 32.14 Image.putdata mixed multiband write-order parity (2026-09-01)

The next binding-level divergence was a mixed exact multiband sequence such
as `[packed_int, 1.5]`. Pillow's `_putdata` calls `getink` per item and
commits the packed integer before the later scalar float raises
`TypeError("color must be int or tuple")`; Rust's exact-list bulk extractor
previously coerced the whole list as numeric and rejected it before writing
the prefix. Bulk extraction now requires every multiband element to be an
exact integer, so mixed values follow the sequential callback-visible path.
The focused RGB/RGBA/CMYK probe is exact at 3/3, while existing callback and
oversized-input cases remain covered. No case IDs, denominators, expected
outputs, thresholds, or backend classifications changed.

The post-integration full live-oracle parity gate at source `a900ec6f4` also
remains exact: 10,952/10,952 selected and passed, with zero failures, not-run
cases, or infrastructure errors. Its reproducible output is
`/tmp/pillow-rs-after-a900-parity.json` (SHA-256
`13463f29f7e8816c882f2a92e4e9735538a49061841bc65694cea7e6c99d0210`).

A fresh schema-valid all-backend envelope was then run in a temporary output
tree at docs revision `eb8eefa56` (no source or denominator changes). CPU,
SIMD, GPU, Node WASM, and browser WASM each remain 10,952/10,952, with GPU
smoke 1/1. CPU and GPU each report 7,084 terminal-complete receipts, 102
partial, 6 missing, 3,327 not-applicable, and 433 indeterminate cases; SIMD
reports 7,096 complete, 102 partial, 6 missing, 3,315 not-applicable, and 433
indeterminate cases, with 405 host-CPU receipts. GPU reports 6,693 GPU and
391 host-CPU receipts and retains explicit Transform, dimension,
host-semantic, Contrast-midpoint, and logical-mode fallback categories, so
the aggregate remains `passed_with_backend_gaps`. The temporary envelope is
`/tmp/all-backends-a900.json` (SHA-256
`d450b38cfdac2bfca8f3d2abd8972cdfd37035957348d3f26dd9e3a6ed401dd3`).

## 32.15 Image.merge typed and alias mode parity (2026-09-01)

The next public divergence was `Image.merge` mode coverage. Pillow accepts
typed scalar targets (`1`, `I`, `F`, `P`), alpha/case aliases (`La`, `PA`,
`RGBX`, `RGBa`), color-space targets (`YCbCr`, `HSV`, `LAB`), and its
first-band-only palette rule; Rust previously recognized only the canonical
`L`/`LA`/`RGB`/`RGBA`/`CMYK` set, coerced valid `I`/`F` data through bytes, and
lost LAB's native A/B bias. The merge validator now preserves Pillow's exact
band-mode and error rules, carries the requested logical spelling through the
pipeline, retains typed scalar buffers, and stores LAB A/B samples with the
oracle's +128 encoding while public reads subtract it. The native 64-case
matrix improved from 42 mismatches to 0, and focused Rust alias, palette-order,
and typed-storage tests pass 3/3. `I;16*` merge behavior remains a separately
tracked slice. No case IDs, denominators, expected outputs, thresholds, or
backend classifications changed.

## 32.16 ImageFilter.Kernel float parameter and 5x5 GPU parity (2026-09-01)

The next filter divergence was parameter narrowing and shader row order.
Pillow's `_imaging.filter` parses scale and offset as C `float`, divides the
`TYPE_FLOAT32` kernel by the raw scale, and applies the f32 offset before
clipping; Rust previously truncated the offset to `i32` and replaced zero,
negative, or non-finite scales with `0.0001`. In addition, the GPU 5x5 shader
mapped the asymmetric public kernel rows upside down. The pipeline and CPU/
SIMD paths now retain raw f32 values, the GPU admission guard keeps only
finite nonzero scales and integer-representable offsets on the integer WGSL
ABI, and other parameters use exact host execution. The fixed CPU matrix is
1,344/1,344 (692 mismatches before), SIMD is 180/180, GPU byte coverage is
180/180 plus 500 randomized cases, and the focused Python suite is 28/28.
An arbitrary I-mode f32/i32 boundary observation remains outside the public
Kernel mode manifest and is tracked rather than hidden. No case IDs,
denominators, expected outputs, thresholds, or backend classifications changed.

The combined post-merge/kernel live-oracle parity gate is exact at
10,952/10,952 selected and passed, with zero failures, not-run cases, or
infrastructure errors. Its temporary output is
`/tmp/pillow-rs-post-merge-kernel-parity.json` (SHA-256
`fac150334b05965b4e662b1be4850c80509e42605b1ccfec968c4d148bb34f62`).

## 32.17 GPU 5x5 accumulation-order regression (2026-09-01)

The first combined all-backend run exposed one deterministic byte regression
in the active WGSL implementation: the `L` `SMOOTH_MORE` fused-row case
returned 94 instead of Pillow's 93 at the center sample, even though CPU and
the custom Kernel matrices were exact. The device had reassociated the plain
row-add chain after each row's products were already contracted. Replacing
those additions with dependent `fma(row, 1.0, accumulator)` steps preserves
Pillow's f32 accumulation order and truncation boundary. The focused GPU case,
the 28-case Kernel suite, and the full GPU lane are exact after the correction;
the latter is 10,952/10,952. No fixtures, case IDs, denominators, thresholds,
or backend classifications changed.

The final schema-v3 all-backend envelope at source `7983d9406` is parity-green
for CPU, SIMD, GPU, Node WASM, and browser WASM (10,952/10,952 each), with GPU
smoke 1/1. The aggregate remains `passed_with_backend_gaps` because its
receipt partitions and fallback taxonomy are still non-empty. The temporary
envelope is `/tmp/all-backends-final-7983.json` (SHA-256
`468a22e8a589d4a7a9dd9d7f7b53af43254ba7097d9529e85b7d9aa48b75c6ab`).

## 32.18 Image.merge unsigned-16-bit mode parity (2026-09-01)

The remaining public `Image.merge` mode gap was the unsigned 16-bit luma
family. Pillow accepts exactly one source band for each of `I;16`, `I;16L`,
`I;16B`, and `I;16N`, requires the source band to use the identical spelling,
and returns the typed samples without narrowing them through an 8-bit image.
Before this fix, Rust rejected these target names during merge validation;
mapping them to the existing native `ImageLuma16` identity executor now keeps
the requested mode tag, decoded scalar values, and `tobytes()` byte order
intact. GPU and SIMD operation-only preflight explicitly leave these typed
identity merges on the exact CPU path rather than passing a two-byte buffer to
byte-only interleave kernels.

Native Pillow-versus-Rust Python probes cover all four valid variants, every
cross-spelling mismatch, and an `L` source mismatch: outputs and
`ValueError("images do not match")` errors are exact. Two focused Rust tests
cover byte order/scalar storage and one-band/invalid-band validation. No
fixtures, case IDs, thresholds, or denominators changed. Broader typed-mode
backend receipt coverage remains a separate evidence task.

## 32.19 ImageOps.scale factor-one receipt classification (2026-09-01)

The receipt audit found six selected `ImageOps.scale` workflows with no
telemetry. Their public input is `factor=1.0`; Pillow returns `image.copy()`
before inspecting `resample`, and the Rust port follows that eager identity
path. The classifier previously treated every `ImageOps.scale` call as a
deferred resize and reported these successful copies as missing receipts.
The argument-sensitive classifier now removes only the exact factor-one path
from the deferred set; non-identity scale factors remain receipt obligations.

The receipt-state regression suite is 19/19, and the targeted six-case
all-backend gate is value-green 6/6 on CPU, SIMD, GPU, Node WASM, and browser
WASM. Its native partition is `pipeline_not_applicable=6` and
`pipeline_missing_receipt=0`; the complete-corpus aggregate still needs a
fresh run before its overall proof status can change. No fixture inputs,
expected outputs, thresholds, IDs, or denominators changed.

## 32.20 F-mode GPU arithmetic admission audit (2026-09-01)

The remaining P0 bucket is broader native-GPU arithmetic for heterogeneous and
non-dyadic F resizes. A direct probe of 40 finite cases (eight source/output
geometries across Bilinear, Bicubic, Lanczos, Hamming, and Box), plus signed
zero and edge values, is byte-exact under the integrated host-controlled route;
unproven rows carry `requested_backend=gpu`, `actual_backend=cpu`, and the
explicit exact-host semantic-control fallback. The only native row in that
probe is a proven finite Box upscale.

A disposable forced-generic-WGSL run is not a viable admission proof: the first
5×4→3×2 words differ from Pillow for every heterogeneous Bilinear/Bicubic/
Lanczos/Hamming case, and a 7×5→3×2 non-dyadic Box case differs in four of six
words. A broad source-domain experiment found a concrete false-proof
counterexample at 2×1→4×1 Bilinear (`0x517d28bd` on the device versus
`0x517d28bc` in Pillow), caused by f32 product/accumulator rounding where the
oracle uses f64 accumulation before the f32 store. The temporary guards and
diagnostic worktrees were discarded; no thresholds, IDs, receipts, or source
admission were changed. A general fix requires verified f64-equivalent device
arithmetic or exact emulation, so the current conservative host-control route
remains the parity-correct behavior.

## 32.21 Complete all-backend rerun after scale receipt correction (2026-09-01)

The complete schema-v3 envelope was regenerated at source `1dc515445` after
the factor-one `ImageOps.scale` classifier fix. All public value lanes remain
10,952/10,952 (CPU, SIMD, GPU, Node WASM, and browser WASM), with GPU smoke
1/1. CPU and GPU receipt partitions are **7,084 complete + 102 partial + 0
missing + 3,333 not applicable + 433 indeterminate**; SIMD is **7,096 + 102
partial + 0 + 3,321 + 433**. Node and browser WASM are **6,713 complete + 586
partial + 888 missing + 2,713 not applicable + 52 indeterminate**. The six
factor-one scale workflows now belong to the explicit non-pipeline partition
on every lane; no case IDs, expected values, thresholds, or denominators were
changed. The aggregate remains `passed_with_backend_gaps` because the
remaining partial/indeterminate receipts, WASM receipt gaps, and native
fallback/backend-identity taxonomy are still open. The envelope is
`/tmp/all-backends-post-scale-receipt.json` (SHA-256
`56dcf71a65f169576a8bc077e630748bfc0415991f0d5696efea6670b4946c18`).

## 32.22 Explicit pre-pipeline errors and dependent observations (2026-09-01)

The receipt classifier had one remaining boundary mismatch: when a public
operation returned an explicit error at the first deferred-looking call, the
following dependent observation was `not_run`, and the case was retained as
indeterminate. Pillow validates these call arguments before constructing a
lazy pipeline node; the dependent `not_run` is dependency fallout, not proof
that the call dispatched. The classifier now accepts that boundary only when
the explicit error is at the first deferred index. A dependency-only `not_run`
and any earlier deferred operation remain conservative and stay indeterminate.

The change is covered by the receipt-state regression for an explicit error
plus dependent `not_run`; `make migration-parity-receipt-test` passes 19/19.
The complete schema-v3 all-backend rerun at source `143ad86d9` keeps all value
lanes at 10,952/10,952 and GPU smoke at 1/1. Native CPU/GPU partitions are
**7,084 complete + 102 partial + 0 missing + 3,423 not applicable + 343
indeterminate**; SIMD is **7,096 + 102 partial + 0 missing + 3,411 not
applicable + 343 indeterminate**. Node and browser WASM remain **6,713
complete + 586 partial + 888 missing + 2,713 not applicable + 52
indeterminate**. The aggregate remains `passed_with_backend_gaps`; the 343
ambiguous native cases, partial receipts, WASM receipt gaps, and fallback/
backend-identity taxonomy are still open. No fixture inputs, expected values,
thresholds, case IDs, or denominators changed. The envelope is
`/tmp/all-backends-post-receipt-classifier.json` (SHA-256
`e3edd78e6421aff1cd168fdf0931d1344c8382a1e19d3d05e73bb6043a114131`).

## 32.23 ImageChops.constant mode metadata parity (2026-09-01)

The next deterministic public divergence was `ImageChops.constant` with an
explicit source mode. Pillow's constant operation materializes an 8-bit `L`
image even when the input is `1`, `CMYK`, `YCbCr`, `HSV`, `I`, or `F`; Rust's
pipeline retained the explicit source-mode tag while carrying the same byte
result. Commit `942542ec7` clears that metadata for `PipelineOp::Constant`,
matching the existing materializing operations. The six-mode native probe and
focused core regression are exact, with no fixture, case-ID, denominator,
threshold, or backend-classification changes.

## 32.24 Eager opened-image receipt classification (2026-09-01)

The receipt audit still treated a fixed set of successful opened-image paths as
missing pipeline receipts. Commit `a07994173` adds narrow, immutable-source
proofs for PNG `P`/`I;16` headers and matching JPEG Exif cases, while leaving
unknown opened assets conservative. The receipt-state suite passes 21/21; the
full denominator remains 10,952. The follow-up all-backend envelope records
319 native indeterminate cases instead of 343 and increases the native
not-applicable partition without changing IDs, values, or backend labels.

## 32.25 Exact one-axis heterogeneous F GPU reduction (2026-09-01)

The prior forced-generic-shader audit established the first divergence: WGSL
multiplied heterogeneous F samples by fixed weights and accumulated in f32,
while Pillow's `Resample.c`/`ImagingResample` path uses the f64 coefficient
table and f64 accumulation before the f32 store. Commit `8032f95f1` adds a
host-verified integer lane for one changed axis of a single F resize: finite
normal same-sign source words, nonnegative 22-bit coefficients proven equal to
the f64 table, and aligned row sums within 53 bits. The WGSL shader multiplies
24-bit significands by the fixed weights with two-limb u64 arithmetic and
performs round-to-nearest-even f32 conversion, preserving the oracle's store
boundary; the existing dyadic proof remains for two-axis/chained domains.

The focused `f_resize_` tests pass 8/8 and the complete GPU-pool group passes
15/15, including terminal native receipts. A disposable 160-probe campaign
had 38 native admissions and zero byte mismatches after the patch. Broad
multi-axis/non-dyadic arithmetic, negative-coefficient filters, and special
values remain on exact host semantic control because they are not covered by
this proof. No fixtures, thresholds, IDs, denominators, or receipt taxonomies
were changed.

## 32.26 Fresh all-backend envelope after parity and evidence fixes (2026-09-01)

The complete schema-v3 envelope was regenerated at source `8032f95f1` after
the ImageChops mode fix, eager opened-image classifier, and F GPU integer lane.
CPU, SIMD, GPU, Node WASM, and browser WASM each remain value-exact at
10,952/10,952, and GPU smoke is 1/1. CPU/GPU receipt partitions are **7,084
complete + 102 partial + 0 missing + 3,447 not applicable + 319
indeterminate**; SIMD is **7,096 + 102 + 0 + 3,435 + 319**. Node and browser
WASM are **6,713 complete + 586 partial + 888 missing + 2,738 not applicable
+ 27 indeterminate**. The aggregate remains `passed_with_backend_gaps` because
partial/indeterminate receipts, WASM receipt gaps, backend identity, and
fallback taxonomy are still open. The schema-valid artifact is
`/tmp/all-backends-post-f64-integer.json` (SHA-256
`0f9136d79c501b9c953e6f78b8c984282df4fd353e38aa8b536f747a07a7c37f`).

## 32.27 Retaining hidden workflow errors for receipt classification (2026-09-01)

The receipt classifier had one evidence-only blind spot: `run_case` recorded
internal setup and call failures while the public parity envelope discarded
them. The classifier therefore could not distinguish an explicit pre-dispatch
error from a dependency-only `not_run` observation. Commit `40c3e9860` keeps
those errors in the execution-evidence sidecar, strips them from the public
parity result, and carries them through stateful child-batch merging. This
preserves the public result schema while giving classification the same
observed error boundary as the workflow.

No case IDs, values, fixtures, thresholds, or denominators changed. The
follow-up receipt suite exercises the hidden-error path together with the
materialization boundary and remains part of the 24/24 passing receipt tests.

## 32.28 Queued prefixes before explicit public errors (2026-09-01)

Commit `635afb555` completes the classifier boundary correction. A queued
prefix followed by an explicit public-call error is classified as an eager
pre-materialization path only when no terminal/result boundary was observed;
materialized prefixes, partial receipts, and dependency-only `not_run` cases
remain conservative. The public parity envelope remains unchanged, while the
sidecar gains the retained internal execution-error evidence.

On the fixed 10,952-case CPU evidence replay, the correction reclassified
exactly 299 cases from `indeterminate` to `not_applicable`: **7,084 complete,
102 partial, 0 missing, 3,746 not applicable, and 20 indeterminate**. The
canonical value run remained 10,952/10,952 with zero failures, and
`make migration-parity-receipt-test` passes 24/24. The reclassification is
evidence-only; no public case was removed or renamed.

## 32.29 Signed two-axis F resize reduction proof (2026-09-01)

The one-axis integer lane in `8032f95f1` still left a deterministic gap for
signed samples when both resize axes changed. The first divergence is the
same as the broader F audit: Pillow's resample path uses f64 coefficients and
accumulation before the f32 store, while an ordinary WGSL shader accumulates
in f32. Commit `a3d2c886b` adds a host-verified signed two-limb integer lane,
proves every sequential partial sum within the aligned 53-bit bound, and
computes the horizontal rounded-f32 words before the vertical pass. The
admission continues to reject nonfinite, subnormal, negative-zero,
non-dyadic, coefficient-mismatch, overflow, and unproven chained inputs.

Focused F tests pass 8/8 and the GPU-pool group passes 15/15. A fresh native
Pillow-vs-Rust probe after the merge is byte-exact for 45/45 cases (seven
native GPU receipts and 38 exact host semantic-control receipts), including
mixed-sign two-axis rows and NaN, infinity, signed-zero, and subnormal edge
probes. The forced generic shader still diverges on heterogeneous/non-dyadic
arithmetic, so those inputs remain on the exact host route. No fixtures,
thresholds, IDs, denominators, or receipt taxonomy changed.

## 32.30 Fresh all-backend envelope after classifier and F fixes (2026-09-01)

The complete schema-v3 envelope was regenerated at source `a3d2c886b` after
the two classifier commits and the signed two-axis F proof. CPU, SIMD, GPU,
Node WASM, and browser WASM each remain value-exact at **10,952/10,952**;
GPU smoke is **1/1**. The fixed case-ID digest is
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.

CPU reports **7,084 complete + 102 partial + 0 missing + 3,766 not applicable
+ 0 indeterminate**. SIMD reports **7,096 complete + 102 partial + 0 missing
+ 3,754 not applicable + 0 indeterminate**, with 405 terminal receipts
actually executed by CPU. GPU reports **7,084 complete + 102 partial + 0
missing + 3,766 not applicable + 0 indeterminate**, with 391 terminal CPU
receipts and 6,693 GPU receipts. Its fallback taxonomy remains explicit:
147 exact host semantic-control, 147 logical-mode, 60 unsafe
primary-dimension, 35 Contrast-midpoint host-image, 3 Transform, and 1
unsafe/incomplete-dimension records. Node and browser WASM each report
**6,713 complete + 586 partial + 888 missing + 2,738 not applicable + 27
indeterminate**.

The aggregate correctly remains `passed_with_backend_gaps`: value parity is
green, but partial receipts, backend identity, fallback taxonomy, and WASM
receipt gaps are not native-backend proof. The schema-valid artifact is
`/tmp/all-backends-post-a3d2.json` (SHA-256
`6f7de544139c6ef047225e00537bb33def1aeaea39084b6c33c07f705d809306`).

## 32.31 Historical benchmark rows and timing gate rechecked (2026-09-01)

The old 70-row GPU failure list was rerun against the current source. All
70 selected workloads completed with requested and actual GPU receipts and no
fallback; the targeted `pipeline-matrix.expanded.rotate.1x1` row is also
native and exact. The historical rows are therefore stale failure evidence,
not current value mismatches. The standard 744-workload benchmark likewise
measured all 744 selected workloads with zero budget failures; its
`not_proven` execution statuses concern benchmark proof, not output parity.

The P2 timing gate remains open. Four fresh fixed-11-ID reports retain 44/44
comparable pairings and terminal no-fallback receipts; consecutive budget
comparisons report 11, 7, and 6 violations. The violating rows do not form a
stable intersection and the GPU draw/filter timing is bimodal with identical
dispatch/copy/mode-conversion receipts, so no deterministic source regression
was found. The factor-1.0 Brightness identity optimization remains a verified
row-level improvement, but two consecutive zero-violation reports are still
required.

The short active queue is
[`benchmark-backend-pending-2026-09-01.md`](benchmark-backend-pending-2026-09-01.md).

## 32.32 Exact one-axis f64 F resize reducer (2026-09-02)

The broader F arithmetic gap now has a verified one-axis device lane. The
first divergence was the generic WGSL f32 product/accumulator: a heterogeneous
5x4 -> 3x2 Bilinear row differed at the first output word, while Pillow's
`Resample.c`/`ImagingResample` path retains f64 coefficients and accumulation
until the observable f32 store. Commit `4fe5535ff` adds marker 9, transporting
each f64 coefficient as its exact integer significand, exponent, and sign; the
shader performs integer products and signed sums and rounds once to f32. The
host admission proof compares every selected row with the ordered f64
`mul_add` result and requires finite normal F words, bounded arithmetic, and
exact final bits. An unchanged axis copies its source/intermediate word rather
than evaluating tiny kernel tails.

The proof intentionally admits only one changed axis. A changed-horizontal
plus changed-vertical operation still consumes a device-written intermediate
whose storage/synchronization contract has not been established on the native
adapter, so it remains exact host semantic control. Subnormal, nonfinite,
negative-zero, overflow/cancellation, and other unproven coefficient domains
also remain host-controlled. The known 2x1 -> 4x1 Bilinear false-proof input
continues to route to host control; it differs by one output ULP when forced
through generic f32 arithmetic.

Focused F tests pass 9/9 and the GPU-pool group 16/16, including a native
`(2,2) -> (1,2)` Bilinear byte/receipt assertion. The rebuilt randomized probe
covered 5,000 finite-F rows (269 actual GPU and 4,731 exact host-control
receipts) with zero mismatches. The committed schema-v3 all-backend envelope
remains value-exact for all 10,952 cases on CPU, SIMD, GPU, Node WASM, and
browser WASM, with GPU smoke 1/1; GPU terminal receipts are 6,698 native GPU
and 386 CPU. The artifact is `/tmp/all-backends-post-4fe5535.json` (SHA-256
`b915cb2f93c172241a2bbe911ba418414aa5cabbd08c39302367a9080e580946`). No
fixtures, thresholds, IDs, denominators, or receipt taxonomy changed; P0
two-axis arithmetic, P1 receipt proof, and P2 timing acceptance remain open.

## 32.33 Pre-materialization validation receipts (2026-09-02)

The receipt audit found a classifier/evidence mismatch rather than 102 native
pipeline executions that had failed to reach a terminal boundary. In those
cases Pillow rejected the public call before constructing deferred work, while
the target telemetry API retained setup records and, on some adapters, a
same-step `partial` record. The prior classifier treated every meaningful
record as a partial pipeline receipt. Commit `cb1813bc8` proves the explicit
step-bound error and rejects the exception whenever an earlier receipt belongs
to a deferred operation; retained setup telemetry is marked
`pipeline_relevant=false` instead of being discarded. A prior deferred receipt,
unknown step, or dependency-only `not_run` remains conservative.

The receipt regression suite passes 27/27. The committed schema-v3 replay is
value-exact for CPU, SIMD, GPU, Node WASM, and browser WASM (10,952/10,952;
GPU smoke 1/1). Native CPU and GPU now report 7,084 complete + 15 genuine
partial + 3,853 not-applicable cases; SIMD reports 7,096 + 15 + 3,841. The
101-case reduction is a classification correction only: all selected IDs,
public errors, values, fixtures, thresholds, and denominators are unchanged.
The aggregate remains `passed_with_backend_gaps` because the 15 partial native
receipts, backend identity/fallback proof, WASM receipt gaps, broader F device
arithmetic, and timing acceptance remain open. The schema-valid artifact is
`/tmp/all-backends-post-cb1813bc8.json` (SHA-256
`aca34d02ab0c31ea6d60587fbddda00edaf8090362ddef5a62412e7115fb22a3`).

## 32.34 Verified two-axis f64 F resize reducer (2026-09-02)

The remaining deterministic F arithmetic gap was the changed-horizontal plus
changed-vertical shape. Pillow's `Resample.c`/`ImagingResample` path stores a
rounded f32 horizontal intermediate before the vertical f64 accumulation; the
marker-9 route previously rejected that shape and sent it to exact host
semantic control even when both device reducers were otherwise provable. The
existing F encoder already places its horizontal and vertical dispatches in
separate compute passes, providing the required ordering boundary.

Commit `f17e1a7da` extends the host proof to materialize exact rounded
horizontal words, then checks the vertical f64 reducer against those words.
Rows with nonfinite, subnormal, negative-zero, overflow, or ordered-f64 versus
exact-reducer disagreement remain conservatively host-controlled. A
heterogeneous `(2,2) -> (1,5)` Bilinear case has a non-dyadic vertical table,
matches Pillow byte-for-byte, and publishes a terminal requested-GPU /
actual-GPU receipt with no fallback. The focused F-resize suite is 10/10 and
the GPU-pool suite is 17/17.

The committed schema-v3 replay at `/tmp/all-backends-post-f17e1a7da.json`
(SHA-256
`ee84c4c4f94aa0c81e1deeea6d712137e1b33299370da3866cacce66fe6c5a7f`) remains
value-exact for all 10,952 CPU, SIMD, GPU, Node WASM, and browser WASM cases;
GPU smoke is 1/1. Native GPU terminal receipts increase from 6,698 to 6,701,
exact-host-control fallbacks decrease from 142 to 139, and GPU CPU fallbacks
are 383. No fixture, expected value, threshold, case ID, denominator, or
receipt taxonomy changed. The remaining P0 families are special-value and
overflow/cancellation rows, Box ratios outside the proven bounds, and chains
outside the cumulative intermediate proof.

## 32.35 Native raw-color ExtractBand GPU lane (2026-09-02)

The next deterministic routing gap was not a shader arithmetic mismatch. The
GPU preflight rejected CMYK, HSV, and YCbCr logical modes before the existing
`extract_band.wgsl` byte-copy path, even though the packed RGBA/RGB transport
retains their native channel order. Pillow's `getchannel` semantics for these
modes copy the selected C/M/Y/K, H/S/V, or Y/Cb/Cr byte into an L8 result;
`PutPixel` followed by `ExtractBand` has the same raw-byte contract.

Commit `f55a770ad` admits only `ExtractBand`/`PutPixel` batches for those three
modes. Index validation, packed channel selection, mode-transition
segmentation, and L8 output conversion are unchanged. A focused native GPU
regression covers all three modes and asserts exact bytes, requested/actual
GPU identity, one dispatch, and no fallback. A filtered 30-case replay
(direct CMYK/HSV/YCbCr cases, maintained suffix rows, and a materialized
CMYK batch) is byte-exact on CPU, SIMD, GPU, Node WASM, and browser WASM;
all 30 GPU receipts are terminal native receipts with no fallback.

The committed full envelope is `/tmp/all-backends-post-f55a770ad.json`
(SHA-256 `7b97442f45ffe3f6db1128bd04cbc6dd438963f1aab900a374fcd2c46a943f4e`)
at revision `f55a770ad8a082ac08064a4ea948c114c836ec71`. All 10,952 selected
cases remain value-exact on every public lane, with GPU smoke 1/1. The GPU
partition is 6,731 native GPU and 353 CPU receipts; the logical-mode fallback
count is 117 (down from 147), while native receipt totals remain 7,084
complete + 15 partial + 3,853 not-applicable. The aggregate therefore stays
`passed_with_backend_gaps`: remaining Draw/Fit/EffectSpread/typed-operation
routes, genuine partial receipts, WASM receipt proof, broader F arithmetic,
and the timing gate are still open. No fixtures, expected values, thresholds,
IDs, denominators, or receipt taxonomy changed.

## 32.36 Native raw-byte EffectSpread GPU lane (2026-09-02)

The next deterministic routing gap was a preflight mode guard, not a pixel
algorithm mismatch. The existing `effect_spread.wgsl` shader already gathers
from a host-generated relocation map, preserving Pillow 12.2.0
`libImaging/Effects.c` RNG/collision/scatter order. Before `ebc7e765a`, logical
P/PA/1/RGBX/RGBa/HSV/YCbCr/CMYK/I/F tags were rejected before this path, so
valid outputs were produced by CPU/exact-host control even though complete
packed bytes are the storage contract. Typed I;16 remains on its separate
path.

Commit `ebc7e765a` admits EffectSpread in those raw packed-mode guards. The
focused native GPU regression covers 13 byte-backed modes, exact bytes, one
dispatch, requested/actual GPU identity, and no fallback. The filtered 34-case
replay is value-exact across CPU/SIMD/GPU/Node/browser; all 34 GPU receipts are
terminal native receipts with no fallback.

The latest schema-v3 envelope is
`/tmp/all-backends-post-ebc7e765a.json` (SHA-256
`17326cec1fd5c70132aa21bb00af6f060b194e1d484491fbb5100f29c712beee`) at
revision `ebc7e765a41b984e237fa5593133fbb7b56a3798`. All 10,952 selected cases
remain value-exact on CPU, SIMD, GPU, Node WASM, and browser WASM, with GPU
smoke 1/1. GPU now reports 6,744 native receipts and 340 CPU receipts; the
logical-mode preflight count is 104 (down from 117). Native receipt totals
remain 7,084 complete + 15 partial + 3,853 not-applicable, so the aggregate
remains `passed_with_backend_gaps`. No fixtures, expected values, thresholds,
IDs, denominators, or receipt taxonomy changed. Remaining P0 work is broader
F special/overflow/cancellation/Box/chained arithmetic; P1 covers partial
native receipts, backend/fallback identity, and WASM receipt proof; P2 is the
zero-violation timing gate.

## 32.37 Native raw-byte Draw GPU lane (2026-09-02)

The next deterministic routing gap was another preflight mode guard rather
than a shader arithmetic mismatch. Pillow 12.2.0's ImageDraw scan conversion
already produced the exact destination canvas on the host; `draw.wgsl` only
copies complete packed bytes. Before `7d1cc0af9`, logical `1`, `P`, `PA`,
`RGBX`, `RGBa`, `CMYK`, `HSV`, `YCbCr`, `I`, and `F` tags were rejected before
that copy path, so valid Draw batches unnecessarily published host receipts.
Typed `I;16` remains on its existing typed path.

Commit `7d1cc0af9` admits only batches made entirely of the existing draw
operations for those raw transport modes. A focused native regression covers
all ten modes and asserts byte equality, requested/actual GPU identity, one
dispatch, and no fallback. The filtered 73-case replay is exact across CPU,
SIMD, GPU, Node WASM, and browser WASM; 72 GPU receipts are terminal native
receipts and the single zero-height safety case remains host-controlled. No
fixtures, expected values, thresholds, IDs, denominators, or receipt
taxonomy changed.

## 32.38 Native nearest indexed Fit GPU lane (2026-09-02)

The first divergence for indexed `ImageOps.fit` was a conservative logical
mode guard: every `P`/`PA` Fit row was routed to host control even when
`filter=NEAREST` performs only integer index/channel selection. Pillow's
nearest Fit preserves P indices and PA index/alpha bytes; the Rust GPU path
already lowers Fit to the exact host-prepared separable resize kernels, with
no palette expansion for this subset.

Commit `0797e71f5` admits only `Fit(filter=NEAREST)` for `P` and `PA`. Filtered
and interpolating indexed Fit operations remain on exact host semantic
control. The focused native regression covers both modes and asserts exact
bytes plus a terminal native receipt. The fixed 15-case replay is exact on
CPU, SIMD, GPU, Node WASM, and browser WASM; 14 GPU receipts are terminal
native receipts and the `pa-putpalette-expansion` row intentionally remains
host-controlled. No fixtures, expected values, thresholds, IDs, denominators,
or receipt taxonomy changed.

## 32.39 Full post-Draw/Fit envelope (2026-09-02)

The schema-v3 envelope was regenerated at committed source `0797e71f5` after
both narrow admissions. The artifact is
`/tmp/all-backends-post-0797e71f5.json` (SHA-256
`d95f880a7393ef078bbd09d7b0364cd0ee53836d31f232e2fa4754546369ba0f`). CPU,
SIMD, GPU, Node WASM, and browser WASM each remain value-exact at
**10,952/10,952**; GPU smoke is **1/1**. CPU has 7,084 complete terminal
receipts; SIMD has 6,691 native SIMD plus 405 CPU receipts; GPU has 6,832
native GPU plus 252 CPU receipts. Each native lane retains 15 genuine partial
receipts, and the fixed case-ID digest remains
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.

The aggregate correctly remains `passed_with_backend_gaps`: value parity is
green, while broader F special/overflow/cancellation/Box/chained arithmetic,
partial native receipts, backend/fallback identity, WASM receipt proof, and
the zero-violation timing gate remain open. No fixture, expected value,
threshold, ID, denominator, or receipt taxonomy changed.

## 32.40 Finite subnormal F resize device reducer (2026-09-02)

The first divergence was a value-domain hole in marker 9: its host decoder
accepted only normal f32 source words, so finite subnormal F resize rows were
sent to exact host semantic control even when their arithmetic was otherwise
provable. Pillow 12.2.0 `Resample.c`/`ImagingResample` accumulates each row in
f64 and stores the final value as f32. That boundary preserves finite
subnormal words and signed nonzero bits; filtered zero results are
canonicalized to positive zero.

Commit `b1962c6dd` extends the marker-9 proof to every finite f32 source word,
representing subnormals at the exact `2^-149` scale and retaining signed-zero
information. Both convolution shaders now round the integer reducer directly
to f32 with ties-to-even subnormal handling, including the smallest-normal
crossover. The host proof still rejects nonfinite values, negative-zero final
stores, and any ordered-f64 result that differs from the integer reducer.

While validating the extension, a separate first divergence was isolated for
horizontal upscaling followed by vertical downscaling: `(2,2) -> (4,1)`
produced `[1.5, 1.625, 1.875, 2]` on the existing device schedule versus
Pillow's `[2, 2, 3, 3]`. That geometry remains on exact host semantic control
until its intermediate ordering and buffer contract are proven; the guard is
independent of the subnormal arithmetic proof.

A deterministic native Pillow-vs-Rust probe covered 1,050 finite,
subnormal, signed-zero, and edge-word F rows across the five filtered
resamplers: 372 rows executed on native GPU and 678 used exact host semantic
control, with **0 mismatches**. The focused GPU module is 23/23, `make
build-dev` and `make -C pillow-rs fmt` pass. The committed schema-v3 envelope
at `/tmp/all-backends-post-b1962c6dd.json` (SHA-256
`9a981a51e018cad9c65390311b6e38c58e40ee75861595c01e0c7baee48af5df`) remains
value-exact for all **10,952/10,952** CPU, SIMD, GPU, Node WASM, and browser
WASM cases, with GPU smoke **1/1**. Native receipt counts remain CPU 7,084;
SIMD 6,691 plus 405 CPU; GPU 6,832 plus 252 CPU, with 15 genuine partials in
each native lane. No fixtures, expected values, thresholds, IDs,
denominators, or receipt taxonomy changed. The remaining P0 families are
nonfinite and negative-zero output words, coefficient overflow/cancellation,
Box ratios outside the proven bounds, chains outside the cumulative proof, and
the guarded mixed up/down two-axis schedule.

## 32.41 Pure filtered F resize-chain proof (2026-09-02)

The next deterministic gap was a routing boundary rather than a byte
divergence: marker 9 accepted one filtered F resize, while a chain of filtered
resizes was still sent to exact host semantic control even when every stage
could be checked independently. Pillow materializes each `Resize` result as a
rounded f32 image before the following operation, so a chain proof must carry
those exact words forward instead of comparing every stage with the original
source.

Commit `33e0f11ec` limits the extension to pure finite F `Resize` chains. For
each stage it validates Pillow's f64 coefficient table, compares ordered f64
accumulation with the integer WGSL reducer, materializes each changed-axis
f32 intermediate, and feeds those words into the next stage's proof. Nearest
stages, non-Resize operations, nonfinite words, negative-zero final stores,
overflow/cancellation disagreements, and the guarded horizontal-upscale /
vertical-downscale schedule remain exact host semantic control.

The native regression covers a two-stage Bicubic-to-Lanczos chain and matches
CPU bytes with four device dispatches and a terminal native-GPU receipt. A
500-case deterministic chain probe had **0 mismatches** (25 native GPU, 475
exact host semantic control). The committed schema-v3 envelope at
`/tmp/all-backends-post-33e0f11ec.json` (SHA-256
`d91175eb93e4580d3a40da029cc86ea6903d6b2bebeb46aa99c6d11a7700be4f`) remains
value-exact for **10,952/10,952** CPU, SIMD, GPU, Node WASM, and browser WASM
cases, with GPU smoke **1/1**. Native receipt counts remain CPU 7,084; SIMD
6,691 plus 405 CPU; GPU 6,832 plus 252 CPU, with 15 genuine partials in each
native lane. No fixtures, expected values, thresholds, IDs, denominators, or
receipt taxonomy changed. Remaining P0 work is nonfinite/negative-zero output,
coefficient overflow/cancellation, Box ratios or mixed operation chains
outside the per-stage proof, and the guarded mixed up/down schedule.

## 32.42 Mixed-axis F Box scheduling guard (2026-09-02)

The filtered-chain extension exposed a separate first divergence in the
older marker-6 admission path. For a finite F source `(1,2)` resized to
`(2,1)` with `BOX`, Pillow returned the repeated f32 word `0xbfd2b818`
(`-1.6462430953979492`), while the device path returned `0xc11cbef1`
(`-9.796616554260254`). Pillow's separable `Resample.c`/
`ImagingResample` writes the horizontal f32 intermediate before the vertical
reduction; marker 6 could admit horizontal upscaling plus vertical
downscaling and read a stale or incorrectly ordered intermediate. Marker 9
already rejected this schedule, but marker 6, the dyadic chain proof, and the
central router did not.

Commit `ea15ac316` rejects that geometry in all three proof functions and in
the central F-mode routing guard. These rows now take exact host semantic
control, preserving Pillow bytes and an explicit requested-GPU/actual-CPU
receipt rather than claiming a native proof. The native regression test
asserts the exact CPU bytes and fallback receipt. A 2,304-case Box geometry
sweep now has **0 mismatches** (three deterministic mismatches before the
guard); focused F-resize tests pass **14/14**, the GPU-pool group **25/25**,
and `make build-dev` passes.

The post-guard schema-v3 envelope at
`/tmp/all-backends-post-ea15ac316.json` (SHA-256
`8fef943b7e5a97188e4aa44ca4d34a54cf99acf7d9cffdf92f9506a1ade035cf`) is
value-exact for **10,952/10,952** CPU, SIMD, GPU, Node WASM, and browser WASM
cases, with GPU smoke **1/1**. Native terminal partitions remain CPU 7,084;
SIMD 6,691 plus 405 CPU; GPU 6,832 plus 252 CPU, with 15 genuine partials in
each native lane. The aggregate status remains `passed_with_backend_gaps`
because those receipt gaps are unchanged. No fixtures, expected values,
thresholds, IDs, denominators, or receipt taxonomy changed. Remaining P0
work is the broader native-GPU F arithmetic domain (nonfinite and negative-
zero outputs, coefficient overflow/cancellation, larger Box ratios, and
chains containing non-Resize stages); the mixed-axis schedule is now
explicitly guarded and parity-safe.

## 32.43 Deferred setup receipts before public-call errors (2026-09-02)

The post-guard envelope exposed 15 apparent partial native receipts in each
CPU, SIMD, and GPU lane. Fourteen of those records were not incomplete image
pipelines: twelve histogram mode-validation cases and the `ImageOps.fit` and
`ImageOps.pad` validation cases queued a setup mutation (`putpixel`) and then
raised a public error before any observed image materialization. The remaining
`pipeline-composition.filter-rgba-5x5-invert` case has an observed filtered
prefix before `invert` raises, so it is a genuine partial receipt and remains
in the proof partition.

Commit `b867867ee` narrows `_receipts_are_pre_materialization_error` to allow
earlier deferred receipts only when they are known pipeline-mutating setup
operations and no result boundary was observed. Earlier deferred results such
as `resize` and `filter` remain authoritative, and step-less receipts remain
conservative. The new regression covers the setup-mutation case while the
existing prior-deferred-result case still reports `partial_receipt`;
`make migration-parity-receipt-test` passes **28/28**.

The fresh schema-v3 envelope at
`/tmp/all-backends-post-b867867ee.json` (SHA-256
`64690d9cdbf3415d69e742347a4410c523fcadc2ad4a4118d6c520a533ad754b`) is
revision `b867867ee5b52dd7674b524380233781b39952a5`. CPU, SIMD, GPU, Node
WASM, and browser WASM remain value-exact at **10,952/10,952**, with GPU smoke
**1/1**. CPU and GPU now report **7,084 complete + 1 genuine partial + 3,867
not-applicable**; SIMD reports **7,096 complete + 1 genuine partial + 3,855
not-applicable**. The WASM lanes are unchanged at 6,713 complete, 586 partial,
888 missing, 2,738 not-applicable, and 27 indeterminate. The aggregate remains
`passed_with_backend_gaps` because the one observed native partial per lane,
backend/fallback identity, WASM receipt gaps, broader F arithmetic, and the
zero-violation timing gate remain open. No fixtures, expected values,
thresholds, IDs, denominators, or public parity outputs changed.

## 32.44 Proven finite F overflow stores (2026-09-02)

The remaining marker-9 value-domain hole was a legitimate finite-input
overflow at Pillow's final f32 store. For source words
`[0x7f7fffff, 0x7f7fffff, 0xff7fffff]` and a 3→2 resize, Pillow's ordered
f64 accumulation produces a positive infinity for the first Bicubic and
Lanczos output and a negative infinity for the corresponding Hamming output;
the other output words remain the exact finite rounded values. The prior
integer reducer rejected every f64 accumulator with an infinite final cast,
so these parity-safe rows stayed on exact host semantic control.

Commit `19acd29ab` extends the host integer conversion and both WGSL reducers
to encode ±infinity when the exact final magnitude overflows f32. NaN results,
intermediate overflow followed by cancellation, and any ordered-f64 versus
exact-integer disagreement remain rejected. The native regression covers the
max/max/−max Bicubic, Lanczos, and Hamming rows, checks Pillow's exact output
words, and observes terminal native-GPU receipts. Focused F-resize tests pass
**17/17** and the serial GPU-pool group passes **28/28**; `make build-dev` and
formatting pass.

The fresh schema-v3 envelope at
`/tmp/all-backends-post-19acd29ab.json` (SHA-256
`3e2f0c5bac51737de40e202ad993de64f28673379dc8bda4ada216631089c6ce`) is
revision `19acd29abdef41da22c3f3875c553e00c3d3c3be`. CPU, SIMD, GPU, Node
WASM, and browser WASM remain value-exact at **10,952/10,952**, with GPU smoke
**1/1**; CPU and GPU report 7,084 complete + 1 genuine partial + 3,867
not-applicable, SIMD reports 7,096 + 1 + 3,855, and the WASM partitions remain
6,713 complete + 586 partial + 888 missing + 2,738 not-applicable + 27
indeterminate. Remaining P0 work is NaN/invalid special-value arithmetic,
unproven negative-zero/cancellation rows whose ordered f64 result disagrees
with the exact reducer, larger Box ratios, and chains outside the proven
per-stage contract. No fixtures, expected values, thresholds, IDs,
denominators, or public parity outputs changed.

## 32.45 Proven signed negative-zero filtered stores (2026-09-02)

Pillow's F-mode Box reducer can intentionally store a signed zero. With source
words `[0x80000001, 0x00000000, 0x00000000]` (the minimum negative f32
subnormal followed by two positive zeros), a 1×3 → 1×1 Box resize accumulates
a negative f64 value whose final f32 store underflows to `0x80000000`. The
previous marker-9 admission check rejected every negative-zero result even
when the exact integer reducer produced the same bit pattern, so this row was
kept on exact host semantic control.

Commit `19acd29ab` removes that blanket sign-bit rejection while retaining the
ordered-f64-versus-exact-sum equality check. The native regression verifies the
Pillow/CPU/GPU bytes and a terminal native-GPU receipt; the integer conversion
unit test also covers a negative value below the f32 subnormal range. A nearby
counterexample, `[-1e-40, +1e-40, +0]`, remains correctly rejected: Pillow's
ordered f64 multiply/add sequence leaves a tiny negative residual that rounds
to `-0`, while the exact integer products cancel to `+0`. This preserves the
conservative boundary for cancellation rows whose ordered result is not
mathematically identical to the exact reducer.

Focused F-resize tests pass **17/17**, the serial GPU-pool group passes
**28/28**, `make migration-parity-receipt-test` passes **28/28**, and
`make build-dev` plus formatting pass. The schema-v3 envelope at
`/tmp/all-backends-post-19acd29ab.json` (SHA-256
`3e2f0c5bac51737de40e202ad993de64f28673379dc8bda4ada216631089c6ce`) is
revision `19acd29abdef41da22c3f3875c553e00c3d3c3be`; all five public lanes
remain **10,952/10,952** value-exact with GPU smoke **1/1**. Native receipt
partitions are unchanged (CPU/GPU 7,084 complete + 1 genuine partial + 3,867
not-applicable; SIMD 7,096 + 1 + 3,855; WASM 6,713 complete + 586 partial +
888 missing + 2,738 not-applicable + 27 indeterminate). Remaining P0 work is
NaN/invalid special-value arithmetic, other unproven cancellation/negative-
zero rows, broader Box ratios, and chains outside the proven per-stage
contract. No fixtures, expected values, thresholds, IDs, denominators, or
public parity outputs changed.

## 32.46 Native GPU Contrast after an exact PutPixel prefix (2026-09-02)

The first divergence in this lane was a receipt/route boundary rather than a
pixel mismatch. A pipeline containing one exact `PutPixel` followed by
`ImageEnhance.Contrast` was sent to host control because the GPU batch carries
one scalar midpoint, and the old preflight computed that midpoint from the
original source. Pillow's `ImageEnhance.Contrast` computes its rounded
grayscale midpoint from the image after the preceding pixel write, then
blends the current image with that midpoint.

Commit `5ed9f152e` adds a deliberately narrow proof for one non-palette
`PutPixel` immediately before a single `Contrast`. The control plane mirrors
that byte-layout write with the existing exact Rust `PutPixel` primitive only
to derive the post-write midpoint; the complete `PutPixel -> Contrast` batch
still executes on the native GPU and publishes one terminal receipt. Palette
writes, longer prefixes, multiple/later Contrast operations, and layouts
without a proven midpoint continue to use exact host control.

The focused native regression covers L, LA, RGB, RGBA, and CMYK and matches the
CPU result with two GPU dispatches, requested/actual GPU identity, and no
fallback. The filtered all-backend replay is
`/tmp/contrast-prefix-all-backends.json` (SHA-256
`2db9e1f47d3e2fce94ccfcf51162cb64ad194e0ccf6a794f462c9e66d3a640ca`): all
**35/35** selected cases are byte-exact on CPU, SIMD, GPU, Node WASM, and
browser WASM, with 35 terminal-complete receipts in each lane and 35 native
GPU receipts without fallback. The serial GPU Contrast tests are 2/2, the
full Rust library tests are 54/54, receipt-state tests are 28/28, and
`make build-dev` plus formatting pass. The full 10,952-case artifact was not
regenerated in this run because the existing per-lane artifacts already
consume the host's temporary-disk budget; the prior committed envelope
remains the denominator authority, while this fixed replay proves the changed
rows.

No fixtures, expected values, thresholds, IDs, denominators, or receipt
taxonomy changed. P0 broader F arithmetic, the remaining native partial and
WASM receipt gaps, backend/fallback reconciliation outside this proven prefix,
and the P2 timing gate remain open.

## 32.47 Native GPU CMYK PutAlpha promotion (2026-09-02)

The next deterministic routing gap was a conservative logical-mode preflight,
not a pixel mismatch. Pillow permits scalar and image-backed `PutAlpha` on a
CMYK source by promoting CMYK through its integer `cmyk2rgb` conversion and
replacing the resulting alpha band. The Rust GPU registry and
`put_alpha.wgsl`/`put_alpha_data.wgsl` already implement that exact conversion,
but the CMYK mode whitelist omitted both terminal operations, so valid cases
were sent to exact host semantic control with an `unsupported logical mode`
fallback.

The guard now admits only a single terminal CMYK `PutAlpha` or `PutAlphaData`
operation. The output is RGBA after promotion; a following operation still
requires segmentation with updated logical-mode metadata and remains on the
conservative path. No shader arithmetic or public error contract changed.

The focused native regression covers scalar and L-mask inputs, compares the
GPU bytes with the CPU result, and requires requested/actual GPU identity, one
dispatch, and no fallback. The fixed all-backend replay is
`/tmp/putalpha-cmyk-all-backends.json` (SHA-256
`50995134050c39326b97158c17b8a9f358c8e6739d1667ebe1d43d1fac8055f7`): both
cases are byte-exact on CPU, SIMD, GPU, Node WASM, and browser WASM, with 2/2
terminal-complete receipts on every lane and 2/2 native GPU receipts. The
focused GPU test and formatting pass; the full 10,952-case denominator was not
regenerated in this narrow run. No fixtures, expected values, thresholds,
IDs, denominators, or receipt taxonomy changed. Remaining work is the broader
F arithmetic proof, the native/WASM receipt gaps, backend/fallback
reconciliation outside proven admissions, and the P2 timing gate.

## 32.48 Native GPU typed I;16 filtered resize (2026-09-02)

The next deterministic routing gap was a valid typed resize that had no
device-side sample contract. `I;16`, `I;16L`, and `I;16B` filtered resizes were
therefore kept on exact host semantic control even though the CPU implementation
already matched Pillow's byte-oriented two-pass behavior. A diagnostic first
divergence also found that a 4x4 -> 5x3 separable resize needs a 5x4 intermediate
buffer; endpoint-only capacity accounting reserved too little storage. On
Lanczos/Bicubic same-size axes, evaluating tiny tails in the device integer
reducer could overflow its alignment envelope even though Pillow's typed
byte-level pass is an identity.

Commit `2ff9a6951` adds marker 10 to the shared resize shaders. The uploader
preserves each declared two-byte sequence, the shader decodes that sequence,
reuses the exact integer f64-coefficient reducer, applies Pillow's per-byte
u16 round/clip store, and copies unchanged axes. Host admission proves both
passes, all intermediate u16 words, declared byte order, and the device
reducer envelope before selecting native GPU. `I;16N`, chains, mixed operation
batches, and any unproven coefficient/value domain remain exact host semantic
control.

The permanent native regression covers Bilinear, Bicubic, Lanczos, Hamming,
and Box across all three admitted byte orders, with exact CPU/GPU bytes and
terminal requested/actual-GPU receipts. The deterministic stress matrix covered
1,365 cases (three modes, seven source geometries, thirteen output geometries,
five filters): **0 mismatches**, with 926 rows admitted by the proof. Focused
typed-resize tests pass **2/2** and the full `pillow-rs` library passes
**57/57**. The selected all-backend replay at revision `2ff9a6951` is
`build/migration-parity/all-backends-test-result.json` (SHA-256
`58f1e1b3fcad066b5b9e82e2d5910fd502c593e53fa15759293fd312ac3c571c`). All
three cases are value-exact with terminal-complete receipts on CPU, SIMD, GPU,
Node WASM, and browser WASM; the GPU partition has **2 native GPU + 1 exact
host semantic-control** receipts because the I;16N case remains deliberately
guarded. GPU smoke is **1/1** and the artifact status remains
`passed_with_backend_gaps`.

No fixtures, expected values, thresholds, IDs, denominators, or receipt
taxonomy changed. Remaining work is the broader F arithmetic domain, the
native/WASM receipt gaps, backend/fallback reconciliation outside proven
admissions, and the P2 zero-violation timing gate.

## 32.49 Observed pipeline prefixes are terminal at their own boundary (2026-09-02)

The remaining native partial was not a pixel or public-error mismatch. The
`pipeline-composition.filter-rgba-5x5-invert` workflow successfully exposed
the filtered RGBA image, then Pillow correctly raised its `ImageOps.invert`
mode-validation error. The runner waited until the entire workflow succeeded
before setting `terminal_complete`, so the already observed `Filter5x5`
receipt was incorrectly retained as a partial prefix in the CPU, SIMD, and
GPU sidecars.

Commit `70a92f4ca` marks a receipt terminal as soon as a successful observed
pipeline/result or terminal observation proves that operation's materialized
boundary. Deferred receipts with no observed boundary remain conservative
partial records; filter constructors and mutating setup observations cannot
promote an unrelated receipt. Public parity output and the later RGBA error
remain unchanged, and the receipt-state regression suite passes **29/29**.

The focused all-backend replay at
`/tmp/receipt-prefix-all-backends-70a92f4ca.json` (SHA-256
`093f18d3ade437eef91ce70053a76a37b14d629883869d06084f4ff84dd1e992`) is
schema-valid and value-exact for the fixed case. CPU, SIMD, and GPU each have
one complete terminal receipt with zero partial cases; the GPU receipt is
actual GPU with no fallback. Node and browser WASM still retain their known
constructor/receipt gap for this case. The historical full-denominator
envelope was still pending at the time of this focused replay; section 32.50
records the regenerated fixed-denominator result.
No fixtures, expected values, thresholds, IDs, denominators, or public error
contracts changed. Remaining work is the broader F arithmetic domain, WASM
receipt gaps and backend identity outside proven rows, and the P2 timing gate.

## 32.50 Full-envelope receipt boundary after the observed-prefix fix (2026-09-02)

The fixed all-public-cases replay was regenerated at committed revision
`2969b323c96b7dc33b5b9c74ded75b77c4dde3c3`. Its schema-v3 artifact is
`/tmp/all-backends-post-2969b323.json` (SHA-256
`50e893989476cacee452f220e6f10e32166a2e0212058e9b5926360e42551d8f`), with
the unchanged case-ID digest
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526` and
10,952 selected cases in every lane. All five public lanes remain
value-exact (10,952/10,952), GPU smoke is 1/1, and the aggregate status is
`passed_with_backend_gaps`.

The observed-boundary correction removes the historical native partial from
the full envelope: CPU has 7,085 complete pipeline cases and zero partials;
SIMD has 7,097 complete and zero partials (6,698 native SIMD plus 405 CPU
receipts); and GPU has 7,085 complete and zero partials (6,880 native GPU
plus 211 CPU receipts). The GPU lane records 142 exact host semantic-control
fallbacks alongside explicit dimension, logical-mode, and Transform guards.
Node and browser WASM remain identical at 6,713 complete, 586 partial, 888
missing, 2,738 not-applicable, and 27 indeterminate pipeline cases; these
are receipt/export coverage gaps, not value mismatches. No fixtures, expected
values, thresholds, IDs, denominators, or public error contracts changed.
Remaining work is the broader F arithmetic domain, WASM receipt/export gaps,
backend identity outside proven admissions, and the P2 timing gate.

## 32.51 JS/WASM observed-boundary receipts (2026-09-02)

The native receipt fix exposed the same candidate-state bug in the shared
Node/browser WASM workflow: it only remembered a receipt when the final
workflow step emitted telemetry. A successful intermediate image observation
therefore could not terminalize the preceding receipt when a later public
call failed. Commit `d0ee51d9a` mirrors the Python operation-boundary
classifier, retains the latest receipt candidate across workflow steps, and
clears it only for an unobserved final operation or a failed final
observation. Filter parameter constructors and mutating setup observations
remain non-materializing.

The former-partial set was replayed on both hosts through `make test-wasm`:
all **586/586** selected cases remained value/error-exact, 485 receipts became
terminal, and the remaining 101 all fail before a successful materialization
boundary. The full fixed-denominator replays are
`/tmp/wasm-boundary-full-node-d0ee51d9a.json` (SHA-256
`5d73ecbd6d6680fb65b7e0b91813ac30ac734c68a8c19522a76ffb7b7a8d0e06`) and
`/tmp/wasm-boundary-full-browser-d0ee51d9a.json` (SHA-256
`b2dca82a37a9332733783d8106373da82df2159c8320feb628fc8a85a8d40c9c`). Each
host compares all **10,952/10,952** cases exactly. Node and browser each now
report 7,198 complete, 101 partial, 888 missing, 2,738 not-applicable, and
27 indeterminate pipeline cases (7,204 terminal receipts, 15,191 completed
receipts, and 3,653 not-recorded cases); the two host partitions are otherwise
identical. The 101 partial records are retained as explicit pre-materialization
errors, while missing/indeterminate records remain binding/export evidence
gaps. No fixtures, expected values, thresholds, IDs, denominators, or public
error contracts changed. Remaining work is the 888 missing/27 indeterminate
WASM receipt/export cases, broader F arithmetic, native backend identity, and
the P2 timing gate.

## 32.52 WASM validation errors are explicit evidence boundaries (2026-09-02)

The next full replay found one remaining evidence-only divergence: the shared
JS workflow kept setup/call exceptions internally but returned only dependent
`not_run` observations. The Python WASM aggregator also classified without the
target result, so identical Pillow/WASM validation failures were reported as
20 indeterminate pipeline gaps. Commit `a2cf8c102` preserves step-bound
`execution_errors` in the internal JS result, passes target results to the
classifier, and strips that diagnostic field from the canonical public parity
comparison envelope. This changes no public values or errors and adds a
regression test for the aggregator boundary.

The focused former-indeterminate set is 20/20 value/error-exact on both Node
and browser, with all 20 classified as pre-materialization validation
boundaries. The full fixed-denominator artifacts are
`/tmp/wasm-errorbound-full-node-a2cf8c102.json` (SHA-256
`3998bd57b9a9ac3dd4ed679a70159957cbe855b6837ecbcdd825861a60d71780`) and
`/tmp/wasm-errorbound-full-browser-a2cf8c102.json` (SHA-256
`97535c5db628350aa3342ad1ee3fa44f5065019b13df7e87d6089e617876e9b7`). Both
hosts remain value/error-exact for **10,952/10,952** cases. Their receipt
partition is now 7,198 complete, 3,754 not-applicable validation boundaries,
zero missing, zero partial, and zero indeterminate cases (7,204 terminal
receipts, 15,191 completed receipts, and 3,653 not-recorded cases). The
remaining WASM work is backend/export identity evidence rather than these
resolved validation rows; broader F arithmetic, native backend identity, and
the P2 timing gate remain open.

## 32.53 Zero-operation observations no longer prove backend execution (2026-09-02)

The next first divergence was in receipt identity, not public image behavior.
The post-`2969b323c` native sidecars contained terminal observation records with
`operation_count=0` and empty `operation_telemetry`. These records were emitted
after a deferred operation had already drained, but the runner treated them as
backend proofs and allowed them to replace the preceding real receipt. The
same boundary appeared in the JS/WASM evidence path. In the three affected
RGBA→PA workflows, Pillow's eager `convert("PA")` correctly materialized the
queued byte `putpixel`; Rust produced exact PA bytes, but `quantize()` returned
a metadata-bearing empty pipeline whose materialization overwrote that
`PutPixel` receipt with a zero-operation sample.

Commit `2164e2226` fixes the source and receipt boundary. Empty
`execute_prepared` batches now return their input without clearing or
publishing telemetry. The Python and JS runners retain raw zero-operation
records for diagnostics, mark them `pipeline_relevant=false`, require actual
pipeline work before terminal/backend accounting, and preserve the latest
meaningful receipt candidate across observed boundaries. Commit `2835ce29a`
aligns the JS/WASM `terminal_incomplete_cases` aggregate with the per-case
classifier, so validation-boundary receipts are not counted as deferred
partial gaps. No public result/error contract changed.

The focused four-case replay (the PA nonstandard-P case and the three
RGBA→PA cases) is schema-valid and value/error-exact on CPU, SIMD, GPU, Node
WASM, and browser WASM, with 4/4 terminal receipts in every lane and no
fallback. The fixed-denominator replay at `2835ce29` is
`/tmp/all-backends-post-2835ce29.json` (SHA-256
`9bd4bf29816f0923a5ef4fbfaf119fbc890a975e70b5c2c7ca5e177905cffc25`), with
the unchanged case-ID digest
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526` and
10,952/10,952 exact public comparisons in all five lanes. CPU reports 6,838
terminal receipts and 6,832 pipeline-complete cases; SIMD reports 6,850 and
6,844; GPU reports 6,627 native GPU plus 211 CPU receipts and 6,832
pipeline-complete cases. Node and browser each report 6,951 terminal receipts
and 6,945 pipeline-complete cases. All lanes have zero partial, missing, or
indeterminate pipeline cases; the remaining 4,007/4,120 not-applicable rows
are explicit non-pipeline or pre-materialization boundaries, and raw
zero-operation observations remain available in the sidecars for diagnosis.

`make migration-parity-receipt-test` passes **34/34**; `make build-dev`,
`make -C pillow-rs fmt`, Python compilation, and the focused all-backend
replay pass. The aggregate remains `passed_with_backend_gaps` because GPU
identity/fallback reconciliation (including 211 CPU receipts and 142 exact
host-control routes), the broader F arithmetic domain, and the P2 timing gate
are still open. No fixtures, expected values, thresholds, IDs, denominators,
or public parity outputs were changed.

## 32.54 Marker-9 F special-value arithmetic (2026-09-02)

The next deterministic P0 divergence was the marker-9 F admission boundary,
not a public byte mismatch. The proof intentionally rejected every non-finite
source word even when Pillow's ordered f64 resampler had a deterministic
result. A native diagnostic also found a separate Box-copy error: a signaling
NaN was copied as `0x7fa00001`, while Pillow's f32-to-f64 product and final
store quieted it to `0x7fe00001`.

Commit `bc8197617` adds a narrowly bounded IEEE special-value state machine to
the host proof and both convolution shaders. The reducer preserves the first
NaN payload/sign in tap order, canonicalizes invalid zero*infinity and
opposite-infinity results, and preserves the sign of a lone infinity. The Box
copy path quiets signaling NaNs while preserving payload/sign and keeps the
existing positive-zero canonicalization. A row is admitted only when this
device model equals Pillow's ordered f64-to-f32 result; mixed special ordering
or cancellation that does not match remains exact host semantic control.

The permanent native GPU regression covers signaling/quiet NaN payloads,
positive and negative infinity, invalid products, opposite-infinity
cancellation, and Box copies. The focused `f_resize_f64` group is **8/8**.
Two deterministic diagnostic probes (1,000 one-special rows and 1,800
two/three-special rows across heterogeneous geometries and filters) found
**0 mismatches** against native Pillow; rows outside the proof continue to
publish an explicit host-control receipt. `make -C pillow-rs fmt`,
`make build-dev`, and the serial focused tests pass. `make -C pillow-rs clippy`
remains blocked by the pre-existing pinned `image-slash-star` libavif
1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

The fixed all-public-cases replay at committed revision
`bc8197617bc0ba880f08aa251f294a51df788d95` is
`/tmp/all-backends-post-bc8197617.json` (SHA-256
`0b75a5cdce922104f6d69b585ca5e0188c1d336c8d6029bf41378a4b755ab7fd`). The
unchanged 10,952-case ID digest remains
`881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`; CPU,
SIMD, GPU, Node WASM, and browser WASM are each **10,952/10,952**
value/error-exact and GPU smoke is **1/1**. CPU has 6,838 terminal receipts
(6,832 pipeline-complete), SIMD 6,850 (6,844 pipeline-complete), and GPU
6,627 native-GPU plus 211 CPU receipts (6,832 pipeline-complete, 142 exact
host-control fallbacks). Every lane has zero partial, missing, or
indeterminate pipeline cases. The aggregate remains
`passed_with_backend_gaps` because backend identity reconciliation and the
remaining mixed-order/cancellation, Box-ratio, chain, and larger-domain F
proofs are still open. No fixtures, expected values, thresholds, IDs,
denominators, or public parity outputs changed.

## 32.55 Thumbnail reduction and typed resampling parity (2026-09-02)

The next deterministic parity failures were all in the reducing-gap path,
not in the public operation set. `Image.thumbnail` computes its
aspect-preserving dimensions before calling `resize`; the lazy operation
carried only the rectangular request, so each backend applied the aspect
calculation a second time (for example, an `11x7` source requested as
`10x7` was planned as `9x6`). CPU Thumbnail also used rounded integer byte
averages instead of Pillow's `Reduce.c` fixed-reciprocal arithmetic. Native
Pillow reduces `RGBa` and `RGBX` as raw four-channel samples, while the
transport had classified their fourth byte as alpha. Finally, typed `I`
Thumbnail reduction and resize differed from `ImagingReduceNxN_32bpc` and
`ImagingResample`: flat `i64` reduction, an `f64` horizontal intermediate,
and non-fused accumulation changed overflow and one-ULP results.

Commit `0013d013e` carries Pillow's final thumbnail dimensions once through
the CPU, SIMD, and GPU planners; routes byte Thumbnail reduction through the
exact `Reduce.c` implementation; keeps `RGBa`/`RGBX` raw in CPU, SIMD, and
the mode-4 WGSL reducer; mirrors the 32bpc pair/quartet grouping for F and I;
keeps typed I horizontal results as INT32 before the vertical pass; uses the
fractional post-reduce box for typed F/I Thumbnail resampling; and uses
explicit fused multiply-add ordering where the native C build does so. A
finite zero-valued F Thumbnail may use the proven constant Resize lowering;
nonzero reducing constants remain on exact host semantic control when the
f32 reduction could overflow or round differently.

The canonical migration parity run is **10,952/10,952** with zero failures.
The committed-source schema-v3 all-backend envelope is
`build/migration-parity/all-backends-test-result.json` (SHA-256
`c1e45bd9951e7881aa4616d26c2a56984df3237ee22f49d3b9fea0e0344893fa`),
revision `0013d013e829216e8dcaa47b6bae4469eaf71a33`, and unchanged case-ID
digest `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
CPU, SIMD, GPU, Node WASM, and browser WASM each remain
**10,952/10,952** value/error-exact; GPU smoke is **1/1**. CPU reports 6,838
terminal receipts (6,832 pipeline-complete), SIMD 6,850 (6,844 complete),
and GPU 6,628 native-GPU plus 210 CPU receipts (6,832 complete, including
141 exact host semantic-control routes). Node and browser each report 6,951
terminal receipts (6,945 pipeline-complete), with zero partial, missing, or
indeterminate pipeline cases. Focused CPU geometry tests are **6/6** and GPU
pool tests are **36/36**. `make -C pillow-rs fmt` and `make build-dev` pass;
Clippy remains blocked by the pre-existing pinned libavif
1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Remaining work is broader native-GPU F arithmetic
(mixed special ordering/cancellation, unproven Box ratios and chains, and
larger heterogeneous domains), backend identity/fallback reconciliation, and
the P2 equal-ID/equal-receipt timing gate.

## 32.56 Constant F-mode ImageOps.pad native admission (2026-09-02)

The next backend gap was a valid value-parity row that the GPU preflight
classified as an unsupported logical mode: `ImageOps.pad` on a constant `F`
source with a named fill. Pillow's contain resize leaves the scalar sample
unchanged and its named `"red"` fill resolves to the `F` value `76.0`. Before
the fix, Rust kept the row on the CPU semantic path; the pad shader's packed
byte normalizer also treated an admitted `F` word as RGBA, which would corrupt
the scalar bits.

The fix admits only a single non-nearest `F` Pad whose source words are proven
constant and finite. The existing exact constant-resize marker carries the
source word through the contain pass, `gpu_pad_fill` transports the complete
little-endian scalar word (including the zero-fill default), and the pad WGSL
placement keeps mode-8 words opaque instead of normalizing them as color
channels. No mixed batch or non-constant `F` row is widened.

The focused native regression
`f_pad_constant_source_preserves_scalar_fill_on_gpu` is **1/1** with exact
bytes and a terminal GPU receipt (`operation_count=3`, no fallback). The
filtered all-backend replay `/tmp/f-pad-all-backends.json` is schema-valid and
value/error-exact on CPU, SIMD, GPU, Node WASM, and browser WASM; the GPU row
is **1/1 native GPU**. `make -C pillow-rs fmt`, `make build-dev`, and the
serial GPU-pool suite (**37/37**) pass. Clippy remains blocked by the
pre-existing pinned `image-slash-star` libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2
environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Remaining work is the broader F arithmetic proof,
other logical-mode/backend identity gaps, and the P2 timing gate.

## 32.57 Constant and heterogeneous F-mode ImageOps.fit nearest admission (2026-09-02)

The next valid logical-mode gap was `ImageOps.fit` with `F` samples and the
`NEAREST` method. The host already builds Pillow's boxed one-tap coefficient
tables for Fit, but the GPU preflight left every F Fit row on exact host
semantic control; the Fit parameter path also used the ordinary byte marker,
which would feed scalar words through the generic f32 convolution branch.

The fix admits only a single nearest F Fit. Its host-generated one-tap tables
use Pillow's f32 crop-boundary conversion and cumulative affine walk; marker 7
copies complete four-byte words in both resize passes, and an explicit compute
pass boundary makes the horizontal intermediate visible before the vertical
pass on Metal. No filtered Fit or mixed F batch is widened.

The permanent heterogeneous native regression
`f_fit_nearest_native_gpu_preserves_scalar_words` is **1/1** with exact bytes
and a terminal GPU receipt (`operation_count=2`, no fallback). The filtered
all-backend replay `/tmp/f-fit-nearest-all-backends.json` (SHA-256
`1bc7a9b21f08554e84762714ebfbcb4f25d4117c9c512d91aef5c6f4059412ad`) is
schema-valid and value/error-exact on CPU, SIMD, GPU, Node WASM, and browser
WASM; the GPU row is **1/1 native GPU**. `make -C pillow-rs fmt`, the serial
GPU-pool suite (**38/38**), and the focused all-backend replay pass.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Remaining work is broader F arithmetic, other
logical-mode/backend identity gaps, and the P2 timing gate.

## 32.58 I-mode Cover→Pad nearest chain native admission (2026-09-02)

The next valid logical-mode gap was the composed `I` workflow
`ImageOps.cover(..., method=NEAREST)` followed by
`ImageOps.pad(..., method=NEAREST)`. The contain/cover planner already lowers
the first operation to a nearest word-copy resize, but the second Pad was
classified as an unsupported logical mode because its placement shader
normalized the four-byte signed sample as an L/RGBA pixel. Pillow keeps `I`
samples as signed little-endian int32 words and a scalar pad color such as
`7` is the complete word `0x00000007`.

Commit `544d0ebc1` admits only nearest `I` Resize/Pad batches. `gpu_pad_fill` now carries
the complete scalar word (with omitted fill as zero), mode-7 placement keeps
the word opaque, and the I Pad's contain horizontal pass, vertical pass, and
placement pass each get an explicit compute-pass boundary. Filtered Pad and
other typed arithmetic remain on exact host semantic control.

The focused signed-word native regression
`i_cover_pad_nearest_native_gpu_preserves_signed_words` is **1/1** with exact
bytes and five dispatches. The filtered all-backend replay
`/tmp/i-cover-pad-all-backends.json` (SHA-256
`a12189686480ea6883e157daf5906116ca3903aae0e13dea46d0bd6942a5b27e`) is
schema-valid and value/error-exact on CPU, SIMD, GPU, Node WASM, and browser
WASM; its GPU materialization has a terminal native-GPU receipt with five
dispatches and no fallback. The GPU-pool suite is **39/39**; fmt and
build-dev pass.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Remaining work is broader F arithmetic, other
logical-mode/backend identity gaps, and the P2 timing gate.

## 32.59 I-mode Filter3x3→nearest Resize native admission (2026-09-02)

The next valid typed composition was `I` `Filter3x3` followed immediately by
`Resize(..., method=NEAREST)`. Pillow's filter stage computes signed i32
samples, while nearest resize only relocates complete four-byte words. Rust
previously kept the complete chain on exact host semantic control because the
generic filtered-resize admission did not prove that composition, even though
the two stages preserve the same signed-word contract.

Commit `202177a39` admits only this narrow `Filter3x3`→nearest chain after the
existing image-aware I-filter safety proof. The typed filter remains arithmetic
over signed words; the nearest coefficient path performs a one-tap word copy.
Filtered resize, `Filter5x5`, reversed ordering, and mixed or mode-changing
chains remain host-controlled.

The focused regression
`i_filter_nearest_resize_native_gpu_preserves_signed_words` is **1/1** with
exact signed-word bytes and a terminal native-GPU receipt (three dispatches,
no fallback). The all-backend replay
`/tmp/i-filter-resize-all-backends-202177a39.json` (SHA-256
`5927e9206ee895d85786ab6de345b28544f129f3f89f49f29e10b5a371fce9c4`) is
schema-valid and value/error-exact on CPU, SIMD, GPU, Node WASM, and browser
WASM; the GPU row is native GPU with no fallback. The narrow guard tests,
`make build-dev`, and `make -C pillow-rs fmt` pass.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Remaining work is broader F arithmetic, other
logical-mode/backend identity gaps, and the P2 timing gate.

## 32.60 Palette-first RGB merge native admission (2026-09-02)

The first divergence in `Image.merge("RGB", [P, L, L])` was backend identity,
not value semantics. Pillow's `ImagingMerge` accepts a P image only as the
first band and consumes its raw palette indices. Rust's CPU path already did
that, and `merge.wgsl` already interleaved raw bytes, but GPU preflight omitted
`Merge` from the contextual P whitelist and reported an `unsupported logical
mode` fallback.

Commit `c68bce674` admits exactly one RGB merge whose bands are P, L, and L in
that order. The native path preserves index bytes (for example
`[1, 32, 48, 2, 33, 49]`) instead of expanding palette colors. LAB, aliases,
typed destinations, mixed chains, and other palette compositions remain on
exact host semantic control.

The focused admission/native tests are **2/2**. The all-backend replay
`/tmp/merge-palette-first-c68bce674.json` (SHA-256
`38a0dffc030e8e4be4cb7cb09c909a164e3aa812d9cf5498342e764fc6976630`) is
schema-valid and value/error-exact on CPU, SIMD, GPU, Node WASM, and browser
WASM; the GPU row has one terminal native-GPU dispatch and no fallback. No
fixtures, expected values, thresholds, IDs, denominators, public errors, or
receipt rules changed.

## 32.61 F-mode PutData prefix before filtered Resize (2026-09-02)

The next backend gap was a valid deferred `PutData(F)` prefix followed by a
non-dyadic filtered resize. Marker 9 originally accepted only a pure
`Resize` chain, so the source replacement remained on exact host semantic
control even when the replacement words and the f64 coefficient proof were
valid. Pillow applies the raw little-endian replacement before evaluating the
resize; proving the stale upload would be a semantic error.

Commit `35fbfbe4d` admits only this prefix shape. The marker-9 proof validates
the replacement byte length against the current dimensions, requires complete
four-byte words, substitutes those words before each subsequent `Resize`, and
continues to reject geometry, mode-changing, arithmetic, and mixed-axis
prefixes without their own storage proof. The native regression uses an
initial `2x2` F image, replaces all four words with `[0.1, -0.3, 1.7, 2.9]`,
then applies `BILINEAR` `Resize(1,5)`: CPU and GPU bytes are exact, the
terminal receipt is GPU/GPU, `operation_count=2`, `dispatch_count=3`, and no
fallback. The focused marker-9 group is **19/19**.

The committed-source schema-v3 all-backend envelope
`build/migration-parity/all-backends-test-result.json` (SHA-256
`176ea46adbb610494509f8945fcaff36cc4223cd16d9d9e7cd3d38a7862ae5f0`, revision
`02e63e4c2de0610d453fbe91bbb61d2259ae3610`) is value/error-exact at
10,952/10,952 on CPU, SIMD, GPU, Node WASM, and browser WASM; GPU smoke is
1/1. CPU reports 6,838 terminal receipts (6,832 pipeline-complete), SIMD 6,850
(6,844 pipeline-complete), and GPU 6,632 native GPU plus 206 exact host
semantic-control CPU receipts (6,832 pipeline-complete). Every lane has zero
partial, missing, or indeterminate pipeline receipts. The GPU execution
sidecar `build/migration-parity/all-backends/parity-gpu-execution.json` has
SHA-256 `40b6172b7dddb4d84a355a578343bbab10de3d83592cb72a2256bd32018d9fb5`.
No fixtures, expected values, thresholds, IDs, denominators, public errors, or
receipt rules changed.

## 32.62 Non-finite F PutData before order-statistic filters (2026-09-02)

The F order-statistic preflight already checked that the initial source words
were finite, but it treated every deferred `PutData(F)` payload as safe. A
payload containing NaN or either infinity could therefore reach the WGSL
float insertion/min comparison path. Pillow's C order-statistic implementation
has a defined scalar special-value behavior; WGSL's partial comparisons do
not establish that same total order, and the result can depend on the device
comparison semantics.

Commit `53d87a44c` closes that false admission. `gpu_float_filter_is_supported`
now checks every F `PutData` payload for the exact current image byte length,
four-byte alignment, and finite little-endian f32 words before allowing
`MaxFilter`, `MinFilter`, `MedianFilter`, or `RankFilter`. Finite raw updates
retain the existing native shader contract; non-finite, malformed, and
wrong-mode updates use exact host semantic control. Focused admission coverage
is **1/1** for the finite case and rejection cases for NaN, positive/negative
infinity, wrong length, and wrong mode; the existing F GPU group remains
**25/25**. This is a parity guard, not a public-operation limitation, and no
fixtures, expected values, thresholds, IDs, denominators, public errors, or
receipt rules changed.

## 32.63 Mixed-axis marker-9 F resize admission (2026-09-02)

The remaining mixed-axis scheduling guard was stale for marker 9. The F
encoder already puts its horizontal and vertical reducers in separate compute
passes, so a proven f64-equivalent row does not observe the old Metal race that
required host control for earlier marker-6 and dyadic reducers. Pillow's
`Resample.c`/`ImagingResample` path stores the horizontal f32 intermediate before
the vertical pass; marker 9's host proof materializes and validates the same
intermediate before admitting the device path.

Commit `a109e0179` removes only the marker-9 and central-router mixed-axis
guard. Marker 6 and the generalized dyadic reducer retain their conservative
guard, and rows that fail the marker-9 f64 proof still use exact host semantic
control. Native regressions cover heterogeneous `2x2 -> 4x1`
Bilinear/Bicubic/Lanczos/Hamming and the prior signed `1x2 -> 2x1` Box
divergence; every case matches CPU byte-for-byte with requested=actual GPU,
two dispatches, and no fallback. A 120-case mixed-axis probe across three
finite source patterns, eight geometries, and five filters has 0 mismatches
(98 native GPU, 22 exact host semantic control).

The canonical schema-v3 all-backend replay at revision
`a109e0179d795d46f0dadf4e30cc395e175af6a9` remains value/error-exact at
10,952/10,952 for CPU, SIMD, GPU, Node WASM, and browser WASM; GPU smoke is
1/1. The artifact
`build/migration-parity/all-backends-test-result.json` has SHA-256
`6da966a869678a49b7e9a5016e79e5f9e01b198fcb99a11181da1dfd29a1c70a`, and its
GPU execution sidecar has SHA-256
`0c918a1be9418fcf55e440eb3633d5f2005b8e2821b3d10680a7d0beedc70951`.
CPU reports 6,838 terminal receipts, SIMD 6,850, and GPU 6,632 native GPU
plus 206 exact host semantic-control CPU receipts; all pipeline partitions
have zero partial, missing, or indeterminate receipts. No fixtures,
thresholds, IDs, denominators, public errors, or receipt rules changed.
