//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! This module mirrors Pillow's `_imagingft.c` adapter layer: public
//! `FreeTypeFont` argument handling, calls into the FreeType-shaped lower API,
//! Pillow-visible result shaping, and Pillow exception mapping.
//!
//! FreeType-original behavior stays below this layer in `fontdone` /
//! `pillow-rs-freetype` (font tables, glyph loading, SBIT, hinting,
//! rasterization, stroker geometry, FreeType ownership, and FreeType error-code
//! classification).  Do not add lower FreeType shortcuts here to make Pillow
//! ImageFont rows pass; fix the real lower implementation instead.

use super::{
    FreeTypeFont, ImageFontLoadOptions, ImageFontTextOptions, ImageFontVariantOptions,
    ImageFontVariationAxis,
};
use crate::error::PilError;
use crate::image::Image;
use fontdone::{ffi, tt};

const MAX_STRING_LENGTH: usize = 1_000_000;

pub(super) struct TrueTypeEngine {
    library: ffi::FT_Library,
    face: ffi::FT_Face,
    font_bytes: Vec<u8>,
    face_index: usize,
    pub(super) size_pt: f32,
    encoding: Option<String>,
    layout_engine: Option<String>,
    family_name: Option<String>,
    style_name: Option<String>,
    metrics: ffi::FT_Size_Metrics,
}

pub(super) fn load_truetype(data: Vec<u8>, size: f32) -> Result<FreeTypeFont, PilError> {
    load_truetype_with_index(data, size, 0, None, None)
}

pub(super) fn load_truetype_with_options(
    data: Vec<u8>,
    size: f32,
    options: &ImageFontLoadOptions,
) -> Result<FreeTypeFont, PilError> {
    load_truetype_with_index(
        data,
        size,
        options.index.unwrap_or(0),
        options.encoding.clone(),
        options.layout_engine.clone(),
    )
}

fn load_truetype_with_index(
    data: Vec<u8>,
    size: f32,
    face_index: usize,
    encoding: Option<String>,
    layout_engine: Option<String>,
) -> Result<FreeTypeFont, PilError> {
    if !(size > 0.0) {
        return Err(PilError::ValueError(format!(
            "font size must be greater than 0, not {}",
            if size.fract() == 0.0 {
                format!("{:.0}", size)
            } else {
                size.to_string()
            }
        )));
    }

    let library = ffi::FT_Init_FreeType();
    let face_index_ffi = ffi::FT_Long::try_from(face_index)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let mut face =
        ffi::FT_New_Memory_Face(&library, &data, face_index_ffi, size).map_err(ft_error_to_pil)?;

    // Pillow's `getfont` selects a requested FreeType charmap immediately
    // after opening the face.  Keep the tag translation in the Rust core so
    // bindings only preserve the caller's string; unknown tags retain
    // FreeType's default Unicode selection, matching Pillow's fallback.
    select_charmap(&mut face, encoding.as_deref())?;

    // Pillow _imagingft.c:getfont requests nominal size with width/height
    // set to size * 64 after FT_New_Memory_Face.
    let width = (size * 64.0) as ffi::FT_Long;
    let request = ffi::FT_Size_RequestRec {
        type_: ffi::FT_SIZE_REQUEST_TYPE_NOMINAL as ffi::FT_Size_Request_Type,
        width,
        height: width,
        horiResolution: 0,
        vertResolution: 0,
    };
    check_ft_error(ffi::FT_Request_Size(Some(&mut face), Some(&request)))?;

    let family_name = face.family_name.clone();
    let style_name = face.style_name.clone();
    let metrics = face.size_metrics;

    let engine = TrueTypeEngine {
        library,
        face,
        font_bytes: data,
        face_index,
        size_pt: size,
        encoding,
        layout_engine,
        family_name,
        style_name,
        metrics,
    };
    Ok(FreeTypeFont { engine })
}

fn select_charmap(face: &mut ffi::FT_Face, encoding: Option<&str>) -> Result<(), PilError> {
    let Some(encoding) = encoding else {
        return Ok(());
    };
    let Some(encoding) = freetype_encoding(encoding) else {
        return Ok(());
    };
    check_ft_error(ffi::FT_Select_Charmap(Some(face), encoding))
}

fn freetype_encoding(encoding: &str) -> Option<ffi::FT_Encoding> {
    let encoding = match encoding {
        "unic" => ffi::FT_ENCODING_UNICODE,
        "symb" => ffi::FT_ENCODING_MS_SYMBOL,
        "ADOB" => ffi::FT_ENCODING_ADOBE_STANDARD,
        "ADBE" => ffi::FT_ENCODING_ADOBE_EXPERT,
        "ADBC" => ffi::FT_ENCODING_ADOBE_CUSTOM,
        "armn" => ffi::FT_ENCODING_APPLE_ROMAN,
        "sjis" => ffi::FT_ENCODING_SJIS,
        "gb  " => ffi::FT_ENCODING_PRC,
        "big5" => ffi::FT_ENCODING_BIG5,
        "wans" => ffi::FT_ENCODING_WANSUNG,
        "joha" => ffi::FT_ENCODING_JOHAB,
        "lat1" => ffi::FT_ENCODING_ADOBE_LATIN_1,
        "lat2" => ffi::FT_ENCODING_OLD_LATIN_2,
        _ => return None,
    };
    Some(encoding as ffi::FT_Encoding)
}

