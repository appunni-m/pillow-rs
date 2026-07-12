#![allow(non_camel_case_types, non_snake_case)]

use std::collections::BTreeMap;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use crate::api;
use crate::font::{KerningMode, SizeRequest, SizeRequestError, SizeRequestType};

use super::constants::*;
use super::convert::{
    FT_LOAD_TARGET_MODE, error_to_ft, glyph_format_from_core, load_flag_for_render_mode,
    load_flags_to_core, render_mode_to_core,
};
use super::types::{
    FT_Angle, FT_BBox, FT_Bitmap, FT_Bool, FT_Byte, FT_Bytes, FT_Char, FT_CharMap,
    FT_CharMapRecPublic, FT_Encoding, FT_Error, FT_F26Dot6, FT_FaceRecPublic, FT_Fixed,
    FT_Glyph_Format, FT_Glyph_Metrics, FT_Int, FT_Int32, FT_LcdFilter, FT_Long, FT_Matrix,
    FT_OutlineSnapshot, FT_Pointer, FT_Pos, FT_Render_Mode, FT_Sfnt_Tag, FT_SfntLangTag,
    FT_SfntName, FT_Size, FT_Size_Metrics as FT_Size_MetricsRec, FT_Size_RequestRec,
    FT_TrueTypeEngineType, FT_UInt, FT_ULong, FT_UShort, FT_Vector, TT_Header, TT_HoriHeader,
    TT_MaxProfile, TT_OS2, TT_PCLT, TT_Postscript, TT_VertHeader,
};

const FT_ADVANCE_FLAG_FAST_ONLY_I32: FT_Int32 = 0x2000_0000;

#[derive(Debug, Clone, Copy, Default)]
pub struct FT_Library {
    inner: api::Library,
    _lcd_geometry: [FT_Vector; 3],
}

#[derive(Clone)]
pub struct FT_Face {
    inner: api::Face,
    probe_only: bool,
    sfnt_os2: Option<Box<TT_OS2>>,
    sfnt_head: Option<Box<TT_Header>>,
    sfnt_maxp: Option<Box<TT_MaxProfile>>,
    sfnt_hhea: Option<Box<TT_HoriHeader>>,
    sfnt_vhea: Option<Box<TT_VertHeader>>,
    sfnt_post: Option<Box<TT_Postscript>>,
    sfnt_pclt: Option<Box<TT_PCLT>>,
    charmaps: Box<[FT_CharMapRecInternal]>,
    transform_matrix: FT_Matrix,
    transform_delta: FT_Vector,
    refcount: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct FT_CharMapRecInternal {
    public: FT_CharMapRecPublic,
    format: FT_Long,
    language_id: FT_ULong,
}

type CharmapMetadata = (FT_Long, FT_ULong, FT_Int);
type CharmapMetadataRegistry = BTreeMap<usize, CharmapMetadata>;

#[derive(Clone)]
pub struct FT_GlyphSlot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub num_subglyphs: FT_UInt,
    pub bitmap: Option<FT_Bitmap>,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    pub outline_cbox: FT_BBox,
    pub outline_bbox: FT_BBox,
    pub outline: Option<FT_OutlineSnapshot>,
    core_slot: api::GlyphSlot,
    source_face: api::Face,
    load_flags: api::LoadFlags,
}

pub fn FT_Init_FreeType() -> FT_Library {
    FT_Library {
        inner: api::Library::init(),
        _lcd_geometry: [
            FT_Vector { x: -21, y: 0 },
            FT_Vector { x: 0, y: 0 },
            FT_Vector { x: 21, y: 0 },
        ],
    }
}

pub fn FT_Done_FreeType(library: Option<FT_Library>) -> FT_Error {
    if library.is_some() {
        FT_Err_Ok
    } else {
        35 // matches FreeType 2.14.3 runtime: FT_Done_FreeType(NULL)
    }
}

pub fn FT_Done_Face(face: Option<FT_Face>) -> FT_Error {
    if face.is_some() {
        FT_Err_Ok
    } else {
        FT_Err_Invalid_Face_Handle as FT_Error
    }
}

pub fn FT_Face_CheckTrueTypePatents(_face: Option<&FT_Face>) -> FT_Bool {
    0
}

pub fn FT_Face_SetUnpatentedHinting(_face: Option<&mut FT_Face>, _value: FT_Bool) -> FT_Bool {
    0
}

pub fn FT_Outline_Get_CBox(outline: Option<&FT_OutlineSnapshot>, acbox: Option<&mut FT_BBox>) {
    let (Some(outline), Some(acbox)) = (outline, acbox) else {
        return;
    };
    if outline.points.is_empty() {
        *acbox = FT_BBox::default();
        return;
    }
    let first = outline.points[0];
    let mut x_min = first.x;
    let mut y_min = first.y;
    let mut x_max = first.x;
    let mut y_max = first.y;
    for point in &outline.points[1..] {
        x_min = x_min.min(point.x);
        y_min = y_min.min(point.y);
        x_max = x_max.max(point.x);
        y_max = y_max.max(point.y);
    }
    *acbox = FT_BBox {
        xMin: x_min,
        yMin: y_min,
        xMax: x_max,
        yMax: y_max,
    };
}

pub fn FT_OpenType_Free(_face: Option<&FT_Face>, _table: FT_Bytes) {}

