# Parity Failure Classification Workflow

This workflow turns `coverage_matrix_tests` failure ID output into a maintained
assignment report.  It is only for triage.  It must not replace parity tests,
lower thresholds, rewrite fixture data, or bless Rust output as expected output.

The classifier is `scripts/classify_failure_ids.py`.  It uses only Python's
standard library and reads failure text files that the Rust harness already
emits at `/tmp/pillow_failure_ids.txt`.

## Capture Failure Files

Run each lane in isolation and copy `/tmp/pillow_failure_ids.txt` immediately
after the test completes.  The next lane overwrites that file.

From the repository root:

```bash
cargo test --test coverage_matrix_tests --locked \
  test_native_tt_default_threshold_baseline_not_parity_gate -- --nocapture
cp /tmp/pillow_failure_ids.txt /tmp/native_tt_default_failure_ids.txt

cargo test --test coverage_matrix_tests --locked \
  test_metrics_only_matrix_baseline_is_executed -- --nocapture
cp /tmp/pillow_failure_ids.txt /tmp/metrics_only_failure_ids.txt

cargo test --test coverage_matrix_tests --locked \
  test_outline_cbox_matrix_baseline_is_executed -- --nocapture
cp /tmp/pillow_failure_ids.txt /tmp/outline_cbox_failure_ids.txt
```

For experiment branches, keep the same lane names and choose explicit file
names, for example `/tmp/native_tt_default_after_sdpvtl_trial_failure_ids.txt`.

## Generate The Report

```bash
python3 scripts/classify_failure_ids.py \
  --source-commit "$(git rev-parse --short HEAD)" \
  --lane native_tt_default=/tmp/native_tt_default_failure_ids.txt \
  --lane metrics_only=/tmp/metrics_only_failure_ids.txt \
  --lane outline_cbox=/tmp/outline_cbox_failure_ids.txt \
  --output /tmp/ft_parity_classification.md
```

If the three default `/tmp/*_failure_ids.txt` files exist, the `--lane`
arguments can be omitted:

```bash
python3 scripts/classify_failure_ids.py \
  --source-commit "$(git rev-parse --short HEAD)" \
  --output /tmp/ft_parity_classification.md
```

The report includes:

- Failure counts by lane, stage, font family, and ppem.
- `metrics_only` field-difference counts, single-field failures, and sample
  deltas.
- `outline_cbox` bbox field-difference counts, numeric deltas, and bucket
  samples.
- `native_tt_default` placement and pixel coverage buckets.

## Contract

- Keep the generated report out of committed fixtures unless it is deliberately
  added as documentation.
- Do not edit `tests/fixtures/*.json`, raw byte outputs, matrix thresholds, or
  `coverage_matrix_tests.rs` to make the report look better.
- Do not use FreeType C as a runtime fallback.  C FreeType remains an oracle and
  generator source only.
- If an implementation experiment changes failure classification, regenerate
  the lane failure file and report from the harness output rather than editing
  the report by hand.