// Pillow 12.2.0 `_imagingft.c::geterror` includes FreeType's `fterrdef.h`
// through `FT_ERRORS_H`, raises `OSError` for every listed code, and uses
// `unknown freetype error` for table misses.
#[rustfmt::skip]
const FT_ERROR_MESSAGES: &[(i32, &str)] = &[
    (
        ffi::FT_Err_Cannot_Open_Resource,
        "cannot open resource",
    ),
    (
        ffi::FT_Err_Unknown_File_Format,
        "unknown file format",
    ),
    (ffi::FT_Err_Invalid_File_Format, "broken file"),
    (
        ffi::FT_Err_Invalid_Version as i32,
        "invalid FreeType version",
    ),
    (
        ffi::FT_Err_Lower_Module_Version as i32,
        "module version is too low",
    ),
    (ffi::FT_Err_Invalid_Argument, "invalid argument"),
    (
        ffi::FT_Err_Unimplemented_Feature,
        "unimplemented feature",
    ),
    (ffi::FT_Err_Invalid_Table, "broken table"),
    (
        ffi::FT_Err_Invalid_Offset as i32,
        "broken offset within table",
    ),
    (
        ffi::FT_Err_Array_Too_Large as i32,
        "array allocation size too large",
    ),
    (ffi::FT_Err_Missing_Module as i32, "missing module"),
    (ffi::FT_Err_Missing_Property as i32, "missing property"),
    (
        ffi::FT_Err_Invalid_Glyph_Index,
        "invalid glyph index",
    ),
    (
        ffi::FT_Err_Invalid_Character_Code,
        "invalid character code",
    ),
    (
        ffi::FT_Err_Invalid_Glyph_Format,
        "unsupported glyph image format",
    ),
    (
        ffi::FT_Err_Cannot_Render_Glyph,
        "cannot render this glyph format",
    ),
    (ffi::FT_Err_Invalid_Outline, "invalid outline"),
    (
        ffi::FT_Err_Invalid_Composite as i32,
        "invalid composite glyph",
    ),
    (ffi::FT_Err_Too_Many_Hints as i32, "too many hints"),
    (ffi::FT_Err_Invalid_Pixel_Size, "invalid pixel size"),
    (
        ffi::FT_Err_Invalid_SVG_Document as i32,
        "invalid SVG document",
    ),
    (ffi::FT_Err_Invalid_Handle as i32, "invalid object handle"),
    (
        ffi::FT_Err_Invalid_Library_Handle as i32,
        "invalid library handle",
    ),
    (
        ffi::FT_Err_Invalid_Driver_Handle as i32,
        "invalid module handle",
    ),
    (
        ffi::FT_Err_Invalid_Face_Handle as i32,
        "invalid face handle",
    ),
    (
        ffi::FT_Err_Invalid_Size_Handle,
        "invalid size handle",
    ),
    (
        ffi::FT_Err_Invalid_Slot_Handle as i32,
        "invalid glyph slot handle",
    ),
    (
        ffi::FT_Err_Invalid_CharMap_Handle,
        "invalid charmap handle",
    ),
    (
        ffi::FT_Err_Invalid_Cache_Handle as i32,
        "invalid cache manager handle",
    ),
    (
        ffi::FT_Err_Invalid_Stream_Handle as i32,
        "invalid stream handle",
    ),
    (ffi::FT_Err_Too_Many_Drivers as i32, "too many modules"),
    (
        ffi::FT_Err_Too_Many_Extensions as i32,
        "too many extensions",
    ),
    (ffi::FT_Err_Out_Of_Memory, "out of memory"),
    (ffi::FT_Err_Unlisted_Object as i32, "unlisted object"),
    (ffi::FT_Err_Cannot_Open_Stream as i32, "cannot open stream"),
    (
        ffi::FT_Err_Invalid_Stream_Seek as i32,
        "invalid stream seek",
    ),
    (
        ffi::FT_Err_Invalid_Stream_Skip as i32,
        "invalid stream skip",
    ),
    (
        ffi::FT_Err_Invalid_Stream_Read as i32,
        "invalid stream read",
    ),
    (
        ffi::FT_Err_Invalid_Stream_Operation as i32,
        "invalid stream operation",
    ),
    (
        ffi::FT_Err_Invalid_Frame_Operation as i32,
        "invalid frame operation",
    ),
    (
        ffi::FT_Err_Nested_Frame_Access as i32,
        "nested frame access",
    ),
    (ffi::FT_Err_Invalid_Frame_Read as i32, "invalid frame read"),
    (
        ffi::FT_Err_Raster_Uninitialized as i32,
        "raster uninitialized",
    ),
    (ffi::FT_Err_Raster_Corrupted as i32, "raster corrupted"),
    (ffi::FT_Err_Raster_Overflow, "raster overflow"),
    (
        ffi::FT_Err_Raster_Negative_Height as i32,
        "negative height while rastering",
    ),
    (
        ffi::FT_Err_Too_Many_Caches as i32,
        "too many registered caches",
    ),
    (ffi::FT_Err_Invalid_Opcode as i32, "invalid opcode"),
    (ffi::FT_Err_Too_Few_Arguments as i32, "too few arguments"),
    (ffi::FT_Err_Stack_Overflow as i32, "stack overflow"),
    (ffi::FT_Err_Code_Overflow as i32, "code overflow"),
    (ffi::FT_Err_Bad_Argument as i32, "bad argument"),
    (ffi::FT_Err_Divide_By_Zero as i32, "division by zero"),
    (ffi::FT_Err_Invalid_Reference as i32, "invalid reference"),
    (ffi::FT_Err_Debug_OpCode as i32, "found debug opcode"),
    (
        ffi::FT_Err_ENDF_In_Exec_Stream as i32,
        "found ENDF opcode in execution stream",
    ),
    (ffi::FT_Err_Nested_DEFS as i32, "nested DEFS"),
    (ffi::FT_Err_Invalid_CodeRange as i32, "invalid code range"),
    (
        ffi::FT_Err_Execution_Too_Long as i32,
        "execution context too long",
    ),
    (ffi::FT_Err_Too_Many_Function_Defs as i32, "too many function definitions"),
    (ffi::FT_Err_Too_Many_Instruction_Defs as i32, "too many instruction definitions"),
    (ffi::FT_Err_Table_Missing as i32, "SFNT font table missing"),
    (
        ffi::FT_Err_Horiz_Header_Missing as i32,
        "horizontal header (hhea) table missing",
    ),
    (
        ffi::FT_Err_Locations_Missing as i32,
        "locations (loca) table missing",
    ),
    (ffi::FT_Err_Name_Table_Missing as i32, "name table missing"),
    (ffi::FT_Err_CMap_Table_Missing as i32, "character map (cmap) table missing"),
    (ffi::FT_Err_Hmtx_Table_Missing as i32, "horizontal metrics (hmtx) table missing"),
    (
        ffi::FT_Err_Post_Table_Missing as i32,
        "PostScript (post) table missing",
    ),
    (
        ffi::FT_Err_Invalid_Horiz_Metrics as i32,
        "invalid horizontal metrics",
    ),
    (
        ffi::FT_Err_Invalid_CharMap_Format,
        "invalid character map (cmap) format",
    ),
    (ffi::FT_Err_Invalid_PPem as i32, "invalid ppem value"),
    (
        ffi::FT_Err_Invalid_Vert_Metrics as i32,
        "invalid vertical metrics",
    ),
    (
        ffi::FT_Err_Could_Not_Find_Context as i32,
        "could not find context",
    ),
    (ffi::FT_Err_Invalid_Post_Table_Format as i32, "invalid PostScript (post) table format"),
    (
        ffi::FT_Err_Invalid_Post_Table as i32,
        "invalid PostScript (post) table",
    ),
    (
        ffi::FT_Err_DEF_In_Glyf_Bytecode as i32,
        "found FDEF or IDEF opcode in glyf bytecode",
    ),
    (
        ffi::FT_Err_Missing_Bitmap as i32,
        "missing bitmap in strike",
    ),
    (
        ffi::FT_Err_Missing_SVG_Hooks as i32,
        "SVG hooks have not been set",
    ),
    (ffi::FT_Err_Syntax_Error as i32, "opcode syntax error"),
    (
        ffi::FT_Err_Stack_Underflow as i32,
        "argument stack underflow",
    ),
    (ffi::FT_Err_Ignore as i32, "ignore"),
    (
        ffi::FT_Err_No_Unicode_Glyph_Name as i32,
        "no Unicode glyph name found",
    ),
    (
        ffi::FT_Err_Glyph_Too_Big as i32,
        "glyph too big for hinting",
    ),
    (
        ffi::FT_Err_Missing_Startfont_Field as i32,
        "`STARTFONT' field missing",
    ),
    (
        ffi::FT_Err_Missing_Font_Field as i32,
        "`FONT' field missing",
    ),
    (
        ffi::FT_Err_Missing_Size_Field as i32,
        "`SIZE' field missing",
    ),
    (ffi::FT_Err_Missing_Fontboundingbox_Field as i32, "`FONTBOUNDINGBOX' field missing"),
    (
        ffi::FT_Err_Missing_Chars_Field as i32,
        "`CHARS' field missing",
    ),
    (
        ffi::FT_Err_Missing_Startchar_Field as i32,
        "`STARTCHAR' field missing",
    ),
    (
        ffi::FT_Err_Missing_Encoding_Field as i32,
        "`ENCODING' field missing",
    ),
    (ffi::FT_Err_Missing_Bbx_Field as i32, "`BBX' field missing"),
    (ffi::FT_Err_Bbx_Too_Big as i32, "`BBX' too big"),
    (ffi::FT_Err_Corrupted_Font_Header as i32, "Font header corrupted or missing fields"),
    (ffi::FT_Err_Corrupted_Font_Glyphs as i32, "Font glyphs corrupted or missing fields"),
];

fn ft_error_to_pil(error: i32) -> PilError {
    for (code, message) in FT_ERROR_MESSAGES {
        if error == *code {
            return PilError::OsError((*message).into());
        }
    }
    PilError::OsError("unknown freetype error".into())
}

fn check_ft_error(error: i32) -> Result<(), PilError> {
    if error == ffi::FT_Err_Ok {
        Ok(())
    } else {
        Err(ft_error_to_pil(error))
    }
}

// ── Public API ───────────────────────────────────────────────────────

pub(crate) fn getname_optional(font: &FreeTypeFont) -> (Option<&str>, Option<&str>) {
    (
        font.engine.family_name.as_deref(),
        font.engine.style_name.as_deref(),
    )
}

