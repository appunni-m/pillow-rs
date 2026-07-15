#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::{
    CStr, CString, c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void,
};
use std::ptr::{self, NonNull};
use std::slice;

use fontdone::ffi as rust_ffi;

pub type FT_Error = c_int;
pub type FT_Bool = c_uchar;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_UInt32 = u32;
pub type FT_Byte = c_uchar;
pub type FT_Bytes = *const FT_Byte;
pub type FT_Long = c_long;
pub type FT_ULong = c_ulong;
pub type FT_Pos = c_long;
pub type FT_Fixed = c_long;
pub type FT_Angle = FT_Fixed;
pub type FT_F26Dot6 = c_long;
pub type FT_F2Dot14 = c_short;
pub type FT_Short = c_short;
pub type FT_UShort = c_ushort;
pub type FT_Render_Mode = c_int;
pub type FT_Pixel_Mode = c_int;
pub type FT_Glyph_Format = c_int;
pub type FT_Orientation = c_int;
pub type FT_Size_Request_Type = c_int;
pub type FT_Encoding = c_int;
pub type FT_Sfnt_Tag = c_uint;
pub type FT_LcdFilter = c_int;
pub type FT_TrueTypeEngineType = c_int;

pub type FT_Library = *mut FT_LibraryRec;
pub type FT_Face = *mut FT_FaceRec;
pub type FT_Size = *mut FT_SizeRec;
pub type FT_GlyphSlot = *mut FT_GlyphSlotRec;
pub type FT_CharMap = *mut FT_CharMapRec;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Matrix {
    pub xx: FT_Fixed,
    pub xy: FT_Fixed,
    pub yx: FT_Fixed,
    pub yy: FT_Fixed,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_BBox {
    pub xMin: FT_Pos,
    pub yMin: FT_Pos,
    pub xMax: FT_Pos,
    pub yMax: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Outline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FT_Vector,
    pub tags: *mut FT_Byte,
    pub contours: *mut FT_UShort,
    pub flags: FT_Int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_UnitVector {
    pub x: FT_F2Dot14,
    pub y: FT_F2Dot14,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Glyph_Metrics {
    pub width: FT_Pos,
    pub height: FT_Pos,
    pub horiBearingX: FT_Pos,
    pub horiBearingY: FT_Pos,
    pub horiAdvance: FT_Pos,
    pub vertBearingX: FT_Pos,
    pub vertBearingY: FT_Pos,
    pub vertAdvance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_Metrics {
    pub x_ppem: FT_UShort,
    pub y_ppem: FT_UShort,
    pub x_scale: FT_Fixed,
    pub y_scale: FT_Fixed,
    pub ascender: FT_Pos,
    pub descender: FT_Pos,
    pub height: FT_Pos,
    pub max_advance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_RequestRec {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: *mut c_uchar,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub palette_mode: c_uchar,
    pub palette: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Color {
    pub blue: FT_Byte,
    pub green: FT_Byte,
    pub red: FT_Byte,
    pub alpha: FT_Byte,
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Init(abitmap: *mut FT_Bitmap) {
    // FreeType accepts NULL here and otherwise overwrites the public record
    // with the static zero `null_bitmap`.
    if let Some(bitmap) = unsafe { abitmap.as_mut() } {
        *bitmap = FT_Bitmap::default();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_New(abitmap: *mut FT_Bitmap) {
    FT_Bitmap_Init(abitmap);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Copy(
    library: FT_Library,
    source: *const FT_Bitmap,
    target: *mut FT_Bitmap,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if source == target.cast_const() {
        return if library_ref(library).is_some() {
            rust_ffi::FT_Err_Ok
        } else {
            rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error
        };
    }

    let mut source_view = bitmap_to_rust(source_ref);
    let mut target_view = bitmap_to_rust(target_ref);
    if let Some(bytes) = bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }

    let err =
        rust_ffi::FT_Bitmap_Copy(library_ref(library), Some(&source_view), Some(&mut target_view));
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Embolden(
    library: FT_Library,
    bitmap: *mut FT_Bitmap,
    xStrength: FT_Pos,
    yStrength: FT_Pos,
) -> FT_Error {
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut bitmap_view = bitmap_to_rust(bitmap_ref);
    if let Some(bytes) = bitmap_bytes(bitmap_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Embolden(
        library_ref(library),
        Some(&mut bitmap_view),
        xStrength,
        yStrength,
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(bitmap_ref, &bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Blend(
    library: FT_Library,
    source: *const FT_Bitmap,
    source_offset: FT_Vector,
    target: *mut FT_Bitmap,
    atarget_offset: *mut FT_Vector,
    color: FT_Color,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(atarget_offset_ref) = (unsafe { atarget_offset.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut source_view = bitmap_to_rust(source_ref);
    let mut target_view = bitmap_to_rust(target_ref);
    if let Some(bytes) = bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }
    if let Some(bytes) = bitmap_bytes(target_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut target_view), bytes);
    }
    let mut rust_target_offset = rust_ffi::FT_Vector {
        x: atarget_offset_ref.x,
        y: atarget_offset_ref.y,
    };
    let err = rust_ffi::FT_Bitmap_Blend(
        library_ref(library),
        Some(&source_view),
        rust_ffi::FT_Vector {
            x: source_offset.x,
            y: source_offset.y,
        },
        Some(&mut target_view),
        Some(&mut rust_target_offset),
        rust_ffi::FT_Color {
            blue: color.blue,
            green: color.green,
            red: color.red,
            alpha: color.alpha,
        },
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
        atarget_offset_ref.x = rust_target_offset.x;
        atarget_offset_ref.y = rust_target_offset.y;
    }
    err
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_SfntName {
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
    pub language_id: FT_UShort,
    pub name_id: FT_UShort,
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_CharMapRec {
    pub face: FT_Face,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TT_OS2 {
    pub version: FT_UShort,
    pub xAvgCharWidth: FT_Short,
    pub usWeightClass: FT_UShort,
    pub usWidthClass: FT_UShort,
    pub fsType: FT_UShort,
    pub ySubscriptXSize: FT_Short,
    pub ySubscriptYSize: FT_Short,
    pub ySubscriptXOffset: FT_Short,
    pub ySubscriptYOffset: FT_Short,
    pub ySuperscriptXSize: FT_Short,
    pub ySuperscriptYSize: FT_Short,
    pub ySuperscriptXOffset: FT_Short,
    pub ySuperscriptYOffset: FT_Short,
    pub yStrikeoutSize: FT_Short,
    pub yStrikeoutPosition: FT_Short,
    pub sFamilyClass: FT_Short,
    pub panose: [FT_Byte; 10],
    pub ulUnicodeRange1: FT_ULong,
    pub ulUnicodeRange2: FT_ULong,
    pub ulUnicodeRange3: FT_ULong,
    pub ulUnicodeRange4: FT_ULong,
    pub achVendID: [i8; 4],
    pub fsSelection: FT_UShort,
    pub usFirstCharIndex: FT_UShort,
    pub usLastCharIndex: FT_UShort,
    pub sTypoAscender: FT_Short,
    pub sTypoDescender: FT_Short,
    pub sTypoLineGap: FT_Short,
    pub usWinAscent: FT_UShort,
    pub usWinDescent: FT_UShort,
    pub ulCodePageRange1: FT_ULong,
    pub ulCodePageRange2: FT_ULong,
    pub sxHeight: FT_Short,
    pub sCapHeight: FT_Short,
    pub usDefaultChar: FT_UShort,
    pub usBreakChar: FT_UShort,
    pub usMaxContext: FT_UShort,
    pub usLowerOpticalPointSize: FT_UShort,
    pub usUpperOpticalPointSize: FT_UShort,
}

#[repr(C)]
pub struct FT_GlyphSlotRec {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: FT_Bitmap,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    buffer: Vec<u8>,
    rust_slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
}

#[repr(C)]
pub struct FT_SizeRec {
    pub metrics: FT_Size_Metrics,
    pub internal: *mut c_void,
    rust_size: rust_ffi::FT_Size,
    owner: FT_Face,
}

#[repr(C)]
pub struct FT_FaceRec {
    pub glyph: FT_GlyphSlot,
    pub size: FT_Size,
    pub internal: *mut c_void,
}

#[repr(C)]
pub struct FT_LibraryRec {
    pub internal: *mut c_void,
}

struct FaceState {
    inner: rust_ffi::FT_Face,
    size_records: Vec<FT_Size>,
    charmaps: Box<[FT_CharMapRec]>,
    charmap_ptrs: Box<[FT_CharMap]>,
    postscript_name: Option<CString>,
    font_format: Option<CString>,
    variant_list: Vec<FT_UInt32>,
}

impl FaceState {
    fn new(inner: rust_ffi::FT_Face) -> Self {
        let postscript_name = postscript_name_cstring(&inner);
        let font_format = font_format_cstring(Some(&inner));
        Self {
            inner,
            size_records: Vec::new(),
            charmaps: Box::new([]),
            charmap_ptrs: Box::new([]),
            postscript_name,
            font_format,
            variant_list: Vec::new(),
        }
    }

    fn refresh_charmaps(&mut self, face: FT_Face) {
        let records = self
            .inner
            .charmaps
            .iter()
            .map(|record| FT_CharMapRec {
                face,
                encoding: record.encoding,
                platform_id: record.platform_id,
                encoding_id: record.encoding_id,
            })
            .collect::<Vec<_>>();
        let mut charmaps = records.into_boxed_slice();
        let charmap_ptrs = charmaps
            .iter_mut()
            .map(|record| record as *mut FT_CharMapRec)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.charmaps = charmaps;
        self.charmap_ptrs = charmap_ptrs;
    }

    fn refresh_postscript_name(&mut self) {
        self.postscript_name = postscript_name_cstring(&self.inner);
    }

    fn charmap_index(&self, charmap: FT_CharMap) -> Option<usize> {
        if charmap.is_null() {
            return None;
        }
        self.charmaps
            .iter()
            .position(|record| ptr::eq(record as *const FT_CharMapRec, charmap.cast_const()))
    }

    fn charmap_by_index(&self, index: FT_UInt) -> Option<FT_CharMap> {
        let index = usize::try_from(index).ok()?;
        self.charmap_ptrs.get(index).copied()
    }

    fn variant_list_ptr(&mut self, values: Option<Vec<FT_UInt32>>) -> *mut FT_UInt32 {
        let Some(values) = values else {
            self.variant_list.clear();
            return ptr::null_mut();
        };
        self.variant_list = values;
        self.variant_list.push(0);
        self.variant_list.as_mut_ptr()
    }

    fn push_size_record(&mut self, size: FT_Size) {
        self.size_records.push(size);
    }

    fn remove_size_record(&mut self, size: FT_Size) -> bool {
        let Some(index) = self
            .size_records
            .iter()
            .position(|record| ptr::eq(*record, size))
        else {
            return false;
        };
        self.size_records.remove(index);
        true
    }
}

impl Drop for FaceState {
    fn drop(&mut self) {
        for size in self.size_records.drain(..) {
            // SAFETY: `size_records` contains only boxes allocated by this wrapper.
            unsafe { drop_size(size) };
        }
    }
}

fn postscript_name_cstring(inner: &rust_ffi::FT_Face) -> Option<CString> {
    rust_ffi::FT_Get_Postscript_Name(inner).and_then(|name| {
        // FreeType exposes a borrowed NUL-terminated C string owned by the face.
        CString::new(name).ok()
    })
}

fn font_format_cstring(inner: Option<&rust_ffi::FT_Face>) -> Option<CString> {
    rust_ffi::FT_Get_Font_Format(inner).and_then(|format| {
        // FreeType exposes the driver-owned FONT_FORMAT service string.
        CString::new(format).ok()
    })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSlotSnapshot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub num_subglyphs: FT_UInt,
    pub outline_cbox: FT_BBox,
    pub outline_bbox: FT_BBox,
    pub outline: Option<rust_ffi::FT_OutlineSnapshot>,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub left: FT_Int,
    pub top: FT_Int,
    pub buffer: Vec<u8>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_byte_slice(ptr: *const FT_Byte, len: FT_UInt) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    let len = usize::try_from(len).unwrap_or(0);
    // SAFETY: test callers pass live FreeType-shaped output pointers with
    // `len` bytes valid for the duration of the snapshot copy.
    unsafe { slice::from_raw_parts(ptr, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_c_string_bytes(ptr: *const c_char) -> Vec<u8> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: test callers pass live FreeType-shaped NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr).to_bytes().to_vec() }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Error_String(error_code: FT_Error) -> *const c_char {
    rust_ffi::FT_Error_String(error_code).map_or(ptr::null(), |text| text.as_ptr().cast())
}

fn write_ft_bytes(out: *mut FT_Bytes, value: FT_Bytes) {
    if let Some(out) = non_null_mut(out) {
        // SAFETY: `out` is non-null and caller provides writable FT_Bytes storage.
        unsafe { *out.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    base_table: *mut FT_Bytes,
    gdef_table: *mut FT_Bytes,
    gpos_table: *mut FT_Bytes,
    gsub_table: *mut FT_Bytes,
    jstf_table: *mut FT_Bytes,
) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut base = ptr::null();
    let mut gdef = ptr::null();
    let mut gpos = ptr::null();
    let mut gsub = ptr::null();
    let mut jstf = ptr::null();
    let err = rust_ffi::FT_OpenType_Validate(
        face,
        validation_flags,
        (!base_table.is_null()).then_some(&mut base),
        (!gdef_table.is_null()).then_some(&mut gdef),
        (!gpos_table.is_null()).then_some(&mut gpos),
        (!gsub_table.is_null()).then_some(&mut gsub),
        (!jstf_table.is_null()).then_some(&mut jstf),
    );
    if err == rust_ffi::FT_Err_Ok {
        write_ft_bytes(base_table, base);
        write_ft_bytes(gdef_table, gdef);
        write_ft_bytes(gpos_table, gpos);
        write_ft_bytes(gsub_table, gsub);
        write_ft_bytes(jstf_table, jstf);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Free(face: FT_Face, table: FT_Bytes) {
    rust_ffi::FT_OpenType_Free(face_state(face).map(|state| &state.inner), table);
}

#[cfg(feature = "abi-test-support")]
pub fn abi_uint32_list(ptr: *const FT_UInt32) -> Option<Vec<FT_UInt32>> {
    if ptr.is_null() {
        return None;
    }
    let mut values = Vec::new();
    for index in 0..4096 {
        // SAFETY: test callers pass live FreeType-shaped zero-terminated lists.
        let value = unsafe { *ptr.add(index) };
        if value == 0 {
            return Some(values);
        }
        values.push(value);
    }
    Some(values)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_info(face: FT_Face) -> Option<rust_ffi::FT_FaceRecPublic> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `state` is owned by the live face for the duration of this scalar copy.
    Some(rust_face_info(&unsafe { state.as_ref() }.inner))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_count(face: FT_Face) -> Option<FT_UInt> {
    let state = face_state(face)?;
    FT_UInt::try_from(state.charmaps.len()).ok()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMap> {
    face_state(face)?.charmap_by_index(index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_info_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMapRec> {
    let state = face_state(face)?;
    let index = usize::try_from(index).ok()?;
    state.charmaps.get(index).copied()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_charmap_index(face: FT_Face) -> Option<FT_Int> {
    let state = face_state(face)?;
    Some(state.inner.active_charmap_index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_slot_snapshot(face: FT_Face) -> Option<AbiSlotSnapshot> {
    let slot = abi_glyph_slot(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let slot = unsafe { slot.as_ref() };
    let len = usize::try_from(i64::from(slot.bitmap.pitch).abs())
        .ok()?
        .checked_mul(usize::try_from(slot.bitmap.rows).ok()?)?;
    let bitmap = if slot.bitmap.buffer.is_null() || len == 0 {
        None
    } else {
        // SAFETY: the buffer is owned by the live slot for the duration of this copy.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, len) }.to_vec();
        Some(AbiBitmapSnapshot {
            rows: slot.bitmap.rows,
            width: slot.bitmap.width,
            pitch: slot.bitmap.pitch,
            num_grays: slot.bitmap.num_grays,
            pixel_mode: slot.bitmap.pixel_mode,
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            buffer,
        })
    };
    Some(AbiSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        num_subglyphs: slot.rust_slot.num_subglyphs,
        outline_cbox: rust_bbox_to_abi(slot.rust_slot.outline_cbox),
        outline_bbox: rust_bbox_to_abi(slot.rust_slot.outline_bbox),
        outline: slot.rust_slot.outline.clone(),
        bitmap,
    })
}

#[cfg(feature = "abi-test-support")]
fn rust_bbox_to_abi(bbox: rust_ffi::FT_BBox) -> FT_BBox {
    FT_BBox {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_render_glyph_from_face(face: FT_Face, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Render_Glyph(slot.as_ptr(), render_mode)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_slant_from_face(
    face: FT_Face,
    xslant: FT_Fixed,
    yslant: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Slant(slot.as_ptr(), xslant, yslant);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_adjust_weight_from_face(
    face: FT_Face,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_AdjustWeight(slot.as_ptr(), xdelta, ydelta);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_embolden_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Embolden(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_oblique_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Oblique(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_subglyph_info_from_face(
    face: FT_Face,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Get_SubGlyph_Info(
        slot.as_ptr(),
        sub_index,
        p_index,
        p_flags,
        p_arg1,
        p_arg2,
        p_transform,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_metrics(face: FT_Face) -> Option<FT_Size_Metrics> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let size = unsafe { (*face.as_ptr()).size };
    let size = NonNull::new(size)?;
    // SAFETY: `size` is owned by the live face for the duration of this copy.
    Some(unsafe { size.as_ref().metrics })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_size(face: FT_Face) -> Option<FT_Size> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    Some(unsafe { (*face.as_ptr()).size })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_os2(face: FT_Face) -> Option<TT_OS2> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_OS2 as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_OS2>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_OS2` pointer.
    let os2 = unsafe { table.as_ref() };
    Some(TT_OS2 {
        version: os2.version,
        xAvgCharWidth: os2.xAvgCharWidth,
        usWeightClass: os2.usWeightClass,
        usWidthClass: os2.usWidthClass,
        fsType: os2.fsType,
        ySubscriptXSize: os2.ySubscriptXSize,
        ySubscriptYSize: os2.ySubscriptYSize,
        ySubscriptXOffset: os2.ySubscriptXOffset,
        ySubscriptYOffset: os2.ySubscriptYOffset,
        ySuperscriptXSize: os2.ySuperscriptXSize,
        ySuperscriptYSize: os2.ySuperscriptYSize,
        ySuperscriptXOffset: os2.ySuperscriptXOffset,
        ySuperscriptYOffset: os2.ySuperscriptYOffset,
        yStrikeoutSize: os2.yStrikeoutSize,
        yStrikeoutPosition: os2.yStrikeoutPosition,
        sFamilyClass: os2.sFamilyClass,
        panose: os2.panose,
        ulUnicodeRange1: os2.ulUnicodeRange1,
        ulUnicodeRange2: os2.ulUnicodeRange2,
        ulUnicodeRange3: os2.ulUnicodeRange3,
        ulUnicodeRange4: os2.ulUnicodeRange4,
        achVendID: os2.achVendID,
        fsSelection: os2.fsSelection,
        usFirstCharIndex: os2.usFirstCharIndex,
        usLastCharIndex: os2.usLastCharIndex,
        sTypoAscender: os2.sTypoAscender,
        sTypoDescender: os2.sTypoDescender,
        sTypoLineGap: os2.sTypoLineGap,
        usWinAscent: os2.usWinAscent,
        usWinDescent: os2.usWinDescent,
        ulCodePageRange1: os2.ulCodePageRange1,
        ulCodePageRange2: os2.ulCodePageRange2,
        sxHeight: os2.sxHeight,
        sCapHeight: os2.sCapHeight,
        usDefaultChar: os2.usDefaultChar,
        usBreakChar: os2.usBreakChar,
        usMaxContext: os2.usMaxContext,
        usLowerOpticalPointSize: os2.usLowerOpticalPointSize,
        usUpperOpticalPointSize: os2.usUpperOpticalPointSize,
    })
}

#[cfg(feature = "abi-test-support")]
fn abi_glyph_slot(face: FT_Face) -> Option<NonNull<FT_GlyphSlotRec>> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    NonNull::new(unsafe { (*face.as_ptr()).glyph })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Init_FreeType(alibrary: *mut FT_Library) -> FT_Error {
    let Some(out) = non_null_mut(alibrary) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = Box::new(FT_LibraryRec {
        internal: Box::into_raw(Box::new(rust_ffi::FT_Init_FreeType())).cast::<c_void>(),
    });
    // SAFETY: `out` is a valid out pointer checked above.
    unsafe { *out.as_ptr() = Box::into_raw(library) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_FreeType(library: FT_Library) -> FT_Error {
    if let Some(library) = non_null_mut(library) {
        // SAFETY: library must be a live handle from FT_Init_FreeType.
        unsafe {
            let library = Box::from_raw(library.as_ptr());
            if !library.internal.is_null() {
                drop(Box::from_raw(library.internal.cast::<rust_ffi::FT_Library>()));
            }
        }
        rust_ffi::FT_Err_Ok
    } else {
        35 // matches C runtime: FT_Done_FreeType(NULL)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilter(
    _library: FT_Library,
    filter: FT_LcdFilter,
) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilter(None, filter)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilterWeights(
    _library: FT_Library,
    weights: *mut FT_Byte,
) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilterWeights(None, weights)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdGeometry(
    library: FT_Library,
    sub: *const FT_Vector,
) -> FT_Error {
    let rust_sub = if sub.is_null() {
        None
    } else {
        let mut vectors = [rust_ffi::FT_Vector::default(); 3];
        for (index, vector) in vectors.iter_mut().enumerate() {
            // SAFETY: `sub` is non-null and the C API requires three vectors.
            let source = unsafe { &*sub.add(index) };
            *vector = rust_ffi::FT_Vector {
                x: source.x,
                y: source.y,
            };
        }
        Some(vectors)
    };
    rust_ffi::FT_Library_SetLcdGeometry(
        library_mut(library),
        rust_sub,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_TrueType_Engine_Type(
    library: FT_Library,
) -> FT_TrueTypeEngineType {
    rust_ffi::FT_Get_TrueType_Engine_Type(library_ref(library))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulDiv(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    rust_ffi::FT_MulDiv(a, b, c)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_MulFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_DivFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_DivFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_RoundFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_RoundFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_CeilFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_CeilFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_FloorFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_FloorFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sin(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Sin(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Cos(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Cos(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Tan(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Tan(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    rust_ffi::FT_Atan2(dx, dy)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Angle_Diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    rust_ffi::FT_Angle_Diff(angle1, angle2)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Unit(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Unit(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Rotate(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Rotate(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Length(vector: *mut FT_Vector) -> FT_Fixed {
    let vector = non_null(vector);
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Length(rust_vector.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Polarize(
    vector: *mut FT_Vector,
    length: *mut FT_Fixed,
    angle: *mut FT_Angle,
) {
    let vector = non_null(vector);
    let length_ptr = non_null_mut(length);
    let angle_ptr = non_null_mut(angle);
    let mut rust_length = length_ptr.map(|length| {
        // SAFETY: `length` is non-null and points to a C ABI `FT_Fixed`.
        unsafe { *length.as_ptr() }
    });
    let mut rust_angle = angle_ptr.map(|angle| {
        // SAFETY: `angle` is non-null and points to a C ABI `FT_Angle`.
        unsafe { *angle.as_ptr() }
    });
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Polarize(
        rust_vector.as_ref(),
        rust_length.as_mut(),
        rust_angle.as_mut(),
    );
    if let (Some(length_ptr), Some(value)) = (length_ptr, rust_length) {
        // SAFETY: `length_ptr` is a valid mutable pointer checked above.
        unsafe { *length_ptr.as_ptr() = value };
    }
    if let (Some(angle_ptr), Some(value)) = (angle_ptr, rust_angle) {
        // SAFETY: `angle_ptr` is a valid mutable pointer checked above.
        unsafe { *angle_ptr.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_From_Polar(
    vector: *mut FT_Vector,
    length: FT_Fixed,
    angle: FT_Angle,
) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_From_Polar(rust_vector.as_mut(), length, angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Transform(vector: *mut FT_Vector, matrix: *const FT_Matrix) {
    let vector = non_null_mut(vector);
    let matrix = non_null(matrix);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    let rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    rust_ffi::FT_Vector_Transform(rust_vector.as_mut(), rust_matrix.as_ref());
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Multiply(a: *const FT_Matrix, b: *mut FT_Matrix) {
    let a = non_null(a);
    let b = non_null_mut(b);
    let rust_a = a.map(|a| {
        // SAFETY: `a` is non-null and points to a C ABI `FT_Matrix`.
        let a = unsafe { a.as_ref() };
        rust_ffi::FT_Matrix {
            xx: a.xx,
            xy: a.xy,
            yx: a.yx,
            yy: a.yy,
        }
    });
    let mut rust_b = b.map(|b| {
        // SAFETY: `b` is non-null and points to a C ABI `FT_Matrix`.
        let b = unsafe { b.as_ref() };
        rust_ffi::FT_Matrix {
            xx: b.xx,
            xy: b.xy,
            yx: b.yx,
            yy: b.yy,
        }
    });
    rust_ffi::FT_Matrix_Multiply(rust_a.as_ref(), rust_b.as_mut());
    if let (Some(b), Some(rust_b)) = (b, rust_b) {
        // SAFETY: `b` is a valid mutable pointer checked above.
        unsafe {
            (*b.as_ptr()).xx = rust_b.xx;
            (*b.as_ptr()).xy = rust_b.xy;
            (*b.as_ptr()).yx = rust_b.yx;
            (*b.as_ptr()).yy = rust_b.yy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Invert(matrix: *mut FT_Matrix) -> FT_Error {
    let matrix = non_null_mut(matrix);
    let mut rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    let err = rust_ffi::FT_Matrix_Invert(rust_matrix.as_mut());
    if let (Some(matrix), Some(rust_matrix)) = (matrix, rust_matrix) {
        // SAFETY: `matrix` is a valid mutable pointer checked above.
        unsafe {
            (*matrix.as_ptr()).xx = rust_matrix.xx;
            (*matrix.as_ptr()).xy = rust_matrix.xy;
            (*matrix.as_ptr()).yx = rust_matrix.yx;
            (*matrix.as_ptr()).yy = rust_matrix.yy;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Memory_Face(
    library: FT_Library,
    file_base: *const c_uchar,
    file_size: FT_Long,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if file_base.is_null() || file_size < 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Ok(file_len) = usize::try_from(file_size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `file_base` is non-null and the caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_len) };
    // SAFETY: `library` is a live handle returned by `FT_Init_FreeType`.
    let rust_library = unsafe { &*((*library.as_ptr()).internal.cast::<rust_ffi::FT_Library>()) };
    match rust_ffi::FT_New_Memory_Face(rust_library, data, face_index, 20.0) {
        Ok(inner) => {
            let metrics = rust_size_metrics_to_abi(inner.size_metrics);
            let rust_size = inner.size;
            let mut face = Box::new(FT_FaceRec {
                glyph: ptr::null_mut(),
                size: Box::into_raw(Box::new(FT_SizeRec {
                    metrics,
                    internal: ptr::null_mut(),
                    rust_size,
                    owner: ptr::null_mut(),
                })),
                internal: ptr::null_mut(),
            });
            let face_ptr = (&mut *face) as *mut FT_FaceRec;
            let mut state = Box::new(FaceState::new(inner));
            // SAFETY: `face.size` was allocated above and is owned by `state`.
            unsafe { (*face.size).owner = face_ptr };
            state.push_size_record(face.size);
            state.refresh_charmaps(face_ptr);
            face.internal = Box::into_raw(state).cast::<c_void>();
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = Box::into_raw(face) };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Face(face: FT_Face) -> FT_Error {
    let Some(face) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: `face` must be a live handle returned by `FT_New_Memory_Face`.
    unsafe {
        let face = Box::from_raw(face.as_ptr());
        drop_glyph(face.glyph);
        if !face.internal.is_null() {
            drop(Box::from_raw(face.internal.cast::<FaceState>()));
        }
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_CheckTrueTypePatents(face: FT_Face) -> FT_Bool {
    rust_ffi::FT_Face_CheckTrueTypePatents(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_SetUnpatentedHinting(face: FT_Face, value: FT_Bool) -> FT_Bool {
    rust_ffi::FT_Face_SetUnpatentedHinting(
        face_state_mut(face).map(|state| &mut state.inner),
        value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_CBox(outline: *const FT_Outline, acbox: *mut FT_BBox) {
    if outline.is_null() || acbox.is_null() {
        return;
    }
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Outline_Get_CBox(Some(&snapshot), Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FT_BBox` storage.
    unsafe {
        *acbox = FT_BBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_Bitmap(
    library: FT_Library,
    outline: *const FT_Outline,
    abitmap: *mut FT_Bitmap,
) -> FT_Error {
    let Some(mut bitmap) = non_null_mut(abitmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let snapshot = outline_snapshot_from_c(outline);
    let bitmap_view = {
        // SAFETY: `bitmap` is non-null and points to caller-owned `FT_Bitmap` storage.
        let bitmap = unsafe { bitmap.as_ref() };
        rust_ffi::FT_Bitmap_C {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: bitmap.buffer,
            num_grays: bitmap.num_grays,
            pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
            palette_mode: bitmap.palette_mode,
            palette: bitmap.palette,
        }
    };
    match rust_ffi::FT_Outline_Get_Bitmap(library_ref(library), snapshot.as_ref(), Some(&bitmap_view))
    {
        Ok(rendered) => {
            // SAFETY: `bitmap` is non-null and still points to caller-owned storage.
            let bitmap = unsafe { bitmap.as_mut() };
            copy_rendered_bitmap_to_c(bitmap, &rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_Orientation(outline: *const FT_Outline) -> FT_Orientation {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_ORIENTATION_TRUETYPE as FT_Orientation;
    };
    rust_ffi::FT_Outline_Get_Orientation(Some(&snapshot)) as FT_Orientation
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Char_Size(
    face: FT_Face,
    char_width: FT_F26Dot6,
    char_height: FT_F26Dot6,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let error = rust_ffi::FT_Set_Char_Size(
        &mut state.inner,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    );
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Pixel_Sizes(
    face: FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let error = rust_ffi::FT_Set_Pixel_Sizes(&mut state.inner, pixel_width, pixel_height);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Request_Size(face: FT_Face, req: *const FT_Size_RequestRec) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let request = if req.is_null() {
        None
    } else {
        // SAFETY: `req` was checked for null and is only copied by value.
        let req = unsafe { *req };
        Some(rust_ffi::FT_Size_RequestRec {
            type_: req.type_,
            width: req.width,
            height: req.height,
            horiResolution: req.horiResolution,
            vertResolution: req.vertResolution,
        })
    };
    let error = rust_ffi::FT_Request_Size(Some(&mut state.inner), request.as_ref());
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Size(face: FT_Face, strike_index: FT_Int) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Select_Size(Some(&mut state.inner), strike_index);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Size(face: FT_Face, asize: *mut FT_Size) -> FT_Error {
    let Some(_face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(asize) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = ptr::null_mut() };
    let mut rust_size: rust_ffi::FT_Size = ptr::null_mut();
    let error = rust_ffi::FT_New_Size(Some(&state.inner), Some(&mut rust_size));
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    let size = Box::into_raw(Box::new(FT_SizeRec {
        metrics: rust_size_metrics_to_abi(state.inner.size_metrics),
        internal: ptr::null_mut(),
        rust_size,
        owner: face,
    }));
    state.push_size_record(size);
    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = size };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Activate_Size(size: FT_Size) -> FT_Error {
    let Some(size_ptr) = non_null_mut(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    // SAFETY: `size_ptr` is a live size record allocated by this wrapper.
    let (owner, rust_size) = unsafe {
        let record = size_ptr.as_ref();
        (record.owner, record.rust_size)
    };
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(_state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Activate_Size(rust_size);
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `face_ptr` is a live parent face and `size` is one of its size records.
        unsafe { (*face_ptr.as_ptr()).size = size };
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Size(size: FT_Size) -> FT_Error {
    let Some(size_ptr) = non_null_mut(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    // SAFETY: `size_ptr` is a live size record allocated by this wrapper.
    let (owner, rust_size) = unsafe {
        let record = size_ptr.as_ref();
        (record.owner, record.rust_size)
    };
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    let error = rust_ffi::FT_Done_Size(rust_size);
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let was_active = unsafe { (*face_ptr.as_ptr()).size == size };
    let removed = state.remove_size_record(size);
    if !removed {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    }
    if was_active {
        let fallback = state
            .size_records
            .first()
            .copied()
            .unwrap_or(ptr::null_mut());
        // SAFETY: `face_ptr` is a live parent face; fallback is either null or still face-owned.
        unsafe { (*face_ptr.as_ptr()).size = fallback };
    }
    // SAFETY: the record has been removed from `state.size_records` and is consumed here.
    unsafe { drop_size(size) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Char_Index(face: FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&state.inner, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIndex(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Face_GetCharVariantIndex(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIsDefault(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(state) = face_state(face) else {
        return -1;
    };
    rust_ffi::FT_Face_GetCharVariantIsDefault(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantSelectors(face: FT_Face) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantSelectors(Some(&state.inner));
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantsOfChar(face: FT_Face, charcode: FT_ULong) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantsOfChar(Some(&state.inner), charcode);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharsOfVariant(
    face: FT_Face,
    variant_selector: FT_ULong,
) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetCharsOfVariant(Some(&state.inner), variant_selector);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Kerning(
    face: FT_Face,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    akerning: *mut FT_Vector,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = NonNull::new(akerning) else {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    };
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_Kerning(
        Some(&state.inner),
        left_glyph,
        right_glyph,
        kern_mode,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_Vector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Charmap(face: FT_Face, encoding: FT_Encoding) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    rust_ffi::FT_Select_Charmap(Some(&mut state.inner), encoding)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Charmap(face: FT_Face, charmap: FT_CharMap) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if state.charmaps.is_empty() || charmap.is_null() {
        return rust_ffi::FT_Err_Invalid_CharMap_Handle as FT_Error;
    }
    let Some(index) = state.charmap_index(charmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    rust_ffi::FT_Set_Charmap(Some(&mut state.inner), rust_charmap)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Charmap_Index(charmap: FT_CharMap) -> FT_Int {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_Charmap_Index(rust_charmap) as FT_Int
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Format(charmap: FT_CharMap) -> FT_Long {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_CMap_Format(rust_charmap) as FT_Long
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Language_ID(charmap: FT_CharMap) -> FT_ULong {
    let Some(charmap) = NonNull::new(charmap) else {
        return 0;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return 0;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return 0;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return 0;
    };
    rust_ffi::FT_Get_CMap_Language_ID(rust_charmap) as FT_ULong
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_FSType_Flags(face: FT_Face) -> FT_UShort {
    rust_ffi::FT_Get_FSType_Flags(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Gasp(face: FT_Face, ppem: FT_UInt) -> FT_Int {
    rust_ffi::FT_Get_Gasp(face_state(face).map(|state| &state.inner), ppem)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Glyph_Name(
    face: FT_Face,
    glyph_index: FT_UInt,
    buffer: *mut c_void,
    buffer_max: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if buffer.is_null() || buffer_max == 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `buffer` is non-null, and the C caller promises at least
    // `buffer_max` writable bytes following FreeType's caller-allocated API.
    let buffer = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), buffer_max as usize) };
    match rust_ffi::FT_Get_Glyph_Name(&state.inner, glyph_index, buffer) {
        Ok(_) => rust_ffi::FT_Err_Ok,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Name_Index(face: FT_Face, glyph_name: *const c_char) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    if glyph_name.is_null() {
        return 0;
    }
    // SAFETY: `glyph_name` is non-null and follows FreeType's C string
    // contract for this borrowed input pointer.
    let glyph_name = unsafe { CStr::from_ptr(glyph_name) };
    let Ok(glyph_name) = glyph_name.to_str() else {
        return 0;
    };
    rust_ffi::FT_Get_Name_Index(Some(&state.inner), Some(glyph_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Postscript_Name(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.postscript_name.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Font_Format(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.font_format.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_X11_Font_Format(face: FT_Face) -> *const c_char {
    FT_Get_Font_Format(face)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Named_Instance(face: FT_Face, instance_index: FT_UInt) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Set_Named_Instance(Some(&mut state.inner), instance_index);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name_Count(face: FT_Face) -> FT_UInt {
    rust_ffi::FT_Get_Sfnt_Name_Count(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name(
    face: FT_Face,
    idx: FT_UInt,
    aname: *mut FT_SfntName,
) -> FT_Error {
    let Some(out) = non_null_mut(aname) else {
        return rust_ffi::FT_Get_Sfnt_Name(face_state(face).map(|state| &state.inner), idx, None);
    };
    let mut name = rust_ffi::FT_SfntName::default();
    let error =
        rust_ffi::FT_Get_Sfnt_Name(face_state(face).map(|state| &state.inner), idx, Some(&mut name));
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_SfntName {
                platform_id: name.platform_id,
                encoding_id: name.encoding_id,
                language_id: name.language_id,
                name_id: name.name_id,
                string: name.string,
                string_len: name.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Table(face: FT_Face, tag: FT_Sfnt_Tag) -> *mut c_void {
    let Some(state) = face_state(face) else {
        return ptr::null_mut();
    };
    rust_ffi::FT_Get_Sfnt_Table(&state.inner, tag)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Sfnt_Table(
    face: FT_Face,
    tag: FT_ULong,
    offset: FT_Long,
    buffer: *mut FT_Byte,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(len_ptr) = non_null_mut(length) else {
        return match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, None) {
            Ok(Some(bytes)) => {
                if let Some(buf) = non_null_mut(buffer) {
                    // SAFETY: caller provides a buffer large enough for the selected table.
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), bytes.len());
                    }
                }
                rust_ffi::FT_Err_Ok as FT_Error
            }
            Ok(None) => rust_ffi::FT_Err_Ok as FT_Error,
            Err(err) => err as FT_Error,
        };
    };
    // SAFETY: caller-provided writable FT_ULong or NULL (caught above).
    let mut len_val = unsafe { *len_ptr.as_ptr() };
    match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, Some(&mut len_val)) {
        Ok(Some(bytes)) => {
            let copy_len = bytes.len().min(len_val as usize);
            if let Some(buf) = non_null_mut(buffer) {
                // SAFETY: caller provides a buffer of at least len_val bytes.
                unsafe {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), copy_len);
                }
            }
            // SAFETY: writable FT_ULong out-param.
            unsafe { *len_ptr.as_ptr() = copy_len as FT_ULong };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Ok(None) => {
            // SAFETY: writable FT_ULong out-param (length probe result).
            unsafe { *len_ptr.as_ptr() = len_val };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Err(err) => err as FT_Error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sfnt_Table_Info(
    face: FT_Face,
    table_index: FT_UInt,
    tag: *mut FT_ULong,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut tag_out: rust_ffi::FT_ULong = 0;
    let mut length_out: rust_ffi::FT_ULong = 0;
    let tag_ref = if tag.is_null() {
        None
    } else {
        Some(&mut tag_out)
    };
    let length_ref = if length.is_null() {
        None
    } else {
        Some(&mut length_out)
    };
    let err = rust_ffi::FT_Sfnt_Table_Info(&state.inner, table_index, tag_ref, length_ref);
    if err == rust_ffi::FT_Err_Ok {
        if let Some(tag_ptr) = non_null_mut(tag) {
            // SAFETY: writable FT_ULong out-param. Copying after the core call
            // avoids creating aliased `&mut` references for caller pointers.
            unsafe { *tag_ptr.as_ptr() = tag_out as FT_ULong };
        }
        if let Some(len_ptr) = non_null_mut(length) {
            // SAFETY: writable FT_ULong out-param. C writes tag before length,
            // so an aliased caller pointer ends with the length value.
            unsafe { *len_ptr.as_ptr() = length_out as FT_ULong };
        }
    }
    err as FT_Error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_First_Char(face: FT_Face, agindex: *mut FT_UInt) -> FT_ULong {
    let mut glyph_index = 0;
    let char_code =
        rust_ffi::FT_Get_First_Char(face_state(face).map(|state| &state.inner), Some(&mut glyph_index));
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    char_code
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Next_Char(
    face: FT_Face,
    char_code: FT_ULong,
    agindex: *mut FT_UInt,
) -> FT_ULong {
    let mut glyph_index = 0;
    let next_char = rust_ffi::FT_Get_Next_Char(
        face_state(face).map(|state| &state.inner),
        char_code,
        Some(&mut glyph_index),
    );
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    next_char
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_Version(
    library: FT_Library,
    amajor: *mut FT_Int,
    aminor: *mut FT_Int,
    apatch: *mut FT_Int,
) {
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    rust_ffi::FT_Library_Version(
        library_ref(library),
        Some(&mut major),
        Some(&mut minor),
        Some(&mut patch),
    );
    if let Some(out) = non_null_mut(amajor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = major };
    }
    if let Some(out) = non_null_mut(aminor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = minor };
    }
    if let Some(out) = non_null_mut(apatch) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = patch };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Char(
    face: FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Char(&state.inner, char_code, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Glyph(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Glyph(&state.inner, glyph_index, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advance(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    padvance: *mut FT_Fixed,
) -> FT_Error {
    let Some(out) = non_null_mut(padvance) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advance(&state.inner, glyph_index, load_flags) {
        Ok(advance) => {
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = advance };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advances(
    face: FT_Face,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
    padvances: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(out_len) = usize::try_from(count) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let out = if out_len == 0 {
        None
    } else {
        let Some(out) = non_null_mut(padvances) else {
            return rust_ffi::FT_Err_Invalid_Argument;
        };
        Some(out)
    };
    match rust_ffi::FT_Get_Advances(&state.inner, start, count, load_flags) {
        Ok(advances) => {
            if advances.len() != out_len {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            if let Some(out) = out {
                // SAFETY: `out` is non-null and caller promises at least `count` writable entries.
                let out = unsafe { slice::from_raw_parts_mut(out.as_ptr(), out_len) };
                out.copy_from_slice(&advances);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_SubGlyph_Info(
    slot: FT_GlyphSlot,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let (
        Some(p_index),
        Some(p_flags),
        Some(p_arg1),
        Some(p_arg2),
        Some(p_transform),
    ) = (
        non_null_mut(p_index),
        non_null_mut(p_flags),
        non_null_mut(p_arg1),
        non_null_mut(p_arg2),
        non_null_mut(p_transform),
    )
    else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    let rust_slot = unsafe { &(*slot_ptr.as_ptr()).rust_slot };
    let mut index = 0;
    let mut flags = 0;
    let mut arg1 = 0;
    let mut arg2 = 0;
    let mut transform = rust_ffi::FT_Matrix::default();
    let error = rust_ffi::FT_Get_SubGlyph_Info(
        Some(rust_slot),
        sub_index,
        Some(&mut index),
        Some(&mut flags),
        Some(&mut arg1),
        Some(&mut arg2),
        Some(&mut transform),
    );
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    // SAFETY: all output pointers are non-null and caller provides writable storage.
    unsafe {
        *p_index.as_ptr() = index;
        *p_flags.as_ptr() = flags;
        *p_arg1.as_ptr() = arg1;
        *p_arg2.as_ptr() = arg2;
        *p_transform.as_ptr() = FT_Matrix {
            xx: transform.xx,
            xy: transform.xy,
            yx: transform.yx,
            yy: transform.yy,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Render_Glyph(slot: FT_GlyphSlot, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    let source_face = unsafe { (*slot_ptr.as_ptr()).source_face };
    // SAFETY: `slot_ptr` points to a live slot allocated by this crate.
    let rust_slot = unsafe { (*slot_ptr.as_ptr()).rust_slot.clone() };
    // SAFETY: `slot_ptr` points to a live slot allocated by this crate.
    let load_flags = unsafe { (*slot_ptr.as_ptr()).load_flags };
    match rust_ffi::FT_Render_Glyph(rust_slot, render_mode) {
        Ok(rendered) => store_slot(source_face, rendered, load_flags | rust_ffi::FT_LOAD_RENDER),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Embolden(slot: FT_GlyphSlot) {
    FT_GlyphSlot_AdjustWeight(slot, 0x0AAA, 0x0AAA);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_AdjustWeight(
    slot: FT_GlyphSlot,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        rust_ffi::FT_GlyphSlot_AdjustWeight(Some(&mut slot_ref.rust_slot), xdelta, ydelta);
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Oblique(slot: FT_GlyphSlot) {
    FT_GlyphSlot_Slant(slot, 0x0366A, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Slant(slot: FT_GlyphSlot, xslant: FT_Fixed, yslant: FT_Fixed) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        rust_ffi::FT_GlyphSlot_Slant(Some(&mut slot_ref.rust_slot), xslant, yslant);
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
}

fn store_slot(face: FT_Face, slot: rust_ffi::FT_GlyphSlot, load_flags: FT_Int32) -> FT_Error {
    let Some(face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `face_ptr` is a live handle and owns the previous `glyph` pointer.
    unsafe {
        drop_glyph((*face_ptr.as_ptr()).glyph);
        (*face_ptr.as_ptr()).glyph =
            Box::into_raw(Box::new(rust_slot_to_abi(slot, face, load_flags)));
    }
    rust_ffi::FT_Err_Ok
}

fn update_size_metrics(face: FT_Face, rust_face: &rust_ffi::FT_Face) {
    let Some(face_ptr) = non_null_mut(face) else {
        return;
    };
    // SAFETY: `face_ptr` is a live handle allocated by this crate.
    let size = unsafe { (*face_ptr.as_ptr()).size };
    let Some(size_ptr) = non_null_mut(size) else {
        return;
    };
    // SAFETY: `size_ptr` points to the live size record owned by `face`.
    unsafe { (*size_ptr.as_ptr()).metrics = rust_size_metrics_to_abi(rust_face.size_metrics) };
}

fn rust_face_info(face: &rust_ffi::FT_Face) -> rust_ffi::FT_FaceRecPublic {
    rust_ffi::FT_FaceRecPublic {
        num_faces: face.num_faces,
        face_index: face.face_index,
        face_flags: face.face_flags,
        style_flags: face.style_flags,
        num_glyphs: face.num_glyphs,
        bbox: face.bbox,
        units_per_EM: face.units_per_EM,
        ascender: face.ascender,
        descender: face.descender,
        height: face.height,
        max_advance_width: face.max_advance_width,
        max_advance_height: face.max_advance_height,
        underline_position: face.underline_position,
        underline_thickness: face.underline_thickness,
        size: face.size,
        ..rust_ffi::FT_FaceRecPublic::default()
    }
}

fn rust_face_charmap(face: &rust_ffi::FT_Face, index: usize) -> Option<rust_ffi::FT_CharMap> {
    face.charmaps.get(index).map(|record| {
        (record as *const rust_ffi::FT_CharMapRecPublic)
            .cast_mut()
            .cast()
    })
}

fn rust_slot_to_abi(
    slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
) -> FT_GlyphSlotRec {
    let rust_slot = slot.clone();
    let mut buffer = slot
        .bitmap
        .as_ref()
        .map(|bitmap| bitmap.buffer.clone())
        .unwrap_or_default();
    let bitmap = slot
        .bitmap
        .map(|bitmap| FT_Bitmap {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: buffer.as_mut_ptr(),
            num_grays: bitmap.num_grays,
            pixel_mode: bitmap.pixel_mode,
            palette_mode: 0,
            palette: ptr::null_mut(),
        })
        .unwrap_or_default();
    FT_GlyphSlotRec {
        glyph_index: slot.glyph_index,
        metrics: rust_metrics_to_abi(slot.metrics),
        advance: FT_Vector {
            x: slot.advance.x,
            y: slot.advance.y,
        },
        format: slot.format,
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
        buffer,
        rust_slot,
        source_face,
        load_flags,
    }
}

fn rust_metrics_to_abi(metrics: rust_ffi::FT_Glyph_Metrics) -> FT_Glyph_Metrics {
    FT_Glyph_Metrics {
        width: metrics.width,
        height: metrics.height,
        horiBearingX: metrics.horiBearingX,
        horiBearingY: metrics.horiBearingY,
        horiAdvance: metrics.horiAdvance,
        vertBearingX: metrics.vertBearingX,
        vertBearingY: metrics.vertBearingY,
        vertAdvance: metrics.vertAdvance,
    }
}

fn rust_size_metrics_to_abi(metrics: rust_ffi::FT_Size_Metrics) -> FT_Size_Metrics {
    FT_Size_Metrics {
        x_ppem: metrics.x_ppem,
        y_ppem: metrics.y_ppem,
        x_scale: metrics.x_scale,
        y_scale: metrics.y_scale,
        ascender: metrics.ascender,
        descender: metrics.descender,
        height: metrics.height,
        max_advance: metrics.max_advance,
    }
}

fn outline_snapshot_from_c(outline: *const FT_Outline) -> Option<rust_ffi::FT_OutlineSnapshot> {
    let outline = non_null(outline)?;
    // SAFETY: `outline` is non-null; callers of the C ABI must pass a valid `FT_Outline`.
    let outline = unsafe { outline.as_ref() };
    let n_points = usize::from(outline.n_points);
    let n_contours = usize::from(outline.n_contours);
    if (n_points > 0 && outline.points.is_null()) || (n_contours > 0 && outline.contours.is_null())
    {
        return None;
    }
    let points = if n_points == 0 {
        Vec::new()
    } else {
        // SAFETY: `points` is non-null for `n_points > 0`; the C ABI caller owns a readable
        // array of `n_points` `FT_Vector` records for the duration of this call.
        unsafe { slice::from_raw_parts(outline.points, n_points) }
            .iter()
            .map(|point| rust_ffi::FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect()
    };
    let tags = if n_points == 0 || outline.tags.is_null() {
        Vec::new()
    } else {
        // SAFETY: `tags` is non-null and points to `n_points` readable tag bytes.
        unsafe { slice::from_raw_parts(outline.tags, n_points) }.to_vec()
    };
    let contours = if n_contours == 0 {
        Vec::new()
    } else {
        // SAFETY: `contours` is non-null for `n_contours > 0`; the caller provides
        // `n_contours` readable contour endpoint values.
        unsafe { slice::from_raw_parts(outline.contours, n_contours) }.to_vec()
    };
    Some(rust_ffi::FT_OutlineSnapshot {
        points,
        tags,
        contours,
        flags: outline.flags,
    })
}

fn copy_rendered_bitmap_to_c(target: &mut FT_Bitmap, rendered: &rust_ffi::FT_Bitmap) {
    let rows = usize::try_from(target.rows).unwrap_or(0);
    let width = usize::try_from(target.width).unwrap_or(0);
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).unwrap_or(0);
    if target.buffer.is_null() || rows == 0 || width == 0 || pitch_abs == 0 {
        return;
    }
    let row_bytes = width.min(pitch_abs);
    let target_len = pitch_abs.saturating_mul(rows);
    // SAFETY: the public C caller provides a writable bitmap buffer of at least
    // `abs(pitch) * rows` bytes, matching FreeType's `FT_Bitmap` contract.
    let target_buffer = unsafe { slice::from_raw_parts_mut(target.buffer, target_len) };
    for row in 0..rows {
        let src = row.saturating_mul(width);
        let dst = row.saturating_mul(pitch_abs);
        let Some(src_row) = rendered.buffer.get(src..src.saturating_add(row_bytes)) else {
            break;
        };
        let Some(dst_row) = target_buffer.get_mut(dst..dst.saturating_add(row_bytes)) else {
            break;
        };
        dst_row.copy_from_slice(src_row);
    }
}

fn bitmap_to_rust(bitmap: &FT_Bitmap) -> rust_ffi::FT_Bitmap_C {
    rust_ffi::FT_Bitmap_C {
        rows: bitmap.rows,
        width: bitmap.width,
        pitch: bitmap.pitch,
        buffer: bitmap.buffer,
        num_grays: bitmap.num_grays,
        pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
        palette_mode: bitmap.palette_mode,
        palette: bitmap.palette,
    }
}

fn copy_rust_bitmap_record_to_c(target: &mut FT_Bitmap, source: &rust_ffi::FT_Bitmap_C) {
    target.rows = source.rows;
    target.width = source.width;
    target.pitch = source.pitch;
    target.buffer = source.buffer;
    target.num_grays = source.num_grays;
    target.pixel_mode = source.pixel_mode.into();
    target.palette_mode = source.palette_mode;
    target.palette = source.palette;
}

fn bitmap_bytes(bitmap: &FT_Bitmap) -> Option<Vec<u8>> {
    let len = usize::try_from(bitmap.pitch.unsigned_abs())
        .ok()?
        .checked_mul(usize::try_from(bitmap.rows).ok()?)?;
    if bitmap.buffer.is_null() || len == 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(bitmap.buffer, len) }.to_vec())
}

fn face_state(face: FT_Face) -> Option<&'static FaceState> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a `FaceState` allocated by this crate.
        Some(unsafe { &*internal.cast::<FaceState>() })
    }
}

fn face_state_mut(face: FT_Face) -> Option<&'static mut FaceState> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    let mut state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `internal` points to a `FaceState` allocated by this crate.
    Some(unsafe { state.as_mut() })
}

fn library_ref(library: FT_Library) -> Option<&'static rust_ffi::FT_Library> {
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a `rust_ffi::FT_Library` allocated by this crate.
        Some(unsafe { &*internal.cast::<rust_ffi::FT_Library>() })
    }
}

fn library_mut(library: FT_Library) -> Option<&'static mut rust_ffi::FT_Library> {
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a uniquely borrowed `rust_ffi::FT_Library`.
        Some(unsafe { &mut *internal.cast::<rust_ffi::FT_Library>() })
    }
}

fn non_null<T>(ptr: *const T) -> Option<NonNull<T>> {
    NonNull::new(ptr.cast_mut())
}

fn non_null_mut<T>(ptr: *mut T) -> Option<NonNull<T>> {
    NonNull::new(ptr)
}

unsafe fn drop_glyph(slot: FT_GlyphSlot) {
    if !slot.is_null() {
        // SAFETY: `slot` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(slot)) };
    }
}

unsafe fn drop_size(size: FT_Size) {
    if !size.is_null() {
        // SAFETY: `size` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(size)) };
    }
}