pub fn FT_OpenType_Validate(
    _face: Option<&FT_Face>,
    _validation_flags: FT_UInt,
    _base_table: Option<&mut FT_Bytes>,
    _gdef_table: Option<&mut FT_Bytes>,
    _gpos_table: Option<&mut FT_Bytes>,
    _gsub_table: Option<&mut FT_Bytes>,
    _jstf_table: Option<&mut FT_Bytes>,
) -> FT_Error {
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_GlyphSlot_AdjustWeight(_slot: FT_GlyphSlot, _x_delta: FT_Fixed, _y_delta: FT_Fixed) {}

pub fn FT_GlyphSlot_Embolden(_slot: FT_GlyphSlot) {}

pub fn FT_GlyphSlot_Oblique(_slot: FT_GlyphSlot) {}

pub fn FT_GlyphSlot_Slant(_slot: FT_GlyphSlot) {}

pub fn FT_Get_Sfnt_LangTag(
    _face: Option<&FT_Face>,
    _lang_id: FT_UInt,
    _lang_tag: Option<&mut FT_SfntLangTag>,
) -> FT_Error {
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_New_Size(face: Option<&FT_Face>, size: Option<&mut FT_Size>) -> FT_Error {
    if face.is_none() {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    }
    if size.is_none() {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Done_Size(size: FT_Size) -> FT_Error {
    if size.is_null() {
        return FT_Err_Invalid_Size_Handle;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Activate_Size(size: FT_Size) -> FT_Error {
    if size.is_null() {
        return FT_Err_Invalid_Size_Handle;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

const FT_TRIG_SCALE: FT_Fixed = 0xDBD9_5B16;
const FT_TRIG_SAFE_MSB: i32 = 29;
const FT_TRIG_MAX_ITERS: usize = 23;
const FT_TRIG_ARCTAN_TABLE: [FT_Angle; FT_TRIG_MAX_ITERS - 1] = [
    1_740_967, 919_879, 466_945, 234_379, 117_304, 58_666, 29_335, 14_668, 7_334, 3_667, 1_833,
    917, 458, 229, 115, 57, 29, 14, 7, 4, 2, 1,
];

pub fn FT_Sin(angle: FT_Angle) -> FT_Fixed {
    let mut vector = FT_Vector { x: 0, y: 0 };
    FT_Vector_Unit(Some(&mut vector), angle);
    vector.y
}

pub fn FT_Cos(angle: FT_Angle) -> FT_Fixed {
    let mut vector = FT_Vector { x: 0, y: 0 };
    FT_Vector_Unit(Some(&mut vector), angle);
    vector.x
}

pub fn FT_Tan(angle: FT_Angle) -> FT_Fixed {
    let mut vector = FT_Vector { x: 1 << 24, y: 0 };
    ft_trig_pseudo_rotate(&mut vector, angle);
    FT_DivFix(vector.y, vector.x)
}

pub fn FT_Atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    if dx == 0 && dy == 0 {
        return 0;
    }

    let mut vector = FT_Vector { x: dx, y: dy };
    ft_trig_prenorm(&mut vector);
    ft_trig_pseudo_polarize(&mut vector);
    vector.y
}

pub fn FT_Angle_Diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    let mut delta = angle2.wrapping_sub(angle1);
    while delta <= FT_ANGLE_PI.wrapping_neg() {
        delta = delta.wrapping_add(FT_ANGLE_2PI);
    }
    while delta > FT_ANGLE_PI {
        delta = delta.wrapping_sub(FT_ANGLE_2PI);
    }
    delta
}

pub fn FT_Vector_Unit(vec: Option<&mut FT_Vector>, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    vec.x = FT_TRIG_SCALE >> 8;
    vec.y = 0;
    ft_trig_pseudo_rotate(vec, angle);
    vec.x = (vec.x + 0x80) >> 8;
    vec.y = (vec.y + 0x80) >> 8;
}

pub fn FT_Vector_From_Polar(vec: Option<&mut FT_Vector>, length: FT_Fixed, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    vec.x = length;
    vec.y = 0;
    FT_Vector_Rotate(Some(vec), angle);
}

pub fn FT_Vector_Length(vec: Option<&FT_Vector>) -> FT_Fixed {
    let Some(vec) = vec else {
        return 0;
    };
    crate::fixed::ft_vector_length_long(vec.x, vec.y)
}

pub fn FT_Vector_Polarize(
    vec: Option<&FT_Vector>,
    length: Option<&mut FT_Fixed>,
    angle: Option<&mut FT_Angle>,
) {
    let (Some(vec), Some(length), Some(angle)) = (vec, length, angle) else {
        return;
    };
    let mut vector = *vec;
    if vector.x == 0 && vector.y == 0 {
        return;
    }

    let shift = ft_trig_prenorm(&mut vector);
    ft_trig_pseudo_polarize(&mut vector);
    vector.x = ft_trig_downscale(vector.x);
    *length = if shift >= 0 {
        vector.x >> shift
    } else {
        (vector.x as u64).wrapping_shl((-shift) as u32) as FT_Fixed
    };
    *angle = vector.y;
}

pub fn FT_Vector_Rotate(vec: Option<&mut FT_Vector>, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    if angle == 0 {
        return;
    }
    let mut vector = *vec;
    if vector.x == 0 && vector.y == 0 {
        return;
    }

    let mut shift = ft_trig_prenorm(&mut vector);
    ft_trig_pseudo_rotate(&mut vector, angle);
    vector.x = ft_trig_downscale(vector.x);
    vector.y = ft_trig_downscale(vector.y);

    if shift > 0 {
        let half = 1 << (shift - 1);
        vec.x = (vector.x + half - i64::from(vector.x < 0)) >> shift;
        vec.y = (vector.y + half - i64::from(vector.y < 0)) >> shift;
    } else {
        shift = -shift;
        vec.x = (vector.x as u64).wrapping_shl(shift as u32) as FT_Pos;
        vec.y = (vector.y as u64).wrapping_shl(shift as u32) as FT_Pos;
    }
}

fn ft_trig_downscale(value: FT_Fixed) -> FT_Fixed {
    let (value, sign) = move_long_sign(value, 1);
    let value = ((value as u128 * FT_TRIG_SCALE as u128 + 0x4000_0000) >> 32) as FT_Fixed;
    if sign < 0 { -value } else { value }
}

fn ft_trig_prenorm(vec: &mut FT_Vector) -> i32 {
    let x = vec.x;
    let y = vec.y;
    let mut shift = ft_msb_u32((ft_abs(x) as u32) | (ft_abs(y) as u32));
    if shift <= FT_TRIG_SAFE_MSB {
        shift = FT_TRIG_SAFE_MSB - shift;
        vec.x = (x as u64).wrapping_shl(shift as u32) as FT_Pos;
        vec.y = (y as u64).wrapping_shl(shift as u32) as FT_Pos;
    } else {
        shift -= FT_TRIG_SAFE_MSB;
        vec.x = x >> shift;
        vec.y = y >> shift;
        shift = -shift;
    }
    shift
}

fn ft_trig_pseudo_rotate(vec: &mut FT_Vector, mut theta: FT_Angle) {
    let mut x = vec.x;
    let mut y = vec.y;

    while theta < -FT_ANGLE_PI4 {
        let xtemp = y;
        y = -x;
        x = xtemp;
        theta += FT_ANGLE_PI2;
    }
    while theta > FT_ANGLE_PI4 {
        let xtemp = -y;
        y = x;
        x = xtemp;
        theta -= FT_ANGLE_PI2;
    }

    let mut b = 1;
    for (i, arctan) in (1..FT_TRIG_MAX_ITERS).zip(FT_TRIG_ARCTAN_TABLE) {
        if theta < 0 {
            let xtemp = x + ((y + b) >> i);
            y -= (x + b) >> i;
            x = xtemp;
            theta += arctan;
        } else {
            let xtemp = x - ((y + b) >> i);
            y += (x + b) >> i;
            x = xtemp;
            theta -= arctan;
        }
        b <<= 1;
    }

    vec.x = x;
    vec.y = y;
}

fn ft_trig_pseudo_polarize(vec: &mut FT_Vector) {
    let mut x = vec.x;
    let mut y = vec.y;
    let mut theta;

    if y > x {
        if y > -x {
            theta = FT_ANGLE_PI2;
            let xtemp = y;
            y = -x;
            x = xtemp;
        } else {
            theta = if y > 0 { FT_ANGLE_PI } else { -FT_ANGLE_PI };
            x = -x;
            y = -y;
        }
    } else if y < -x {
        theta = -FT_ANGLE_PI2;
        let xtemp = -y;
        y = x;
        x = xtemp;
    } else {
        theta = 0;
    }

    let mut b = 1;
    for (i, arctan) in (1..FT_TRIG_MAX_ITERS).zip(FT_TRIG_ARCTAN_TABLE) {
        if y > 0 {
            let xtemp = x + ((y + b) >> i);
            y -= (x + b) >> i;
            x = xtemp;
            theta += arctan;
        } else {
            let xtemp = x - ((y + b) >> i);
            y += (x + b) >> i;
            x = xtemp;
            theta -= arctan;
        }
        b <<= 1;
    }

    theta = if theta >= 0 {
        ft_pad_round(theta, 16)
    } else {
        -ft_pad_round(-theta, 16)
    };
    vec.x = x;
    vec.y = theta;
}

fn ft_msb_u32(value: u32) -> i32 {
    if value == 0 {
        -1
    } else {
        31 - value.leading_zeros() as i32
    }
}

fn ft_abs(value: FT_Long) -> FT_Long {
    if value < 0 { -value } else { value }
}

fn ft_pad_round(value: FT_Long, n: FT_Long) -> FT_Long {
    (value + n / 2) & !(n - 1)
}

pub fn FT_Library_SetLcdFilter(
    _library: Option<&mut FT_Library>,
    _filter: FT_LcdFilter,
) -> FT_Error {
    FT_Err_Unimplemented_Feature
}

pub fn FT_Library_SetLcdFilterWeights(
    _library: Option<&mut FT_Library>,
    _weights: *mut FT_Byte,
) -> FT_Error {
    FT_Err_Unimplemented_Feature
}

pub fn FT_Library_SetLcdGeometry(
    library: Option<&mut FT_Library>,
    sub: Option<[FT_Vector; 3]>,
) -> FT_Error {
    let Some(library) = library else {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(sub) = sub else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    library._lcd_geometry = sub;
    FT_Err_Ok
}

pub fn FT_Get_TrueType_Engine_Type(library: Option<&FT_Library>) -> FT_TrueTypeEngineType {
    if library.is_some() {
        FT_TRUETYPE_ENGINE_TYPE_PATENTED as FT_TrueTypeEngineType
    } else {
        FT_TRUETYPE_ENGINE_TYPE_NONE as FT_TrueTypeEngineType
    }
}

pub fn FT_Set_Transform(
    face: Option<&mut FT_Face>,
    matrix: Option<&FT_Matrix>,
    delta: Option<&FT_Vector>,
) {
    let Some(face) = face else {
        return;
    };
    // FreeType `ftobjs.c:791-817` resets null transform pointers to the
    // identity matrix and zero delta instead of preserving previous values.
    face.transform_matrix = matrix.copied().unwrap_or(FT_Matrix {
        xx: 0x10000,
        xy: 0,
        yx: 0,
        yy: 0x10000,
    });
    face.transform_delta = delta.copied().unwrap_or(FT_Vector { x: 0, y: 0 });
}

pub fn FT_Get_Transform(
    face: Option<&FT_Face>,
    matrix: Option<&mut FT_Matrix>,
    delta: Option<&mut FT_Vector>,
) {
    let Some(face) = face else {
        return;
    };
    if let Some(matrix) = matrix {
        *matrix = face.transform_matrix;
    }
    if let Some(delta) = delta {
        *delta = face.transform_delta;
    }
}

pub fn FT_Reference_Face(face: Option<&mut FT_Face>) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    face.refcount = face.refcount.saturating_add(1);
    FT_Err_Ok
}

// Stub implementations for unported FreeType features.
// These return Unimplemented_Feature or sentinel values as documented.

pub fn FT_Get_Gasp(face: Option<&FT_Face>, ppem: FT_UInt) -> FT_Int {
    face.map_or(FT_GASP_NO_TABLE as FT_Int, |face| face.inner.get_gasp(ppem))
}

pub fn FT_Select_Size(_face: Option<&mut FT_Face>, _strike_index: FT_Int) -> FT_Error {
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Get_SubGlyph_Info(
    glyph: Option<&FT_GlyphSlot>,
    sub_index: FT_UInt,
    index: Option<&mut FT_Int>,
    flags: Option<&mut FT_UInt>,
    glyph1: Option<&mut FT_Int>,
    glyph2: Option<&mut FT_Int>,
    transform: Option<&mut FT_Matrix>,
) -> FT_Error {
    let Some(glyph) = glyph else {
        return FT_Err_Invalid_Argument;
    };
    if glyph.format != FT_GLYPH_FORMAT_COMPOSITE {
        return FT_Err_Invalid_Argument;
    }
    let Ok(sub_index) = usize::try_from(sub_index) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(subglyph) = glyph.core_slot.subglyphs.get(sub_index) else {
        return FT_Err_Invalid_Argument;
    };
    let (Some(index), Some(flags), Some(glyph1), Some(glyph2), Some(transform)) =
        (index, flags, glyph1, glyph2, transform)
    else {
        return FT_Err_Invalid_Argument;
    };

    *index = subglyph.index as FT_Int;
    *flags = subglyph.flags as FT_UInt;
    *glyph1 = subglyph.arg1 as FT_Int;
    *glyph2 = subglyph.arg2 as FT_Int;
    transform.xx = subglyph.transform.xx as FT_Fixed;
    transform.xy = subglyph.transform.xy as FT_Fixed;
    transform.yx = subglyph.transform.yx as FT_Fixed;
    transform.yy = subglyph.transform.yy as FT_Fixed;
    FT_Err_Ok
}

pub fn FT_Get_Glyph_Name(
    face: &FT_Face,
    glyph_index: FT_UInt,
    buffer: &mut [u8],
) -> Result<usize, FT_Error> {
    if buffer.is_empty() {
        return Err(FT_Err_Invalid_Argument);
    }
    // FreeType `FT_Get_Glyph_Name` in `src/base/ftobjs.c` clears the first
    // output byte before invalid-glyph and no-glyph-name service failures.
    buffer[0] = 0;
    if glyph_index >= FT_UInt::from(face.inner.info().num_glyphs) {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    if (face.inner.info().face_flags & (1 << 9)) == 0 {
        return Err(FT_Err_Invalid_Argument);
    }
    let Some(name) = face.inner.glyph_name(glyph_index) else {
        return Err(FT_Err_Invalid_Argument);
    };
    let bytes = name.as_bytes();
    let copy_len = bytes.len().min(buffer.len().saturating_sub(1));
    buffer[..copy_len].copy_from_slice(&bytes[..copy_len]);
    buffer[copy_len] = 0;
    Ok(copy_len)
}

pub fn FT_Get_Name_Index(face: Option<&FT_Face>, glyph_name: Option<&str>) -> FT_UInt {
    let (Some(face), Some(glyph_name)) = (face, glyph_name) else {
        return 0;
    };
    if (face.inner.info().face_flags & (1 << 9)) == 0 {
        return 0;
    }
    face.inner.name_index(glyph_name)
}

pub fn FT_Get_Postscript_Name(face: &FT_Face) -> Option<&str> {
    face.inner.postscript_name()
}

pub fn FT_Set_Named_Instance(face: Option<&mut FT_Face>, instance_index: FT_UInt) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(instance_index) = usize::try_from(instance_index) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    match face.inner.set_named_instance(instance_index) {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Get_CMap_Format(charmap: FT_CharMap) -> FT_Long {
    registered_charmap_metadata(charmap).map_or(-1, |(format, _, _)| format)
}

pub fn FT_Get_CMap_Language_ID(charmap: FT_CharMap) -> FT_ULong {
    registered_charmap_metadata(charmap).map_or(0, |(_, language_id, _)| language_id)
}

pub fn FT_New_Face(
    _library: &FT_Library,
    _pathname: &str,
    _face_index: FT_Long,
    _size_pt: f32,
) -> Result<FT_Face, FT_Error> {
    // The binding crate (ffi-c, ffi-wasm) should handle file I/O
    // and call FT_New_Memory_Face. The core stub returns Cannot_Open_Resource.
    Err(FT_Err_Cannot_Open_Resource as FT_Error)
}

pub fn FT_New_Memory_Face(
    library: &FT_Library,
    data: &[u8],
    face_index: FT_Long,
    size_pt: f32,
) -> Result<FT_Face, FT_Error> {
    let (face_index, probe_only) = c_face_index_to_core(face_index)?;
    library
        .inner
        .new_memory_face(data, face_index, size_pt)
        .map(|inner| face_to_ffi(inner, probe_only))
        .map_err(error_to_ft)
}

fn face_to_ffi(inner: api::Face, probe_only: bool) -> FT_Face {
    let font = inner.font();
    let sfnt_os2 = font.os2_table().map(os2_to_ffi).map(Box::new);
    let sfnt_head = font
        .load_sfnt_table(0x68656164, 0, None)
        .ok()
        .and_then(|data| parse_tt_header(&data))
        .map(Box::new);
    let sfnt_maxp = font
        .load_sfnt_table(0x6D617870, 0, None)
        .ok()
        .and_then(|data| parse_tt_maxprofile(&data))
        .map(Box::new);
    let sfnt_hhea = font
        .load_sfnt_table(0x68686561, 0, None)
        .ok()
        .and_then(|data| parse_tt_horiheader(&data))
        .map(Box::new);
    let sfnt_vhea = font
        .load_sfnt_table(0x76686561, 0, None)
        .ok()
        .and_then(|data| parse_tt_vertheader(&data))
        .map(Box::new);
    let sfnt_post = font
        .load_sfnt_table(0x706F7374, 0, None)
        .ok()
        .and_then(|data| parse_tt_postscript(&data))
        .map(Box::new);
    let sfnt_pclt = font
        .load_sfnt_table(0x50434C54, 0, None)
        .ok()
        .and_then(|data| parse_tt_pclt(&data))
        .map(Box::new);
    let charmaps = charmaps_to_ffi(&inner);
    FT_Face {
        inner,
        probe_only,
        sfnt_os2,
        sfnt_head,
        sfnt_maxp,
        sfnt_hhea,
        sfnt_vhea,
        sfnt_post,
        sfnt_pclt,
        charmaps,
        transform_matrix: FT_Matrix {
            xx: 1 << 16,
            xy: 0,
            yx: 0,
            yy: 1 << 16,
        },
        transform_delta: FT_Vector { x: 0, y: 0 },
        refcount: 1,
    }
}

fn parse_tt_header(data: &[u8]) -> Option<TT_Header> {
    if data.len() < 54 {
        return None;
    }
    Some(TT_Header {
        Table_Version: i64::from(i32::from_be_bytes([data[0], data[1], data[2], data[3]])),
        Font_Revision: i64::from(i32::from_be_bytes([data[4], data[5], data[6], data[7]])),
        CheckSum_Adjust: i64::from(i32::from_be_bytes([data[8], data[9], data[10], data[11]])),
        Magic_Number: i64::from(i32::from_be_bytes([data[12], data[13], data[14], data[15]])),
        Flags: u16::from_be_bytes([data[16], data[17]]) as FT_UShort,
        Units_Per_EM: u16::from_be_bytes([data[18], data[19]]) as FT_UShort,
        Created: [
            u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as FT_ULong,
            u32::from_be_bytes([data[24], data[25], data[26], data[27]]) as FT_ULong,
        ],
        Modified: [
            u32::from_be_bytes([data[28], data[29], data[30], data[31]]) as FT_ULong,
            u32::from_be_bytes([data[32], data[33], data[34], data[35]]) as FT_ULong,
        ],
        xMin: i16::from_be_bytes([data[36], data[37]]),
        yMin: i16::from_be_bytes([data[38], data[39]]),
        xMax: i16::from_be_bytes([data[40], data[41]]),
        yMax: i16::from_be_bytes([data[42], data[43]]),
        Mac_Style: u16::from_be_bytes([data[44], data[45]]) as FT_UShort,
        Lowest_Rec_PPEM: u16::from_be_bytes([data[46], data[47]]) as FT_UShort,
        Font_Direction: i16::from_be_bytes([data[48], data[49]]),
        Index_To_Loc_Format: i16::from_be_bytes([data[50], data[51]]),
        Glyph_Data_Format: i16::from_be_bytes([data[52], data[53]]),
    })
}

fn parse_tt_maxprofile(data: &[u8]) -> Option<TT_MaxProfile> {
    if data.len() < 6 {
        return None;
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    Some(TT_MaxProfile {
        version: i64::from(version as i32),
        numGlyphs: u16::from_be_bytes([data[4], data[5]]) as FT_UShort,
        maxPoints: data
            .get(6..8)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxContours: data
            .get(8..10)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxCompositePoints: data
            .get(10..12)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxCompositeContours: data
            .get(12..14)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxZones: data
            .get(14..16)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxTwilightPoints: data
            .get(16..18)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxStorage: data
            .get(18..20)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxFunctionDefs: data
            .get(20..22)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxInstructionDefs: data
            .get(22..24)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxStackElements: data
            .get(24..26)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]])) as FT_UShort,
        maxSizeOfInstructions: data
            .get(26..28)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxComponentElements: data
            .get(28..30)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
        maxComponentDepth: data
            .get(30..32)
            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
            as FT_UShort,
    })
}

fn parse_tt_horiheader(data: &[u8]) -> Option<TT_HoriHeader> {
    if data.len() < 36 {
        return None;
    }
    Some(TT_HoriHeader {
        Version: i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as FT_Fixed,
        Ascender: i16::from_be_bytes([data[4], data[5]]),
        Descender: i16::from_be_bytes([data[6], data[7]]),
        Line_Gap: i16::from_be_bytes([data[8], data[9]]),
        advance_Width_Max: u16::from_be_bytes([data[10], data[11]]) as FT_UShort,
        min_Left_Side_Bearing: i16::from_be_bytes([data[12], data[13]]),
        min_Right_Side_Bearing: i16::from_be_bytes([data[14], data[15]]),
        xMax_Extent: i16::from_be_bytes([data[16], data[17]]),
        caret_Slope_Rise: i16::from_be_bytes([data[18], data[19]]),
        caret_Slope_Run: i16::from_be_bytes([data[20], data[21]]),
        caret_Offset: i16::from_be_bytes([data[22], data[23]]),
        Reserved: [
            i16::from_be_bytes([data[24], data[25]]),
            i16::from_be_bytes([data[26], data[27]]),
            i16::from_be_bytes([data[28], data[29]]),
            i16::from_be_bytes([data[30], data[31]]),
        ],
        metric_Data_Format: i16::from_be_bytes([data[32], data[33]]),
        number_Of_HMetrics: u16::from_be_bytes([data[34], data[35]]) as FT_UShort,
        long_metrics: ptr::null_mut(),
        short_metrics: ptr::null_mut(),
    })
}

fn parse_tt_vertheader(data: &[u8]) -> Option<TT_VertHeader> {
    if data.len() < 36 {
        return None;
    }
    Some(TT_VertHeader {
        Version: i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as FT_Fixed,
        Ascender: i16::from_be_bytes([data[4], data[5]]),
        Descender: i16::from_be_bytes([data[6], data[7]]),
        Line_Gap: i16::from_be_bytes([data[8], data[9]]),
        advance_Height_Max: u16::from_be_bytes([data[10], data[11]]) as FT_UShort,
        min_Top_Side_Bearing: i16::from_be_bytes([data[12], data[13]]),
        min_Bottom_Side_Bearing: i16::from_be_bytes([data[14], data[15]]),
        yMax_Extent: i16::from_be_bytes([data[16], data[17]]),
        caret_Slope_Rise: i16::from_be_bytes([data[18], data[19]]),
        caret_Slope_Run: i16::from_be_bytes([data[20], data[21]]),
        caret_Offset: i16::from_be_bytes([data[22], data[23]]),
        Reserved: [
            i16::from_be_bytes([data[24], data[25]]),
            i16::from_be_bytes([data[26], data[27]]),
            i16::from_be_bytes([data[28], data[29]]),
            i16::from_be_bytes([data[30], data[31]]),
        ],
        metric_Data_Format: i16::from_be_bytes([data[32], data[33]]),
        number_Of_VMetrics: u16::from_be_bytes([data[34], data[35]]) as FT_UShort,
        long_metrics: ptr::null_mut(),
        short_metrics: ptr::null_mut(),
    })
}

fn parse_tt_postscript(data: &[u8]) -> Option<TT_Postscript> {
    if data.len() < 32 {
        return None;
    }
    Some(TT_Postscript {
        FormatType: i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as FT_Fixed,
        italicAngle: i32::from_be_bytes([data[4], data[5], data[6], data[7]]) as FT_Fixed,
        underlinePosition: i16::from_be_bytes([data[8], data[9]]),
        underlineThickness: i16::from_be_bytes([data[10], data[11]]),
        isFixedPitch: u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as FT_ULong,
        minMemType42: u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as FT_ULong,
        maxMemType42: u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as FT_ULong,
        minMemType1: u32::from_be_bytes([data[24], data[25], data[26], data[27]]) as FT_ULong,
        maxMemType1: u32::from_be_bytes([data[28], data[29], data[30], data[31]]) as FT_ULong,
    })
}

fn parse_tt_pclt(data: &[u8]) -> Option<TT_PCLT> {
    if data.len() < 54 {
        return None;
    }
    Some(TT_PCLT {
        Version: i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as FT_Fixed,
        FontNumber: u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as FT_ULong,
        Pitch: u16::from_be_bytes([data[8], data[9]]) as FT_UShort,
        xHeight: u16::from_be_bytes([data[10], data[11]]) as FT_UShort,
        Style: u16::from_be_bytes([data[12], data[13]]) as FT_UShort,
        TypeFamily: u16::from_be_bytes([data[14], data[15]]) as FT_UShort,
        CapHeight: u16::from_be_bytes([data[16], data[17]]) as FT_UShort,
        SymbolSet: u16::from_be_bytes([data[18], data[19]]) as FT_UShort,
        TypeFace: [
            data[20] as FT_Char,
            data[21] as FT_Char,
            data[22] as FT_Char,
            data[23] as FT_Char,
            data[24] as FT_Char,
            data[25] as FT_Char,
            data[26] as FT_Char,
            data[27] as FT_Char,
            data[28] as FT_Char,
            data[29] as FT_Char,
            data[30] as FT_Char,
            data[31] as FT_Char,
            data[32] as FT_Char,
            data[33] as FT_Char,
            data[34] as FT_Char,
            data[35] as FT_Char,
        ],
        CharacterComplement: [
            data[36] as FT_Char,
            data[37] as FT_Char,
            data[38] as FT_Char,
            data[39] as FT_Char,
            data[40] as FT_Char,
            data[41] as FT_Char,
            data[42] as FT_Char,
            data[43] as FT_Char,
        ],
        FileName: [
            data[44] as FT_Char,
            data[45] as FT_Char,
            data[46] as FT_Char,
            data[47] as FT_Char,
            data[48] as FT_Char,
            data[49] as FT_Char,
        ],
        StrokeWeight: data[50] as FT_Char,
        WidthType: data[51] as FT_Char,
        SerifStyle: data[52],
        Reserved: data[53],
    })
}

fn charmaps_to_ffi(face: &api::Face) -> Box<[FT_CharMapRecInternal]> {
    let infos = face.font().charmaps();
    let charmaps = infos
        .iter()
        .map(|info| FT_CharMapRecInternal {
            public: FT_CharMapRecPublic {
                face: ptr::null_mut(),
                encoding: charmap_encoding(info.platform_id, info.encoding_id),
                platform_id: info.platform_id,
                encoding_id: info.encoding_id,
            },
            format: FT_Long::from(info.format),
            language_id: FT_ULong::from(info.language_id),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    register_charmap_metadata(&charmaps);
    charmaps
}

fn charmap_metadata_registry() -> &'static Mutex<CharmapMetadataRegistry> {
    static REGISTRY: OnceLock<Mutex<CharmapMetadataRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_charmap_metadata(charmaps: &[FT_CharMapRecInternal]) {
    if let Ok(mut registry) = charmap_metadata_registry().lock() {
        for (index, record) in charmaps.iter().enumerate() {
            register_charmap_record_locked(&mut registry, index, record);
        }
    }
}

fn register_charmap_record(index: usize, record: &FT_CharMapRecInternal) {
    if let Ok(mut registry) = charmap_metadata_registry().lock() {
        register_charmap_record_locked(&mut registry, index, record);
    }
}

fn register_charmap_record_locked(
    registry: &mut CharmapMetadataRegistry,
    index: usize,
    record: &FT_CharMapRecInternal,
) {
    let key = (&record.public as *const FT_CharMapRecPublic) as usize;
    registry.insert(key, (record.format, record.language_id, index as FT_Int));
}

fn registered_charmap_metadata(charmap: FT_CharMap) -> Option<CharmapMetadata> {
    if charmap.is_null() {
        return None;
    }
    let key = charmap.cast_const().cast::<FT_CharMapRecPublic>() as usize;
    charmap_metadata_registry()
        .lock()
        .ok()
        .and_then(|registry| registry.get(&key).copied())
}

fn charmap_encoding(platform_id: FT_UShort, encoding_id: FT_UShort) -> FT_Encoding {
    match (platform_id, encoding_id) {
        (TT_PLATFORM_ISO_U16, _) | (TT_PLATFORM_APPLE_UNICODE_U16, _) => {
            FT_ENCODING_UNICODE as FT_Encoding
        }
        (TT_PLATFORM_MACINTOSH_U16, TT_MAC_ID_ROMAN_U16) => FT_ENCODING_APPLE_ROMAN as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_SYMBOL_CS_U16) => FT_ENCODING_MS_SYMBOL as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_UCS_4_U16 | TT_MS_ID_UNICODE_CS_U16) => {
            FT_ENCODING_UNICODE as FT_Encoding
        }
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_SJIS_U16) => FT_ENCODING_SJIS as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_PRC_U16) => FT_ENCODING_PRC as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_BIG_5_U16) => FT_ENCODING_BIG5 as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_WANSUNG_U16) => FT_ENCODING_WANSUNG as FT_Encoding,
        (TT_PLATFORM_MICROSOFT_U16, TT_MS_ID_JOHAB_U16) => FT_ENCODING_JOHAB as FT_Encoding,
        _ => FT_ENCODING_NONE as FT_Encoding,
    }
}

const TT_PLATFORM_APPLE_UNICODE_U16: FT_UShort = TT_PLATFORM_APPLE_UNICODE as FT_UShort;
const TT_PLATFORM_MACINTOSH_U16: FT_UShort = TT_PLATFORM_MACINTOSH as FT_UShort;
const TT_PLATFORM_ISO_U16: FT_UShort = TT_PLATFORM_ISO as FT_UShort;
const TT_PLATFORM_MICROSOFT_U16: FT_UShort = TT_PLATFORM_MICROSOFT as FT_UShort;
const TT_MAC_ID_ROMAN_U16: FT_UShort = TT_MAC_ID_ROMAN as FT_UShort;
const TT_MS_ID_SYMBOL_CS_U16: FT_UShort = TT_MS_ID_SYMBOL_CS as FT_UShort;
const TT_MS_ID_UNICODE_CS_U16: FT_UShort = TT_MS_ID_UNICODE_CS as FT_UShort;
const TT_MS_ID_SJIS_U16: FT_UShort = TT_MS_ID_SJIS as FT_UShort;
const TT_MS_ID_PRC_U16: FT_UShort = TT_MS_ID_PRC as FT_UShort;
const TT_MS_ID_BIG_5_U16: FT_UShort = TT_MS_ID_BIG_5 as FT_UShort;
const TT_MS_ID_WANSUNG_U16: FT_UShort = TT_MS_ID_WANSUNG as FT_UShort;
const TT_MS_ID_JOHAB_U16: FT_UShort = TT_MS_ID_JOHAB as FT_UShort;
const TT_MS_ID_UCS_4_U16: FT_UShort = TT_MS_ID_UCS_4 as FT_UShort;

fn os2_to_ffi(os2: &crate::tt::os2::Os2Table) -> TT_OS2 {
    TT_OS2 {
        version: os2.version as FT_UShort,
        xAvgCharWidth: os2.x_avg_char_width,
        usWeightClass: os2.us_weight_class as FT_UShort,
        usWidthClass: os2.us_width_class as FT_UShort,
        fsType: os2.fs_type as FT_UShort,
        ySubscriptXSize: os2.y_subscript_x_size,
        ySubscriptYSize: os2.y_subscript_y_size,
        ySubscriptXOffset: os2.y_subscript_x_offset,
        ySubscriptYOffset: os2.y_subscript_y_offset,
        ySuperscriptXSize: os2.y_superscript_x_size,
        ySuperscriptYSize: os2.y_superscript_y_size,
        ySuperscriptXOffset: os2.y_superscript_x_offset,
        ySuperscriptYOffset: os2.y_superscript_y_offset,
        yStrikeoutSize: os2.y_strikeout_size,
        yStrikeoutPosition: os2.y_strikeout_position,
        sFamilyClass: os2.s_family_class,
        panose: os2.panose,
        ulUnicodeRange1: FT_ULong::from(os2.ul_unicode_range1),
        ulUnicodeRange2: FT_ULong::from(os2.ul_unicode_range2),
        ulUnicodeRange3: FT_ULong::from(os2.ul_unicode_range3),
        ulUnicodeRange4: FT_ULong::from(os2.ul_unicode_range4),
        achVendID: os2.ach_vend_id.map(|value| value as FT_Char),
        fsSelection: os2.fs_selection(),
        usFirstCharIndex: os2.us_first_char_index as FT_UShort,
        usLastCharIndex: os2.us_last_char_index as FT_UShort,
        sTypoAscender: os2.s_typo_ascender,
        sTypoDescender: os2.s_typo_descender,
        sTypoLineGap: os2.s_typo_line_gap,
        usWinAscent: os2.us_win_ascent as FT_UShort,
        usWinDescent: os2.us_win_descent as FT_UShort,
        ulCodePageRange1: FT_ULong::from(os2.ul_code_page_range1),
        ulCodePageRange2: FT_ULong::from(os2.ul_code_page_range2),
        sxHeight: os2.sx_height,
        sCapHeight: os2.s_cap_height,
        usDefaultChar: os2.us_default_char as FT_UShort,
        usBreakChar: os2.us_break_char as FT_UShort,
        usMaxContext: os2.us_max_context as FT_UShort,
        usLowerOpticalPointSize: os2.us_lower_optical_point_size as FT_UShort,
        usUpperOpticalPointSize: os2.us_upper_optical_point_size as FT_UShort,
    }
}

pub fn FT_Set_Char_Size(
    face: &mut FT_Face,
    char_width: FT_F26Dot6,
    char_height: FT_F26Dot6,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    if face.probe_only {
        return FT_Err_Invalid_Size_Handle;
    }
    let Ok(char_width) = i32::try_from(char_width) else {
        // FreeType reaches FT_Request_Metrics and reports Invalid_Pixel_Size
        // for host-width dimensions that produce an oversized ppem.
        // See freetype/src/base/ftobjs.c:3355-3356.
        return FT_Err_Invalid_Pixel_Size;
    };
    let Ok(char_height) = i32::try_from(char_height) else {
        return FT_Err_Invalid_Pixel_Size;
    };
    match face
        .inner
        .try_set_char_size(char_width, char_height, horz_resolution, vert_resolution)
    {
        Ok(()) => FT_Err_Ok,
        Err(SizeRequestError::DivideByZero) => FT_Err_Divide_By_Zero as FT_Error,
        Err(SizeRequestError::InvalidPixelSize) => FT_Err_Invalid_Pixel_Size,
    }
}

pub fn FT_Set_Pixel_Sizes(
    face: &mut FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    if face.probe_only {
        return FT_Err_Invalid_Size_Handle;
    }
    face.inner.set_pixel_sizes(pixel_width, pixel_height);
    FT_Err_Ok
}

pub fn FT_Request_Size(face: Option<&mut FT_Face>, req: Option<&FT_Size_RequestRec>) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if face.probe_only {
        return FT_Err_Invalid_Size_Handle;
    }
    let Some(req) = req else {
        return FT_Err_Invalid_Argument;
    };
    if req.width < 0 || req.height < 0 {
        return FT_Err_Invalid_Argument;
    }
    let request_type = match i64::from(req.type_) {
        FT_SIZE_REQUEST_TYPE_NOMINAL => SizeRequestType::Nominal,
        FT_SIZE_REQUEST_TYPE_REAL_DIM => SizeRequestType::RealDim,
        FT_SIZE_REQUEST_TYPE_BBOX => SizeRequestType::BBox,
        FT_SIZE_REQUEST_TYPE_CELL => SizeRequestType::Cell,
        FT_SIZE_REQUEST_TYPE_SCALES => SizeRequestType::Scales,
        _ => return FT_Err_Invalid_Argument,
    };
    let request = SizeRequest {
        request_type,
        width: req.width,
        height: req.height,
        hori_resolution: req.horiResolution,
        vert_resolution: req.vertResolution,
    };
    match face.inner.request_size(request) {
        Ok(()) => FT_Err_Ok,
        Err(SizeRequestError::DivideByZero) => FT_Err_Divide_By_Zero as FT_Error,
        Err(SizeRequestError::InvalidPixelSize) => FT_Err_Invalid_Pixel_Size,
    }
}

pub fn FT_Get_Char_Index(face: &FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Ok(char_code) = u32::try_from(char_code) else {
        return 0;
    };
    u32::from(face.inner.get_char_index(char_code))
}

pub fn FT_Get_Kerning(
    face: Option<&FT_Face>,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    akerning: Option<&mut FT_Vector>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(akerning) = akerning else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    akerning.x = 0;
    akerning.y = 0;
    let mode = match kern_mode {
        mode if mode == FT_KERNING_UNFITTED as FT_UInt => KerningMode::Unfitted,
        mode if mode == FT_KERNING_UNSCALED as FT_UInt => KerningMode::Unscaled,
        _ => KerningMode::Default,
    };
    let vector = face.inner.kerning_by_glyphs(left_glyph, right_glyph, mode);
    akerning.x = FT_Long::from(vector.x);
    akerning.y = FT_Long::from(vector.y);
    FT_Err_Ok
}

pub fn FT_Face_Charmap_Count(face: &FT_Face) -> FT_UInt {
    FT_UInt::try_from(face.charmaps.len()).unwrap_or(FT_UInt::MAX)
}

pub fn FT_Face_Charmap(face: &FT_Face, index: FT_UInt) -> FT_CharMap {
    let Ok(index) = usize::try_from(index) else {
        return ptr::null_mut();
    };
    face.charmaps.get(index).map_or(ptr::null_mut(), |record| {
        register_charmap_record(index, record);
        (&record.public as *const FT_CharMapRecPublic)
            .cast_mut()
            .cast()
    })
}

pub fn FT_Face_Charmap_Info(face: &FT_Face, index: FT_UInt) -> Option<FT_CharMapRecPublic> {
    let index = usize::try_from(index).ok()?;
    face.charmaps.get(index).map(|record| record.public)
}

pub fn FT_Face_Active_Charmap_Index(face: &FT_Face) -> FT_Int {
    face.inner
        .charmap_index()
        .and_then(|index| FT_Int::try_from(index).ok())
        .unwrap_or(-1)
}

pub fn FT_Charmap_Info(face: &FT_Face, charmap: FT_CharMap) -> Option<FT_CharMapRecPublic> {
    let index = charmap_index_in_face(face, charmap)?;
    face.charmaps.get(index).map(|record| record.public)
}

pub fn FT_Charmap_Format(face: &FT_Face, charmap: FT_CharMap) -> Option<FT_Long> {
    let index = charmap_index_in_face(face, charmap)?;
    face.charmaps.get(index).map(|record| record.format)
}

pub fn FT_Charmap_Language_ID(face: &FT_Face, charmap: FT_CharMap) -> Option<FT_ULong> {
    let index = charmap_index_in_face(face, charmap)?;
    face.charmaps.get(index).map(|record| record.language_id)
}

pub fn FT_Get_Charmap_Index(charmap: FT_CharMap) -> FT_Int {
    registered_charmap_metadata(charmap).map_or(-1, |(_, _, index)| index)
}

pub fn FT_Get_Charmap_Index_For_Face(face: &FT_Face, charmap: FT_CharMap) -> FT_Int {
    charmap_index_in_face(face, charmap)
        .and_then(|index| FT_Int::try_from(index).ok())
        .unwrap_or(-1)
}

pub fn FT_Set_Charmap(face: Option<&mut FT_Face>, charmap: FT_CharMap) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if face.charmaps.is_empty() || charmap.is_null() {
        return FT_Err_Invalid_CharMap_Handle as FT_Error;
    }
    let Some(index) = charmap_index_in_face(face, charmap) else {
        return FT_Err_Invalid_Argument;
    };
    if face.charmaps.get(index).map(|record| record.format) == Some(14) {
        return FT_Err_Invalid_Argument;
    }
    match face.inner.set_charmap(index) {
        Ok(()) => FT_Err_Ok,
        Err(_) => FT_Err_Invalid_Argument,
    }
}

