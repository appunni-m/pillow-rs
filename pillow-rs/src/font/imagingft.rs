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
    FontLengthAndBounds, FontMaskAndBounds, FontMaskAndOffset, FontSizeAndOffset, FreeTypeFont,
    ImageFontFaceInfo, ImageFontLoadOptions, ImageFontTextOptions, ImageFontVariantOptions,
    ImageFontVariationAxis,
};
use crate::error::PilError;
use crate::image::Image;
use fontdone::ffi;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

mod layout;
mod render;
mod variation;

#[cfg(feature = "test-api")]
pub(crate) use layout::getbbox_binary;
pub(crate) use layout::{
    getbbox, getbbox_with_options, getlength, getlength_and_bbox, getlength_with_options,
    getmetrics, native_face_info, native_getlength_26dot6, native_getsize,
};
#[cfg(feature = "test-api")]
pub(crate) use render::render_text_binary;
pub(crate) use render::{
    getmask, getmask_with_options, getmask2, getmask2_with_options, getmask2_with_start,
    native_render,
};
pub(crate) use variation::{
    font_variant, font_variant_with_options, get_variation_axes, get_variation_names,
    has_variations, native_getvaraxes, native_getvarnames, native_setvaraxes, native_setvarname,
    set_variation_by_axes, set_variation_by_name,
};

const MAX_STRING_LENGTH: usize = 1_000_000;
const MAX_CACHED_SOURCE_BYTES: usize = 256 * 1024;
const BASIC_LAYOUT_CACHE_CAPACITY: usize = 512;

#[derive(Default)]
struct BasicLayoutCache {
    glyph_indices: HashMap<char, u32>,
    advances: HashMap<(u32, i32), i32>,
    kernings: HashMap<(u32, u32), i32>,
}

impl BasicLayoutCache {
    fn clear_variable_metrics(&mut self) {
        self.advances.clear();
        self.kernings.clear();
    }
}

fn insert_bounded<K: Eq + Hash, V>(map: &mut HashMap<K, V>, key: K, value: V) {
    if map.len() < BASIC_LAYOUT_CACHE_CAPACITY || map.contains_key(&key) {
        map.insert(key, value);
    }
}

struct CachedSourceFace {
    face_index: ffi::FT_Long,
    face: ffi::FT_Face,
}

#[derive(Clone, Default)]
struct VariationMetadata {
    names: Vec<Vec<u8>>,
    axes: Vec<ImageFontVariationAxis>,
    named_instance_coordinates: Vec<Vec<ffi::FT_Fixed>>,
    named_instance_style_names: Vec<String>,
}

struct CachedVariationMetadata {
    face_index: ffi::FT_Long,
    source_bytes: Rc<Vec<u8>>,
    metadata: VariationMetadata,
}

thread_local! {
    // A one-entry cache reuses parsed immutable tables when callers repeatedly
    // open the same small static font. The source face never escapes this
    // thread; FT_New_Memory_Face_From_Source verifies byte equality and makes
    // an independent mutable face for each caller.
    static CACHED_SOURCE_FACE: RefCell<Option<CachedSourceFace>> = const { RefCell::new(None) };
    // Variable faces need independent coordinate state, so they do not use
    // CACHED_SOURCE_FACE. Their immutable names can still be reused by exact
    // source bytes and collection-face index.
    static CACHED_VARIATION_METADATA: RefCell<Option<CachedVariationMetadata>> = const { RefCell::new(None) };
}

pub(super) struct FontEngine {
    library: ffi::FT_Library,
    face: ffi::FT_Face,
    pub(super) font_bytes: Rc<Vec<u8>>,
    face_index: usize,
    pub(super) size_pt: f32,
    encoding: Option<String>,
    layout_engine: Option<String>,
    family_name: Option<String>,
    style_name: Option<String>,
    metrics: ffi::FT_Size_Metrics,
    // BASIC getlength only needs glyph IDs, hinted advances, and pair kerning.
    // Keep a small cache for repeated measurements; variation setters clear
    // the size-dependent entries after changing the active design instance.
    basic_layout_cache: RefCell<BasicLayoutCache>,
    // Pillow remembers the last public named-variation index. FreeType can
    // report an unknown error when the same named instance is selected twice,
    // so keep this state in the Rust core instead of relying on Python-side
    // wrapper bookkeeping.
    last_variation_index: Option<usize>,
    // Variation names and axes are immutable for a given byte sequence and
    // collection face. Keep their Pillow-shaped values beside the face so
    // queries and setters do not reparse `fvar` and `name` on every call.
    source_face_index: ffi::FT_Long,
    variation_metadata: RefCell<Option<VariationMetadata>>,
}

