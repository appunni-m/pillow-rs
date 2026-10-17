# Screen GPU queue-depth A–B–B–A diagnostic

This focused comparison tests whether raising the queued GPU executor's
`max_in_flight` from 4 to 8 closes the measured RGB Screen gap. It uses only
`pipeline-batch.screen.rgb-1024x768`: 32 deterministic native-RGB pairs per
window, two warmups, 20 measured windows, and the same completed boundary from
graph construction through submission, execution, synchronization, host
materialization, and `tobytes()`.

## Evidence and identity

- A1 control: `../screen-gpu-batch-arm64-20261016/batch-throughput.json`, run
  `gpu-batch-1792162055716906000`, `max_in_flight=4`.
- B1 candidate: `batch-throughput.json`, run `gpu-batch-1792162710225754000`,
  `max_in_flight=8`.
- B2 candidate: `batch-throughput-b2.json`, run
  `gpu-batch-1792162741177232000`, `max_in_flight=8`.
- A2 control: `batch-throughput-a2.json`, run
  `gpu-batch-1792162756909560000`, `max_in_flight=4`.
- All runs used benchmark script SHA-256
  `d87b9c6bf631ff86b4e761bea2cb6b04f7a61ba4f10556f9b4ec4385c7a08c88`,
  source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` with a dirty worktree,
  macOS 15.7.7 ARM64, Pillow 12.2.0, and the same loaded `_core.abi3.so` SHA-256
  `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`.
- Each report compares raw output bytes across Pillow, serial CPU, SIMD, eager
  GPU, and queued GPU for all 32 inputs. Every measured window matched exactly.
  SIMD reports 147,456 vector blocks and zero scalar-tail bytes.
- Both GPU subjects report 704 actual GPU dispatches, no fallback, terminal
  completion, 3,321,888,768 uploaded bytes, and 1,660,944,384 readback bytes.
  The queued executor drained all jobs/resources in each run.

## Results

| Run | `max_in_flight` | Queued GPU images/s | SIMD images/s | GPU / SIMD | Submissions | Largest submission |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 | 4 | 547.585 | 1,670.678 | 0.3278× | 198 | 4 |
| B1 | 8 | 622.186 | 2,409.941 | 0.2582× | 374 | 2 |
| B2 | 8 | 528.841 | 1,796.865 | 0.2943× | 374 | 2 |
| A2 | 4 | 549.921 | 1,702.505 | 0.3230× | 200 | 4 |

The A control repeats closely: queued GPU throughput differs by 0.4%, and its
GPU/SIMD ratio differs by 1.5%. B's GPU/SIMD ratio is lower in both runs; the
median ratio is 0.2763× for B versus 0.3254× for A. Raising the configured
limit therefore did not improve GPU relative to SIMD. The first B run's higher
absolute rates also affect SIMD and are not a GPU-specific gain. B submitted
more often and never formed a group larger than two, while A reached four.

**Disposition:** reject `max_in_flight=8` as an optimization for this workload.
The candidate does not close the GPU/SIMD gap; keep the limit at 4 for this
path. This is a single-workload dirty-tree diagnostic, not sustained concurrent
kernel throughput or published matrix credit. No runtime source was changed.

The direct call remains slower on GPU than vector SIMD (1.114 ms vs 0.387 ms).
Kata `pillow-rs#85kj` contains the next source-level allocation hypothesis and
focused guards. Do not infer a shader arithmetic problem from these results.