fn charmap_index_in_face(face: &FT_Face, charmap: FT_CharMap) -> Option<usize> {
    if charmap.is_null() {
        return None;
    }
    let target = charmap.cast_const().cast::<FT_CharMapRecPublic>();
    face.charmaps
        .iter()
        .position(|record| ptr::eq(&record.public as *const FT_CharMapRecPublic, target))
}

pub fn FT_Select_Charmap(face: Option<&mut FT_Face>, encoding: FT_Encoding) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match i64::from(encoding) {
        FT_ENCODING_UNICODE => match face.inner.select_unicode_charmap() {
            Ok(()) => FT_Err_Ok,
            Err(_) => FT_Err_Invalid_Argument,
        },
        _ => {
            let Some(index) = face
                .charmaps
                .iter()
                .position(|charmap| charmap.public.encoding == encoding)
            else {
                return FT_Err_Invalid_Argument;
            };
            match face.inner.set_charmap(index) {
                Ok(()) => FT_Err_Ok,
                Err(_) => FT_Err_Invalid_Argument,
            }
        }
    }
}

pub fn FT_Get_FSType_Flags(face: Option<&FT_Face>) -> FT_UShort {
    face.map_or(0, |face| face.inner.get_fstype_flags())
}

pub fn FT_Get_Sfnt_Name_Count(face: Option<&FT_Face>) -> FT_UInt {
    face.map_or(0, |face| {
        FT_UInt::try_from(face.inner.sfnt_name_count()).unwrap_or(FT_UInt::MAX)
    })
}