/// Normalize a wrapped font bounding box using Pillow's `TransposedFont` rules.
///
/// Pillow moves the top-left to the origin and swaps the extents only for
/// `ROTATE_90` and `ROTATE_270`. Other transpose methods retain the extents.
#[must_use]
pub(crate) fn transposed_bbox(
    (left, top, right, bottom): (i32, i32, i32, i32),
    orientation: Option<&str>,
) -> (i32, i32, i32, i32) {
    let width = right - left;
    let height = bottom - top;
    if transposed_swaps_axes(orientation) {
        (0, 0, height, width)
    } else {
        (0, 0, width, height)
    }
}

/// Validate whether Pillow defines text length for a transposed font.
///
/// # Errors
///
/// Returns Pillow's exact [`PilError::ValueError`] for 90° and 270° rotation.
pub(crate) fn validate_transposed_length(orientation: Option<&str>) -> Result<(), PilError> {
    if transposed_swaps_axes(orientation) {
        return Err(PilError::ValueError(
            "text length is undefined for text rotated by 90 or 270 degrees".into(),
        ));
    }
    Ok(())
}

fn transposed_swaps_axes(orientation: Option<&str>) -> bool {
    matches!(orientation, Some("ROTATE_90" | "ROTATE_270"))
}

/// Render a font mask and apply Pillow's optional transpose operation.
///
/// # Errors
///
/// Returns [`PilError`] when the requested transpose is invalid or the mask
/// pipeline cannot be materialized.
pub(crate) fn get_transposed_mask(
    font: &FreeTypeFont,
    text: &str,
    orientation: Option<&str>,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height, pixels) = getmask(font, text)?;
    let image = Image::from_luma_mask(width, height, pixels)?;
    let transformed = if let Some(method) = orientation {
        image.transpose(method)?
    } else {
        image
    };
    let (width, height) = transformed.size()?;
    Ok((width, height, transformed.tobytes_unpacked()?))
}

pub(crate) fn getmetrics(font: &FreeTypeFont) -> (u32, u32) {
    (
        pixel(font.engine.metrics.ascender) as u32,
        (-pixel(font.engine.metrics.descender)) as u32,
    )
}

/// Return whether the loaded face exposes OpenType or Type 1 variation axes.
pub(crate) fn has_variations(font: &FreeTypeFont) -> bool {
    font.engine.face.face_flags & ffi::FT_FACE_FLAG_MULTIPLE_MASTERS != 0
}

pub(crate) fn font_variant(
    font: &FreeTypeFont,
    size: Option<f32>,
) -> Result<FreeTypeFont, PilError> {
    font_variant_with_options(
        font,
        &ImageFontVariantOptions {
            size,
            ..ImageFontVariantOptions::default()
        },
    )
}

pub(crate) fn font_variant_with_options(
    font: &FreeTypeFont,
    options: &ImageFontVariantOptions,
) -> Result<FreeTypeFont, PilError> {
    let load_options = ImageFontLoadOptions {
        index: Some(options.index.unwrap_or(font.engine.face_index)),
        encoding: options
            .encoding
            .clone()
            .or_else(|| font.engine.encoding.clone()),
        layout_engine: options
            .layout_engine
            .clone()
            .or_else(|| font.engine.layout_engine.clone()),
    };
    load_truetype_with_options(
        options
            .font_bytes
            .clone()
            .unwrap_or_else(|| font.engine.font_bytes.clone()),
        options.size.unwrap_or(font.engine.size_pt),
        &load_options,
    )
}

pub(crate) fn get_variation_axes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .axes
        .iter()
        .map(|axis| ImageFontVariationAxis {
            minimum: fixed_16_16_to_pillow_int(axis.min_value),
            default: fixed_16_16_to_pillow_int(axis.default_value),
            maximum: fixed_16_16_to_pillow_int(axis.max_value),
            name: name_bytes(&name_table, axis.name_id)
                .unwrap_or_default()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect(),
        })
        .collect())
}

pub(crate) fn get_variation_names(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .instances
        .iter()
        .map(|instance| {
            name_bytes(&name_table, instance.subfamily_name_id)
                .unwrap_or_default()
                .into_iter()
                .filter(|byte| *byte != 0)
                .collect()
        })
        .collect())
}

pub(crate) fn set_variation_by_name(font: &mut FreeTypeFont, name: &[u8]) -> Result<(), PilError> {
    let names = get_variation_names(font)?;
    let Some(index) = names.iter().position(|candidate| candidate == name) else {
        return Err(PilError::ValueError(format!(
            "b'{}' is not in list",
            String::from_utf8_lossy(name)
        )));
    };
    let status =
        ffi::FT_Set_Named_Instance(Some(&mut font.engine.face), (index + 1) as ffi::FT_UInt);
    check_ft_error(status)?;
    refresh_engine_metadata(font);
    font.engine.style_name = Some(String::from_utf8_lossy(&names[index]).into_owned());
    Ok(())
}

pub(crate) fn set_variation_by_axes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    if !has_variations(font) {
        return Err(PilError::OsError("invalid argument".into()));
    }
    let coords = axes
        .iter()
        .map(|axis| pillow_axis_to_fixed(*axis))
        .collect::<Vec<_>>();
    check_ft_error(ffi::FT_Set_Var_Design_Coordinates(
        Some(&mut font.engine.face),
        coords.len() as ffi::FT_UInt,
        Some(&coords),
    ))?;
    refresh_engine_metadata(font);
    Ok(())
}

pub(crate) fn native_getvaraxes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .axes
        .iter()
        .map(|axis| ImageFontVariationAxis {
            minimum: fixed_16_16_to_pillow_int(axis.min_value),
            default: fixed_16_16_to_pillow_int(axis.default_value),
            maximum: fixed_16_16_to_pillow_int(axis.max_value),
            name: raw_name_bytes(&name_table, axis.name_id).unwrap_or_default(),
        })
        .collect())
}

pub(crate) fn native_getvarnames(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    let (fvar, name_table) = variation_tables(font)?;
    Ok(fvar
        .instances
        .iter()
        .map(|instance| raw_name_bytes(&name_table, instance.subfamily_name_id).unwrap_or_default())
        .collect())
}

pub(crate) fn native_setvarname(
    font: &mut FreeTypeFont,
    instance_index: i64,
) -> Result<(), PilError> {
    let instance_index =
        u32::try_from(instance_index).map_err(|_| PilError::OsError("invalid argument".into()))?;
    let names = if instance_index == 0 {
        Vec::new()
    } else {
        get_variation_names(font)?
    };
    let status = ffi::FT_Set_Named_Instance(Some(&mut font.engine.face), instance_index);
    check_ft_error(status)?;
    refresh_engine_metadata(font);
    if instance_index != 0 {
        // `FT_Set_Named_Instance` succeeds only for a 1-based index within the
        // same fvar instance table used by `get_variation_names`, so this index
        // is in-bounds whenever the FreeType status above is OK.
        let name = &names[instance_index as usize - 1];
        if name.is_empty() {
            // Pillow 12.2.0 `_imagingft.c::font_setvarname` accepts the
            // FreeType named instance first. If the selected instance has
            // no usable subfamily name, public `getname()` preserves
            // `None` rather than FreeType's refreshed empty style string.
            font.engine.style_name = None;
        } else {
            font.engine.style_name = Some(String::from_utf8_lossy(name).into_owned());
        }
    }
    Ok(())
}

pub(crate) fn native_setvaraxes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    set_variation_by_axes(font, axes)
}

fn pillow_axis_to_fixed(axis: f32) -> ffi::FT_Fixed {
    let scaled = f64::from(axis) * 65536.0;
    let fixed = if scaled > ffi::FT_Long::MAX as f64 {
        i32::MIN
    } else if scaled < ffi::FT_Long::MIN as f64 {
        i32::MIN
    } else if scaled > f64::from(i32::MAX) {
        i32::MAX
    } else if scaled < f64::from(i32::MIN) {
        i32::MIN
    } else {
        scaled as i32
    };
    fixed as ffi::FT_Fixed
}

