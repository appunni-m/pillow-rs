#![allow(missing_docs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{c_uchar, c_void};
use std::ptr;
use std::slice;

use fontdone::ffi as rust_ffi;

pub type FT_Error = i32;
pub type FT_Fixed = i64;
pub type FT_Int32 = i32;
pub type FT_Long = i64;
pub type FT_ULong = u64;
pub type FT_UInt = u32;

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
    pub bitmap: Option<AbiBitmapSnapshot>,
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
pub fn abi_slot_snapshot(handle: usize) -> Option<AbiSlotSnapshot> {
    let mut slot = FontdoneWasmGlyphSlot::default();
    if fontdone_wasm_get_slot(handle, &mut slot) != rust_ffi::FT_Err_Ok {
        return None;
    }
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
        bitmap,
    })
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
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let ptr = ptr::with_exposed_provenance_mut::<WasmFaceState>(handle);
    // SAFETY: `handle` must come from `fontdone_wasm_open_face` and is consumed here.
    unsafe { drop(Box::from_raw(ptr)) };
    rust_ffi::FT_Err_Ok
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
pub extern "C" fn fontdone_wasm_get_char_index(handle: usize, char_code: FT_ULong) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&face.face, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_char(
    handle: usize,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
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
        return rust_ffi::FT_Err_Invalid_Argument;
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
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
    }
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