pub fn FT_Get_Sfnt_Name(
    face: Option<&FT_Face>,
    idx: FT_UInt,
    aname: Option<&mut FT_SfntName>,
) -> FT_Error {
    let Some(aname) = aname else {
        return FT_Err_Invalid_Argument;
    };
    let Some(face) = face else {
        return FT_Err_Invalid_Argument;
    };
    let Some(record) = face.inner.sfnt_name(idx as usize) else {
        return FT_Err_Invalid_Argument;
    };
    aname.platform_id = record.platform_id;
    aname.encoding_id = record.encoding_id;
    aname.language_id = record.language_id;
    aname.name_id = record.name_id;
    aname.string = record.string.as_ptr().cast_mut().cast::<FT_Byte>();
    aname.string_len = FT_UInt::try_from(record.string.len()).unwrap_or(FT_UInt::MAX);
    FT_Err_Ok
}

pub fn FT_Get_First_Char(face: Option<&FT_Face>, agindex: Option<&mut FT_UInt>) -> FT_ULong {
    let mut glyph_index = 0;
    let mut char_code = 0;
    if let Some(face) = face {
        if let Some((code, glyph)) = face.inner.first_char() {
            char_code = FT_ULong::from(code);
            glyph_index = FT_UInt::from(glyph);
        }
    }
    if let Some(out) = agindex {
        *out = glyph_index;
    }
    char_code
}

