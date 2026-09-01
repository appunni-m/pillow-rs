"""Exact Pillow 12.2 public ``Image.putdata`` regressions."""

import struct

import pytest
from PIL import Image as PILImage

from pillow_rs import Image


PALETTE = [
    255, 0, 0,
    0, 255, 0,
    0, 0, 255,
]


def pa_image():
    image = Image.frombytes("P", (3, 1), bytes([7, 8, 9]))
    image.putpalette(PALETTE, "RGB")
    image.putalpha(77)
    return image


@pytest.mark.covers("Image.putdata")
def test_putdata_tuples_are_atomic_and_short_input_preserves_tail():
    image = Image.frombytes(
        "RGB",
        (3, 1),
        bytes.fromhex("0b0c0d0b0c0d0b0c0d"),
    )

    image.putdata([(1, 2, 3), (4, 5, 6)])

    assert image.tobytes() == bytes.fromhex("0102030405060b0c0d")


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ((2,), bytes.fromhex("0200084d094d")),
        ((2, 33), bytes.fromhex("0221084d094d")),
        (2, bytes.fromhex("0200084d094d")),
    ],
)
def test_putdata_pa_accepts_one_or_two_tuple_elements_and_zero_fills_alpha(
    value,
    expected,
):
    image = pa_image()

    image.putdata([value])

    assert image.mode == "PA"
    assert image.tobytes() == expected


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("mode", "packed", "initial", "expected"),
    [
        (
            "RGB",
            0x010203,
            "0b0c0d282932",
            "030201282932",
        ),
        (
            "RGBA",
            0x01020304,
            "0b0c0d0e28293246",
            "0403020128293246",
        ),
        (
            "CMYK",
            0x01020304,
            "0b0c0d0e28293246",
            "0403020128293246",
        ),
        (
            "RGBA",
            -1,
            "0b0c0d0e28293246",
            "ffffffff28293246",
        ),
    ],
)
def test_putdata_packed_multiband_integers_are_little_endian(
    mode,
    packed,
    initial,
    expected,
):
    image = Image.frombytes(mode, (2, 1), bytes.fromhex(initial))

    image.putdata([packed], scale=17, offset=23)

    assert image.tobytes() == bytes.fromhex(expected)


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ((-1, 2, 3), bytes([0, 2, 3])),
        ((256, 2, 3), bytes([255, 2, 3])),
    ],
)
def test_putdata_clips_first_tuple_component_like_pillow(value, expected):
    image = Image.new("RGB", (1, 1))

    image.putdata([value])

    assert image.tobytes() == expected


@pytest.mark.covers("Image.putdata")
def test_putdata_scale_and_offset_match_l_integer_truncation():
    image = Image.frombytes("L", (3, 1), bytes([91, 92, 93]))

    image.putdata([1, 2, -3], scale=2.5, offset=10)

    assert image.tobytes() == bytes([12, 15, 2])


@pytest.mark.covers("Image.putdata")
def test_putdata_i_uses_signed_i32_little_endian_with_scale_and_offset():
    image = Image.frombytes(
        "I",
        (3, 1),
        struct.pack("<iii", 111, 222, 333),
    )

    image.putdata([1.9, -2.2, 2_147_483_647], scale=2, offset=3)

    assert image.tobytes() == struct.pack("<iii", 6, -1, 2_147_483_647)


@pytest.mark.covers("Image.putdata")
def test_putdata_f_uses_f32_little_endian_with_scale_and_offset():
    image = Image.frombytes(
        "F",
        (3, 1),
        struct.pack("<fff", 1.25, 2.5, 3.75),
    )

    image.putdata([1.25, -2, 3.5], scale=2, offset=3)

    assert image.tobytes() == struct.pack("<fff", 5.5, -1.0, 10.0)


@pytest.mark.covers("Image.putdata")
def test_putdata_mode_1_retains_logical_values_and_packs_truthiness():
    image = Image.new("1", (3, 1))

    image.putdata([2, 0, -3])

    assert list(image.getdata()) == [2, 0, 0]
    assert image.tobytes() == bytes([0x80])


@pytest.mark.covers("Image.putdata")
def test_putdata_too_many_entries_is_exact_and_does_not_mutate():
    image = Image.frombytes("RGB", (1, 1), bytes.fromhex("0b0c0d"))

    with pytest.raises(TypeError, match=r"^too many data entries$"):
        image.putdata([(1, 2, 3), (4, 5, 6)])

    assert image.tobytes() == bytes.fromhex("0b0c0d")


