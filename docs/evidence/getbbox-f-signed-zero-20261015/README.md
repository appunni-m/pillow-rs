# GetBBox F signed-zero parity

Pillow 12.2.0 treats an F-mode pixel containing `-0.0` (`0x80000000`) as
non-empty in `Image.getbbox()`. The target previously decoded the word as
`f32` and tested `value != 0.0`, which treats both signed zeros as empty.
`getbbox_materialized` now checks the stored 32-bit word directly.

The parity input creates the sample with raw F-mode `frombytes` data. This
isolates GetBBox from a separate `putpixel` signed-zero conversion mismatch.

Focused CPU parity used Pillow 12.2.0, CPython 3.12, and the same inline sample
asset in both runs:

| Run | Selected cases | Result |
|---|---|---|
| Before the fix, `migration-parity-254a6d48bd0b4a178d2bd95a925e0756` | `PIL.Image.Image.getbbox.parity.f-negative-zero-is-data` | 1 failed: Pillow returned `(1, 1, 2, 2)`; target returned `None` |
| After the fix, `migration-parity-feb0f65452984b7486640809ce6eaee4` | negative-zero case, `getbbox.nuanced.blank-f`, and `getbbox.nuanced.nonzero-f` | 3 passed |

The retained JSON reports are `getbbox-f-negative-zero-before-fix-20261015.json`
and `getbbox-f-signed-zero-fixed-20261015.json`. These runs establish the
focused semantic correction only; they do not claim full GetBBox mode coverage
or performance improvement. Kata tracking: `pillow-rs#pd3w`.