pub fn FT_Get_Next_Char(
    face: Option<&FT_Face>,
    char_code: FT_ULong,
    agindex: Option<&mut FT_UInt>,
) -> FT_ULong {
    let mut glyph_index = 0;
    let mut next_char = 0;
    if let Some(face) = face {
        if let Some((code, glyph)) = face.inner.next_char(char_code as u32) {
            next_char = FT_ULong::from(code);
            glyph_index = FT_UInt::from(glyph);
        }
    }
    if let Some(out) = agindex {
        *out = glyph_index;
    }
    next_char
}

pub fn FT_Library_Version(
    library: Option<&FT_Library>,
    amajor: Option<&mut FT_Int>,
    aminor: Option<&mut FT_Int>,
    apatch: Option<&mut FT_Int>,
) {
    // FreeType 2.14.3 `FT_Library_Version` writes zeroes for a null library;
    // the pinned oracle build reports 2.14.3 for a live default library.
    let (major, minor, patch) = if library.is_some() {
        (2, 14, 3)
    } else {
        (0, 0, 0)
    };
    if let Some(out) = amajor {
        *out = major;
    }
    if let Some(out) = aminor {
        *out = minor;
    }
    if let Some(out) = apatch {
        *out = patch;
    }
}

pub fn FT_Load_Char(
    face: &FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> Result<FT_GlyphSlot, FT_Error> {
    FT_Load_Glyph(face, FT_Get_Char_Index(face, char_code), load_flags)
}

pub fn FT_Load_Glyph(
    face: &FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> Result<FT_GlyphSlot, FT_Error> {
    if face.probe_only {
        return Err(FT_Err_Invalid_Size_Handle);
    }
    let glyph_index = u16::try_from(glyph_index).map_err(|_| FT_Err_Invalid_Glyph_Index)?;
    let flags = load_flags_to_core(load_flags)?;
    if glyph_index >= face.inner.info().num_glyphs {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    let transform = if load_flags & FT_LOAD_IGNORE_TRANSFORM != 0 {
        None
    } else if face.transform_matrix.xx != 1 << 16
        || face.transform_matrix.xy != 0
        || face.transform_matrix.yx != 0
        || face.transform_matrix.yy != 1 << 16
        || face.transform_delta.x != 0
        || face.transform_delta.y != 0
    {
        Some((
            face.transform_matrix.xx as i32,
            face.transform_matrix.xy as i32,
            face.transform_matrix.yx as i32,
            face.transform_matrix.yy as i32,
            face.transform_delta.x as i32,
            face.transform_delta.y as i32,
        ))
    } else {
        None
    };
    face.inner
        .load_glyph_with_transform(glyph_index, flags, transform)
        .map(|slot| slot_to_ffi(face, slot, flags))
        .map_err(error_to_ft)
}

pub fn FT_Get_Advance(
    face: &FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> Result<FT_Fixed, FT_Error> {
    if face.probe_only {
        return Err(FT_Err_Invalid_Size_Handle);
    }
    let fast_only = load_flags & FT_ADVANCE_FLAG_FAST_ONLY_I32 != 0;
    let load_flags = load_flags & !FT_ADVANCE_FLAG_FAST_ONLY_I32;
    if fast_only && !advance_fast_path_supported(load_flags) {
        return Err(FT_Err_Unimplemented_Feature);
    }
    let glyph_index = u16::try_from(glyph_index).map_err(|_| FT_Err_Invalid_Glyph_Index)?;
    let flags = load_flags_to_core(load_flags)?;
    // Match the same driver-side glyph index guard used by `FT_Load_Glyph`.
    if glyph_index >= face.inner.info().num_glyphs {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    if use_fast_horizontal_advance(flags) {
        // C `tt_get_advances` returns raw hmtx advances; `ft_face_scale_advances_`
        // scales them directly to 16.16 with `FT_MulFix(1024 * advance, x_scale)`.
        return Ok(FT_Fixed::from(
            face.inner.glyph_hori_advance_16dot16(glyph_index),
        ));
    }
    let slot = face
        .inner
        .load_glyph(glyph_index, flags)
        .map_err(error_to_ft)?;
    let advance = if flags.contains(api::LoadFlags::VERTICAL_LAYOUT) {
        slot.advance.y
    } else {
        slot.advance.x
    };
    if flags.contains(api::LoadFlags::NO_SCALE) {
        Ok(FT_Fixed::from(advance))
    } else {
        Ok(FT_Fixed::from(advance) << 10)
    }
}

pub fn FT_Get_Advances(
    face: &FT_Face,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
) -> Result<Vec<FT_Fixed>, FT_Error> {
    let count_usize = usize::try_from(count).map_err(|_| FT_Err_Invalid_Argument)?;
    let mut advances = Vec::with_capacity(count_usize);
    for offset in 0..count {
        let glyph_index = start
            .checked_add(offset)
            .ok_or(FT_Err_Invalid_Glyph_Index)?;
        advances.push(FT_Get_Advance(face, glyph_index, load_flags)?);
    }
    Ok(advances)
}

pub fn FT_Render_Glyph(
    slot: FT_GlyphSlot,
    render_mode: FT_Render_Mode,
) -> Result<FT_GlyphSlot, FT_Error> {
    let mode = render_mode_to_core(render_mode).ok_or(FT_Err_Cannot_Render_Glyph)?;
    if slot.format == FT_GLYPH_FORMAT_BITMAP {
        return Ok(slot);
    }
    let source_face = slot.source_face.clone();
    let load_flags = slot.load_flags;
    slot.core_slot
        .render(mode)
        .map(|rendered| {
            let render_flags =
                load_flags | api::LoadFlags::RENDER | load_flag_for_render_mode(mode);
            slot_to_ffi(&face_to_ffi(source_face, false), rendered, render_flags)
        })
        .map_err(error_to_ft)
}

pub fn FT_Size_Metrics(face: &FT_Face) -> FT_Size_MetricsRec {
    face.inner.size_metrics().into()
}

#[inline]
fn ft_long_to_i64(value: FT_Long) -> i64 {
    // `FT_Long` is `i64` on this host and can be narrower on other targets.
    // Keep the conversion explicit at the FFI/core boundary without making
    // host-specific type aliases leak into the fixed-math helpers.
    #[allow(clippy::useless_conversion)]
    {
        i64::from(value)
    }
}

pub fn FT_MulDiv(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    crate::fixed::ft_mul_div_long(ft_long_to_i64(a), ft_long_to_i64(b), ft_long_to_i64(c))
        as FT_Long
}

pub fn FT_MulFix(a: FT_Long, b: FT_Long) -> FT_Long {
    crate::fixed::ft_mul_fix_long(ft_long_to_i64(a), ft_long_to_i64(b)) as FT_Long
}

pub fn FT_DivFix(a: FT_Long, b: FT_Long) -> FT_Long {
    crate::fixed::ft_div_fix_long(ft_long_to_i64(a), ft_long_to_i64(b)) as FT_Long
}

pub fn FT_RoundFix(a: FT_Fixed) -> FT_Fixed {
    crate::fixed::ft_round_fix(ft_long_to_i64(a)) as FT_Fixed
}

pub fn FT_CeilFix(a: FT_Fixed) -> FT_Fixed {
    crate::fixed::ft_ceil_fix(ft_long_to_i64(a)) as FT_Fixed
}

pub fn FT_FloorFix(a: FT_Fixed) -> FT_Fixed {
    crate::fixed::ft_floor_fix(ft_long_to_i64(a)) as FT_Fixed
}

pub fn FT_Vector_Transform(vector: Option<&mut FT_Vector>, matrix: Option<&FT_Matrix>) {
    let (Some(vector), Some(matrix)) = (vector, matrix) else {
        return;
    };
    let xz = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(vector.x, matrix.xx)),
        ft_long_to_i64(FT_MulFix(vector.y, matrix.xy)),
    ) as FT_Pos;
    let yz = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(vector.x, matrix.yx)),
        ft_long_to_i64(FT_MulFix(vector.y, matrix.yy)),
    ) as FT_Pos;
    vector.x = xz;
    vector.y = yz;
}

pub fn FT_Matrix_Multiply(a: Option<&FT_Matrix>, b: Option<&mut FT_Matrix>) {
    let (Some(a), Some(b)) = (a, b) else {
        return;
    };
    let xx = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(a.xx, b.xx)),
        ft_long_to_i64(FT_MulFix(a.xy, b.yx)),
    ) as FT_Fixed;
    let xy = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(a.xx, b.xy)),
        ft_long_to_i64(FT_MulFix(a.xy, b.yy)),
    ) as FT_Fixed;
    let yx = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(a.yx, b.xx)),
        ft_long_to_i64(FT_MulFix(a.yy, b.yx)),
    ) as FT_Fixed;
    let yy = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(a.yx, b.xy)),
        ft_long_to_i64(FT_MulFix(a.yy, b.yy)),
    ) as FT_Fixed;
    b.xx = xx;
    b.xy = xy;
    b.yx = yx;
    b.yy = yy;
}

