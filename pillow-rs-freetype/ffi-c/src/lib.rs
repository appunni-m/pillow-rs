#![allow(missing_docs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{c_int, c_long, c_uchar, c_uint, c_ulong, c_void};
use std::ptr::{self, NonNull};
use std::slice;

use fontdone::ffi as rust_ffi;

pub type FT_Error = c_int;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_Long = c_long;
pub type FT_ULong = c_ulong;
pub type FT_Pos = i64;
pub type FT_Fixed = i64;
pub type FT_F26Dot6 = i64;
pub type FT_UShort = u16;
pub type FT_Render_Mode = i32;
pub type FT_Pixel_Mode = i32;
pub type FT_Glyph_Format = i32;

pub type FT_Library = *mut FT_LibraryRec;
pub type FT_Face = *mut FT_FaceRec;
pub type FT_GlyphSlot = *mut FT_GlyphSlotRec;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
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
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: *mut c_uchar,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
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
    source_face: FT_Face,
    load_flags: FT_Int32,
}

#[repr(C)]
pub struct FT_FaceRec {
    pub glyph: FT_GlyphSlot,
    pub internal: *mut c_void,
}

#[repr(C)]
pub struct FT_LibraryRec {
    pub internal: *mut c_void,
}

struct FaceState {
    inner: rust_ffi::FT_Face,
}

#[derive(Clone)]
pub struct FontdoneCSlotSnapshot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: Option<FontdoneCBitmapSnapshot>,
}

#[derive(Clone)]
pub struct FontdoneCBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub left: FT_Int,
    pub top: FT_Int,
    pub buffer: Vec<u8>,
}

pub fn fontdone_test_slot_snapshot(face: FT_Face) -> Option<FontdoneCSlotSnapshot> {
    let face = NonNull::new(face)?;
    // SAFETY: this helper is for tests using handles produced by this crate.
    let slot = unsafe { (*face.as_ptr()).glyph };
    let slot = NonNull::new(slot)?;
    // SAFETY: `slot` is owned by `face` and remains live for this call.
    let slot = unsafe { slot.as_ref() };
    let bitmap = if slot.bitmap.buffer.is_null() {
        None
    } else {
        let len = usize::try_from(slot.bitmap.pitch.abs())
            .ok()?
            .checked_mul(usize::try_from(slot.bitmap.rows).ok()?)?;
        // SAFETY: buffer and length are owned by the live slot.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, len) }.to_vec();
        Some(FontdoneCBitmapSnapshot {
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
    Some(FontdoneCSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        bitmap,
    })
}

pub fn fontdone_test_render_glyph(face: FT_Face, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(face) = NonNull::new(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: this helper is for tests using handles produced by this crate.
    let slot = unsafe { (*face.as_ptr()).glyph };
    FT_Render_Glyph(slot, render_mode)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_malloc(size: usize) -> *mut c_void {
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return ptr::null_mut();
    };
    // SAFETY: `layout` is constructed above and has non-zero size.
    unsafe { alloc(layout).cast::<c_void>() }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_free(ptr: *mut c_void, size: usize) {
    if ptr.is_null() {
        return;
    }
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return;
    };
    // SAFETY: callers must pass a pointer returned by `fontdone_malloc` with the same size.
    unsafe { dealloc(ptr.cast::<u8>(), layout) };
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
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `library` must be a live handle returned by `FT_Init_FreeType`.
    unsafe {
        let library = Box::from_raw(library.as_ptr());
        if !library.internal.is_null() {
            drop(Box::from_raw(
                library.internal.cast::<rust_ffi::FT_Library>(),
            ));
        }
    }
    rust_ffi::FT_Err_Ok
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
            let face = Box::new(FT_FaceRec {
                glyph: ptr::null_mut(),
                internal: Box::into_raw(Box::new(FaceState { inner })).cast::<c_void>(),
            });
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
        return rust_ffi::FT_Err_Invalid_Argument;
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
    rust_ffi::FT_Set_Char_Size(
        &mut state.inner,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    )
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
    rust_ffi::FT_Set_Pixel_Sizes(&mut state.inner, pixel_width, pixel_height)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Char_Index(face: FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&state.inner, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Char(
    face: FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
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
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Load_Glyph(&state.inner, glyph_index, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Render_Glyph(slot: FT_GlyphSlot, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    let source_face = unsafe { (*slot_ptr.as_ptr()).source_face };
    let Some(source_state) = face_state(source_face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let load_flags = unsafe { (*slot_ptr.as_ptr()).load_flags };
    let render_flags = load_flags_for_render_mode(load_flags, render_mode);
    match rust_ffi::FT_Load_Glyph(
        &source_state.inner,
        unsafe { (*slot_ptr.as_ptr()).glyph_index },
        render_flags,
    ) {
        Ok(rendered) => store_slot(source_face, rendered, render_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Size_Metrics_Get(face: FT_Face) -> FT_Size_Metrics {
    let Some(state) = face_state(face) else {
        return FT_Size_Metrics::default();
    };
    rust_size_metrics_to_abi(rust_ffi::FT_Size_Metrics(&state.inner))
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

fn rust_slot_to_abi(
    slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
) -> FT_GlyphSlotRec {
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

fn load_flags_for_render_mode(load_flags: FT_Int32, render_mode: FT_Render_Mode) -> FT_Int32 {
    const TARGET_MASK: FT_Int32 = 15 << 16;
    let target = match render_mode {
        0 | 1 => 0,
        2 => 2 << 16,
        3 => 3 << 16,
        4 => 4 << 16,
        _ => 0,
    };
    (load_flags | rust_ffi::FT_LOAD_RENDER) & !TARGET_MASK | target
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