@pytest.mark.covers("Image.putdata")
def test_putdata_too_many_preflight_does_not_materialize_outer_sequence():
    calls = []

    class TooMany:
        def __len__(self):
            return 2

        def __getitem__(self, index):
            calls.append(index)
            return (1, 2, 3)

    image = Image.frombytes("RGB", (1, 1), bytes.fromhex("0b0c0d"))

    with pytest.raises(TypeError, match=r"^too many data entries$"):
        image.putdata(TooMany())

    assert calls == []
    assert image.tobytes() == bytes.fromhex("0b0c0d")


@pytest.mark.covers("Image.putdata")
def test_putdata_invalid_multiband_element_preserves_written_prefix():
    image = Image.frombytes(
        "RGB",
        (3, 1),
        bytes.fromhex("0b0c0d0b0c0d0b0c0d"),
    )

    with pytest.raises(TypeError, match=r"^color must be int or tuple$"):
        image.putdata([(1, 2, 3), "invalid", (4, 5, 6)])

    assert image.tobytes() == bytes.fromhex("0102030b0c0d0b0c0d")


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("mode", "packed", "initial", "expected"),
    [
        (
            "RGB",
            0x000102,
            bytes([9, 9, 9] * 2),
            bytes([2, 1, 0, 9, 9, 9]),
        ),
        (
            "RGBA",
            0x00010203,
            bytes([9, 9, 9, 9] * 2),
            bytes([3, 2, 1, 0, 9, 9, 9, 9]),
        ),
        (
            "CMYK",
            0x00010203,
            bytes([9, 9, 9, 9] * 2),
            bytes([3, 2, 1, 0, 9, 9, 9, 9]),
        ),
    ],
)
def test_putdata_mixed_exact_multiband_values_preserve_written_prefix(
    mode,
    packed,
    initial,
    expected,
):
    def run(image_type):
        image = image_type.frombytes(mode, (2, 1), initial)
        with pytest.raises(TypeError, match=r"^color must be int or tuple$"):
            image.putdata([packed, 1.5])
        return image.tobytes()

    assert run(Image) == run(PILImage) == expected


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("image_factory", "data", "message"),
    [
        (
            lambda: Image.new("RGB", (1, 1), (11, 12, 13)),
            [(1, 2)],
            "color must be int, or tuple of one, three or four elements",
        ),
        (
            pa_image,
            [(2, 33, 44)],
            "color must be int, or tuple of one or two elements",
        ),
    ],
)
def test_putdata_rejects_wrong_tuple_arity_exactly(
    image_factory,
    data,
    message,
):
    image = image_factory()
    before = image.tobytes()

    with pytest.raises(TypeError, match=rf"^{message}$"):
        image.putdata(data)

    assert image.tobytes() == before


@pytest.mark.covers("Image.putdata")
def test_putdata_single_band_nested_sequence_writes_only_valid_prefix():
    image = Image.frombytes("L", (3, 1), bytes([91, 92, 93]))

    with pytest.raises(TypeError, match=r"^sequence must be flattened$"):
        image.putdata([1, (2,), 3])

    assert image.tobytes() == bytes([1, 92, 93])


@pytest.mark.covers("Image.putdata")
def test_putdata_nested_getitem_only_object_is_a_sequence():
    class GetOnly:
        def __getitem__(self, index):
            if index == 0:
                return 7
            raise IndexError

    image = Image.frombytes("L", (2, 1), bytes([91, 92]))

    with pytest.raises(TypeError, match=r"^sequence must be flattened$"):
        image.putdata([1, GetOnly()])

    assert image.tobytes() == bytes([1, 92])


@pytest.mark.covers("Image.putdata")
def test_putdata_materializes_outer_sequence_before_declared_count_writes():
    image = Image.frombytes("RGB", (1, 1), bytes([9, 9, 9]))
    observations = []

    class DeclaredOne:
        def __len__(self):
            return 1

        def __getitem__(self, index):
            observations.append((index, image.tobytes()))
            if index == 0:
                return (1, 2, 3)
            if index == 1:
                return "not coerced"
            raise IndexError

    image.putdata(DeclaredOne())

    assert observations == [
        (0, bytes([9, 9, 9])),
        (1, bytes([9, 9, 9])),
        (2, bytes([9, 9, 9])),
    ]
    assert image.tobytes() == bytes([1, 2, 3])


@pytest.mark.covers("Image.putdata")
def test_putdata_exact_list_reads_later_items_live_like_pillow():
    def run(image_type):
        image = image_type.new("L", (3, 1), 9)
        data = [1, None, 3]

        class MutateLaterItem:
            def __float__(self):
                data[2] = 7
                return 2.0

        data[1] = MutateLaterItem()
        image.putdata(data)
        return image.tobytes()

    assert run(Image) == run(PILImage) == bytes([1, 2, 7])