pub fn FT_Matrix_Invert(matrix: Option<&mut FT_Matrix>) -> FT_Error {
    let Some(matrix) = matrix else {
        return FT_Err_Invalid_Argument;
    };
    let delta = crate::fixed::ft_add_long(
        ft_long_to_i64(FT_MulFix(matrix.xx, matrix.yy)),
        crate::fixed::ft_neg_long(ft_long_to_i64(FT_MulFix(matrix.xy, matrix.yx))),
    ) as FT_Fixed;
    if delta == 0 {
        return FT_Err_Invalid_Argument;
    }
    let xx = matrix.xx;
    let yy = matrix.yy;
    matrix.xy = crate::fixed::ft_neg_long(ft_long_to_i64(FT_DivFix(matrix.xy, delta))) as FT_Fixed;
    matrix.yx = crate::fixed::ft_neg_long(ft_long_to_i64(FT_DivFix(matrix.yx, delta))) as FT_Fixed;
    matrix.xx = FT_DivFix(yy, delta);
    matrix.yy = FT_DivFix(xx, delta);
    FT_Err_Ok
}

fn move_long_sign(value: FT_Long, sign: i32) -> (FT_ULong, i32) {
    if value < 0 {
        ((0 as FT_ULong).wrapping_sub(value as FT_ULong), -sign)
    } else {
        (value as FT_ULong, sign)
    }
}