pub(crate) fn getlength(font: &FreeTypeFont, text: &str) -> Result<f32, PilError> {
    validate_text_length(text)?;
    Ok(length_from_basic_layout_with_flags(font, text, 0)? as f32 / 64.0)
}

pub(crate) fn native_getlength_26dot6(font: &FreeTypeFont, text: &str) -> Result<i32, PilError> {
    validate_text_length(text)?;
    length_from_basic_layout_with_flags(font, text, 0)
}

pub(crate) fn native_getsize(
    font: &FreeTypeFont,
    text: &str,
) -> Result<((i32, i32), (i32, i32)), PilError> {
    let (left, top, right, bottom) = getbbox(font, text)?;
    Ok(((right - left, bottom - top), (left, top)))
}

pub(crate) fn native_render(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    getmask2_with_options(font, text, options)
}

pub(crate) fn native_face_attrs(
    font: &FreeTypeFont,
) -> (Option<&str>, Option<&str>, u32, u32, u32, u32, u32, i64) {
    let (family, style) = getname_optional(font);
    let metrics = font.engine.metrics;
    (
        family,
        style,
        pixel(metrics.ascender) as u32,
        (-pixel(metrics.descender)) as u32,
        pixel(metrics.height) as u32,
        u32::from(metrics.x_ppem),
        u32::from(metrics.y_ppem),
        font.engine.face.num_glyphs,
    )
}

pub(crate) fn getlength_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    if options.mode.as_deref() != Some("1") {
        return getlength(font, text);
    }
    Ok(length_from_basic_layout_with_flags(font, text, text_load_flags(options))? as f32 / 64.0)
}

pub(crate) fn getbbox(font: &FreeTypeFont, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    validate_text_length(text)?;
    bbox_from_run(font, text)
}

pub(crate) fn getbbox_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    let bbox = bbox_from_run_with_flags(font, text, text_load_flags(options))?;
    let (left, top, right, bottom) = anchored_bbox(
        font,
        bbox,
        options.anchor.as_deref(),
        options.anchor_invalid_length_error,
    )?;
    let stroke = options.stroke_width;
    Ok((left - stroke, top - stroke, right + stroke, bottom + stroke))
}

#[cfg(feature = "test-api")]
pub(crate) fn getbbox_binary(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(font, text, TGT_MONO)
}

pub(crate) fn getmask(font: &FreeTypeFont, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
    validate_text_length(text)?;
    mask_from_run_with_start(font, text, TGT_NORM, (0.0, 0.0))
}

pub(crate) fn getmask_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height, pixels, _) = getmask2_with_options(font, text, options)?;
    Ok((width, height, pixels))
}

/// Render a Pillow-compatible mask together with its BASIC-layout offset.
pub(crate) fn getmask2(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    validate_text_length(text)?;
    getmask2_with_start(font, text, (0.0, 0.0))
}

/// Render a Pillow-compatible mask with a fractional raster start.
///
/// Pillow applies `start` to the mask canvas and glyph origin while leaving
/// the returned BASIC-layout offset unchanged.
pub(crate) fn getmask2_with_start(
    font: &FreeTypeFont,
    text: &str,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    validate_text_length(text)?;
    let (width, height, pixels) = mask_from_run_with_start(font, text, TGT_NORM, start)?;
    let bbox = getbbox(font, text)?;
    Ok((width, height, pixels, (bbox.0, bbox.1)))
}

pub(crate) fn getmask2_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    if options.start_invalid {
        return Err(PilError::TypeError(
            "render() argument 11 must be 2-item sequence, not float".into(),
        ));
    }
    if options_are_default_mask_options(options) {
        return getmask2(font, text);
    }
    // Pillow's BASIC `getmask2` accepts mode="RGBA" as a renderer hint and
    // still returns the ordinary grayscale mask. The mode is used by some
    // drivers, but it is not an error condition for this public endpoint.
    let load_flags = text_load_flags(options);
    let _pillow_ignored_public_args = (options.ink, options.has_args, options.has_kwargs);
    let start = options.start.unwrap_or((0.0, 0.0));
    let (width, height, pixels) = if options.stroke_width != 0.0 {
        stroked_mask_from_run_with_start(
            font,
            text,
            load_flags,
            start,
            options.stroke_width,
            options.stroke_filled,
        )?
    } else {
        mask_from_run_with_start(font, text, load_flags, start)?
    };
    let bbox = bbox_from_run_with_flags(
        font,
        text,
        if options.stroke_width != 0.0 {
            load_flags | ffi::FT_LOAD_NO_BITMAP
        } else {
            load_flags
        },
    )?;
    let (left, top, _, _) = anchored_bbox(
        font,
        bbox,
        options.anchor.as_deref(),
        options.anchor_invalid_length_error,
    )?;
    let left = left - options.stroke_width;
    let top = top - options.stroke_width;
    let offset = if options.stroke_width != 0.0 {
        let top = if top < 0.0 { top.floor() } else { top.ceil() };
        (left.floor() as i32, top as i32)
    } else {
        (left as i32, top as i32)
    };
    Ok((width, height, pixels, offset))
}

fn options_are_default_mask_options(options: &ImageFontTextOptions) -> bool {
    matches!(options.mode.as_deref(), None | Some(""))
        && options.stroke_width == 0.0
        && options.anchor.is_none()
        && options.start.is_none()
        && options.ink.map_or(true, |ink| ink == 0)
}

fn text_load_flags(options: &ImageFontTextOptions) -> i32 {
    if options.mode.as_deref() == Some("1") {
        TGT_MONO
    } else {
        TGT_NORM
    }
}

#[cfg(feature = "test-api")]
pub(crate) fn render_text_binary(
    font: &FreeTypeFont,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    validate_text_length(text)?;
    let _ = spacing;
    pack_rgba(
        mask_from_run_with_start(font, text, TGT_MONO, (0.0, 0.0))?,
        fill,
    )
}

fn validate_basic_layout_options(options: &ImageFontTextOptions) -> Result<(), PilError> {
    if options.direction.is_some()
        || options.features.is_some()
        || options.features_invalid
        || options.language.is_some()
    {
        return Err(PilError::unsupported_libraqm());
    }
    Ok(())
}

fn validate_text_length(text: &str) -> Result<(), PilError> {
    if text.chars().count() > MAX_STRING_LENGTH {
        return Err(PilError::ValueError("too many characters in string".into()));
    }
    Ok(())
}

fn anchored_bbox(
    font: &FreeTypeFont,
    (left, top, right, bottom): (i32, i32, i32, i32),
    anchor: Option<&str>,
    invalid_length_uses_legacy_error: bool,
) -> Result<(f32, f32, f32, f32), PilError> {
    let Some(anchor) = anchor else {
        return Ok((left as f32, top as f32, right as f32, bottom as f32));
    };
    if anchor.len() != 2 {
        if invalid_length_uses_legacy_error {
            return Err(PilError::ValueError(
                "anchor must be a 2 character string".into(),
            ));
        }
        return Err(bad_anchor_error(anchor));
    }
    let width = right - left;
    let ascent = pixel(font.engine.metrics.ascender);
    let descent = -pixel(font.engine.metrics.descender);
    let x_shift = match anchor.as_bytes()[0] {
        b'l' => 0,
        b'm' => -((width + 1) / 2),
        b'r' => -width,
        _ => {
            return Err(bad_anchor_error(anchor));
        }
    };
    let y_shift = match anchor.as_bytes()[1] {
        b'a' => 0,
        b't' => -top,
        b'm' => -((ascent + descent) / 2),
        b's' => -ascent,
        b'b' => -bottom,
        b'd' => -(ascent + descent),
        _ => {
            return Err(bad_anchor_error(anchor));
        }
    };
    Ok((
        (left + x_shift) as f32,
        (top + y_shift) as f32,
        (right + x_shift) as f32,
        (bottom + y_shift) as f32,
    ))
}

fn bad_anchor_error(anchor: &str) -> PilError {
    PilError::ValueError("bad anchor specified: ".to_owned() + anchor)
}

