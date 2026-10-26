use super::{
    DEFAULT_BITMAP, DEFAULT_BITMAP_HEIGHT, DEFAULT_BITMAP_LUMA, DEFAULT_BITMAP_WIDTH, PilFont,
    PilFontGlyphImage, PilFontMask, PilFontMode, PilFontTextInput,
};

#[test]
fn default_bitmap_matches_embedded_png_decode() -> Result<(), crate::error::PilError> {
    let glyph_image = PilFont::open_pilfont_glyph_image(DEFAULT_BITMAP.to_vec())?;
    let image = match glyph_image {
        PilFontGlyphImage::Image(image) => image,
        PilFontGlyphImage::DeferredRenderError { .. } => {
            panic!("the embedded default bitmap must decode successfully")
        }
    };
    let decoded = image.materialize()?.to_luma8();

    assert_eq!(
        decoded.dimensions(),
        (DEFAULT_BITMAP_WIDTH, DEFAULT_BITMAP_HEIGHT)
    );
    assert_eq!(decoded.as_raw().as_slice(), DEFAULT_BITMAP_LUMA);
    assert!(
        PilFont::load_default_if_sources_match(super::DEFAULT_METRICS, DEFAULT_BITMAP).is_some()
    );
    assert!(
        PilFont::load_default_if_sources_match(super::DEFAULT_METRICS, &DEFAULT_BITMAP[1..])
            .is_none()
    );
    Ok(())
}

#[test]
fn owned_mask_image_matches_regular_conversion() -> Result<(), crate::error::PilError> {
    for (mode, pixels) in [
        (PilFontMode::One, vec![0, 1, 0, 255, 128, 0, 255, 1]),
        (PilFontMode::Luma, vec![0, 1, 16, 127, 128, 255, 64, 2]),
    ] {
        let mask = PilFontMask {
            width: 4,
            height: 2,
            mode,
            pixels,
        };
        let expected = mask.to_image()?;
        let actual = mask.into_image()?;

        assert_eq!(actual.mode()?, expected.mode()?);
        assert_eq!(actual.size()?, expected.size()?);
        assert_eq!(actual.tobytes()?, expected.tobytes()?);
    }
    Ok(())
}

#[test]
fn borrowed_string_getlength_matches_latin1_encoding() -> Result<(), crate::error::PilError> {
    let font = PilFont::load_default()?;
    for text in ["Hello", "caf\u{e9}", "A\0B"] {
        let bytes = PilFontTextInput::Text(text.to_owned()).into_bytes()?;
        assert_eq!(font.getlength_str(text)?, font.getlength(&bytes)?);
    }

    let error = font
        .getlength_str("A\0\u{100}")
        .expect_err("text after NUL still has to be encodable as Latin-1");
    assert!(matches!(
        error,
        crate::error::PilError::UnicodeEncodeError {
            start: 2,
            end: 3,
            ..
        }
    ));

    let too_long = "A".repeat(super::MAX_STRING_LENGTH + 1);
    assert!(matches!(
        font.getlength_str(&too_long),
        Err(crate::error::PilError::ValueError(message))
            if message == "too many characters in string"
    ));

    let mut too_long_with_invalid_text = too_long;
    too_long_with_invalid_text.push('\u{100}');
    assert!(matches!(
        font.getlength_str(&too_long_with_invalid_text),
        Err(crate::error::PilError::UnicodeEncodeError {
            start,
            end,
            ..
        }) if start == super::MAX_STRING_LENGTH + 1 && end == super::MAX_STRING_LENGTH + 2
    ));
    Ok(())
}