pub fn FT_Face_Info(face: &FT_Face) -> FT_FaceRecPublic {
    let info = face.inner.info();
    FT_FaceRecPublic {
        num_faces: info.num_faces as FT_Long,
        face_index: info.face_index as FT_Long,
        face_flags: FT_Long::from(info.face_flags),
        style_flags: FT_Long::from(info.style_flags),
        num_glyphs: FT_Long::from(info.num_glyphs),
        bbox: FT_BBox {
            xMin: FT_Long::from(info.bbox.x_min),
            yMin: FT_Long::from(info.bbox.y_min),
            xMax: FT_Long::from(info.bbox.x_max),
            yMax: FT_Long::from(info.bbox.y_max),
        },
        units_per_EM: info.units_per_em,
        ascender: info.ascender,
        descender: info.descender,
        height: info.height,
        max_advance_width: info.max_advance_width as i16,
        max_advance_height: info.max_advance_height as i16,
        underline_position: info.underline_position,
        underline_thickness: info.underline_thickness,
        ..FT_FaceRecPublic::default()
    }
}

pub fn FT_Get_Sfnt_Table(face: &FT_Face, tag: FT_Sfnt_Tag) -> FT_Pointer {
    let tag = i64::from(tag);
    if tag == FT_SFNT_OS2 {
        return face
            .sfnt_os2
            .as_deref()
            .map_or(ptr::null_mut(), |os2| os2 as *const TT_OS2 as FT_Pointer);
    }
    if tag == FT_SFNT_HEAD {
        return face
            .sfnt_head
            .as_deref()
            .map_or(ptr::null_mut(), |h| h as *const TT_Header as FT_Pointer);
    }
    if tag == FT_SFNT_MAXP {
        return face
            .sfnt_maxp
            .as_deref()
            .map_or(ptr::null_mut(), |m| m as *const TT_MaxProfile as FT_Pointer);
    }
    if tag == FT_SFNT_HHEA {
        return face
            .sfnt_hhea
            .as_deref()
            .map_or(ptr::null_mut(), |h| h as *const TT_HoriHeader as FT_Pointer);
    }
    if tag == FT_SFNT_VHEA {
        return face
            .sfnt_vhea
            .as_deref()
            .map_or(ptr::null_mut(), |v| v as *const TT_VertHeader as FT_Pointer);
    }
    if tag == FT_SFNT_POST {
        return face
            .sfnt_post
            .as_deref()
            .map_or(ptr::null_mut(), |p| p as *const TT_Postscript as FT_Pointer);
    }
    if tag == FT_SFNT_PCLT {
        return face
            .sfnt_pclt
            .as_deref()
            // FreeType sfnt/sfdriver.c returns PCLT only when Version is nonzero.
            .filter(|p| p.Version != 0)
            .map_or(ptr::null_mut(), |p| p as *const TT_PCLT as FT_Pointer);
    }
    // FT_SFNT_MAX or any unrecognised tag returns null.
    ptr::null_mut()
}