#[cfg(feature = "test-api")]
fn pack_rgba(
    (w, h, mask): (u32, u32, Vec<u8>),
    fill: (u8, u8, u8, u8),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if w == 0 || h == 0 {
        return Ok((w, h, mask));
    }
    debug_assert_eq!(
        mask.len(),
        (w as usize) * (h as usize),
        "text mask dimensions are produced by mask_from_run_with_start"
    );
    let len = mask.len() * 4;
    let mut canvas = vec![0u8; len];
    for (i, cov) in mask.into_iter().enumerate() {
        if cov == 0 {
            continue;
        }
        let o = i * 4;
        canvas[o] = fill.0;
        canvas[o + 1] = fill.1;
        canvas[o + 2] = fill.2;
        canvas[o + 3] = cov;
    }
    Ok((w, h, canvas))
}

// ── FFI helpers ──────────────────────────────────────────────────────

const KERN_DEFAULT: u32 = 0; // FT_KERNING_DEFAULT as u32
const RDR: i32 = 4; // FT_LOAD_RENDER
const TGT_NORM: i32 = 0; // FT_LOAD_TARGET_NORMAL
const TGT_MONO: i32 = 2 << 16; // FT_LOAD_TARGET_MONO

fn gid(face: &ffi::FT_Face, ch: char) -> u32 {
    ffi::FT_Get_Char_Index(face, ch as u64)
}

fn kern_26dot6(face: &ffi::FT_Face, l: u32, r: u32) -> i32 {
    let mut v = ffi::FT_Vector::default();
    ffi::FT_Get_Kerning(Some(face), l, r, KERN_DEFAULT, Some(&mut v));
    v.x as i32
}

fn basic_layout_kern(face: &ffi::FT_Face, left: u32, right: u32) -> i32 {
    // Pillow 12.2.0 `text_layout_fallback` in `_imagingft.c` adds
    // `PIXEL(delta.x)` directly to the preceding 26.6 `x_advance`.
    pixel(i64::from(kern_26dot6(face, left, right)))
}

fn round26(v: i32) -> i32 {
    pixel(i64::from(v))
}

fn pixel(x: i64) -> i32 {
    (((x + 32) & -64) >> 6) as i32
}

fn floor26(x: i64) -> i32 {
    ((x & -64) >> 6) as i32
}

fn ceil26(x: i64) -> i32 {
    (((x + 63) & -64) >> 6) as i32
}

fn length_from_basic_layout_with_flags(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
) -> Result<i32, PilError> {
    Ok(glyph_run(ttf, text, load_flags)?.final_pen)
}

fn validate_advance_26_6(advance: i64) -> Result<(), PilError> {
    if advance.unsigned_abs() >= (0x8000 * 64) as u64 {
        Err(PilError::OsError("invalid argument".into()))
    } else {
        Ok(())
    }
}

// ── Glyph run (no render, for metrics/advance/bbox) ─────────────────

struct GlyphRun {
    glyphs: Vec<RunGlyph>,
    final_pen: i32, // final pen position in 26.6
    max_pen: i32,   // maximum pen position in 26.6
}

struct RunGlyph {
    glyph_index: u32,
    pen_before: i32,
    advance: i32,
    layout_cbox: ffi::FT_BBox,
}

struct RenderedBitmap {
    pen_before: i32,
    bitmap_left: i32,
    bitmap_top: i32,
    bitmap: ffi::FT_Bitmap,
}

/// Load each glyph WITHOUT rendering, collect advances and metrics.
fn glyph_run(ttf: &FreeTypeFont, text: &str, load_flags: i32) -> Result<GlyphRun, PilError> {
    if text.is_empty() {
        return Ok(GlyphRun {
            glyphs: vec![],
            final_pen: 0,
            max_pen: 0,
        });
    }
    let face = &ttf.engine.face;
    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    let mut out = Vec::new();
    let mut max_pen = 0i32;

    for ch in text.chars() {
        let g = gid(face, ch);
        // Match Pillow's BASIC layout order: load the current glyph first,
        // then adjust the preceding advance with pixel-rounded kerning.
        let slot = ffi::FT_Load_Glyph(face, g, load_flags).map_err(ft_error_to_pil)?;
        validate_advance_26_6(slot.advance.x)?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            pen = pen.saturating_add(basic_layout_kern(face, p, g));
        }

        let pen_before = pen;
        let adv = slot.metrics.horiAdvance as i32;

        out.push(RunGlyph {
            glyph_index: g,
            pen_before,
            advance: adv,
            layout_cbox: glyph_layout_cbox(&slot),
        });

        pen = pen.saturating_add(adv);
        max_pen = max_pen.max(pen);
        prev = Some(g);
    }
    Ok(GlyphRun {
        glyphs: out,
        final_pen: pen,
        max_pen,
    })
}

fn bbox_from_run(ttf: &FreeTypeFont, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(ttf, text, TGT_NORM)
}

fn bbox_from_run_with_flags(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
) -> Result<(i32, i32, i32, i32), PilError> {
    let run = glyph_run(ttf, text, load_flags)?;
    bbox_from_glyph_run(ttf, &run)
}

fn bbox_from_glyph_run(
    ttf: &FreeTypeFont,
    run: &GlyphRun,
) -> Result<(i32, i32, i32, i32), PilError> {
    if run.glyphs.is_empty() {
        return Ok((0, 0, 0, 0));
    }

    let mut x_min = 0;
    let mut x_max = 0;
    let mut y_min = 0;
    let mut y_max = 0;

    for g in &run.glyphs {
        let px = pixel(i64::from(g.pen_before));
        let advanced = pixel(i64::from(g.pen_before.saturating_add(g.advance)));
        x_max = x_max.max(px).max(advanced);

        let cbox = g.layout_cbox;
        let glyph_x_min = px + floor26(cbox.xMin);
        let glyph_x_max = px + ceil26(cbox.xMax);
        let glyph_y_min = floor26(cbox.yMin);
        let glyph_y_max = ceil26(cbox.yMax);

        x_min = x_min.min(glyph_x_min);
        x_max = x_max.max(glyph_x_max);
        y_min = y_min.min(glyph_y_min);
        y_max = y_max.max(glyph_y_max);
    }

    x_max = x_max.max(round26(run.max_pen));
    let y_anchor = pixel(ttf.engine.metrics.ascender);
    Ok((x_min, y_anchor - y_max, x_max, y_anchor - y_min))
}

fn glyph_layout_cbox(slot: &ffi::FT_GlyphSlot) -> ffi::FT_BBox {
    if let Some(bitmap) = &slot.bitmap {
        // Pillow 12.2.0 `_imagingft.c::bounding_box_and_anchors` sizes the
        // public text mask from the loaded glyph's bitmap extents when a glyph
        // is an embedded bitmap strike. Bitmap-only SBIT glyph slots have no
        // outline cbox, so using `outline_cbox` here collapses the mask even
        // though the render pass has pixels.
        let x_min = i64::from(slot.bitmap_left) * 64;
        let x_max = (i64::from(slot.bitmap_left) + i64::from(bitmap.width)) * 64;
        let y_min = (i64::from(slot.bitmap_top) - i64::from(bitmap.rows)) * 64;
        let y_max = i64::from(slot.bitmap_top) * 64;
        return ffi::FT_BBox {
            xMin: x_min,
            yMin: y_min,
            xMax: x_max,
            yMax: y_max,
        };
    }

    slot.outline_cbox
}

// ── Mask render ──────────────────────────────────────────────────────