@pytest.mark.covers("Image.putdata")
def test_putdata_generic_sequence_remains_materialized_like_pillow():
    def run(image_type):
        image = image_type.new("L", (3, 1), 9)
        values = [1, None, 3]

        class MutateSource:
            def __float__(self):
                values[2] = 7
                return 2.0

        values[1] = MutateSource()

        class GenericSequence:
            def __len__(self):
                return 3

            def __getitem__(self, index):
                return values[index]

        image.putdata(GenericSequence())
        return image.tobytes()

    assert run(Image) == run(PILImage) == bytes([1, 2, 3])


@pytest.mark.covers("Image.putdata")
def test_putdata_outer_materialization_error_is_exact_and_atomic():
    class BrokenSequence:
        def __len__(self):
            return 2

        def __getitem__(self, index):
            if index == 0:
                return 1
            raise RuntimeError("not propagated")

    image = Image.frombytes("L", (2, 1), bytes([91, 92]))

    with pytest.raises(TypeError, match=r"^argument must be a sequence$"):
        image.putdata(BrokenSequence())

    assert image.tobytes() == bytes([91, 92])


@pytest.mark.covers("Image.putdata")
def test_putdata_rgb_coercion_callback_observes_prior_pixel_write():
    image = Image.frombytes("RGB", (2, 1), bytes([9, 9, 9] * 2))
    observations = []

    class ObserveIndex:
        def __index__(self):
            observations.append(image.getpixel((0, 0)))
            return 4

    image.putdata([(1, 2, 3), (ObserveIndex(), 5, 6)])

    assert observations == [(1, 2, 3)]
    assert image.tobytes() == bytes([1, 2, 3, 4, 5, 6])


@pytest.mark.covers("Image.putdata")
def test_putdata_l_coercion_callback_observes_prior_pixel_write():
    image = Image.frombytes("L", (2, 1), bytes([9, 9]))
    observations = []

    class ObserveFloat:
        def __float__(self):
            observations.append(image.getpixel((0, 0)))
            return 4.0

    image.putdata([1, ObserveFloat()])

    assert observations == [1]
    assert image.tobytes() == bytes([1, 4])


@pytest.mark.covers("Image.putdata")
def test_putdata_bytes_subclass_uses_raw_bytes_not_overridden_iteration():
    class WeirdBytes(bytes):
        def __iter__(self):
            return iter([9, 9])

    image = Image.new("L", (2, 1))

    image.putdata(WeirdBytes([1, 2]), scale=2)

    assert image.tobytes() == bytes([2, 4])


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    ("declared_length", "scale", "offset", "expected"),
    [
        (4, 1, 0, bytes([1, 2, 3, 0])),
        (2, 1, 0, bytes([1, 2, 9, 9])),
        (4, 2, 1, bytes([3, 5, 7, 1])),
    ],
)
def test_putdata_bytes_subclass_declared_length_matches_pillow_safely(
    declared_length,
    scale,
    offset,
    expected,
):
    class DeclaredBytes(bytes):
        def __len__(self):
            return declared_length

    def run(image_type):
        image = image_type.new("L", (4, 1), 9)
        image.putdata(DeclaredBytes([1, 2, 3]), scale, offset)
        return image.tobytes()

    assert run(Image) == run(PILImage) == expected


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize("mode", ["RGB", "RGBA", "CMYK"])
def test_putdata_multiband_noninteger_one_tuple_has_exact_arity_error(mode):
    image = Image.new(mode, (1, 1))

    with pytest.raises(
        TypeError,
        match=r"^color must be int, or tuple of one, three or four elements$",
    ):
        image.putdata([(1.5,)])


@pytest.mark.covers("Image.putdata")
@pytest.mark.parametrize(
    "image_factory",
    [
        lambda: Image.new("LA", (1, 1)),
        pa_image,
    ],
)
def test_putdata_two_band_noninteger_one_tuple_has_exact_system_error(
    image_factory,
):
    image = image_factory()

    with pytest.raises(
        SystemError,
        match=r"^new style getargs format but argument is not a tuple$",
    ):
        image.putdata([(1.5,)])


@pytest.mark.covers("Image.putdata")
def test_putdata_requires_a_sequence_and_leaves_image_unchanged():
    image = Image.frombytes("L", (2, 1), bytes([91, 92]))

    with pytest.raises(TypeError, match=r"^argument must be a sequence$"):
        image.putdata(iter([1, 2]))

    assert image.tobytes() == bytes([91, 92])


@pytest.mark.covers("Image.putdata")
def test_putdata_accepts_public_imaging_core_sequence():
    source = Image.frombytes("L", (2, 1), bytes([1, 2]))
    destination = Image.frombytes("L", (2, 1), bytes([91, 92]))

    destination.putdata(source.getdata())

    assert destination.tobytes() == bytes([1, 2])
