# RGB GetBBox blank-image CPU result — native x86_64

## Result

Keep the portable word-scan candidate for the blank RGB path as an x86_64
candidate. In the A–B–B–A bracket, the two candidate medians were 0.121641 and
0.121541 ms, versus 1.213511 and 1.214336 ms in the controls. That is a
9.98× reduction in blank-image target latency; each candidate was 2.395–2.396×
faster than its same-report Pillow baseline.

The filled and noise guards did not regress against their controls, but neither
candidate run reached 2× Pillow on those inputs. Do not mark GetBBox complete:
this evidence covers only serial CPU, RGB, and three 1024×768 data patterns.
Other modes, sizes, SIMD, Parallel CPU, GPU latency, and sustained throughput
remain open.

## Workloads and measurement

- Blank primary: `pil-image-image.getbbox.materialized.rgb-blank-1024x768`
- Filled guard: `pil-image-image.getbbox.materialized.rgb-filled-1024x768`
- Noise guard: `pil-image-image.getbbox.materialized.rgb-noise-1024x768`
- Exact parity cases: matching `PIL.Image.Image.getbbox.nuanced.performance-rgb-{blank,filled,noise}-1024x768` cases.
- Public `getbbox()` call on an already materialized RGB image; warm cache; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Native x86_64 Linux, CPython 3.12.3, Pillow 12.2.0, runner ID `native-x86_64-host-01`; benchmark pinned to an idle CPU core.
- Each leg passed exact parity 3/3. Target receipts reported actual `cpu`, terminal completion, and no fallback.
- Candidate cdylib SHA-256: `dee09f63e602025d70551cb8e960bb8cc0c9cc0a636e1e00b323d4de223c5d19`; control SHA-256: `83af13ac8fda38a21ae69a57e7837a89f9745cfb57116d2da29dadd5b41fd92e`.

## A–B–B–A results

Times are per-call medians in milliseconds. `Pillow / CPU` uses the Pillow and
CPU values from the same report.

| Leg | Workload | Pillow | CPU | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| A1 control | blank RGB | 0.293831 | 1.213511 | 0.242× |
| B1 candidate | blank RGB | 0.291272 | 0.121641 | 2.395× |
| B2 candidate | blank RGB | 0.291266 | 0.121541 | 2.396× |
| A2 control | blank RGB | 0.291521 | 1.214336 | 0.240× |
| A1 control | filled RGB | 0.006280 | 0.005570 | 1.127× |
| B1 candidate | filled RGB | 0.006425 | 0.005436 | 1.182× |
| B2 candidate | filled RGB | 0.006425 | 0.005400 | 1.190× |
| A2 control | filled RGB | 0.006490 | 0.005540 | 1.171× |
| A1 control | noise RGB | 0.010425 | 0.007635 | 1.365× |
| B1 candidate | noise RGB | 0.010505 | 0.007881 | 1.333× |
| B2 candidate | noise RGB | 0.010110 | 0.006565 | 1.540× |
| A2 control | noise RGB | 0.011300 | 0.006850 | 1.650× |

Candidate mean target latency for blank RGB was 0.121591 ms versus 1.213923 ms
for the controls. Filled RGB was 2.5% faster on the candidate mean. Noise was
effectively unchanged between bracket means (0.007223 ms candidate,
0.007243 ms control); its Pillow timing varied across the controls, so retain
the same-report ratios as observations.

## Evidence and admission status

The selected reports, raw samples, parity results, execution attestations,
producer attestations, and source-build receipts are retained under `A1-run2/`,
`B1/`, `B2/`, and `A2-run3/`. `A1-run2` and `A2-run3` are the outer controls;
the two B legs use the same candidate binary. The B1 benchmark report SHA-256
starts `4cc032552abb`; its parity report SHA-256 starts `2abfc1412842`.

The initial B1 direct-operation link contained a transposed producer SHA-256.
The attestation file hashes to
`12a9ccc5d2db78cc43f951aa40180cc26961409967791ae652888aba8fa5e4f5`; the
original link referenced `...91ea...`. The artifact index was correct. The
original link remains preserved, and
[`direct-operation-link-corrected.json`](B1/direct-operation-link-corrected.json)
changes only that reference. Its SHA-256 is
`8d040068ffa80420a2437ec589971cbd54df104da8d0df02efb601a49d8cccdc`.

The corrected link passes strict admission: parity, runtime identity, producer
equivalence, source-build binding, and actual CPU backend all pass. The first
strict check also exposed a producer/verifier contract defect: for an untimed
`benchmark_execution_proof`, the runner retains the selected `call` step and
`observed_steps` boundary, reducing only repeat to 1. The verifier incorrectly
expected no boundary and no steps. The verifier and fixtures now preserve the
selected scope; its focused attestation contracts passed.

PerfDB's proof index initially rejected multiple same-run direct-operation
links before checking whether each link was valid. It now evaluates all
matching links, accepts only complete verified contexts, and rejects
conflicting verified contexts. Focused proof tests passed. A strict generated
baseline was promoted for only
`pil-image-image.getbbox.materialized.rgb-blank-1024x768`:

- Baseline: `getbbox-rgb-blank-x86-20261020`
- Baseline ID: `d3fa52c98a02`
- Source artifact: `4cc032552abb`
- B1 target: 0.121641 ms; same-report Pillow: 0.291272 ms (2.395×)
- The generated baseline and `baseline_sets` record live in the local
  `pillow-perfdb/performance-results.duckdb`; no image campaign was rerun.

The existing focused regression contract
`baseline::tests::promoted_baseline_persists_and_compatible_regression_fails`
passed. Filled and noise guards remain below 2× Pillow, so this does not
complete GetBBox or establish a baseline for those workloads. The direct-link
assembler's multi-input limitation is fixed: it resolves the unique
hash-anchored benchmark input containing the requested workload and parity
input containing the requested case, while retaining strict zero- and
multiple-match rejection. Four focused builder contracts passed. Reusing the
already-imported B1 artifacts, the CLI assembled
[`direct-operation-link-assembled.json`](B1/direct-operation-link-assembled.json)
(SHA-256 `207986a2865f79f22e2a57e40c6c3d8a1d28d360127815ac60cabdef3fc72a99`)
and its strict check returned `status=complete` and
`performance_admission=verified`, with parity, CPU backend execution,
producer/runtime equivalence, and source-build binding all passing. No image
benchmark or parity campaign was rerun.

## Host handling

The run reused `/home/appunni/work/pillow-rs-current-x86-20261020`, the existing
x86 checkout. This experiment created no additional checkout, did not copy the
remote `target/` directory, and did not delete remote files. Other preexisting
directories on that host were left untouched.
