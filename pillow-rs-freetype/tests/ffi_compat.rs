#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::fs;
use std::mem::{align_of, size_of};
use std::os::raw::{c_long, c_ushort};
use std::path::Path;

use fontdone::ffi::*;
use fontdone::font::GlyphSlotMetrics;
use fontdone::{BBox, GlyphFormat, PixelMode, SizeMetrics, Vector};

fn fixture_font(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("fonts")
        .join(name);
    fs::read(path).expect("read font fixture")
}

#[test]
fn ffi_constants_match_pinned_freetype_headers() {
    assert_eq!(
        (
            FT_Err_Ok,
            FT_Err_Invalid_File_Format,
            FT_Err_Unimplemented_Feature,
            FT_Err_Invalid_Glyph_Index,
            FT_Err_Raster_Overflow,
            FT_Err_Invalid_CharMap_Format,
        ),
        (0x00, 0x03, 0x07, 0x10, 0x62, 0x96)
    );
    assert_eq!(
        (
            FT_LOAD_DEFAULT,
            FT_LOAD_NO_SCALE,
            FT_LOAD_NO_HINTING,
            FT_LOAD_RENDER,
            FT_LOAD_FORCE_AUTOHINT,
            FT_LOAD_MONOCHROME,
            FT_LOAD_NO_AUTOHINT,
            FT_LOAD_TARGET_LCD,
        ),
        (0, 1, 2, 4, 32, 4096, 32768, 3 << 16)
    );
    assert_eq!(
        (
            FT_RENDER_MODE_NORMAL,
            FT_RENDER_MODE_LIGHT,
            FT_RENDER_MODE_MONO,
            FT_RENDER_MODE_LCD,
            FT_RENDER_MODE_LCD_V,
            FT_RENDER_MODE_SDF,
            FT_RENDER_MODE_MAX,
        ),
        (0, 1, 2, 3, 4, 5, 6)
    );
    assert_eq!(
        (
            FT_PIXEL_MODE_NONE,
            FT_PIXEL_MODE_MONO,
            FT_PIXEL_MODE_GRAY,
            FT_PIXEL_MODE_GRAY2,
            FT_PIXEL_MODE_GRAY4,
            FT_PIXEL_MODE_LCD,
            FT_PIXEL_MODE_LCD_V,
            FT_PIXEL_MODE_BGRA,
            FT_PIXEL_MODE_MAX,
        ),
        (0, 1, 2, 3, 4, 5, 6, 7, 8)
    );
    assert_eq!(
        (
            FT_GLYPH_FORMAT_COMPOSITE,
            FT_GLYPH_FORMAT_BITMAP,
            FT_GLYPH_FORMAT_OUTLINE,
            FT_GLYPH_FORMAT_PLOTTER,
            FT_GLYPH_FORMAT_SVG,
        ),
        (
            0x636f_6d70,
            0x6269_7473,
            0x6f75_746c,
            0x706c_6f74,
            0x5356_4720
        )
    );
}

#[test]
fn ffi_leaf_records_use_c_long_coordinate_layout() {
    #[repr(C)]
    struct ExpectedSizeMetrics {
        x_ppem: c_ushort,
        y_ppem: c_ushort,
        x_scale: c_long,
        y_scale: c_long,
        ascender: c_long,
        descender: c_long,
        height: c_long,
        max_advance: c_long,
    }

    assert_eq!(size_of::<FT_Vector>(), size_of::<c_long>() * 2);
    assert_eq!(align_of::<FT_Vector>(), align_of::<c_long>());
    assert_eq!(size_of::<FT_BBox>(), size_of::<c_long>() * 4);
    assert_eq!(align_of::<FT_BBox>(), align_of::<c_long>());
    assert_eq!(size_of::<FT_Glyph_Metrics>(), size_of::<[c_long; 8]>());
    assert_eq!(align_of::<FT_Glyph_Metrics>(), align_of::<c_long>());
    assert_eq!(
        size_of::<FT_Size_Metrics>(),
        size_of::<ExpectedSizeMetrics>()
    );
    assert_eq!(align_of::<FT_Size_Metrics>(), align_of::<c_long>());
}

