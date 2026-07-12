#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{c_uchar, c_void};
use std::ptr;
use std::slice;

use fontdone::ffi as rust_ffi;

pub type FT_Error = i32;
pub type FT_Bool = u8;
pub type FT_F2Dot14 = i16;
pub type FT_Fixed = i64;
pub type FT_Angle = FT_Fixed;
pub type FT_Int = i32;
pub type FT_Int32 = i32;
pub type FT_Long = i64;
pub type FT_ULong = u64;
pub type FT_UInt = u32;
pub type FT_Sfnt_Tag = u32;
pub type FT_Short = i16;
pub type FT_UShort = u16;
pub type FT_Byte = u8;
pub type FT_Size_Request_Type = i32;
pub type FT_Encoding = i32;
pub type FT_LcdFilter = i32;
pub type FT_TrueTypeEngineType = i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmStatus {
    pub error: FT_Error,
    pub handle: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmVector {
    pub x: i64,
    pub y: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmMatrix {
    pub xx: i64,
    pub xy: i64,
    pub yx: i64,
    pub yy: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBBox {
    pub xMin: i64,
    pub yMin: i64,
    pub xMax: i64,
    pub yMax: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmOutline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FontdoneWasmVector,
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
pub struct FontdoneWasmGlyphMetrics {
    pub width: i64,
    pub height: i64,
    pub horiBearingX: i64,
    pub horiBearingY: i64,
    pub horiAdvance: i64,
    pub vertBearingX: i64,
    pub vertBearingY: i64,
    pub vertAdvance: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: i32,
    pub buffer: *const c_uchar,
    pub buffer_len: usize,
    pub num_grays: u16,
    pub pixel_mode: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmGlyphSlot {
    pub glyph_index: u32,
    pub metrics: FontdoneWasmGlyphMetrics,
    pub advance: FontdoneWasmVector,
    pub format: i32,
    pub num_subglyphs: u32,
    pub bitmap: FontdoneWasmBitmap,
    pub bitmap_left: i32,
    pub bitmap_top: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSizeMetrics {
    pub x_ppem: u16,
    pub y_ppem: u16,
    pub x_scale: i64,
    pub y_scale: i64,
    pub ascender: i64,
    pub descender: i64,
    pub height: i64,
    pub max_advance: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSfntName {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: u16,
    pub string: *const c_uchar,
    pub string_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmString {
    pub string: *const c_uchar,
    pub string_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmCharmap {
    pub index: FT_UInt,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmOs2 {
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
    pub panose: [u8; 10],
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
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSizeRequest {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

struct WasmFaceState {
    face: rust_ffi::FT_Face,
    slot: Option<rust_ffi::FT_GlyphSlot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSlotSnapshot {
    pub glyph_index: u32,
    pub metrics: FontdoneWasmGlyphMetrics,
    pub advance: FontdoneWasmVector,
    pub format: i32,
    pub num_subglyphs: u32,
    pub outline_cbox: AbiBBoxSnapshot,
    pub outline_bbox: AbiBBoxSnapshot,
    pub outline: Option<rust_ffi::FT_OutlineSnapshot>,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBBoxSnapshot {
    pub xMin: i64,
    pub yMin: i64,
    pub xMax: i64,
    pub yMax: i64,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: i32,
    pub num_grays: u16,
    pub pixel_mode: i32,
    pub left: i32,
    pub top: i32,
    pub buffer: Vec<u8>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_info(handle: usize) -> Option<rust_ffi::FT_FaceRecPublic> {
    let face = face_ref(handle)?;
    Some(rust_ffi::FT_Face_Info(&face.face))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_slot_snapshot(handle: usize) -> Option<AbiSlotSnapshot> {
    let mut slot = FontdoneWasmGlyphSlot::default();
    if fontdone_wasm_get_slot(handle, &mut slot) != rust_ffi::FT_Err_Ok {
        return None;
    }
    let rust_slot = face_ref(handle)?.slot.as_ref()?;
    let bitmap = if slot.bitmap.buffer.is_null() || slot.bitmap.buffer_len == 0 {
        None
    } else {
        // SAFETY: `fontdone_wasm_get_slot` returns a buffer owned by the live handle.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, slot.bitmap.buffer_len) };
        Some(AbiBitmapSnapshot {
            rows: slot.bitmap.rows,
            width: slot.bitmap.width,
            pitch: slot.bitmap.pitch,
            num_grays: slot.bitmap.num_grays,
            pixel_mode: slot.bitmap.pixel_mode,
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            buffer: buffer.to_vec(),
        })
    };
    Some(AbiSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        num_subglyphs: slot.num_subglyphs,
        outline_cbox: wasm_bbox_snapshot(rust_slot.outline_cbox),
        outline_bbox: wasm_bbox_snapshot(rust_slot.outline_bbox),
        outline: rust_slot.outline.clone(),
        bitmap,
    })
}

#[cfg(feature = "abi-test-support")]
fn wasm_bbox_snapshot(bbox: rust_ffi::FT_BBox) -> AbiBBoxSnapshot {
    AbiBBoxSnapshot {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_malloc(size: usize) -> *mut c_void {
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return ptr::null_mut();
    };
    // SAFETY: `layout` is valid and has non-zero size.
    unsafe { alloc(layout).cast::<c_void>() }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_free(ptr: *mut c_void, size: usize) {
    if ptr.is_null() {
        return;
    }
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return;
    };
    // SAFETY: callers pass a pointer returned by `fontdone_wasm_malloc` with the same size.
    unsafe { dealloc(ptr.cast::<u8>(), layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_face(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
) -> FontdoneWasmStatus {
    if file_base.is_null() {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    }
    // SAFETY: `file_base` is non-null and caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let library = rust_ffi::FT_Init_FreeType();
    match rust_ffi::FT_New_Memory_Face(&library, data, face_index, size_pt) {
        Ok(face) => FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Ok,
            handle: Box::into_raw(Box::new(WasmFaceState { face, slot: None })).addr(),
        },
        Err(error) => FontdoneWasmStatus { error, handle: 0 },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_face(handle: usize) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    }
    let ptr = ptr::with_exposed_provenance_mut::<WasmFaceState>(handle);
    // SAFETY: `handle` must come from `fontdone_wasm_open_face` and is consumed here.
    unsafe { drop(Box::from_raw(ptr)) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_freetype(library_present: i32) -> FT_Error {
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    rust_ffi::FT_Done_FreeType(library)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_face_check_truetype_patents(handle: usize) -> FT_Bool {
    rust_ffi::FT_Face_CheckTrueTypePatents(face_ref(handle).map(|state| &state.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_face_set_unpatented_hinting(
    handle: usize,
    value: FT_Bool,
) -> FT_Bool {
    rust_ffi::FT_Face_SetUnpatentedHinting(face_mut(handle).map(|state| &mut state.face), value)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_cbox(
    outline: *const FontdoneWasmOutline,
    acbox: *mut FontdoneWasmBBox,
) {
    if outline.is_null() || acbox.is_null() {
        return;
    }
    let Some(snapshot) = outline_snapshot_from_wasm(outline) else {
        return;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Outline_Get_CBox(Some(&snapshot), Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FontdoneWasmBBox` storage.
    unsafe {
        *acbox = FontdoneWasmBBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_filter(filter: FT_LcdFilter) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilter(None, filter)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_filter_weights(
    weights: *mut c_uchar,
) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilterWeights(None, weights)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_geometry(
    library_present: i32,
    sub: *const FontdoneWasmVector,
) -> FT_Error {
    let mut library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    let rust_sub = if sub.is_null() {
        None
    } else {
        let mut vectors = [rust_ffi::FT_Vector::default(); 3];
        for (index, vector) in vectors.iter_mut().enumerate() {
            // SAFETY: `sub` is non-null and the wasm ABI mirrors the C API's
            // three-vector array contract.
            let source = unsafe { &*sub.add(index) };
            *vector = rust_ffi::FT_Vector {
                x: source.x,
                y: source.y,
            };
        }
        Some(vectors)
    };
    rust_ffi::FT_Library_SetLcdGeometry(
        library.as_mut(),
        rust_sub,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_truetype_engine_type(
    library_present: i32,
) -> FT_TrueTypeEngineType {
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    rust_ffi::FT_Get_TrueType_Engine_Type(library.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_mul_div(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    rust_ffi::FT_MulDiv(a, b, c)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_mul_fix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_MulFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_div_fix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_DivFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_round_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_RoundFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_ceil_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_CeilFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_floor_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_FloorFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_sin(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Sin(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_cos(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Cos(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_tan(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Tan(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    rust_ffi::FT_Atan2(dx, dy)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_angle_diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    rust_ffi::FT_Angle_Diff(angle1, angle2)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_unit(vector: *mut FontdoneWasmVector, angle: FT_Angle) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Unit(rust_vector.as_mut(), angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_rotate(vector: *mut FontdoneWasmVector, angle: FT_Angle) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Rotate(rust_vector.as_mut(), angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_length(vector: *const FontdoneWasmVector) -> FT_Fixed {
    let rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Length(rust_vector.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_polarize(
    vector: *const FontdoneWasmVector,
    length: *mut FT_Fixed,
    angle: *mut FT_Angle,
) {
    let rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    let mut rust_length = if length.is_null() {
        None
    } else {
        // SAFETY: `length` is non-null and points to a wasm ABI `FT_Fixed`.
        Some(unsafe { *length })
    };
    let mut rust_angle = if angle.is_null() {
        None
    } else {
        // SAFETY: `angle` is non-null and points to a wasm ABI `FT_Angle`.
        Some(unsafe { *angle })
    };
    rust_ffi::FT_Vector_Polarize(
        rust_vector.as_ref(),
        rust_length.as_mut(),
        rust_angle.as_mut(),
    );
    if let Some(value) = rust_length {
        // SAFETY: `length` is non-null in the branch that created `rust_length`.
        unsafe { *length = value };
    }
    if let Some(value) = rust_angle {
        // SAFETY: `angle` is non-null in the branch that created `rust_angle`.
        unsafe { *angle = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_from_polar(
    vector: *mut FontdoneWasmVector,
    length: FT_Fixed,
    angle: FT_Angle,
) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_From_Polar(rust_vector.as_mut(), length, angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_transform(
    vector: *mut FontdoneWasmVector,
    matrix: *const FontdoneWasmMatrix,
) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    let rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a wasm ABI matrix.
        let matrix = unsafe { &*matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    rust_ffi::FT_Vector_Transform(rust_vector.as_mut(), rust_matrix.as_ref());
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_matrix_multiply(
    a: *const FontdoneWasmMatrix,
    b: *mut FontdoneWasmMatrix,
) {
    let rust_a = if a.is_null() {
        None
    } else {
        // SAFETY: `a` is non-null and points to a wasm ABI matrix.
        let a = unsafe { &*a };
        Some(rust_ffi::FT_Matrix {
            xx: a.xx,
            xy: a.xy,
            yx: a.yx,
            yy: a.yy,
        })
    };
    let mut rust_b = if b.is_null() {
        None
    } else {
        // SAFETY: `b` is non-null and points to a wasm ABI matrix.
        let b = unsafe { &*b };
        Some(rust_ffi::FT_Matrix {
            xx: b.xx,
            xy: b.xy,
            yx: b.yx,
            yy: b.yy,
        })
    };
    rust_ffi::FT_Matrix_Multiply(rust_a.as_ref(), rust_b.as_mut());
    if let Some(rust_b) = rust_b {
        // SAFETY: `b` is non-null in the branch that created `rust_b`.
        unsafe {
            (*b).xx = rust_b.xx;
            (*b).xy = rust_b.xy;
            (*b).yx = rust_b.yx;
            (*b).yy = rust_b.yy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_matrix_invert(matrix: *mut FontdoneWasmMatrix) -> FT_Error {
    let mut rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a wasm ABI matrix.
        let matrix = unsafe { &*matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    let err = rust_ffi::FT_Matrix_Invert(rust_matrix.as_mut());
    if let Some(rust_matrix) = rust_matrix {
        // SAFETY: `matrix` is non-null in the branch that created `rust_matrix`.
        unsafe {
            (*matrix).xx = rust_matrix.xx;
            (*matrix).xy = rust_matrix.xy;
            (*matrix).yx = rust_matrix.yx;
            (*matrix).yy = rust_matrix.yy;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_pixel_sizes(
    handle: usize,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    rust_ffi::FT_Set_Pixel_Sizes(&mut face.face, pixel_width, pixel_height)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_char_size(
    handle: usize,
    char_width: FT_Long,
    char_height: FT_Long,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    rust_ffi::FT_Set_Char_Size(
        &mut face.face,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_request_size(
    handle: usize,
    req: *const FontdoneWasmSizeRequest,
) -> FT_Error {
    let request = if req.is_null() {
        None
    } else {
        // SAFETY: `req` is non-null and copied by value only.
        let req = unsafe { *req };
        Some(rust_ffi::FT_Size_RequestRec {
            type_: req.type_,
            width: req.width,
            height: req.height,
            horiResolution: req.horiResolution,
            vertResolution: req.vertResolution,
        })
    };
    rust_ffi::FT_Request_Size(
        face_mut(handle).map(|face| &mut face.face),
        request.as_ref(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_index(handle: usize, char_code: FT_ULong) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&face.face, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_variant_index(
    handle: usize,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    rust_ffi::FT_Face_GetCharVariantIndex(Some(&face.face), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_variant_is_default(
    handle: usize,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(face) = face_ref(handle) else {
        return -1;
    };
    rust_ffi::FT_Face_GetCharVariantIsDefault(Some(&face.face), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_kerning(
    handle: usize,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    out: *mut FontdoneWasmVector,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_Kerning(
        Some(&face.face),
        left_glyph,
        right_glyph,
        kern_mode,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out = FontdoneWasmVector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_select_charmap(
    handle: usize,
    encoding: FT_Encoding,
) -> FT_Error {
    rust_ffi::FT_Select_Charmap(
        face_mut(handle).map(|face| &mut face.face),
        encoding,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_charmap_count(handle: usize) -> FT_UInt {
    face_ref(handle).map_or(0, |face| rust_ffi::FT_Face_Charmap_Count(&face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_active_charmap_index(handle: usize) -> FT_Int {
    face_ref(handle).map_or(-1, |face| {
        rust_ffi::FT_Face_Active_Charmap_Index(&face.face)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_charmap(
    handle: usize,
    index: FT_UInt,
    out: *mut FontdoneWasmCharmap,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(info) = rust_ffi::FT_Face_Charmap_Info(&face.face, index) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmCharmap {
            index,
            encoding: info.encoding,
            platform_id: info.platform_id,
            encoding_id: info.encoding_id,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cmap_format(handle: usize, index: FT_UInt) -> FT_Long {
    let Some(face) = face_ref(handle) else {
        return -1;
    };
    let charmap = rust_ffi::FT_Face_Charmap(&face.face, index);
    rust_ffi::FT_Get_CMap_Format(charmap) as FT_Long
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cmap_language_id(handle: usize, index: FT_UInt) -> FT_ULong {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    let charmap = rust_ffi::FT_Face_Charmap(&face.face, index);
    rust_ffi::FT_Get_CMap_Language_ID(charmap) as FT_ULong
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_charmap(handle: usize, index: FT_UInt) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let charmap = rust_ffi::FT_Face_Charmap(&face.face, index);
    rust_ffi::FT_Set_Charmap(Some(&mut face.face), charmap)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_charmap_from_face(
    handle: usize,
    charmap_face_handle: usize,
    index: FT_UInt,
) -> FT_Error {
    let charmap = {
        let Some(charmap_face) = face_ref(charmap_face_handle) else {
            return rust_ffi::FT_Err_Invalid_CharMap_Handle as FT_Error;
        };
        rust_ffi::FT_Face_Charmap(&charmap_face.face, index)
    };
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    rust_ffi::FT_Set_Charmap(Some(&mut face.face), charmap)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_fstype_flags(handle: usize) -> FT_UShort {
    rust_ffi::FT_Get_FSType_Flags(face_ref(handle).map(|face| &face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_gasp(handle: usize, ppem: FT_UInt) -> FT_Int {
    rust_ffi::FT_Get_Gasp(face_ref(handle).map(|face| &face.face), ppem)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_glyph_name(
    handle: usize,
    glyph_index: FT_UInt,
    buffer: *mut FT_Byte,
    buffer_max: FT_UInt,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if buffer.is_null() || buffer_max == 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `buffer` is non-null, and the WASM caller provides a writable
    // linear-memory range of `buffer_max` bytes.
    let buffer = unsafe { slice::from_raw_parts_mut(buffer, buffer_max as usize) };
    match rust_ffi::FT_Get_Glyph_Name(&face.face, glyph_index, buffer) {
        Ok(_) => rust_ffi::FT_Err_Ok,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_name_index(
    handle: usize,
    glyph_name: *const FT_Byte,
    glyph_name_len: FT_UInt,
) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    if glyph_name.is_null() {
        return 0;
    }
    // SAFETY: `glyph_name` is non-null, and the WASM caller provides a
    // readable linear-memory range of `glyph_name_len` bytes.
    let bytes = unsafe { slice::from_raw_parts(glyph_name, glyph_name_len as usize) };
    let Ok(glyph_name) = std::str::from_utf8(bytes) else {
        return 0;
    };
    rust_ffi::FT_Get_Name_Index(Some(&face.face), Some(glyph_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_postscript_name(
    handle: usize,
    out: *mut FontdoneWasmString,
) -> FT_Bool {
    // SAFETY: the caller provides writable storage for the output record or null.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return 0;
    };
    let Some(name) = face_ref(handle).and_then(|face| rust_ffi::FT_Get_Postscript_Name(&face.face))
    else {
        *out = FontdoneWasmString::default();
        return 0;
    };
    out.string = name.as_ptr();
    out.string_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_named_instance(
    handle: usize,
    instance_index: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Set_Named_Instance(Some(&mut face.face), instance_index);
    if err == rust_ffi::FT_Err_Ok {
        face.slot = None;
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_name_count(handle: usize) -> FT_UInt {
    rust_ffi::FT_Get_Sfnt_Name_Count(face_ref(handle).map(|face| &face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_name(
    handle: usize,
    idx: FT_UInt,
    out: *mut FontdoneWasmSfntName,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Get_Sfnt_Name(face_ref(handle).map(|face| &face.face), idx, None);
    }
    let mut name = rust_ffi::FT_SfntName::default();
    let error = rust_ffi::FT_Get_Sfnt_Name(
        face_ref(handle).map(|face| &face.face),
        idx,
        Some(&mut name),
    );
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out = FontdoneWasmSfntName {
                platform_id: name.platform_id,
                encoding_id: name.encoding_id,
                language_id: name.language_id,
                name_id: name.name_id,
                string: name.string.cast_const(),
                string_len: name.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_os2(
    handle: usize,
    tag: FT_Sfnt_Tag,
    out: *mut FontdoneWasmOs2,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let table = rust_ffi::FT_Get_Sfnt_Table(&face.face, tag);
    if table.is_null() {
        return rust_ffi::FT_Err_Invalid_Table as FT_Error;
    }
    // SAFETY: `FT_Get_Sfnt_Table` returns a live face-owned `TT_OS2` pointer for this tag.
    let os2 = unsafe { &*table.cast::<rust_ffi::TT_OS2>() };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmOs2 {
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
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_sfnt_table(
    handle: usize,
    tag: FT_ULong,
    offset: FT_Long,
    out_buffer: *mut FT_Byte,
    out_length: *mut FT_ULong,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if out_length.is_null() {
        return match rust_ffi::FT_Load_Sfnt_Table(&face.face, tag, offset, None) {
            Ok(Some(bytes)) => {
                if !out_buffer.is_null() {
                    // SAFETY: out_buffer has enough space for the selected table bytes.
                    unsafe {
                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buffer, bytes.len());
                    }
                }
                rust_ffi::FT_Err_Ok as FT_Error
            }
            Ok(None) => rust_ffi::FT_Err_Ok as FT_Error,
            Err(err) => err as FT_Error,
        };
    }
    // SAFETY: caller provides a writable FT_ULong.
    let mut len_val = unsafe { *out_length };
    match rust_ffi::FT_Load_Sfnt_Table(&face.face, tag, offset, Some(&mut len_val)) {
        Ok(Some(bytes)) => {
            let copy_len = bytes.len().min(len_val as usize);
            if !out_buffer.is_null() {
                // SAFETY: out_buffer has at least len_val bytes.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buffer, copy_len);
                }
            }
            // SAFETY: writable FT_ULong out-param.
            unsafe { *out_length = copy_len as FT_ULong };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Ok(None) => {
            // SAFETY: writable FT_ULong out-param (length probe result).
            unsafe { *out_length = len_val };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Err(err) => err as FT_Error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_sfnt_table_info(
    handle: usize,
    table_index: FT_UInt,
    out_tag: *mut FT_ULong,
    out_length: *mut FT_ULong,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut tag_out: rust_ffi::FT_ULong = 0;
    let mut length_out: rust_ffi::FT_ULong = 0;
    let tag_ref = if out_tag.is_null() {
        None
    } else {
        Some(&mut tag_out)
    };
    let length_ref = if out_length.is_null() {
        None
    } else {
        Some(&mut length_out)
    };
    let err = rust_ffi::FT_Sfnt_Table_Info(&face.face, table_index, tag_ref, length_ref);
    if err == rust_ffi::FT_Err_Ok {
        if !out_tag.is_null() {
            // SAFETY: writable FT_ULong out-param. Copying after the core call
            // avoids creating aliased `&mut` references for caller pointers.
            unsafe { *out_tag = tag_out as FT_ULong };
        }
        if !out_length.is_null() {
            // SAFETY: writable FT_ULong out-param. C writes tag before length,
            // so an aliased caller pointer ends with the length value.
            unsafe { *out_length = length_out as FT_ULong };
        }
    }
    err as FT_Error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_first_char(handle: usize, agindex: *mut FT_UInt) -> FT_ULong {
    let mut glyph_index = 0;
    let char_code =
        rust_ffi::FT_Get_First_Char(face_ref(handle).map(|face| &face.face), Some(&mut glyph_index));
    if !agindex.is_null() {
        // SAFETY: `agindex` is non-null and caller provides writable storage.
        unsafe { *agindex = glyph_index };
    }
    char_code
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_next_char(
    handle: usize,
    char_code: FT_ULong,
    agindex: *mut FT_UInt,
) -> FT_ULong {
    let mut glyph_index = 0;
    let next_char = rust_ffi::FT_Get_Next_Char(
        face_ref(handle).map(|face| &face.face),
        char_code,
        Some(&mut glyph_index),
    );
    if !agindex.is_null() {
        // SAFETY: `agindex` is non-null and caller provides writable storage.
        unsafe { *agindex = glyph_index };
    }
    next_char
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_version(
    library_present: i32,
    amajor: *mut i32,
    aminor: *mut i32,
    apatch: *mut i32,
) {
    let library = rust_ffi::FT_Init_FreeType();
    let library = if library_present != 0 {
        Some(&library)
    } else {
        None
    };
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    rust_ffi::FT_Library_Version(library, Some(&mut major), Some(&mut minor), Some(&mut patch));
    if !amajor.is_null() {
        // SAFETY: `amajor` is non-null and caller provides writable storage.
        unsafe { *amajor = major };
    }
    if !aminor.is_null() {
        // SAFETY: `aminor` is non-null and caller provides writable storage.
        unsafe { *aminor = minor };
    }
    if !apatch.is_null() {
        // SAFETY: `apatch` is non-null and caller provides writable storage.
        unsafe { *apatch = patch };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_char(
    handle: usize,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Char(&face.face, char_code, load_flags) {
        Ok(slot) => {
            face.slot = Some(slot);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_glyph(
    handle: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Glyph(&face.face, glyph_index, load_flags) {
        Ok(slot) => {
            face.slot = Some(slot);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_advance(
    handle: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    padvance: *mut FT_Fixed,
) -> FT_Error {
    if padvance.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advance(&face.face, glyph_index, load_flags) {
        Ok(advance) => {
            // SAFETY: `padvance` is non-null and caller provides writable storage.
            unsafe { *padvance = advance };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_advances(
    handle: usize,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
    padvances: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(out_len) = usize::try_from(count) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let out = if out_len == 0 {
        None
    } else if padvances.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    } else {
        Some(padvances)
    };
    match rust_ffi::FT_Get_Advances(&face.face, start, count, load_flags) {
        Ok(advances) => {
            if advances.len() != out_len {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            if let Some(out) = out {
                // SAFETY: `out` is non-null and caller promises at least `count` writable entries.
                let out = unsafe { slice::from_raw_parts_mut(out, out_len) };
                out.copy_from_slice(&advances);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_subglyph_info(
    handle: usize,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FontdoneWasmMatrix,
) -> FT_Error {
    if p_index.is_null()
        || p_flags.is_null()
        || p_arg1.is_null()
        || p_arg2.is_null()
        || p_transform.is_null()
    {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_ref() else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut index = 0;
    let mut flags = 0;
    let mut arg1 = 0;
    let mut arg2 = 0;
    let mut transform = rust_ffi::FT_Matrix::default();
    let error = rust_ffi::FT_Get_SubGlyph_Info(
        Some(slot),
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

    // SAFETY: output pointers are checked non-null and caller provides writable storage.
    unsafe {
        *p_index = index;
        *p_flags = flags;
        *p_arg1 = arg1;
        *p_arg2 = arg2;
        *p_transform = FontdoneWasmMatrix {
            xx: transform.xx,
            xy: transform.xy,
            yx: transform.yx,
            yy: transform.yy,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_render_glyph(handle: usize, render_mode: i32) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.clone() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    match rust_ffi::FT_Render_Glyph(slot, render_mode) {
        Ok(rendered) => {
            face.slot = Some(rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_slot(
    handle: usize,
    out: *mut FontdoneWasmGlyphSlot,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_ref() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe { *out = slot_to_wasm(slot) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_size_metrics(
    handle: usize,
    out: *mut FontdoneWasmSizeMetrics,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let metrics = rust_ffi::FT_Size_Metrics(&face.face);
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmSizeMetrics {
            x_ppem: metrics.x_ppem,
            y_ppem: metrics.y_ppem,
            x_scale: metrics.x_scale,
            y_scale: metrics.y_scale,
            ascender: metrics.ascender,
            descender: metrics.descender,
            height: metrics.height,
            max_advance: metrics.max_advance,
        }
    };
    rust_ffi::FT_Err_Ok
}

fn slot_to_wasm(slot: &rust_ffi::FT_GlyphSlot) -> FontdoneWasmGlyphSlot {
    let bitmap = slot
        .bitmap
        .as_ref()
        .map(|bitmap| FontdoneWasmBitmap {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: bitmap.buffer.as_ptr(),
            buffer_len: bitmap.buffer.len(),
            num_grays: bitmap.num_grays,
            pixel_mode: bitmap.pixel_mode,
        })
        .unwrap_or_default();
    FontdoneWasmGlyphSlot {
        glyph_index: slot.glyph_index,
        metrics: FontdoneWasmGlyphMetrics {
            width: slot.metrics.width,
            height: slot.metrics.height,
            horiBearingX: slot.metrics.horiBearingX,
            horiBearingY: slot.metrics.horiBearingY,
            horiAdvance: slot.metrics.horiAdvance,
            vertBearingX: slot.metrics.vertBearingX,
            vertBearingY: slot.metrics.vertBearingY,
            vertAdvance: slot.metrics.vertAdvance,
        },
        advance: FontdoneWasmVector {
            x: slot.advance.x,
            y: slot.advance.y,
        },
        format: slot.format,
        num_subglyphs: slot.num_subglyphs,
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
    }
}

fn outline_snapshot_from_wasm(
    outline: *const FontdoneWasmOutline,
) -> Option<rust_ffi::FT_OutlineSnapshot> {
    if outline.is_null() {
        return None;
    }
    // SAFETY: `outline` is non-null; wasm ABI callers must pass a valid outline record.
    let outline = unsafe { &*outline };
    let n_points = usize::from(outline.n_points);
    let n_contours = usize::from(outline.n_contours);
    if (n_points > 0 && outline.points.is_null()) || (n_contours > 0 && outline.contours.is_null())
    {
        return None;
    }
    let points = if n_points == 0 {
        Vec::new()
    } else {
        // SAFETY: `points` is non-null for `n_points > 0`; the caller provides
        // `n_points` readable vector records for the duration of this call.
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

fn face_ref(handle: usize) -> Option<&'static WasmFaceState> {
    if handle == 0 {
        return None;
    }
    let ptr = ptr::with_exposed_provenance::<WasmFaceState>(handle);
    // SAFETY: non-zero handles are expected to be produced by `fontdone_wasm_open_face`.
    Some(unsafe { &*ptr })
}

fn face_mut(handle: usize) -> Option<&'static mut WasmFaceState> {
    if handle == 0 {
        return None;
    }
    let ptr = ptr::with_exposed_provenance_mut::<WasmFaceState>(handle);
    // SAFETY: non-zero handles are expected to be produced by `fontdone_wasm_open_face`.
    Some(unsafe { &mut *ptr })
}