fn mask_from_run_with_start(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let run = glyph_run(ttf, text, load_flags)?;
    if run.glyphs.is_empty() {
        return Ok((0, 0, vec![]));
    }
    // Pillow 12.2.0 `_imagingft.c` uses FT_LOAD_TARGET_MONO consistently
    // during BASIC layout, bbox calculation, and both render passes for
    // `fontmode="1"`. Thresholding the normal grayscale mask is not
    // equivalent: monochrome hinting changes advances and glyph geometry.
    let bbox = bbox_from_glyph_run(ttf, &run)?;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` expands the allocated
    // mask by ceil(start), then rounds the shifted 26.6 pen origin.
    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = bbox.2 - bbox.0;
    let base_h = bbox.3 - bbox.1;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if positive_dimension_collapsed(base_w, adjusted_w)
        || positive_dimension_collapsed(base_h, adjusted_h)
    {
        return Err(PilError::ValueError("bad image size".into()));
    }
    let w = adjusted_w.max(0) as u32;
    let h = adjusted_h.max(0) as u32;
    let wu = w as usize;
    let hu = h as usize;
    let canvas_len = wu
        .checked_mul(hu)
        .ok_or_else(|| PilError::DimensionError("text mask dimensions overflow".into()))?;
    let mut canvas = vec![0u8; canvas_len];
    if canvas_len == 0 {
        return Ok((w, h, canvas));
    }

    let face = &ttf.engine.face;
    let mut rendered = Vec::new();
    let mut x_min = 0;
    let mut y_max = 0;

    for glyph in &run.glyphs {
        let slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, RDR | load_flags)
            .map_err(ft_error_to_pil)?;

        let px = round26(glyph.pen_before);
        x_min = x_min.min(px + slot.bitmap_left as i32);
        y_max = y_max.max(slot.bitmap_top as i32);
        if let Some(bitmap) = slot.bitmap {
            rendered.push(RenderedBitmap {
                pen_before: glyph.pen_before,
                bitmap_left: slot.bitmap_left as i32,
                bitmap_top: slot.bitmap_top as i32,
                bitmap,
            });
        }
    }

    let x_origin = ((f64::from(-x_min) + start.0) * 64.0).round() as i32;
    let y_origin = ((f64::from(-y_max) - start.1) * 64.0).round() as i32;
    paste_rendered_bitmaps(&rendered, &mut canvas, w, h, x_origin, y_origin);
    Ok((w, h, canvas))
}

fn stroked_mask_from_run_with_start(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
    stroke_width: f32,
    stroke_filled: bool,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if text.is_empty() {
        let side = (stroke_width * 2.0).ceil() as i32;
        if side < 0 {
            return Err(PilError::ValueError("bad image size".into()));
        }
        let side = side as u32;
        return Ok((side, side, vec![0; side.saturating_mul(side) as usize]));
    }

    let run = glyph_run(ttf, text, load_flags)?;
    let face = &ttf.engine.face;
    let stroker = new_stroker(&ttf.engine.library)?;
    let radius = (stroke_width * 64.0).round() as ffi::FT_Fixed;
    ffi::FT_Stroker_Set(
        stroker.handle,
        radius,
        ffi::FT_STROKER_LINECAP_ROUND as ffi::FT_Int,
        ffi::FT_STROKER_LINEJOIN_ROUND as ffi::FT_Int,
        0,
    );
    let mut rendered = Vec::new();
    let mut render_x_min = 0;
    let mut first_y_max = 0;

    // Pillow 12.2.0 `_imagingft.c::font_render_impl` loads with
    // `FT_LOAD_NO_BITMAP` whenever `stroke_width != 0`, so scalable faces
    // render stroked outlines instead of embedded strikes.  Bitmap-only
    // CBLC/CBDT faces still return their strike (or a synthesized whitespace
    // bitmap) because `TT_Load_Glyph` only honors `FT_LOAD_NO_BITMAP` for
    // scalable faces (`truetype/ttgload.c:2401-2404`).
    let stroked_load_flags = load_flags | ffi::FT_LOAD_NO_BITMAP;
    for glyph in &run.glyphs {
        // Pillow's first loop measures the pen from the unstroked rendered
        // extents (`FT_LOAD_NO_BITMAP | FT_LOAD_RENDER`), then its second
        // loop loads the outline again and strokes it.
        let rendered_slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, stroked_load_flags | RDR)
            .map_err(ft_error_to_pil)?;
        let px = round26(glyph.pen_before);
        if let Some(_bitmap) = rendered_slot.bitmap {
            render_x_min = render_x_min.min(px + rendered_slot.bitmap_left as i32);
            first_y_max = first_y_max.max(rendered_slot.bitmap_top as i32);
        }

        let layout_slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, stroked_load_flags)
            .map_err(ft_error_to_pil)?;
        let bitmap_glyph = stroked_bitmap_glyph(&layout_slot, stroke_filled, stroker.handle)?;
        rendered.push(RenderedBitmap {
            pen_before: glyph.pen_before,
            bitmap_left: bitmap_glyph.left as i32,
            bitmap_top: bitmap_glyph.top as i32,
            bitmap: bitmap_glyph.bitmap,
        });
    }

    // Pillow's `bounding_box_and_anchors` mixes the BASIC-layout advances
    // (loaded with `FT_LOAD_DEFAULT` in `text_layout_fallback`) with glyph
    // cboxes loaded under `FT_LOAD_NO_BITMAP`.  Strikes and outlines can
    // report different advances, so the pen line must come from the
    // default-load run while the cboxes come from the no-bitmap run.
    let bbox_run = glyph_run(ttf, text, stroked_load_flags)?;
    let merged_run = GlyphRun {
        glyphs: run
            .glyphs
            .iter()
            .zip(bbox_run.glyphs.iter())
            .map(|(layout_glyph, cbox_glyph)| RunGlyph {
                glyph_index: layout_glyph.glyph_index,
                pen_before: layout_glyph.pen_before,
                advance: layout_glyph.advance,
                layout_cbox: cbox_glyph.layout_cbox,
            })
            .collect(),
        final_pen: run.final_pen,
        max_pen: run.max_pen,
    };
    let bbox = bbox_from_glyph_run(ttf, &merged_run)?;
    let expected_w = ((bbox.2 - bbox.0) as f32 + stroke_width * 2.0)
        .ceil()
        .max(0.0) as i32;
    let expected_h = ((bbox.3 - bbox.1) as f32 + stroke_width * 2.0)
        .ceil()
        .max(0.0) as i32;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` allocates the mask from
    // `bounding_box_and_anchors` plus `ceil(stroke_width * 2 + start)`, not
    // from the stroked glyph bitmap extents.  Descender-heavy strings such as
    // DejaVuSans "jQ" can therefore include a leading blank column when
    // `floor(left - stroke_width)` is left of the rendered stroked bitmap, and
    // bitmap-only glyphs keep their bbox-derived height even when the stroked
    // bitmap is empty.
    let x_min = ((bbox.0 as f32) - stroke_width).floor() as i32;
    let x_max = x_min + expected_w;

    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = x_max - x_min;
    let base_h = expected_h;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if positive_dimension_collapsed(base_w, adjusted_w)
        || positive_dimension_collapsed(base_h, adjusted_h)
    {
        return Err(PilError::ValueError("bad image size".into()));
    }
    let w = adjusted_w.max(0) as u32;
    let h = adjusted_h.max(0) as u32;
    let wu = w as usize;
    let hu = h as usize;
    let canvas_len = wu
        .checked_mul(hu)
        .ok_or_else(|| PilError::DimensionError("text mask dimensions overflow".into()))?;
    let mut canvas = vec![0u8; canvas_len];

    let x_origin =
        ((f64::from(-render_x_min) + f64::from(stroke_width) + start.0) * 64.0).round() as i32;
    let y_origin =
        ((f64::from(-first_y_max) - f64::from(stroke_width) - start.1) * 64.0).round() as i32;
    paste_rendered_bitmaps(&rendered, &mut canvas, w, h, x_origin, y_origin);
    Ok((w, h, canvas))
}