#[test]
fn ffi_conversions_preserve_public_field_values() {
    let vector: FT_Vector = Vector { x: -64, y: 128 }.into();
    let bbox: FT_BBox = BBox {
        x_min: -1,
        y_min: -2,
        x_max: 30,
        y_max: 40,
    }
    .into();
    let metrics: FT_Glyph_Metrics = GlyphSlotMetrics {
        width: 1,
        height: 2,
        hori_bearing_x: 3,
        hori_bearing_y: 4,
        hori_advance: 5,
        vert_bearing_x: 6,
        vert_bearing_y: 7,
        vert_advance: 8,
    }
    .into();
    let size: FT_Size_Metrics = SizeMetrics {
        x_ppem: 9,
        y_ppem: 10,
        x_scale: 11,
        y_scale: 12,
        ascender: 13,
        descender: -14,
        height: 15,
        max_advance: 16,
        x_dpi: 72,
        y_dpi: 72,
        char_width: 0,
        char_height: 12 * 64,
    }
    .into();

    assert_eq!((vector.x, vector.y), (-64, 128));
    assert_eq!(
        (bbox.xMin, bbox.yMin, bbox.xMax, bbox.yMax),
        (-1, -2, 30, 40)
    );
    assert_eq!(
        (
            metrics.width,
            metrics.height,
            metrics.horiBearingX,
            metrics.horiBearingY,
            metrics.horiAdvance,
            metrics.vertBearingX,
            metrics.vertBearingY,
            metrics.vertAdvance,
        ),
        (1, 2, 3, 4, 5, 6, 7, 8)
    );
    assert_eq!(
        (
            size.x_ppem,
            size.y_ppem,
            size.x_scale,
            size.y_scale,
            size.ascender,
            size.descender,
            size.height,
            size.max_advance,
        ),
        (9, 10, 11, 12, 13, -14, 15, 16)
    );
}

#[test]
fn ffi_mapping_translates_c_load_flags_to_core_surface() {
    assert_eq!(
        FT_LOAD_TARGET_MODE(FT_LOAD_RENDER | FT_LOAD_TARGET_LCD_V),
        FT_RENDER_MODE_LCD_V
    );
    assert!(load_flags_to_core(FT_LOAD_RENDER | FT_LOAD_TARGET_LCD).is_ok());
    assert!(load_flags_to_core(FT_LOAD_RENDER | FT_LOAD_MONOCHROME).is_ok());
    assert_eq!(
        load_flags_to_core(FT_LOAD_RENDER | FT_LOAD_NO_SCALE),
        Err(FT_Err_Unimplemented_Feature)
    );
    assert_eq!(pixel_mode_from_core(PixelMode::Gray), FT_PIXEL_MODE_GRAY);
    assert_eq!(
        glyph_format_from_core(GlyphFormat::Outline),
        FT_GLYPH_FORMAT_OUTLINE
    );
}

#[test]
fn ffi_facade_loads_and_renders_glyph_from_memory() {
    let data = fixture_font("DejaVuSans.ttf");
    let library = FT_Init_FreeType();
    let mut face = FT_New_Memory_Face(&library, &data, 0, 20.0).unwrap();

    assert_eq!(FT_Set_Pixel_Sizes(&mut face, 0, 20), FT_Err_Ok);
    assert_eq!(FT_Get_Char_Index(&face, 'A' as FT_ULong), 36);

    let outline_slot = FT_Load_Char(&face, 'A' as FT_ULong, FT_LOAD_DEFAULT).unwrap();
    assert_eq!(outline_slot.glyph_index, 36);
    assert_eq!(outline_slot.format, FT_GLYPH_FORMAT_OUTLINE);
    assert_eq!(outline_slot.advance.x, outline_slot.metrics.horiAdvance);

    let rendered = FT_Render_Glyph(outline_slot, FT_RENDER_MODE_MONO).unwrap();
    let bitmap = rendered.bitmap.expect("rendered bitmap");

    assert_eq!(rendered.format, FT_GLYPH_FORMAT_BITMAP);
    assert_eq!(bitmap.pixel_mode, FT_PIXEL_MODE_MONO);
    assert_eq!(bitmap.num_grays, 2);
    let pitch = usize::try_from(bitmap.pitch).expect("positive pitch");
    assert_eq!(bitmap.rows as usize * pitch, bitmap.buffer.len());
    assert!(bitmap.buffer.iter().any(|pixel| *pixel != 0));
    assert_eq!(rendered.bitmap_left, 0);
    assert!(rendered.bitmap_top > 0);
}

#[test]
fn ffi_facade_maps_unsupported_flags_to_freetype_error_code() {
    let data = fixture_font("DejaVuSans.ttf");
    let library = FT_Init_FreeType();
    let face = FT_New_Memory_Face(&library, &data, 0, 20.0).unwrap();

    let err = FT_Load_Char(&face, 'A' as FT_ULong, FT_LOAD_RENDER | FT_LOAD_NO_SCALE)
        .err()
        .expect("unsupported flags should fail");
    assert_eq!(err, FT_Err_Unimplemented_Feature);
}
