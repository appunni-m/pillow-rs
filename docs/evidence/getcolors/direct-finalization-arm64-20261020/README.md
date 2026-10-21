# GetColors direct finalization candidate — no-go

The candidate streamed occupied direct-table slots into the final formatted
vector, avoiding the intermediate slot-order vector. Counting, probe order,
fallback behavior, and sparse-table sorting were unchanged. The candidate was
removed because the selected public-call run did not demonstrate a median gain.

## Selected evidence

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Profile: ARM64 serial CPU; warm `call-fallback-4097`, concurrency 1
- Policy: 5 warmups, 20 iterations × 5 samples (100 measured calls)
- Candidate: Pillow median 177.125 µs / P95 187.500 µs; pillow-rs median 172.542 µs / P95 184.375 µs
- Candidate same-run ratio: 1.0266× Pillow by median
- Receipt: actual CPU `GetColors`, one operation, terminal complete, no fallback
- Parity: selected benchmark gate 1/1; high-cardinality I, varied-I slot-order,
  and nonzero-F focused guards each passed 1/1

The latest pre-candidate selected report recorded 170.292 µs CPU median and
174.7915 µs Pillow median under the same workload ID and sample policy. Its
parity input corpus hash differs from this run (`ae4e8e21…` vs
`fffed899…`), so this is not an exact selected-input A/B pair. The candidate
median is 1.32% above that prior CPU observation; the P95 is lower, but that
does not establish a repeatable full-call improvement. No speedup is claimed.

The candidate unit test initially used `maxcolors=5` with six unique test
pixels; the counter correctly rejected the sixth color. Raising the test limit
to six made the focused direct-vs-sparse order check pass. The candidate and
that test were then removed after the public-call result failed to demonstrate a
median improvement. No assertions or workload thresholds were weakened.

## Artifacts

The exact selected benchmark, parity gate, raw samples, and execution/build
attestations are retained beside this note. No full benchmark, full parity,
coverage, commit, or push was run for this candidate.