fn paste_rendered_bitmaps(
    rendered: &[RenderedBitmap],
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x_origin: i32,
    y_origin: i32,
) {
    let wu = w as usize;
    let hu = h as usize;
    for rendered_bitmap in rendered {
        let bitmap = &rendered_bitmap.bitmap;
        let sx = bitmap.width as usize;
        let sy = bitmap.rows as usize;
        let px = pixel(i64::from(
            x_origin.saturating_add(rendered_bitmap.pen_before),
        ));
        let py = pixel(i64::from(y_origin));
        let dx = px + rendered_bitmap.bitmap_left;
        let dy = -(py + rendered_bitmap.bitmap_top);
        let source_x = if dx < 0 { (-dx) as usize } else { 0 };
        let target_x = (dx.max(0) as usize).min(wu);
        if source_x >= sx {
            continue;
        }
        let cw = (sx - source_x).min(wu.saturating_sub(target_x));
        let source_y = if dy < 0 { (-dy) as usize } else { 0 };
        if source_y >= sy {
            continue;
        }
        let end_y = if dy >= 0 {
            sy.min(hu.saturating_sub(dy as usize))
        } else {
            sy
        };
        for row in source_y..end_y {
            let target_y = dy + row as i32;
            let dst = target_y as usize * wu + target_x;
            let dr = &mut canvas[dst..dst + cw];
            for (column, dc) in dr.iter_mut().enumerate() {
                let sc = bitmap_coverage(bitmap, row, source_x + column);
                if sc > 0 {
                    let under = crate::color::muldiv255(u32::from(*dc), u32::from(255 - sc));
                    *dc = sc.saturating_add(under as u8);
                }
            }
        }
    }
}

fn stroked_bitmap_glyph(
    slot: &ffi::FT_GlyphSlot,
    stroke_filled: bool,
    stroker: ffi::FT_Stroker,
) -> Result<ffi::FT_BitmapGlyphOwned, PilError> {
    if slot.bitmap.is_some() {
        // Pillow's `FT_Glyph_Stroke` fails with `Invalid_Argument` for bitmap
        // glyphs (the bitmap glyph class has no stroke method in FreeType's
        // `ftglyph.c`), surfacing as `OSError: invalid argument`.  This is
        // reachable for bitmap-only CBLC/CBDT faces whose strikes win even
        // under `FT_LOAD_NO_BITMAP`.
        return Err(PilError::OsError("invalid argument".into()));
    }
    let outline = ffi::FT_Get_Outline_Glyph(Some(slot)).map_err(ft_error_to_pil)?;
    let stroked = if stroke_filled {
        ffi::FT_Outline_Glyph_StrokeBorder(Some(&outline), stroker, 0).map_err(ft_error_to_pil)
    } else {
        ffi::FT_Outline_Glyph_Stroke(Some(&outline), stroker).map_err(ft_error_to_pil)
    };
    let stroked = stroked?;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` always renders stroked
    // outline glyphs with `FT_RENDER_MODE_NORMAL`, even when the public
    // `mode="1"` path set `FT_LOAD_TARGET_MONO` for glyph loading.
    ffi::FT_Outline_Glyph_To_Bitmap(&stroked, ffi::FT_RENDER_MODE_NORMAL).map_err(ft_error_to_pil)
}

struct StrokerGuard {
    handle: ffi::FT_Stroker,
}

impl Drop for StrokerGuard {
    fn drop(&mut self) {
        ffi::FT_Stroker_Done(self.handle);
    }
}

fn new_stroker(library: &ffi::FT_Library) -> Result<StrokerGuard, PilError> {
    let mut stroker = std::ptr::null_mut();
    // Keep the status visible at the Pillow boundary. `fontdone::ffi` models
    // FreeType's output-handle contract, so a future allocator/library failure
    // must become Pillow's normal OSError instead of flowing into a null
    // stroker. The guard also mirrors the C render operation's cleanup when a
    // later glyph load or stroke step returns an error.
    check_ft_error(ffi::FT_Stroker_New(Some(library), Some(&mut stroker)))?;
    if stroker.is_null() {
        return Err(PilError::OsError("invalid argument".into()));
    }
    Ok(StrokerGuard { handle: stroker })
}

fn positive_dimension_collapsed(base: i32, adjusted: i32) -> bool {
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` accepts an exactly
    // zero-sized mask dimension when `start` consumes the base dimension;
    // only a negative result raises "bad image size".  `Image::from_luma_mask`
    // likewise preserves a legal `(width, 0)` mask with no pixel bytes.
    base > 0 && adjusted < 0
}

fn bitmap_coverage(bitmap: &ffi::FT_Bitmap, row: usize, column: usize) -> u8 {
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    // `fontdone` preserves FreeType's signed-pitch convention: with a
    // negative pitch the buffer starts at the last logical row and rows are
    // addressed backwards.  `_imagingft.c` consumes that same descriptor
    // directly, so do not normalize the bitmap by reversing its owned bytes
    // here.  This matters for embedded bitmap strikes and any future
    // fontdone route that returns a bottom-up glyph bitmap.
    let physical_row = if bitmap.pitch < 0 {
        bitmap.rows.saturating_sub(1).saturating_sub(row as u32) as usize
    } else {
        row
    };
    let row_start = physical_row.saturating_mul(pitch);
    match bitmap.pixel_mode {
        ffi::FT_PIXEL_MODE_MONO => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 8)
                .copied()
                .unwrap_or(0);
            if byte & (0x80 >> (column & 7)) != 0 {
                255
            } else {
                0
            }
        }
        ffi::FT_PIXEL_MODE_GRAY2 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 4)
                .copied()
                .unwrap_or(0);
            let value = (byte >> (6 - 2 * (column & 3))) & 0x03;
            value.saturating_mul(85)
        }
        ffi::FT_PIXEL_MODE_GRAY4 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 2)
                .copied()
                .unwrap_or(0);
            let value = if column & 1 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
            value.saturating_mul(17)
        }
        ffi::FT_PIXEL_MODE_BGRA => {
            let offset = row_start + column * 4;
            // `fontdone` SBIT decoding mirrors FreeType's BGRA bitmap shape:
            // pitch is `width * 4`, buffer length is `pitch * rows`, and this
            // helper is called only for pixels inside `bitmap.width/rows`.
            // Malformed embedded bitmap tables must be rejected in the lower
            // FreeType-compatible decoder, not hidden by Pillow `_imagingft`.
            debug_assert!(offset + 4 <= bitmap.buffer.len());
            let bgra = &bitmap.buffer[offset..offset + 4];
            gray_for_premultiplied_srgb_bgra(bgra)
        }
        _ => {
            debug_assert_eq!(
                bitmap.pixel_mode,
                ffi::FT_PIXEL_MODE_GRAY,
                "Pillow ImageFont BASIC rendering reaches imagingft as MONO or GRAY"
            );
            bitmap.buffer.get(row_start + column).copied().unwrap_or(0)
        }
    }
}

fn gray_for_premultiplied_srgb_bgra(bgra: &[u8]) -> u8 {
    let alpha = u32::from(bgra[3]);
    if alpha == 0 {
        return 0;
    }
    let luminance = (4731u32 * u32::from(bgra[0]) * u32::from(bgra[0])
        + 46868u32 * u32::from(bgra[1]) * u32::from(bgra[1])
        + 13937u32 * u32::from(bgra[2]) * u32::from(bgra[2]))
        >> 16;
    alpha.wrapping_sub(luminance / alpha) as u8
}

fn refresh_engine_metadata(font: &mut FreeTypeFont) {
    if let Ok(face_index) = usize::try_from(font.engine.face.face_index) {
        // `fontdone` refreshes the public face record after named-instance or
        // design-coordinate changes. Keep the adapter's constructor state in
        // sync so a later `font_variant()` reopens the same collection face
        // and variation instance instead of silently falling back to face 0.
        font.engine.face_index = face_index;
    }
    font.engine.family_name = font.engine.face.family_name.clone();
    font.engine.style_name = font.engine.face.style_name.clone();
    font.engine.metrics = font.engine.face.size_metrics;
}