pub(super) fn load_truetype(data: Vec<u8>, size: f32) -> Result<FreeTypeFont, PilError> {
    load_truetype_with_index(Rc::new(data), size, 0, None, None, None)
}

pub(super) fn load_truetype_with_options(
    data: Vec<u8>,
    size: f32,
    options: &ImageFontLoadOptions,
) -> Result<FreeTypeFont, PilError> {
    load_truetype_with_index(
        Rc::new(data),
        size,
        options.index.unwrap_or(0),
        options.encoding.clone(),
        options.layout_engine.clone(),
        None,
    )
}

fn load_truetype_with_index(
    data: Rc<Vec<u8>>,
    size: f32,
    face_index: usize,
    encoding: Option<String>,
    layout_engine: Option<String>,
    variant_source: Option<&FreeTypeFont>,
) -> Result<FreeTypeFont, PilError> {
    if size.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
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
    let mut face = open_memory_face(
        &library,
        Rc::clone(&data),
        face_index_ffi,
        size,
        variant_source,
    )?;

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
    let variation_metadata = variation::cached_source_variation_metadata(&data, face_index_ffi);

    let engine = FontEngine {
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
        basic_layout_cache: RefCell::new(BasicLayoutCache::default()),
        last_variation_index: None,
        source_face_index: face_index_ffi,
        variation_metadata: RefCell::new(variation_metadata),
    };
    let font = FreeTypeFont { engine };
    if variation::has_variations(&font)
        && ffi::FT_Get_Font_Format(Some(&font.engine.face)) != Some("Type 1")
    {
        // Named-instance selection otherwise parses and decodes the fvar/name
        // tables on its first call. Populate that immutable metadata while the
        // variable face is being opened, so the setter's cold call stays on
        // the same short lookup path as repeated calls. Keep load behavior
        // intact for malformed or unsupported variation tables; the public
        // query/setter will report the error when requested.
        let _ = variation::ensure_variation_metadata(&font);
    }
    Ok(font)
}

fn open_memory_face(
    library: &ffi::FT_Library,
    data: Rc<Vec<u8>>,
    face_index: ffi::FT_Long,
    size: f32,
    variant_source: Option<&FreeTypeFont>,
) -> Result<ffi::FT_Face, PilError> {
    // Pillow's `_imagingft.getfont` classifies an empty in-memory source as
    // an unavailable resource before FreeType's memory-stream probe can
    // report `Invalid_Stream_Operation`.
    if data.is_empty() {
        return Err(PilError::OsError("cannot open resource".into()));
    }

    if let Some(source) = variant_source {
        if let Some(face) = ffi::FT_New_Memory_Face_From_Source(
            library,
            &source.engine.face,
            data.as_slice(),
            face_index,
        )
        .map_err(ft_error_to_pil)?
        {
            return Ok(face);
        }
        return ffi::FT_New_Memory_Face_Owned(library, data, face_index, size)
            .map_err(ft_error_to_pil);
    }

    if data.len() <= MAX_CACHED_SOURCE_BYTES {
        let cached_face = CACHED_SOURCE_FACE.with(|cache| {
            let cache = cache.borrow();
            match cache.as_ref() {
                Some(cached) if cached.face_index == face_index => {
                    ffi::FT_New_Memory_Face_From_Source(
                        library,
                        &cached.face,
                        data.as_slice(),
                        face_index,
                    )
                }
                _ => Ok(None),
            }
        });
        if let Some(face) = cached_face.map_err(ft_error_to_pil)? {
            return Ok(face);
        }
    }

    let source_face = ffi::FT_New_Memory_Face_Owned(library, Rc::clone(&data), face_index, size)
        .map_err(ft_error_to_pil)?;
    if data.len() <= MAX_CACHED_SOURCE_BYTES
        && let Some(face) =
            ffi::FT_New_Memory_Face_From_Source(library, &source_face, data.as_slice(), face_index)
                .map_err(ft_error_to_pil)?
    {
        CACHED_SOURCE_FACE.with(|cache| {
            *cache.borrow_mut() = Some(CachedSourceFace {
                face_index,
                face: source_face,
            });
        });
        return Ok(face);
    }
    Ok(source_face)
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
