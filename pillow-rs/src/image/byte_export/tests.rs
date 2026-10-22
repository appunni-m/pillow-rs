use super::super::{Image, PutDataValue, is_l16_mode, l16_uses_big_endian};
use crate::error::PilError;

const MODES: &[(&str, usize)] = &[
    ("1", 1),
    ("L", 1),
    ("P", 1),
    ("LA", 2),
    ("PA", 2),
    ("RGB", 3),
    ("LAB", 3),
    ("RGBA", 4),
    ("RGBa", 4),
    ("RGBX", 4),
    ("CMYK", 4),
    ("HSV", 3),
    ("YCbCr", 3),
    ("I", 4),
    ("F", 4),
    ("I;16", 2),
    ("I;16L", 2),
    ("I;16B", 2),
    ("I;16N", 2),
];

#[test]
fn native_default_exports_share_storage() -> Result<(), PilError> {
    for &(mode, channels) in MODES {
        if matches!(mode, "1" | "P")
            || (is_l16_mode(mode) && cfg!(target_endian = "big") != l16_uses_big_endian(mode))
        {
            continue;
        }
        for (width, height) in [(3, 2), (0, 3), (3, 0)] {
            let bytes: Vec<u8> = (0..channels * width as usize * height as usize)
                .map(|index| index.wrapping_mul(37).wrapping_add(11).to_le_bytes()[0])
                .collect();
            let image = Image::frombytes(mode, (width, height), &bytes)?;
            let storage = image.materialized_shared()?;
            let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
            assert_eq!(exported.as_ref(), bytes, "{mode} {width}x{height}");
            assert_eq!(
                exported.as_ref().as_ptr(),
                storage.as_bytes().as_ptr(),
                "native {mode} export should retain the materialized allocation"
            );
        }
    }
    Ok(())
}

#[test]
fn owned_frombytes_adopts_raw_raster_allocation() -> Result<(), PilError> {
    let bytes = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
    let input_ptr = bytes.as_ptr();
    let image = Image::frombytes_owned("RGB", (2, 1), bytes)?;
    let storage = image.materialized_shared()?;

    assert_eq!(storage.as_bytes().as_ptr(), input_ptr);
    assert_eq!(storage.as_bytes(), &[0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
    Ok(())
}

#[test]
fn shared_exports_preserve_encoder_layouts_and_errors() -> Result<(), PilError> {
    for &(source_mode, channels) in MODES {
        let bytes: Vec<u8> = (0..9 * 2 * channels)
            .map(|index| index.wrapping_mul(43).wrapping_add(17).to_le_bytes()[0])
            .collect();
        let image = Image::frombytes(source_mode, (9, 2), &bytes)?;
        for mode in [source_mode, "1", "RGB", "BGRA", "I;16L", "I;16B"] {
            for raw_mode in [
                None,
                Some(mode),
                Some("BGR"),
                Some("BGRA"),
                Some("RGBA"),
                Some("invalid"),
            ] {
                let args: Vec<String> = raw_mode.into_iter().map(str::to_owned).collect();
                for encoder in ["raw", "invalid"] {
                    let existing = image
                        .tobytes_encoded(mode, encoder, &args)
                        .map_err(|error| format!("{error:?}"));
                    let shared = image
                        .tobytes_encoded_shared(mode, encoder, &args)
                        .map(|bytes| bytes.as_ref().to_vec())
                        .map_err(|error| format!("{error:?}"));
                    assert_eq!(shared, existing, "{source_mode}: {mode}/{encoder}/{args:?}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn shared_scalar_exports_preserve_all_sample_bits() -> Result<(), PilError> {
    let words: [u32; 6] = [
        0x8000_0000,
        0x7fc0_1234,
        0x7f80_0001,
        0xff80_0000,
        0x8000_0001,
        0xffff_ffff,
    ];
    let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    for mode in ["I", "F"] {
        let image = Image::frombytes(mode, (3, 2), &bytes)?;
        let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
        assert_eq!(exported.as_ref(), bytes, "{mode}");
    }
    Ok(())
}

#[test]
fn exported_bytes_survive_image_mutation_and_drop() -> Result<(), PilError> {
    for mode in ["L", "P", "I;16N"] {
        let bytes = if mode == "I;16N" {
            vec![1, 2, 3, 4]
        } else {
            vec![1, 2]
        };
        let mut image = Image::frombytes(mode, (2, 1), &bytes)?;
        let exported = image.tobytes_encoded_shared(mode, "raw", &[])?;
        image.putdata_value_at(0, &PutDataValue::Number(77.0), 1.0, 0.0)?;
        assert_ne!(
            image.tobytes()?,
            bytes,
            "{mode} mutation must be observable"
        );
        drop(image);
        assert_eq!(
            exported.as_ref(),
            bytes,
            "{mode} export must stay immutable"
        );
    }
    Ok(())
}

#[cfg(test)]
mod mode1_byte_block_tests {
    use super::super::super::construction::unpack_mode1_rows;
    use super::super::pack_mode1_rows;

    #[test]
    fn every_packed_byte_expands_in_msb_order() {
        for value in 0..=255u8 {
            let mut pixels = [0; 8];
            unpack_mode1_rows(&[value], 8, &mut pixels);
            for (bit, pixel) in pixels.into_iter().enumerate() {
                assert_eq!(pixel, if value & (128 >> bit) != 0 { 255 } else { 0 });
            }
        }
    }

    #[test]
    fn every_nonzero_byte_packs_as_a_set_bit() {
        for position in 0..8 {
            for background in [0, 1, 127, 128, 255] {
                for value in 0..=255u8 {
                    let mut pixels = [background; 8];
                    pixels[position] = value;
                    let expected = pixels.iter().enumerate().fold(0, |byte, (bit, &pixel)| {
                        byte | (u8::from(pixel != 0) << (7 - bit))
                    });
                    assert_eq!(pack_mode1_rows(&pixels, 8, 1), [expected]);
                }
            }
        }
    }

    #[test]
    fn rows_keep_padding_separate_at_each_byte_boundary() {
        for width in 0usize..=33 {
            for height in 0..=3 {
                let row_bytes = width.div_ceil(8);
                let mut expected: Vec<u8> = (0..row_bytes * height)
                    .map(|index| (index * 73 + 29) as u8)
                    .collect();
                let mut pixels = vec![0; width * height];
                unpack_mode1_rows(&expected, width, &mut pixels);
                if !width.is_multiple_of(8) {
                    for row in expected.chunks_exact_mut(row_bytes) {
                        row[row_bytes - 1] &= 255 << (8 - width % 8);
                    }
                }
                assert_eq!(pack_mode1_rows(&pixels, width, height), expected);
            }
        }
    }
}