fn variation_tables(
    font: &FreeTypeFont,
) -> Result<(tt::fvar::FvarTable, tt::name::NameTable), PilError> {
    let data = &font.engine.font_bytes;
    // `fontdone` encodes a selected named instance in bits 16..30 of the
    // face index; the collection face remains in the low 16 bits. The table
    // directory is face-specific, so resolving face 0 here returns the wrong
    // variation metadata for a non-zero TTC face.
    let collection_face_index = font.engine.face_index & 0xFFFF;
    let (_, face_offset) = tt::resolve_face_index(data, collection_face_index)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let directory = tt::parse_table_directory_at(data, face_offset)
        .map_err(|_| PilError::OsError("invalid argument".into()))?;
    let fvar = directory
        .find(data, tag(b"fvar"))
        .ok_or_else(|| PilError::OsError("invalid argument".into()))
        .and_then(|bytes| {
            tt::fvar::parse_fvar(bytes).map_err(|_| PilError::OsError("invalid argument".into()))
        })?;
    let name_table = directory
        .find(data, tag(b"name"))
        .ok_or_else(|| PilError::OsError("invalid argument".into()))
        .and_then(|bytes| {
            tt::name::parse_name(bytes).map_err(|_| PilError::OsError("invalid argument".into()))
        })?;
    Ok((fvar, name_table))
}

fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

fn fixed_16_16_to_pillow_int(value: i32) -> i32 {
    value / 65536
}

fn name_bytes(table: &tt::name::NameTable, name_id: u16) -> Option<Vec<u8>> {
    let record = preferred_name_record(table, name_id)?;
    if record.platform_id == 3 {
        Some(decode_utf16be_to_utf8(&record.string).into_bytes())
    } else {
        Some(record.string.clone())
    }
}

fn raw_name_bytes(table: &tt::name::NameTable, name_id: u16) -> Option<Vec<u8>> {
    preferred_name_record(table, name_id).map(|record| record.string.clone())
}

fn preferred_name_record(
    table: &tt::name::NameTable,
    name_id: u16,
) -> Option<&tt::name::SfntNameRecord> {
    let preferred = table
        .records
        .iter()
        .position(|record| {
            record.name_id == name_id
                && record.platform_id == 3
                && matches!(record.encoding_id, 1 | 10)
                && record.language_id == 0x0409
        })
        .or_else(|| {
            table.records.iter().position(|record| {
                record.name_id == name_id
                    && record.platform_id == 3
                    && matches!(record.encoding_id, 1 | 10)
            })
        })
        .or_else(|| {
            table
                .records
                .iter()
                .position(|record| record.name_id == name_id && record.platform_id == 1)
        })?;
    Some(&table.records[preferred])
}

fn decode_utf16be_to_utf8(bytes: &[u8]) -> String {
    char::decode_utf16(
        bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
    )
    .map(|value| value.unwrap_or(char::REPLACEMENT_CHARACTER))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{bitmap_coverage, ffi, freetype_encoding};
    use crate::{FreeTypeFont, ImageFontLoadOptions};

    #[test]
    fn freetype_encoding_tags_match_fontdone_values() {
        let tags = [
            ("unic", ffi::FT_ENCODING_UNICODE),
            ("symb", ffi::FT_ENCODING_MS_SYMBOL),
            ("ADOB", ffi::FT_ENCODING_ADOBE_STANDARD),
            ("ADBE", ffi::FT_ENCODING_ADOBE_EXPERT),
            ("ADBC", ffi::FT_ENCODING_ADOBE_CUSTOM),
            ("armn", ffi::FT_ENCODING_APPLE_ROMAN),
            ("sjis", ffi::FT_ENCODING_SJIS),
            ("gb  ", ffi::FT_ENCODING_PRC),
            ("big5", ffi::FT_ENCODING_BIG5),
            ("wans", ffi::FT_ENCODING_WANSUNG),
            ("joha", ffi::FT_ENCODING_JOHAB),
            ("lat1", ffi::FT_ENCODING_ADOBE_LATIN_1),
            ("lat2", ffi::FT_ENCODING_OLD_LATIN_2),
        ];
        for (tag, expected) in tags {
            assert_eq!(
                freetype_encoding(tag),
                Some(expected as ffi::FT_Encoding),
                "encoding tag {tag}"
            );
        }
        assert_eq!(freetype_encoding("unknown"), None);
    }

    #[test]
    fn bitmap_coverage_reads_fontdone_negative_pitch_rows() {
        let bitmap = ffi::FT_Bitmap {
            rows: 2,
            width: 2,
            pitch: -2,
            buffer: vec![10, 20, 30, 40],
            num_grays: 256,
            pixel_mode: ffi::FT_PIXEL_MODE_GRAY,
        };

        // fontdone follows FreeType's bottom-up bitmap contract: a negative
        // pitch places the first visual row in the last physical row.
        assert_eq!(bitmap_coverage(&bitmap, 0, 0), 30);
        assert_eq!(bitmap_coverage(&bitmap, 0, 1), 40);
        assert_eq!(bitmap_coverage(&bitmap, 1, 0), 10);
        assert_eq!(bitmap_coverage(&bitmap, 1, 1), 20);
    }

    fn make_ttc(fonts: &[&[u8]]) -> Vec<u8> {
        assert!(!fonts.is_empty());
        let header_len = 12 + fonts.len() * 4;
        let first_face = (header_len + 3) & !3;
        let mut ttc = vec![0; first_face];
        ttc[0..4].copy_from_slice(b"ttcf");
        ttc[4..8].copy_from_slice(&0x0001_0000_u32.to_be_bytes());
        let face_count = match u32::try_from(fonts.len()) {
            Ok(count) => count,
            Err(_) => return Vec::new(),
        };
        ttc[8..12].copy_from_slice(&face_count.to_be_bytes());
        let mut cursor = first_face;
        let mut offsets = Vec::with_capacity(fonts.len());
        for font in fonts {
            cursor = (cursor + 3) & !3;
            let Some(base) = u32::try_from(cursor).ok() else {
                return Vec::new();
            };
            offsets.push(base);
            let end = cursor.saturating_add(font.len());
            ttc.resize(end, 0);
            ttc[cursor..end].copy_from_slice(font);

            let Some(table_count) = font
                .get(4..6)
                .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
            else {
                return Vec::new();
            };
            for table in 0..usize::from(table_count) {
                let offset = cursor.saturating_add(12 + table * 16 + 8);
                let Some(bytes) = ttc.get(offset..offset + 4) else {
                    return Vec::new();
                };
                let old = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let Some(adjusted) = old.checked_add(base) else {
                    return Vec::new();
                };
                ttc[offset..offset + 4].copy_from_slice(&adjusted.to_be_bytes());
            }
            cursor = end;
        }
        for (index, offset) in offsets.into_iter().enumerate() {
            let start = 12 + index * 4;
            ttc[start..start + 4].copy_from_slice(&offset.to_be_bytes());
        }
        ttc
    }

    #[test]
    fn nonzero_ttc_face_and_variant_preserve_fontdone_selection() {
        let ttc = make_ttc(&[
            include_bytes!("../../tests/fixtures/assets/font/fonts/DejaVuSans.ttf"),
            include_bytes!("../../tests/fixtures/assets/font/fonts/variable-named-instances.ttf"),
        ]);
        let first = FreeTypeFont::from_bytes_with_options(
            ttc.clone(),
            20.0,
            &ImageFontLoadOptions {
                index: Some(0),
                ..ImageFontLoadOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("TTC face 0 must load: {error}"));
        let second = FreeTypeFont::from_bytes_with_options(
            ttc,
            20.0,
            &ImageFontLoadOptions {
                index: Some(1),
                ..ImageFontLoadOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("TTC face 1 must load: {error}"));

        assert_ne!(first.getname(), second.getname());
        assert!(
            !second
                .get_variation_axes()
                .unwrap_or_else(|error| panic!("TTC face 1 variation metadata: {error}"))
                .is_empty()
        );

        let variant = second
            .font_variant(None)
            .unwrap_or_else(|error| panic!("TTC face 1 variant must load: {error}"));
        assert_eq!(variant.getname(), second.getname());
        assert!(
            !variant
                .get_variation_axes()
                .unwrap_or_else(|error| panic!("TTC variant variation metadata: {error}"))
                .is_empty()
        );
    }
}
