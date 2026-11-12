"""Shared mode/size/input axes for image parity and benchmark workflows."""

# Pillow's complete raster mode set represented by the Rust Mode strategy.
# Internal CoreOnly raster identities are declared separately as target-only
# contracts because Pillow cannot construct them.
MODE_SIZE_PILLOW_MODES = (
    "1",
    "L",
    "LA",
    "La",
    "P",
    "PA",
    "RGB",
    "RGBA",
    "RGBa",
    "RGBX",
    "CMYK",
    "YCbCr",
    "LAB",
    "HSV",
    "I",
    "F",
    "I;16",
    "I;16L",
    "I;16B",
    "I;16N",
)

MODE_SIZE_MATRIX_SHARD_SURFACES = ("PIL.Image.Image", "PIL.ImageChops")
MODE_SIZE_LONG_AXIS_SIZES = ((2048, 1), (2049, 1), (1, 2048), (1, 2049))

# Keep the original cases first so generated input order remains stable.
MODE_SIZE_MATRIX_SIZES = (
    (17, 19),
    (33, 25),
    (256, 192),
    # Odd dimensions one pixel beyond the common material tiers catch row,
    # column, and vector-tail assumptions that aligned sizes can hide.
    (257, 193),
    (5, 7),
    (65, 47),
    (512, 384),
    (513, 385),
    (64, 64),
    (128, 128),
    (1024, 768),
    (1025, 769),
    # Degenerate and lane-boundary shapes exercise short rows, narrow images,
    # and scalar tails around common 16/32/64/128/256-pixel vector widths.
    (1, 1),
    (1, 17),
    (17, 1),
    (15, 17),
    (31, 33),
    (32, 32),
    (63, 65),
    (127, 129),
    (255, 257),
    # Additional size tiers cover tiny neighborhoods, further scalar/SIMD
    # tails, common desktop sizes, and a larger material image.
    (2, 2),
    (3, 5),
    (8, 8),
    (16, 16),
    (48, 33),
    (129, 65),
    (320, 240),
    (640, 480),
    (1280, 720),
    # Add a common 16:9 material workload beyond the existing HD size tier.
    (1600, 900),
    # Full HD, its portrait orientation, and an odd-width near-FHD frame
    # exercise material-size row/column routing and wide SIMD tails.
    (1920, 1080),
    (1080, 1920),
    (1921, 1080),
    # Long row/column workloads pair an aligned 2048-sample span with a
    # one-sample tail, isolating scan and vector-tail behavior from 2D tiling.
    *MODE_SIZE_LONG_AXIS_SIZES,
)

MODE_SIZE_INPUT_VARIANTS = (
    "varied",
    "checkerboard",
    "boundary",
    "impulse",
    "seeded-noise",
)
_MODE_SIZE_FULL_PATTERN_SIZES = (
    (17, 19),
    (33, 25),
    (256, 192),
    (5, 7),
    (65, 47),
    (2, 2),
    (3, 5),
    (8, 8),
    (16, 16),
    (48, 33),
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE = {
    size: ("varied", "checkerboard", "boundary", "impulse")
    for size in _MODE_SIZE_FULL_PATTERN_SIZES
}
# Add noise at two distinct scales without multiplying every large workflow
# into a full pattern cross. Every listed size still has varied pixels.
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(512, 384)] = ("varied", "seeded-noise")
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(257, 193)] = (
    "varied",
    "checkerboard",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(513, 385)] = ("varied", "seeded-noise")
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(64, 64)] = ("varied", "checkerboard")
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(128, 128)] = (
    "varied",
    "checkerboard",
    "seeded-noise",
)
# The 1024x768 row adds a material-size point with one deterministic input.
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1024, 768)] = ("varied",)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1025, 769)] = ("varied", "seeded-noise")
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1280, 720)] = ("varied",)
# Keep this larger workload to one deterministic pattern so every eligible
# mode and operation gets the same material-size input without duplicating a
# very large benchmark and recorded-output cohort.
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1600, 900)] = ("varied",)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1920, 1080)] = ("varied",)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1080, 1920)] = ("varied",)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1921, 1080)] = ("varied",)
# Exercise long contiguous rows and columns with aligned, tail, and value-
# distribution patterns while keeping their total sample count small.
for _long_axis_size in MODE_SIZE_LONG_AXIS_SIZES:
    MODE_SIZE_INPUT_VARIANTS_BY_SIZE[_long_axis_size] = (
        "varied",
        "checkerboard",
        "boundary",
        "seeded-noise",
    )
# Tiny and vector-boundary shapes emphasize edge handling and scalar tails.
# Keep the larger rows selective so the fixture assets and benchmark campaign
# remain bounded while still covering several deterministic data patterns.
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1, 1)] = ("varied", "boundary")
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(1, 17)] = (
    "varied",
    "checkerboard",
    "impulse",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(17, 1)] = (
    "varied",
    "checkerboard",
    "impulse",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(15, 17)] = (
    "varied",
    "checkerboard",
    "boundary",
    "impulse",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(31, 33)] = (
    "varied",
    "checkerboard",
    "boundary",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(32, 32)] = (
    "varied",
    "checkerboard",
    "boundary",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(63, 65)] = (
    "varied",
    "checkerboard",
    "boundary",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(127, 129)] = (
    "varied",
    "checkerboard",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(255, 257)] = (
    "varied",
    "boundary",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(129, 65)] = (
    "varied",
    "checkerboard",
    "boundary",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(320, 240)] = (
    "varied",
    "checkerboard",
    "seeded-noise",
)
MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(640, 480)] = (
    "varied",
    "boundary",
    "seeded-noise",
)
MODE_SIZE_PARITY_SPLIT_SIZES = tuple(
    size
    for size in MODE_SIZE_MATRIX_SIZES
    if size[0] * size[1] > 16_384 or size in MODE_SIZE_LONG_AXIS_SIZES
)


def seeded_noise_sample(index: int, seed: int, lane: int = 0) -> int:
    """Return a stable 16-bit sample mixed from position, lane, and seed."""

    mixed = (index + lane * 0x9E3779B9 + seed * 0x85EBCA6B) & 0xFFFFFFFF
    mixed ^= mixed >> 16
    mixed = (mixed * 0x7FEB352D) & 0xFFFFFFFF
    mixed ^= mixed >> 15
    mixed = (mixed * 0x846CA68B) & 0xFFFFFFFF
    mixed ^= mixed >> 16
    return mixed & 0xFFFF
