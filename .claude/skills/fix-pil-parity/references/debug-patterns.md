# Debug Patterns for PIL Parity Tests

## Compare source Pillow and target `PIL` output

The source and replacement intentionally use the same public module path.
Never import both implementations in one interpreter: whichever package is
loaded first owns `sys.modules["PIL"]`. The maintained runner starts an
isolated source process and an isolated target process, then compares their
serialized observations:

```bash
make migration-parity-case MIGRATION_PARITY_CASE="<case_id>"
```

For a focused algorithm probe, run the identical script twice. The first
process uses the installed Pillow package; the second prepends the checkout
facade, so both scripts still say `from PIL import ...`:

```bash
python3 probe.py > /tmp/pil-source.json
PYTHONPATH="$PWD/pillow-rs-py/python" python3 probe.py > /tmp/pil-target.json
diff -u /tmp/pil-source.json /tmp/pil-target.json
```

`probe.py` should print the output hash and the first differing byte or pixel.
Do not import `pillow_rs` in the probe to stand in for the public target; use
the same `PIL` import that downstream applications use.

## Classify Differences

- **All off by 1**: Rounding issue — check truncation vs rounding vs ceiling
- **Only border pixels**: Edge handling differs — check clamping vs copy vs skip
- **All pixels differ**: Kernel/algorithm is fundamentally wrong — re-research PIL source
- **Large random diffs**: Wrong kernel values, orientation, or formula

## Verify PIL's Actual Runtime Values

Documentation can be wrong. Always verify with:

```bash
python3 -c "from PIL import ImageFilter as PILF; print(PILF.<FILTER>.filterargs)"
```

## Determine Kernel Orientation

To test if kernel ordering matches PIL's C code:

1. Create a simple test image with a single bright pixel
2. Apply the filter in both isolated `PIL` processes
3. Compare the output pattern — if the pattern is flipped, the kernel orientation is wrong

PIL C code applies kernels bottom-to-top (ky=0 maps to row y+1). The filterargs kernel values are stored in the order PIL expects for this bottom-to-top application.
