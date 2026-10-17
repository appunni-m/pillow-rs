# ImageDraw.text direct CPU receipt attempt, not credited

This retained diagnostic run is not performance-matrix evidence. It was the
first rerun after adding the direct CPU proof receipt:

- Benchmark: `migration-benchmark-a42092da59104260beefd2f3c37fbb27`.
- Selected workloads: `pil-imagedraw-imagedraw.text.standard` and
  `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768`.
- Target proof reported `actual_backend=cpu` and one operation, but
  `terminal_complete=false`; the aggregate correctly classified it as
  `not_proven`.
- The cause was a genuine proof-run defect: `run_case` returned early when
  benchmark proof skipped observation serialization, before terminalizing a
  successful direct CPU receipt.
- A focused contract now exercises `serialize_observations=False`. The repair
  records terminal completion only after all public workflow calls and any
  timed observation checks succeed.

The accepted rerun is in
`../imagedraw-text-receipt-terminal-arm64-20261017/`. Do not use this attempt's
latencies as a speedup or matrix comparison.
