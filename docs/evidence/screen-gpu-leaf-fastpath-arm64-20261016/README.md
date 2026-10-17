# Screen secondary-leaf buffer experiment

This experiment tested whether the queued GPU encoder can avoid recursively
allocating a full `BufferPool` for a materialized Screen secondary image. The
candidate was limited to one-stage Screen graphs in L/RGB. It uploaded the
secondary into the parent `img2_arena`; pending or composed secondary graphs
continued through the existing recursive encoder.

## Frozen scope and proof

- Performance workload: `pipeline-batch.screen.rgb-1024x768`; 32 deterministic
  input pairs per window, two warmups, 20 timed windows, `max_jobs=16`,
  `max_in_flight=4`, through image and graph creation, submission, execution,
  synchronization, materialization, and `tobytes()`.
- Tail guards: `pipeline-batch.screen.l-5x5` and
  `pipeline-batch.screen.rgb-5x5`, 32 outputs each, three timed windows. These
  exercise odd native byte lengths through the queued path.
- Each report compares raw output bytes across Pillow, CPU, SIMD, eager GPU,
  and queued GPU. All 32 outputs matched in every measured window. GPU reports
  prove requested=actual GPU, 160 dispatches per small workload / 704 per large
  workload, zero fallbacks, terminal completion, and drained resources. SIMD
  reports positive vector work with zero scalar tail on the large case.
- Direct parity selectors for all four byte modes, all four 5×5 byte tails,
  the full-size RGB Screen case, and the shared-source Multiply→Screen
  composition passed 10/10. The execution attestation shows these selectors
  ran under `python-cpu`; they are not GPU evidence. The queued benchmark's raw
  byte comparisons and per-job GPU receipts are the GPU proof for this change.
- The performance source revision was dirty
  `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; the loaded extension hashes were
  `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749` for
  controls and `d8d7d54e3c08f34ba541f7b64ba5de3f58e7d02c775a542f5fbfca58d672840a`
  for the candidate. Benchmark script SHA-256 was
  `d87b9c6bf631ff86b4e761bea2cb6b04f7a61ba4f10556f9b4ec4385c7a08c88`.

## RGB A–B–B–A results

| Arm | Run | Extension | Queued GPU images/s | SIMD images/s | GPU / SIMD | Submissions / largest group |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| A1 control | `gpu-batch-1792162055716906000` | `8b0dda44…09d749` | 547.585 | 1,670.678 | 0.3278× | 198 / 4 |
| B1 candidate | `gpu-batch-1792163292102677000` | `d8d7d54e…72840a` | 498.570 | 963.969 | 0.5172× | 200 / 4 |
| B2 candidate | `gpu-batch-1792163308988394000` | `d8d7d54e…72840a` | 523.441 | 2,184.782 | 0.2396× | 198 / 4 |
| A2 control | `gpu-batch-1792163417508693000` | `8b0dda44…09d749` | 548.493 | 2,313.253 | 0.2371× | 200 / 4 |

The control GPU rate repeats within 0.2%. Candidate GPU rate is 6.8% below the
control median. The GPU/SIMD ratio does not improve consistently: B1 is worse
than A1, while B2 and A2 are nearly tied. The candidate therefore has no
repeatable end-to-end win. Upload/readback totals and dispatch counts are
unchanged; this experiment did not show a transfer or kernel improvement.

**Disposition:** reject and revert the secondary-leaf buffer fast path. Keep
the existing recursive encoder. The direct Screen call's GPU latency was not
changed by this queued-only experiment. The batch results are dirty-tree,
single-host diagnostics, not long-duration throughput or published matrix
credit.

## Artifacts

- `batch-b1.json`, `batch-b2.json`: candidate full-size RGB runs.
- `batch-a2.json`: post-candidate control; A1 is retained in
  `../screen-gpu-batch-arm64-20261016/batch-throughput.json`.
- `batch-tail-guards.json`: candidate queued L/RGB 5×5 byte-tail runs.
- `parity.json`, `execution-attestation.json`, `producer-attestation.json`:
  focused parity evidence. Its CPU-only profile is explicitly not counted as
  GPU proof.
