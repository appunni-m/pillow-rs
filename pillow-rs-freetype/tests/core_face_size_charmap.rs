#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::Path;

use pillow_rs_freetype::Font;

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
fn face_open_exposes_count_and_index_semantics() {
    let data = fixture_font("DejaVuSans.ttf");

    let font = Font::truetype_face(&data, 0, 12.0).unwrap();

    assert_eq!(
        (
            Font::face_count(&data).unwrap(),
            font.num_faces(),
            font.face_index()
        ),
        (1, 1, 0)
    );
}

#[test]
fn face_open_rejects_out_of_range_index() {
    let data = fixture_font("DejaVuSans.ttf");

    let err = match Font::truetype_face(&data, 1, 12.0) {
        Ok(_) => panic!("out-of-range face index succeeded"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("face index 1 out of range"));
}

#[test]
fn face_metadata_exposes_names_flags_metrics_and_format() {
    let data = fixture_font("DejaVuSans.ttf");
    let font = Font::truetype(&data, 12.0).unwrap();

    let info = font.face_info();

    assert_eq!(
        (
            info.family_name.as_str(),
            info.style_name.as_str(),
            info.postscript_name.as_deref(),
            info.font_format,
            info.units_per_em,
            info.num_glyphs,
            info.ascender,
            info.descender,
        ),
        (
            "DejaVu Sans",
            "Book",
            Some("DejaVuSans"),
            "TrueType",
            2048,
            6253,
            1901,
            -483,
        )
    );
}

#[test]
fn size_select_distinguishes_char_size_pixel_size_dpi_ppem_and_scale() {
    let data = fixture_font("DejaVuSans.ttf");
    let mut font = Font::truetype(&data, 12.0).unwrap();

    font.set_char_size(0, 10 * 64, 0, 144);
    let char_size = font.size_metrics();
    font.set_pixel_sizes(0, 18);
    let pixel_size = font.size_metrics();

    assert_eq!(
        (
            char_size.x_dpi,
            char_size.y_dpi,
            char_size.y_ppem,
            char_size.y_scale,
            pixel_size.y_dpi,
            pixel_size.y_ppem,
            pixel_size.y_scale,
        ),
        (72, 144, 20, 40960, 72, 18, 36864)
    );
}

#[test]
fn charmap_select_set_get_and_iteration_match_expected_unicode_mapping() {
    let data = fixture_font("DejaVuSans.ttf");
    let mut font = Font::truetype(&data, 12.0).unwrap();
    let charmaps = font.charmaps();
    let unicode_index = charmaps
        .iter()
        .find(|charmap| charmap.platform_id == 3 && charmap.encoding_id == 10)
        .or_else(|| {
            charmaps
                .iter()
                .find(|charmap| charmap.platform_id == 3 && charmap.encoding_id == 1)
        })
        .expect("unicode charmap")
        .index;

    font.set_charmap(unicode_index).unwrap();
    let current = font.charmap().unwrap();
    let first = font.first_char().unwrap();
    let next = font.next_char('A' as u32).unwrap();

    assert_eq!(
        (
            current.index,
            font.charmap_index().unwrap(),
            font.char_index('A' as u32),
            font.char_index(0x10FFFF),
            first,
            next.0,
        ),
        (unicode_index, unicode_index, 36, 0, (32, 3), 'B' as u32)
    );
}

#[test]
fn charmap_select_by_platform_encoding_updates_current_charmap() {
    let data = fixture_font("DejaVuSans.ttf");
    let mut font = Font::truetype(&data, 12.0).unwrap();

    font.select_charmap(3, 1).unwrap();

    let current = font.charmap().unwrap();
    assert_eq!((current.platform_id, current.encoding_id), (3, 1));
}

#[test]
fn sfnt_table_access_matches_raw_table_bytes() {
    let data = fixture_font("DejaVuSans.ttf");
    let font = Font::truetype(&data, 12.0).unwrap();
    let cmap_info = font
        .sfnt_tables()
        .into_iter()
        .find(|info| info.tag == pillow_rs_freetype::tt::tag(b"cmap"))
        .expect("cmap table");

    let raw = font.load_sfnt_table(cmap_info.tag, 0, None).unwrap();
    let partial = font.load_sfnt_table(cmap_info.tag, 2, Some(2)).unwrap();
    let expected =
        &data[cmap_info.offset as usize..cmap_info.offset as usize + cmap_info.length as usize];

    assert_eq!(
        (raw.as_slice(), partial.as_slice()),
        (expected, &expected[2..4])
    );
}
