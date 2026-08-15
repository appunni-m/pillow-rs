# Variable-font positive-overflow parity note

This is a Pillow-RS binding/parity note. It does not modify the pinned
`fontdone` source, fixtures, oracle outputs, or coverage denominator.

## Observed case

`PIL.ImageFont.FreeTypeFont.set_variation_by_axes.nuanced.variable-font-positive-axis-overflow`

With axes `[3.4028235e38, 3.4028235e38]`, Pillow 12.2.0 reports the observed
length `19`, while Pillow-RS reports `24`. Ordinary positive and negative axis
values still agree.

## First divergence

The PyO3 binding extracts the input as `Vec<f32>` in
`pillow-rs-py/src/lib.rs`. Pillow’s C wrapper converts the unscaled float to
`FT_Fixed` before multiplying by `65536`; the Pillow-RS path scales first and
saturates positive overflow to `i32::MAX` in `pillow-rs/src/font/imagingft.rs`.
The pinned remote fontdone implementation then correctly normalizes the fixed
coordinate it receives, so fontdone is not the source of this mismatch.

The case remains an explicit parity failure until a separately authorized
Pillow-RS binding fix reproduces the oracle’s conversion order. It is not a
reason to remove the case or alter expected outputs.
