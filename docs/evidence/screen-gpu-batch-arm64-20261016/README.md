# ImageChops.screen RGB queued GPU cohort — 2026-10-16

This is a dirty-tree ARM64 completed-window diagnostic for one native RGB
workload. It records a GPU gap; it does not close Screen across modes, sizes,
or architectures.

## Workload and parity

- Workload: `pipeline-batch.screen.rgb-1024x768`.
- Public operation: `PIL.ImageChops.screen` on two native RGB 1024×768 images.
- Existing direct parity case: `PIL.ImageChops.screen.nuanced.performance-materialized-rgb-noise-1024x768`.
- Existing direct report: `migration-benchmark-89a73a72b879400fb1a093058a436448`; its parity gate passed, and a separate strict GPU parity run passed 9/9.
- Batch inputs use ascending image index `i`, with left and right byte buffers generated from `random.Random(i + 1024*31 + 768*17 + 3)` and the adjacent seed `+1`. Input creation is outside the timed window.
- The corrected harness compared output byte buffers within each subject across all measured windows and compared length-prefixed captures byte-for-byte across Pillow, CPU, SIMD, eager GPU, and queued GPU. All 32 inputs matched exactly.

The batch run is `gpu-batch-1792162055716906000`, recorded in
[`batch-throughput.json`](batch-throughput.json). It contains exactly one
workload. The report records benchmark script SHA-256
`d87b9c6bf631ff86b4e761bea2cb6b04f7a61ba4f10556f9b4ec4385c7a08c88`.

## Completed-window results

Two warmup windows preceded 20 timed windows of 32 images. Each window includes
fresh input-image and graph construction, queue submission, execution,
synchronization, CPU result materialization, and `tobytes()`. Input generation,
output capture, exact byte comparison, and digest calculation are outside the
timer.

| Subject | Median completed images/s | Median 32-image window |
| --- | ---: | ---: |
| Pillow 12.2.0 | 259.155 | 123.478 ms |
| Serial CPU | 2,149.006 | 14.891 ms |
| SIMD | 1,670.678 | 19.154 ms |
| Eager GPU | 329.238 | 97.194 ms |
| Queued GPU | 547.585 | 58.438 ms |

CPU and SIMD both exceed Pillow in this batch cohort. SIMD's receipt proves the
actual vector path with 147,456 blocks and zero scalar tail. Both GPU lanes
report requested and actual GPU execution, 704 actual dispatches, terminal
completion, and no fallback. Each GPU lane uploads two full RGB inputs per job
(3,321,888,768 total upload bytes) and reads back one full output
(1,660,944,384 bytes). Queued GPU reports 176 submissions, a maximum group of
four, and zero live jobs or retained bytes at completion.

Queued GPU throughput is only 0.328× SIMD (about 3.05× lower), although it is
2.113× Pillow. The separate public-call report also measured GPU latency at
1.114 ms versus SIMD at 0.387 ms, so GPU latency was 2.88× slower. This cohort
therefore confirms that the current queued path misses both GPU comparison
targets for this row; it is a selection signal for a focused Screen GPU
optimization. Completed-window measurements do not prove long-duration
throughput or overlapping kernels.

The retained direct report identifies a dirty source revision and the batch
report binds its script SHA, but the batch helper does not bind a loaded core
hash. Do not treat this local result as clean published-matrix credit.