pub fn FT_Get_Sfnt_OS2(face: &FT_Face) -> Option<TT_OS2> {
    face.sfnt_os2.as_deref().copied()
}

pub fn FT_Load_Sfnt_Table(
    face: &FT_Face,
    tag: FT_ULong,
    offset: FT_Long,
    length: Option<&mut FT_ULong>,
) -> Result<Option<Vec<u8>>, FT_Error> {
    let tag_u32 = match u32::try_from(tag) {
        Ok(t) => t,
        Err(_) => return Err(FT_Err_Table_Missing as FT_Error),
    };
    let font = face.inner.font();
    let table_len = match font.sfnt_table_len(tag_u32) {
        Ok(len) => len,
        Err(_) => return Err(FT_Err_Table_Missing as FT_Error),
    };
    match length {
        Some(len) if *len == 0 => {
            // C `tt_face_load_any` returns the table/font size before using
            // `offset` when `*length == 0` (sfnt/ttload.c:617-621).
            *len = table_len as FT_ULong;
            Ok(None)
        }
        Some(len) => {
            let data =
                match font.load_sfnt_table(tag_u32, ft_long_to_i64(offset), Some(*len as usize)) {
                    Ok(data) => data,
                    Err(_) => return Err(FT_Err_Invalid_Stream_Operation as FT_Error),
                };
            let copy_len = data.len();
            let bytes = data[..copy_len].to_vec();
            *len = copy_len as FT_ULong;
            Ok(Some(bytes))
        }
        None => match font.load_sfnt_table(tag_u32, ft_long_to_i64(offset), None) {
            Ok(data) => Ok(Some(data)),
            Err(_) => Err(FT_Err_Invalid_Stream_Operation as FT_Error),
        },
    }
}

/// Safe Rust equivalent of `FT_Sfnt_Table_Info`.
///
/// When `table_index_or_count` is `None`, returns the total number of SFNT
/// tables (the count-query mode). When it is `Some(index)`, returns the
/// tag and byte length of the table at that index.
pub fn FT_Sfnt_Table_Info(
    face: &FT_Face,
    table_index: FT_UInt,
    tag: Option<&mut FT_ULong>,
    length: Option<&mut FT_ULong>,
) -> FT_Error {
    let Some(length) = length else {
        return FT_Err_Invalid_Argument;
    };
    if tag.is_none() {
        // C `sfnt_table_info` returns the table count when `tag == NULL`,
        // ignoring `table_index` (sfnt/sfdriver.c:156-158).
        *length = FT_Sfnt_Table_Count(face) as FT_ULong;
        return FT_Err_Ok;
    }
    let font = face.inner.font();
    let index = match usize::try_from(table_index) {
        Ok(i) => i,
        Err(_) => return FT_Err_Table_Missing as FT_Error,
    };
    let Some(info) = font.sfnt_table_info(index) else {
        return FT_Err_Table_Missing as FT_Error;
    };
    if let Some(tag) = tag {
        *tag = info.tag as FT_ULong;
    }
    *length = info.length as FT_ULong;
    FT_Err_Ok
}

/// Returns the total number of SFNT tables in the font.
pub fn FT_Sfnt_Table_Count(face: &FT_Face) -> usize {
    face.inner.font().sfnt_tables().len()
}

fn slot_to_ffi(face: &FT_Face, slot: api::GlyphSlot, load_flags: api::LoadFlags) -> FT_GlyphSlot {
    // Destructure slot to move into core_slot without cloning the entire
    // GlyphSlot (which includes Outline Vecs and RenderedBitmap buffers).
    let glyph_index = FT_UInt::from(slot.glyph_index);
    let metrics: FT_Glyph_Metrics = slot.metrics.into();
    let advance: FT_Vector = slot.advance.into();
    let format = glyph_format_from_core(slot.format);
    let num_subglyphs = FT_UInt::try_from(slot.subglyphs.len()).unwrap_or(FT_UInt::MAX);
    let bitmap = slot.bitmap.clone().map(Into::into);
    let bitmap_left = slot.bitmap_left;
    let bitmap_top = slot.bitmap_top;
    let outline_cbox = bbox_to_ffi(slot.outline_cbox);
    let outline_bbox = bbox_to_ffi(slot.outline_bbox);
    let outline = slot.slot_outline().map(outline_to_ffi_snapshot);
    let source_face = face.inner.clone();
    FT_GlyphSlot {
        glyph_index,
        metrics,
        advance,
        format,
        num_subglyphs,
        bitmap,
        bitmap_left,
        bitmap_top,
        outline_cbox,
        outline_bbox,
        outline,
        core_slot: slot,
        source_face,
        load_flags,
    }
}

fn outline_to_ffi_snapshot(outline: &crate::outline::Outline) -> FT_OutlineSnapshot {
    FT_OutlineSnapshot {
        points: outline
            .points
            .iter()
            .map(|point| FT_Vector {
                x: i64::from(point.x),
                y: i64::from(point.y),
            })
            .collect(),
        tags: if outline.tags.is_empty() {
            outline
                .points
                .iter()
                .map(|point| if point.on_curve { 1 } else { 0 })
                .collect()
        } else {
            outline.tags.clone()
        },
        contours: outline
            .contours
            .iter()
            .map(|&contour| contour as FT_UShort)
            .collect(),
        flags: outline.flags as FT_Int,
    }
}

fn bbox_to_ffi(bbox: crate::font::BBox) -> FT_BBox {
    bbox.into()
}

fn advance_fast_path_supported(load_flags: FT_Int32) -> bool {
    load_flags & FT_LOAD_NO_SCALE != 0
        || load_flags & FT_LOAD_NO_HINTING != 0
        || FT_LOAD_TARGET_MODE(load_flags) == FT_RENDER_MODE_LIGHT
}

fn use_fast_horizontal_advance(flags: api::LoadFlags) -> bool {
    !flags.contains(api::LoadFlags::VERTICAL_LAYOUT)
        && !flags.contains(api::LoadFlags::NO_SCALE)
        && (flags.contains(api::LoadFlags::NO_HINTING)
            || flags.contains(api::LoadFlags::TARGET_LIGHT))
}

fn c_face_index_to_core(face_index: FT_Long) -> Result<(usize, bool), FT_Error> {
    if face_index >= 0 {
        // FreeType encodes named instance selection in bits 16..30 of
        // `face_index`; the low 16 bits remain the selected face number
        // (ftobjs.c, FT_Open_Face face_index handling).
        // Preserve the encoded value for public FT_FaceRec::face_index; the
        // core validates instanceCount and resolves the low collection bits.
        let face_index = usize::try_from(face_index).map_err(|_| FT_Err_Invalid_Argument)?;
        return Ok((face_index, false));
    }
    // FreeType treats negative face indexes as probes: `-(N+1)` opens face N
    // without allocating glyph-slot or size objects.
    let selected = face_index
        .checked_neg()
        .and_then(|value| value.checked_sub(1))
        .ok_or(FT_Err_Invalid_Argument)?;
    let face_index = usize::try_from(selected).map_err(|_| FT_Err_Invalid_Argument)?;
    Ok((face_index, true))
}
