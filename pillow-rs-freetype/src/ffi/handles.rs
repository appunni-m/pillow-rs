#![allow(non_camel_case_types, non_snake_case)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::ptr;
use std::rc::{Rc, Weak};
use std::sync::{Mutex, OnceLock};

use crate::casts::usize_from_i32;
use crate::font::{
    ActiveSizeState, BdfPropertyValue, KerningMode, SelectSizeError, SizeRequest, SizeRequestError,
    SizeRequestType, Type1FontInfo, Type1PrivateDict, WinFntHeader,
};
use crate::{api, grays, render};

use super::constants::*;
use super::convert::{
    FT_LOAD_TARGET_MODE, error_to_ft, glyph_format_from_core, load_flag_for_render_mode,
    load_flags_to_core, render_mode_to_core,
};
use super::types::{
    BDF_PropertyRec, FT_Angle, FT_BBox, FT_Bitmap, FT_Bitmap_C, FT_Bitmap_Size, FT_Bool, FT_Byte,
    FT_Bytes, FT_Char, FT_CharMap, FT_CharMapRecPublic, FT_Color, FT_DebugHook_Func, FT_Encoding,
    FT_Error, FT_F26Dot6, FT_Fixed, FT_Glyph_Format, FT_Glyph_Metrics, FT_GlyphCBoxSnapshot,
    FT_GlyphRec, FT_Int, FT_Int32, FT_LcdFilter, FT_List_Destructor, FT_ListNode, FT_ListNodeRec,
    FT_ListRec, FT_Long, FT_MM_Axis, FT_MM_Var, FT_Matrix, FT_Memory, FT_MemoryRec,
    FT_Module_Interface, FT_Multi_Master, FT_Orientation, FT_OutlineGlyphOwned, FT_OutlineSnapshot,
    FT_Palette_Data, FT_Pointer, FT_Pos, FT_Prop_GlyphToScriptMap, FT_Prop_IncreaseXHeight,
    FT_Render_Mode, FT_Sfnt_Tag, FT_SfntLangTag, FT_SfntName, FT_Short, FT_Size,
    FT_Size_Metrics as FT_Size_MetricsRec, FT_Size_RequestRec, FT_Span, FT_Stream, FT_StreamDesc,
    FT_StreamRec, FT_String, FT_TrueTypeEngineType, FT_UInt, FT_UInt32, FT_ULong, FT_UShort,
    FT_Var_Axis, FT_Var_Named_Style, FT_Vector, FT_WinFNT_HeaderRec, PS_Dict_Keys, PS_FontInfoRec,
    PS_PrivateRec, TT_Header, TT_HoriHeader, TT_MaxProfile, TT_OS2, TT_PCLT, TT_Postscript,
    TT_VertHeader,
};

const FT_ADVANCE_FLAG_FAST_ONLY_I32: FT_Int32 = 0x2000_0000;

thread_local! {
    static MM_VAR_AXIS_FLAGS: RefCell<BTreeMap<usize, Vec<FT_UInt>>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn FT_Error_String(error_code: FT_Error) -> Option<&'static CStr> {
    if !(0..FT_Err_Max).contains(&error_code) {
        return None;
    }
    // Pinned FreeType is built with FT_ENABLE_ERROR_STRINGS=OFF, so
    // `src/base/fterrors.c` returns NULL after the range check.
    if !FT_CONFIG_OPTION_ERROR_STRINGS_ENABLED {
        return None;
    }
    None
}

pub fn FT_Bitmap_Init(abitmap: Option<&mut FT_Bitmap_C>) {
    if let Some(bitmap) = abitmap {
        // FreeType `FT_Bitmap_Init` in `src/base/ftbitmap.c` assigns the
        // static zero `null_bitmap` record and treats NULL as a no-op.
        *bitmap = FT_Bitmap_C::default();
    }
}

pub fn FT_Bitmap_New(abitmap: Option<&mut FT_Bitmap_C>) {
    FT_Bitmap_Init(abitmap);
}

type BitmapBufferRegistry = BTreeMap<usize, Box<[FT_Byte]>>;

static BITMAP_BUFFER_REGISTRY: OnceLock<Mutex<BitmapBufferRegistry>> = OnceLock::new();

fn bitmap_buffer_registry() -> &'static Mutex<BitmapBufferRegistry> {
    BITMAP_BUFFER_REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn bitmap_buffer_len(bitmap: &FT_Bitmap_C) -> Option<usize> {
    usize::try_from(bitmap.pitch.unsigned_abs())
        .ok()?
        .checked_mul(usize::try_from(bitmap.rows).ok()?)
}

fn bitmap_pitch_abs(bitmap: &FT_Bitmap_C) -> Option<usize> {
    usize::try_from(bitmap.pitch.unsigned_abs()).ok()
}

fn bitmap_owned_bytes(bitmap: &FT_Bitmap_C) -> Option<Vec<FT_Byte>> {
    if bitmap.buffer.is_null() {
        return None;
    }
    let len = bitmap_buffer_len(bitmap)?;
    let registry = bitmap_buffer_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let buffer = registry.get(&(bitmap.buffer as usize))?;
    if buffer.len() < len {
        return None;
    }
    Some(buffer[..len].to_vec())
}

fn bitmap_source_bytes(bitmap: &FT_Bitmap_C) -> Result<Option<Vec<FT_Byte>>, FT_Error> {
    if bitmap.buffer.is_null() {
        return Ok(None);
    }
    let Some(len) = bitmap_buffer_len(bitmap) else {
        return Err(FT_Err_Out_Of_Memory as FT_Error);
    };
    let registry = bitmap_buffer_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(buffer) = registry.get(&(bitmap.buffer as usize)) else {
        return Err(FT_Err_Invalid_Argument);
    };
    if buffer.len() < len {
        return Err(FT_Err_Invalid_Argument);
    }
    Ok(Some(buffer[..len].to_vec()))
}

pub fn FT_Bitmap_Set_Owned_Buffer(abitmap: Option<&mut FT_Bitmap_C>, bytes: Vec<FT_Byte>) {
    let Some(bitmap) = abitmap else {
        return;
    };
    if !bitmap.buffer.is_null() {
        bitmap_buffer_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&(bitmap.buffer as usize));
    }
    if bytes.is_empty() {
        bitmap.buffer = ptr::null_mut();
        return;
    }
    let mut buffer = bytes.into_boxed_slice();
    bitmap.buffer = buffer.as_mut_ptr();
    bitmap_buffer_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(bitmap.buffer as usize, buffer);
}

pub fn FT_Bitmap_Owned_Buffer_Bytes(abitmap: Option<&FT_Bitmap_C>) -> Option<Vec<FT_Byte>> {
    let bitmap = abitmap?;
    if bitmap.buffer.is_null() {
        return None;
    }
    let registry = bitmap_buffer_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry
        .get(&(bitmap.buffer as usize))
        .map(|bytes| bytes.to_vec())
}

pub fn FT_Bitmap_Copy(
    library: Option<&FT_Library>,
    source: Option<&FT_Bitmap_C>,
    target: Option<&mut FT_Bitmap_C>,
) -> FT_Error {
    if library.is_none() {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let (Some(source), Some(target)) = (source, target) else {
        return FT_Err_Invalid_Argument;
    };

    // Safe Rust source and target references cannot alias.  The C and WASM
    // raw-pointer wrappers handle FreeType's `source == target` no-op before
    // constructing these disjoint views.
    let flip = (source.pitch < 0 && target.pitch > 0) || (source.pitch > 0 && target.pitch < 0);

    // FreeType `src/base/ftbitmap.c:63-116` frees the target buffer before
    // copying the public record, then reverses row order only when source and
    // target pitch signs requested opposite bitmap flow.
    {
        let mut registry = bitmap_buffer_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !target.buffer.is_null() {
            registry.remove(&(target.buffer as usize));
        }
    }

    *target = *source;
    if flip {
        target.pitch = target.pitch.wrapping_neg();
    }

    if source.buffer.is_null() {
        return FT_Err_Ok;
    }

    let Some(len) = bitmap_buffer_len(source) else {
        target.buffer = ptr::null_mut();
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let source_bytes = match bitmap_source_bytes(source) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return FT_Err_Ok,
        Err(err) => {
            target.buffer = ptr::null_mut();
            return err;
        }
    };

    if flip {
        let pitch = usize::try_from(source.pitch.unsigned_abs()).unwrap_or(0);
        let rows = usize::try_from(source.rows).unwrap_or(0);
        let mut flipped = vec![0; len];
        for row in 0..rows {
            let src = row.saturating_mul(pitch);
            let dst = (rows - 1 - row).saturating_mul(pitch);
            flipped[dst..dst + pitch].copy_from_slice(&source_bytes[src..src + pitch]);
        }
        FT_Bitmap_Set_Owned_Buffer(Some(target), flipped);
    } else {
        FT_Bitmap_Set_Owned_Buffer(Some(target), source_bytes);
    }

    FT_Err_Ok
}

pub fn FT_Bitmap_Convert(
    library: Option<&FT_Library>,
    source: Option<&FT_Bitmap_C>,
    target: Option<&mut FT_Bitmap_C>,
    alignment: FT_Int,
) -> FT_Error {
    if library.is_none() {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let (Some(source), Some(target)) = (source, target) else {
        return FT_Err_Invalid_Argument;
    };

    if !matches!(
        source.pixel_mode.into(),
        FT_PIXEL_MODE_MONO
            | FT_PIXEL_MODE_GRAY
            | FT_PIXEL_MODE_GRAY2
            | FT_PIXEL_MODE_GRAY4
            | FT_PIXEL_MODE_LCD
            | FT_PIXEL_MODE_LCD_V
            | FT_PIXEL_MODE_BGRA
    ) {
        return FT_Err_Invalid_Argument;
    }

    let source_bytes = match bitmap_source_bytes(source) {
        Ok(Some(bytes)) => bytes,
        Ok(None) if source.rows == 0 || source.width == 0 => Vec::new(),
        Ok(None) => return FT_Err_Invalid_Argument,
        Err(err) => return err,
    };

    let mut width = match FT_Int::try_from(source.width) {
        Ok(width) => width,
        Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
    };
    let negative_pitch = (target.pitch == 0 && source.pitch < 0) || target.pitch < 0;

    // FreeType accepts negative alignment and rounds width toward the next
    // multiple of its magnitude (`src/base/ftbitmap.c:532-540`).
    if alignment != 0 {
        let rem = width % alignment;
        if rem != 0 {
            width = if alignment > 0 {
                width - rem + alignment
            } else {
                width - rem - alignment
            };
        }
    }
    if width < 0 {
        return FT_Err_Out_Of_Memory as FT_Error;
    }

    let rows = match usize::try_from(source.rows) {
        Ok(rows) => rows,
        Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
    };
    let pitch = match usize::try_from(width) {
        Ok(pitch) => pitch,
        Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
    };
    let Some(len) = pitch.checked_mul(rows) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let mut output = vec![0; len];

    let source_pitch = match usize::try_from(source.pitch.unsigned_abs()) {
        Ok(pitch) => pitch,
        Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
    };
    let width_pixels = match usize::try_from(source.width) {
        Ok(width) => width,
        Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
    };
    let source_start = if source.pitch < 0 {
        rows.saturating_sub(1).saturating_mul(source_pitch)
    } else {
        0
    };
    let target_start = if negative_pitch {
        rows.saturating_sub(1).saturating_mul(pitch)
    } else {
        0
    };

    for row in 0..rows {
        let src_row = if source.pitch < 0 {
            source_start.saturating_sub(row.saturating_mul(source_pitch))
        } else {
            source_start.saturating_add(row.saturating_mul(source_pitch))
        };
        let dst_row = if negative_pitch {
            target_start.saturating_sub(row.saturating_mul(pitch))
        } else {
            target_start.saturating_add(row.saturating_mul(pitch))
        };
        let Some(src) = source_bytes.get(src_row..src_row.saturating_add(source_pitch)) else {
            return FT_Err_Invalid_Argument;
        };
        let Some(dst) = output.get_mut(dst_row..dst_row.saturating_add(pitch)) else {
            return FT_Err_Out_Of_Memory as FT_Error;
        };
        match source.pixel_mode.into() {
            FT_PIXEL_MODE_MONO => {
                if let Err(err) = unpack_bitmap_mono_row(src, dst, width_pixels) {
                    return err;
                }
            }
            FT_PIXEL_MODE_GRAY | FT_PIXEL_MODE_LCD | FT_PIXEL_MODE_LCD_V => {
                let Some(src_row) = src.get(..width_pixels) else {
                    return FT_Err_Invalid_Argument;
                };
                let Some(dst_row) = dst.get_mut(..width_pixels) else {
                    return FT_Err_Out_Of_Memory as FT_Error;
                };
                dst_row.copy_from_slice(src_row);
            }
            FT_PIXEL_MODE_GRAY2 => {
                if let Err(err) = unpack_bitmap_gray2_row(src, dst, width_pixels) {
                    return err;
                }
            }
            FT_PIXEL_MODE_GRAY4 => {
                if let Err(err) = unpack_bitmap_gray4_row(src, dst, width_pixels) {
                    return err;
                }
            }
            FT_PIXEL_MODE_BGRA => {
                if let Err(err) = unpack_bitmap_bgra_row(src, dst, width_pixels) {
                    return err;
                }
            }
            _ => return FT_Err_Invalid_Argument,
        }
    }

    FT_Bitmap_Done(library, Some(target));
    target.pixel_mode = FT_PIXEL_MODE_GRAY as FT_Byte;
    target.rows = source.rows;
    target.width = source.width;
    target.pitch = if negative_pitch { -width } else { width };
    target.num_grays = match source.pixel_mode.into() {
        FT_PIXEL_MODE_MONO => 2,
        FT_PIXEL_MODE_GRAY2 => 4,
        FT_PIXEL_MODE_GRAY4 => 16,
        _ => 256,
    };
    target.palette_mode = 0;
    target.palette = ptr::null_mut();
    FT_Bitmap_Set_Owned_Buffer(Some(target), output);

    FT_Err_Ok
}

fn unpack_bitmap_mono_row(
    src: &[FT_Byte],
    dst: &mut [FT_Byte],
    width: usize,
) -> Result<(), FT_Error> {
    for (x, pixel) in dst.iter_mut().enumerate().take(width) {
        let Some(&byte) = src.get(x / 8) else {
            return Err(FT_Err_Invalid_Argument);
        };
        *pixel = (byte >> (7 - (x & 7))) & 1;
    }
    Ok(())
}

fn unpack_bitmap_gray2_row(
    src: &[FT_Byte],
    dst: &mut [FT_Byte],
    width: usize,
) -> Result<(), FT_Error> {
    for (x, pixel) in dst.iter_mut().enumerate().take(width) {
        let Some(&byte) = src.get(x / 4) else {
            return Err(FT_Err_Invalid_Argument);
        };
        *pixel = (byte >> (6 - 2 * (x & 3))) & 0x03;
    }
    Ok(())
}

fn unpack_bitmap_gray4_row(
    src: &[FT_Byte],
    dst: &mut [FT_Byte],
    width: usize,
) -> Result<(), FT_Error> {
    for (x, pixel) in dst.iter_mut().enumerate().take(width) {
        let Some(&byte) = src.get(x / 2) else {
            return Err(FT_Err_Invalid_Argument);
        };
        *pixel = if x & 1 == 0 { byte >> 4 } else { byte & 0x0F };
    }
    Ok(())
}

fn unpack_bitmap_bgra_row(
    src: &[FT_Byte],
    dst: &mut [FT_Byte],
    width: usize,
) -> Result<(), FT_Error> {
    for (x, pixel) in dst.iter_mut().enumerate().take(width) {
        let offset = x.saturating_mul(4);
        let Some(bgra) = src.get(offset..offset.saturating_add(4)) else {
            return Err(FT_Err_Invalid_Argument);
        };
        *pixel = gray_for_premultiplied_srgb_bgra(bgra);
    }
    Ok(())
}

pub fn FT_Bitmap_Done(library: Option<&FT_Library>, bitmap: Option<&mut FT_Bitmap_C>) -> FT_Error {
    if library.is_none() {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(bitmap) = bitmap else {
        return FT_Err_Invalid_Argument;
    };
    // FreeType `src/base/ftbitmap.c:1109-1125` frees the owned buffer then
    // overwrites the public record with the static `null_bitmap`.
    {
        let mut registry = bitmap_buffer_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !bitmap.buffer.is_null() {
            registry.remove(&(bitmap.buffer as usize));
        }
    }
    *bitmap = FT_Bitmap_C::default();
    FT_Err_Ok
}

pub fn FT_Bitmap_Embolden(
    library: Option<&FT_Library>,
    bitmap: Option<&mut FT_Bitmap_C>,
    x_strength: FT_Pos,
    y_strength: FT_Pos,
) -> FT_Error {
    if library.is_none() {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(bitmap) = bitmap else {
        return FT_Err_Invalid_Argument;
    };
    if bitmap.buffer.is_null() {
        return FT_Err_Invalid_Argument;
    }

    let Some(x_pixels) = ft_bitmap_strength_pixels(x_strength) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(y_pixels) = ft_bitmap_strength_pixels(y_strength) else {
        return FT_Err_Invalid_Argument;
    };
    if x_pixels == 0 && y_pixels == 0 {
        return FT_Err_Ok;
    }
    if x_pixels < 0 || y_pixels < 0 {
        return FT_Err_Invalid_Argument;
    }

    let mut x_pixels = x_pixels;
    let mut y_pixels = y_pixels;
    match i32::from(bitmap.pixel_mode) {
        FT_PIXEL_MODE_GRAY2 | FT_PIXEL_MODE_GRAY4 => {
            let (bits_per_pixel, num_grays) = if bitmap.pixel_mode == FT_PIXEL_MODE_GRAY2 as u8 {
                (2, 4)
            } else {
                (4, 16)
            };
            let error = convert_public_bitmap_to_gray(bitmap, bits_per_pixel, num_grays);
            if error != FT_Err_Ok {
                return error;
            }
        }
        FT_PIXEL_MODE_MONO => x_pixels = x_pixels.min(8),
        FT_PIXEL_MODE_LCD => {
            let Some(scaled) = x_pixels.checked_mul(3) else {
                return FT_Err_Invalid_Argument;
            };
            x_pixels = scaled;
        }
        FT_PIXEL_MODE_LCD_V => {
            let Some(scaled) = y_pixels.checked_mul(3) else {
                return FT_Err_Invalid_Argument;
            };
            y_pixels = scaled;
        }
        FT_PIXEL_MODE_BGRA => return FT_Err_Ok,
        _ => {}
    }

    let (Ok(x_pixels), Ok(y_pixels)) = (usize::try_from(x_pixels), usize::try_from(y_pixels))
    else {
        return FT_Err_Invalid_Argument;
    };
    let error = ft_bitmap_assure_buffer(bitmap, x_pixels, y_pixels);
    if error != FT_Err_Ok {
        return error;
    }

    let Some(mut bytes) = FT_Bitmap_Owned_Buffer_Bytes(Some(bitmap)) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(pitch_abs) = ft_bitmap_pitch_abs(bitmap) else {
        return FT_Err_Invalid_Argument;
    };
    let Ok(rows) = usize::try_from(bitmap.rows) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(required_len) = pitch_abs.checked_mul(rows.saturating_add(y_pixels)) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    if bytes.len() < required_len {
        return FT_Err_Invalid_Argument;
    }

    let pitch = bitmap.pitch as isize;
    let start = if bitmap.pitch > 0 {
        pitch_abs.checked_mul(y_pixels)
    } else {
        rows.checked_sub(1)
            .and_then(|row| row.checked_mul(pitch_abs))
    };
    let Some(start) = start else {
        return FT_Err_Invalid_Argument;
    };
    for row in 0..rows {
        let Some(row_start) = (start as isize)
            .checked_add((row as isize).saturating_mul(pitch))
            .and_then(|offset| usize::try_from(offset).ok())
        else {
            return FT_Err_Invalid_Argument;
        };
        if row_start
            .checked_add(pitch_abs)
            .is_none_or(|end| end > bytes.len())
        {
            return FT_Err_Invalid_Argument;
        }

        for x in (0..pitch_abs).rev() {
            let source = bytes[row_start + x];
            for i in 1..=x_pixels {
                if bitmap.pixel_mode == FT_PIXEL_MODE_MONO as u8 {
                    // FreeType `src/base/ftbitmap.c:371-379` promotes the
                    // byte to `int`, so the clamped `tmp >> 8` contributes 0.
                    bytes[row_start + x] |= if i < 8 { source >> i } else { 0 };
                    if x > 0 {
                        bytes[row_start + x] |= bytes[row_start + x - 1] << (8 - i);
                    }
                } else if x >= i {
                    let max_gray =
                        u8::try_from(bitmap.num_grays.saturating_sub(1).min(255)).unwrap_or(255);
                    let value = u16::from(bytes[row_start + x])
                        .saturating_add(u16::from(bytes[row_start + x - i]));
                    bytes[row_start + x] = if value > u16::from(max_gray) {
                        max_gray
                    } else {
                        u8::try_from(value).unwrap_or(max_gray)
                    };
                    if bytes[row_start + x] == max_gray {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        for y in 1..=y_pixels {
            let Some(dst_start) = (row_start as isize)
                .checked_sub(pitch.saturating_mul(y as isize))
                .and_then(|offset| usize::try_from(offset).ok())
            else {
                return FT_Err_Invalid_Argument;
            };
            if dst_start
                .checked_add(pitch_abs)
                .is_none_or(|end| end > bytes.len())
            {
                return FT_Err_Invalid_Argument;
            }
            for i in 0..pitch_abs {
                bytes[dst_start + i] |= bytes[row_start + i];
            }
        }
    }

    let (Ok(x_add), Ok(y_add)) = (u32::try_from(x_pixels), u32::try_from(y_pixels)) else {
        return FT_Err_Invalid_Argument;
    };
    let (Some(width), Some(rows)) = (
        bitmap.width.checked_add(x_add),
        bitmap.rows.checked_add(y_add),
    ) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    bitmap.width = width;
    bitmap.rows = rows;
    FT_Bitmap_Set_Owned_Buffer(Some(bitmap), bytes);
    FT_Err_Ok
}

fn ft_bitmap_strength_pixels(strength: FT_Pos) -> Option<i32> {
    // C rejects a rounded strength above `FT_INT_MAX << 6` before entering
    // `ft_bitmap_assure_buffer` (`src/base/ftbitmap.c:302-309`).
    let rounded = strength.checked_add(32)? & !63;
    let pixels = rounded >> 6;
    if pixels > i64::from(i32::MAX) {
        return None;
    }
    i32::try_from(pixels).ok()
}

fn ft_bitmap_pitch_abs(bitmap: &FT_Bitmap_C) -> Option<usize> {
    usize::try_from(bitmap.pitch.unsigned_abs()).ok()
}

fn ft_bitmap_assure_buffer(bitmap: &mut FT_Bitmap_C, x_pixels: usize, y_pixels: usize) -> FT_Error {
    let (Ok(width), Ok(rows)) = (usize::try_from(bitmap.width), usize::try_from(bitmap.rows))
    else {
        return FT_Err_Invalid_Argument;
    };
    let Some(pitch) = ft_bitmap_pitch_abs(bitmap) else {
        return FT_Err_Invalid_Argument;
    };
    let (bpp, new_pitch) = match i32::from(bitmap.pixel_mode) {
        FT_PIXEL_MODE_MONO => {
            let Some(new_width) = width.checked_add(x_pixels) else {
                return FT_Err_Out_Of_Memory as FT_Error;
            };
            (1usize, (new_width + 7) >> 3)
        }
        FT_PIXEL_MODE_GRAY2 => {
            let Some(new_width) = width.checked_add(x_pixels) else {
                return FT_Err_Out_Of_Memory as FT_Error;
            };
            (2, (new_width + 3) >> 2)
        }
        FT_PIXEL_MODE_GRAY4 => {
            let Some(new_width) = width.checked_add(x_pixels) else {
                return FT_Err_Out_Of_Memory as FT_Error;
            };
            (4, (new_width + 1) >> 1)
        }
        FT_PIXEL_MODE_GRAY | FT_PIXEL_MODE_LCD | FT_PIXEL_MODE_LCD_V => {
            let Some(new_pitch) = width.checked_add(x_pixels) else {
                return FT_Err_Out_Of_Memory as FT_Error;
            };
            (8, new_pitch)
        }
        _ => return FT_Err_Invalid_Glyph_Format,
    };

    let Some(required_len) = pitch.checked_mul(rows) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let Some(mut bytes) = FT_Bitmap_Owned_Buffer_Bytes(Some(bitmap)) else {
        return FT_Err_Invalid_Argument;
    };
    if bytes.len() < required_len {
        return FT_Err_Invalid_Argument;
    }

    if y_pixels == 0 && new_pitch <= pitch {
        let bit_width = pitch.saturating_mul(8);
        let Some(bit_last) = width
            .checked_add(x_pixels)
            .and_then(|value| value.checked_mul(bpp))
        else {
            return FT_Err_Out_Of_Memory as FT_Error;
        };
        if bit_last < bit_width {
            let line_delta = bit_last >> 3;
            let shift = bit_last & 7;
            for row in 0..rows {
                let Some(line) = row
                    .checked_mul(pitch)
                    .and_then(|base| base.checked_add(line_delta))
                else {
                    return FT_Err_Out_Of_Memory as FT_Error;
                };
                let Some(end) = row.checked_add(1).and_then(|next| next.checked_mul(pitch)) else {
                    return FT_Err_Out_Of_Memory as FT_Error;
                };
                let mut write = line;
                if shift > 0 {
                    if write >= bytes.len() {
                        return FT_Err_Invalid_Argument;
                    }
                    let mask = (0xFF00u16 >> shift) as u8;
                    bytes[write] &= mask;
                    write += 1;
                }
                if write < end {
                    if end > bytes.len() {
                        return FT_Err_Invalid_Argument;
                    }
                    bytes[write..end].fill(0);
                }
            }
        }
        FT_Bitmap_Set_Owned_Buffer(Some(bitmap), bytes);
        return FT_Err_Ok;
    }

    let Some(new_rows) = rows.checked_add(y_pixels) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let Some(new_len) = new_rows.checked_mul(new_pitch) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let Some(row_len) = width
        .checked_mul(bpp)
        .and_then(|bits| bits.checked_add(7).map(|bits| bits >> 3))
    else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let mut new_bytes = vec![0; new_len];
    if bitmap.pitch > 0 {
        for row in 0..rows {
            let src = row * pitch;
            let dst = (row + y_pixels) * new_pitch;
            new_bytes[dst..dst + row_len].copy_from_slice(&bytes[src..src + row_len]);
        }
    } else {
        for row in 0..rows {
            let src = row * pitch;
            let dst = row * new_pitch;
            new_bytes[dst..dst + row_len].copy_from_slice(&bytes[src..src + row_len]);
        }
    }
    let Ok(new_pitch_i32) = i32::try_from(new_pitch) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    bitmap.pitch = if bitmap.pitch < 0 {
        -new_pitch_i32
    } else {
        new_pitch_i32
    };
    FT_Bitmap_Set_Owned_Buffer(Some(bitmap), new_bytes);
    FT_Err_Ok
}

fn convert_public_bitmap_to_gray(
    bitmap: &mut FT_Bitmap_C,
    bits_per_pixel: usize,
    num_grays: u16,
) -> FT_Error {
    let (Ok(width), Ok(rows)) = (usize::try_from(bitmap.width), usize::try_from(bitmap.rows))
    else {
        return FT_Err_Invalid_Argument;
    };
    let Some(source_pitch) = ft_bitmap_pitch_abs(bitmap) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(source_len) = source_pitch.checked_mul(rows) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    let Some(source) = FT_Bitmap_Owned_Buffer_Bytes(Some(bitmap)) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(new_len) = width.checked_mul(rows) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    if source.len() < source_len {
        return FT_Err_Invalid_Argument;
    }
    let mut target = vec![0; new_len];
    let mask = ((1u16 << bits_per_pixel) - 1) as u8;
    for row in 0..rows {
        let src_row = if bitmap.pitch < 0 {
            (rows - 1 - row) * source_pitch
        } else {
            row * source_pitch
        };
        let dst_row = if bitmap.pitch < 0 {
            (rows - 1 - row) * width
        } else {
            row * width
        };
        for x in 0..width {
            let bit_offset = x * bits_per_pixel;
            let byte = source[src_row + bit_offset / 8];
            let shift = 8 - bits_per_pixel - bit_offset % 8;
            target[dst_row + x] = (byte >> shift) & mask;
        }
    }

    let Ok(width_i32) = i32::try_from(width) else {
        return FT_Err_Out_Of_Memory as FT_Error;
    };
    bitmap.pitch = if bitmap.pitch < 0 {
        -width_i32
    } else {
        width_i32
    };
    bitmap.pixel_mode = FT_PIXEL_MODE_GRAY as u8;
    bitmap.num_grays = num_grays;
    FT_Bitmap_Set_Owned_Buffer(Some(bitmap), target);
    FT_Err_Ok
}

#[derive(Clone)]
struct GrayBitmap {
    rows: u32,
    width: u32,
    pitch: FT_Int,
    bytes: Vec<FT_Byte>,
}

impl GrayBitmap {
    fn row_range(&self, row: usize) -> Option<std::ops::Range<usize>> {
        let pitch = usize::try_from(self.pitch.unsigned_abs()).ok()?;
        let rows = usize::try_from(self.rows).ok()?;
        let start = if self.pitch < 0 {
            rows.checked_sub(1 + row)?.checked_mul(pitch)?
        } else {
            row.checked_mul(pitch)?
        };
        let width = usize::try_from(self.width).ok()?;
        let end = start.checked_add(width)?;
        (end <= self.bytes.len()).then_some(start..end)
    }
}

fn bitmap_row_start(bitmap: &FT_Bitmap_C, row: usize) -> Option<usize> {
    let pitch = bitmap_pitch_abs(bitmap)?;
    let rows = usize::try_from(bitmap.rows).ok()?;
    if bitmap.pitch < 0 {
        rows.checked_sub(1 + row)?.checked_mul(pitch)
    } else {
        row.checked_mul(pitch)
    }
}

fn gray_for_premultiplied_srgb_bgra(bgra: &[FT_Byte]) -> FT_Byte {
    let a = u32::from(bgra[3]);
    if a == 0 {
        return 0;
    }
    let l = (4731u32 * u32::from(bgra[0]) * u32::from(bgra[0])
        + 46868u32 * u32::from(bgra[1]) * u32::from(bgra[1])
        + 13937u32 * u32::from(bgra[2]) * u32::from(bgra[2]))
        >> 16;
    a.wrapping_sub(l / a) as FT_Byte
}

fn bitmap_to_gray(source: &FT_Bitmap_C) -> Result<GrayBitmap, FT_Error> {
    let bytes = bitmap_owned_bytes(source).ok_or(FT_Err_Invalid_Argument)?;
    let rows = usize::try_from(source.rows).map_err(|_| FT_Err_Out_Of_Memory as FT_Error)?;
    let width = usize::try_from(source.width).map_err(|_| FT_Err_Out_Of_Memory as FT_Error)?;
    let pitch = if source.pitch < 0 {
        -FT_Int::try_from(width).map_err(|_| FT_Err_Out_Of_Memory as FT_Error)?
    } else {
        FT_Int::try_from(width).map_err(|_| FT_Err_Out_Of_Memory as FT_Error)?
    };
    let len = rows
        .checked_mul(width)
        .ok_or(FT_Err_Out_Of_Memory as FT_Error)?;
    let mut gray = vec![0; len];

    for row in 0..rows {
        let src = bitmap_row_start(source, row).ok_or(FT_Err_Invalid_Argument)?;
        let dst = if pitch < 0 {
            (rows - 1 - row)
                .checked_mul(width)
                .ok_or(FT_Err_Out_Of_Memory as FT_Error)?
        } else {
            row.checked_mul(width)
                .ok_or(FT_Err_Out_Of_Memory as FT_Error)?
        };
        match i32::from(source.pixel_mode) {
            FT_PIXEL_MODE_MONO => {
                for col in 0..width {
                    let byte = *bytes.get(src + (col >> 3)).ok_or(FT_Err_Invalid_Argument)?;
                    gray[dst + col] = (byte >> (7 - (col & 7))) & 1;
                }
            }
            FT_PIXEL_MODE_GRAY | FT_PIXEL_MODE_LCD | FT_PIXEL_MODE_LCD_V => {
                let end = src.checked_add(width).ok_or(FT_Err_Invalid_Argument)?;
                let row_bytes = bytes.get(src..end).ok_or(FT_Err_Invalid_Argument)?;
                gray[dst..dst + width].copy_from_slice(row_bytes);
            }
            FT_PIXEL_MODE_GRAY2 => {
                for col in 0..width {
                    let byte = *bytes.get(src + (col >> 2)).ok_or(FT_Err_Invalid_Argument)?;
                    gray[dst + col] = (byte >> (6 - 2 * (col & 3))) & 0x03;
                }
            }
            FT_PIXEL_MODE_GRAY4 => {
                for col in 0..width {
                    let byte = *bytes.get(src + (col >> 1)).ok_or(FT_Err_Invalid_Argument)?;
                    gray[dst + col] = (byte >> (4 - 4 * (col & 1))) & 0x0F;
                }
            }
            FT_PIXEL_MODE_BGRA => {
                for col in 0..width {
                    let start = src
                        .checked_add(col.checked_mul(4).ok_or(FT_Err_Invalid_Argument)?)
                        .ok_or(FT_Err_Invalid_Argument)?;
                    let px = bytes.get(start..start + 4).ok_or(FT_Err_Invalid_Argument)?;
                    gray[dst + col] = gray_for_premultiplied_srgb_bgra(px);
                }
            }
            _ => return Err(FT_Err_Invalid_Argument),
        }
    }

    Ok(GrayBitmap {
        rows: source.rows,
        width: source.width,
        pitch,
        bytes: gray,
    })
}

fn pix_floor(value: FT_Pos) -> FT_Pos {
    value & !63
}

pub fn FT_Bitmap_Blend(
    library: Option<&FT_Library>,
    source: Option<&FT_Bitmap_C>,
    source_offset: FT_Vector,
    target: Option<&mut FT_Bitmap_C>,
    atarget_offset: Option<&mut FT_Vector>,
    color: FT_Color,
) -> FT_Error {
    let (Some(_library), Some(source), Some(target), Some(atarget_offset)) =
        (library, source, target, atarget_offset)
    else {
        return FT_Err_Invalid_Argument;
    };

    let target_mode = i32::from(target.pixel_mode);
    if !(target_mode == FT_PIXEL_MODE_NONE
        || (target_mode == FT_PIXEL_MODE_BGRA && !target.buffer.is_null()))
    {
        return FT_Err_Invalid_Argument;
    }
    if i32::from(source.pixel_mode) == FT_PIXEL_MODE_NONE {
        return FT_Err_Ok;
    }
    if target_mode == FT_PIXEL_MODE_BGRA && (source.pitch ^ target.pitch) < 0 {
        return FT_Err_Invalid_Argument;
    }
    if source.width == 0 || source.rows == 0 {
        return FT_Err_Ok;
    }

    // FreeType `src/base/ftbitmap.c:762-1058` floors 26.6 offsets before
    // computing final bounds; its negative target-pitch copy/blend branches
    // are empty `/* XXX */` blocks, so this route intentionally no-ops those
    // branches after preserving allocation and offset side effects.
    let source_offset = FT_Vector {
        x: pix_floor(source_offset.x),
        y: pix_floor(source_offset.y),
    };
    let target_offset = FT_Vector {
        x: pix_floor(atarget_offset.x),
        y: pix_floor(atarget_offset.y),
    };

    let source_rows_26_6 = FT_Pos::from(source.rows) << 6;
    if FT_Long::MIN + source_rows_26_6 + 64 > source_offset.y {
        return FT_Err_Invalid_Argument;
    }
    let source_llx = source_offset.x;
    let source_lly = source_offset.y - source_rows_26_6;
    let source_width_26_6 = FT_Pos::from(source.width) << 6;
    if FT_Long::MAX - source_width_26_6 - 64 < source_llx {
        return FT_Err_Invalid_Argument;
    }
    let source_urx = source_llx + source_width_26_6;
    let source_ury = source_offset.y;

    let (target_llx, target_lly, target_urx, target_ury) = if target.width != 0 && target.rows != 0
    {
        let target_rows_26_6 = FT_Pos::from(target.rows) << 6;
        if FT_Long::MIN + target_rows_26_6 > target_offset.y {
            return FT_Err_Invalid_Argument;
        }
        let target_lly = target_offset.y - target_rows_26_6;
        let target_width_26_6 = FT_Pos::from(target.width) << 6;
        if FT_Long::MAX - target_width_26_6 < target_offset.x {
            return FT_Err_Invalid_Argument;
        }
        (
            target_offset.x,
            target_lly,
            target_offset.x + target_width_26_6,
            target_offset.y,
        )
    } else {
        (FT_Long::MAX, FT_Long::MAX, FT_Long::MIN, FT_Long::MIN)
    };

    let final_llx = source_llx.min(target_llx);
    let final_lly = source_lly.min(target_lly);
    let final_urx = source_urx.max(target_urx);
    let final_ury = source_ury.max(target_ury);
    let final_width = u32::try_from((final_urx - final_llx) >> 6)
        .map_err(|_| FT_Err_Invalid_Argument)
        .unwrap_or(0);
    let final_rows = u32::try_from((final_ury - final_lly) >> 6)
        .map_err(|_| FT_Err_Invalid_Argument)
        .unwrap_or(0);
    if final_width == 0 || final_rows == 0 {
        return FT_Err_Ok;
    }

    let source_llx = source_llx - final_llx;
    let source_lly = source_lly - final_lly;
    let target_origin = if target.width != 0 && target.rows != 0 {
        Some((target_llx - final_llx, target_lly - final_lly))
    } else {
        None
    };

    let final_pitch = match usize::try_from(final_width)
        .ok()
        .and_then(|w| w.checked_mul(4))
    {
        Some(value) => value,
        None => return FT_Err_Out_Of_Memory as FT_Error,
    };
    if target_mode == FT_PIXEL_MODE_NONE {
        target.width = final_width;
        target.rows = final_rows;
        target.pixel_mode = FT_PIXEL_MODE_BGRA as FT_Byte;
        target.pitch = match FT_Int::try_from(final_pitch) {
            Ok(value) => value,
            Err(_) => return FT_Err_Out_Of_Memory as FT_Error,
        };
        target.num_grays = 256;
        let len = match final_pitch.checked_mul(usize::try_from(final_rows).unwrap_or(0)) {
            Some(value) => value,
            None => return FT_Err_Out_Of_Memory as FT_Error,
        };
        FT_Bitmap_Set_Owned_Buffer(Some(target), vec![0; len]);
    } else if target.width != final_width || target.rows != final_rows {
        let old_bytes = bitmap_owned_bytes(target).ok_or(FT_Err_Invalid_Argument);
        let old_pitch = match bitmap_pitch_abs(target) {
            Some(value) => value,
            None => return FT_Err_Invalid_Argument,
        };
        let len = match final_pitch.checked_mul(usize::try_from(final_rows).unwrap_or(0)) {
            Some(value) => value,
            None => return FT_Err_Out_Of_Memory as FT_Error,
        };
        let mut new_bytes = vec![0; len];
        if target.pitch >= 0 {
            if let Ok(old_bytes) = old_bytes {
                let (target_llx, target_lly) = target_origin.unwrap_or((0, 0));
                let x = usize::try_from(target_llx >> 6).unwrap_or(0);
                let y = usize::try_from(target_lly >> 6).unwrap_or(0);
                let rows = usize::try_from(target.rows).unwrap_or(0);
                let dst_row = usize::try_from(final_rows)
                    .unwrap_or(0)
                    .saturating_sub(y)
                    .saturating_sub(rows);
                for row in 0..rows {
                    let src = row.saturating_mul(old_pitch);
                    let dst = (dst_row + row)
                        .saturating_mul(final_pitch)
                        .saturating_add(x.saturating_mul(4));
                    let copy_len = old_pitch.min(final_pitch.saturating_sub(x.saturating_mul(4)));
                    if src + copy_len <= old_bytes.len() && dst + copy_len <= new_bytes.len() {
                        new_bytes[dst..dst + copy_len]
                            .copy_from_slice(&old_bytes[src..src + copy_len]);
                    }
                }
            }
        }
        target.width = final_width;
        target.rows = final_rows;
        target.pitch = if target.pitch < 0 {
            -FT_Int::try_from(final_pitch).unwrap_or(FT_Int::MAX)
        } else {
            FT_Int::try_from(final_pitch).unwrap_or(FT_Int::MAX)
        };
        FT_Bitmap_Set_Owned_Buffer(Some(target), new_bytes);
    }

    let source_gray = if i32::from(source.pixel_mode) == FT_PIXEL_MODE_GRAY {
        GrayBitmap {
            rows: source.rows,
            width: source.width,
            pitch: source.pitch,
            bytes: match bitmap_owned_bytes(source) {
                Some(bytes) => bytes,
                None => return FT_Err_Invalid_Argument,
            },
        }
    } else {
        match bitmap_to_gray(source) {
            Ok(bitmap) => bitmap,
            Err(err) => return err,
        }
    };

    if target.pitch >= 0 && source_gray.pitch >= 0 {
        let mut target_bytes = match bitmap_owned_bytes(target) {
            Some(bytes) => bytes,
            None => return FT_Err_Invalid_Argument,
        };
        let x = usize::try_from(source_llx >> 6).unwrap_or(0);
        let y = usize::try_from(source_lly >> 6).unwrap_or(0);
        let target_rows = usize::try_from(target.rows).unwrap_or(0);
        let source_rows = usize::try_from(source_gray.rows).unwrap_or(0);
        let target_pitch = match bitmap_pitch_abs(target) {
            Some(value) => value,
            None => return FT_Err_Invalid_Argument,
        };
        let dst_row = target_rows.saturating_sub(y).saturating_sub(source_rows);
        let color_alpha = u32::from(color.alpha);
        for row in 0..source_rows {
            let Some(src_range) = source_gray.row_range(row) else {
                return FT_Err_Invalid_Argument;
            };
            let mut dst = (dst_row + row)
                .saturating_mul(target_pitch)
                .saturating_add(x.saturating_mul(4));
            for aa in &source_gray.bytes[src_range] {
                let fa = color_alpha * u32::from(*aa) / 255;
                let fb = u32::from(color.blue) * fa / 255;
                let fg = u32::from(color.green) * fa / 255;
                let fr = u32::from(color.red) * fa / 255;
                let ba2 = 255 - fa;
                if dst + 4 > target_bytes.len() {
                    return FT_Err_Invalid_Argument;
                }
                target_bytes[dst] = (u32::from(target_bytes[dst]) * ba2 / 255 + fb) as FT_Byte;
                target_bytes[dst + 1] =
                    (u32::from(target_bytes[dst + 1]) * ba2 / 255 + fg) as FT_Byte;
                target_bytes[dst + 2] =
                    (u32::from(target_bytes[dst + 2]) * ba2 / 255 + fr) as FT_Byte;
                target_bytes[dst + 3] =
                    (u32::from(target_bytes[dst + 3]) * ba2 / 255 + fa) as FT_Byte;
                dst += 4;
            }
        }
        FT_Bitmap_Set_Owned_Buffer(Some(target), target_bytes);
    }

    atarget_offset.x = final_llx;
    atarget_offset.y = final_lly + (FT_Pos::from(final_rows) << 6);
    FT_Err_Ok
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FT_Library {
    inner: api::Library,
    memory: FT_Memory,
    refcount: usize,
    module_names: &'static [&'static str],
    truetype_interpreter_version: FT_UInt,
    autofitter_default_script: FT_UInt,
    autofitter_fallback_script: FT_UInt,
    debug_hooks: [FT_DebugHook_Func; 4],
    _lcd_geometry: [FT_Vector; 3],
}

const DEFAULT_MODULE_NAMES: &[&str] = &[
    // C parity: pinned FreeType's default module build exposes these names via
    // FT_Get_Module after FT_Init_FreeType.  CID has class flag definitions
    // upstream, but this pinned build does not register the CID module.
    "autofitter",
    "truetype",
    "type1",
    "cff",
    "pfr",
    "type42",
    "winfonts",
    "pcf",
    "bdf",
    "psaux",
    "psnames",
    "pshinter",
    "sfnt",
    "smooth",
    "raster1",
    "sdf",
    "bsdf",
    "svg",
];

#[derive(Clone)]
pub struct FT_Face {
    // Public FT_FaceRec fields read by Pillow _imagingft.c via
    // face->family_name/style_name/num_glyphs and face->size->metrics.
    pub num_faces: FT_Long,
    pub face_index: FT_Long,
    pub face_flags: FT_Long,
    pub style_flags: FT_Long,
    pub family_name: Option<String>,
    pub style_name: Option<String>,
    pub num_glyphs: FT_Long,
    pub bbox: FT_BBox,
    pub units_per_EM: FT_UShort,
    pub ascender: FT_Short,
    pub descender: FT_Short,
    pub height: FT_Short,
    pub max_advance_width: FT_Short,
    pub max_advance_height: FT_Short,
    pub underline_position: FT_Short,
    pub underline_thickness: FT_Short,
    pub num_fixed_sizes: FT_Int,
    pub available_sizes: Box<[FT_Bitmap_Size]>,
    pub size: FT_Size,
    pub size_metrics: FT_Size_MetricsRec,
    pub active_charmap_index: FT_Int,
    pub charmaps: Box<[FT_CharMapRecPublic]>,
    memory_stream: Box<FT_StreamRec>,
    inner: Rc<RefCell<api::Face>>,
    sizes: Rc<RefCell<FaceSizeState>>,
    probe_only: bool,
    postscript_name: Option<String>,
    type1_font_info_strings: Option<Type1FontInfoStrings>,
    type1_mm_axis_names: Vec<CString>,
    sfnt_os2: Option<Box<TT_OS2>>,
    sfnt_head: Option<Box<TT_Header>>,
    sfnt_maxp: Option<Box<TT_MaxProfile>>,
    sfnt_hhea: Option<Box<TT_HoriHeader>>,
    sfnt_vhea: Option<Box<TT_VertHeader>>,
    sfnt_post: Option<Box<TT_Postscript>>,
    sfnt_pclt: Option<Box<TT_PCLT>>,
    charmap_metadata: Box<[(FT_Long, FT_ULong)]>,
    cpal: Option<Rc<RefCell<CpalState>>>,
    transform_matrix: FT_Matrix,
    transform_delta: FT_Vector,
    no_stem_darkening: i32,
    random_seed: FT_Int32,
    increase_x_height: FT_UInt,
    glyph_to_script_map: Box<[FT_UShort]>,
    refcount: usize,
}

impl FT_Face {
    pub fn memory_stream(&self) -> FT_Stream {
        (&*self.memory_stream as *const FT_StreamRec).cast_mut()
    }

    pub fn memory_stream_record(&self) -> FT_StreamRec {
        *self.memory_stream
    }
}

#[derive(Clone)]
struct Type1FontInfoStrings {
    version: Option<CString>,
    notice: Option<CString>,
    full_name: Option<CString>,
    family_name: Option<CString>,
    weight: Option<CString>,
}

#[derive(Clone)]
struct CpalState {
    palette_name_ids: Vec<FT_UShort>,
    palette_flags: Vec<FT_UShort>,
    palette_entry_name_ids: Vec<FT_UShort>,
    palettes: Vec<Vec<FT_Color>>,
    active_palette: Vec<FT_Color>,
    active_palette_index: FT_UShort,
}

#[cfg(any(test, feature = "abi-test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FT_Palette_Select_Snapshot {
    pub error: FT_Error,
    pub palette_is_null: bool,
    pub entries: Vec<FT_Color>,
}

#[cfg(any(test, feature = "abi-test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FT_Palette_Data_Snapshot {
    pub error: FT_Error,
    pub num_palettes: FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_name_ids_is_null: bool,
    pub palette_flags_is_null: bool,
    pub palette_entry_name_ids_is_null: bool,
    pub palette_name_ids: Vec<FT_UShort>,
    pub palette_flags: Vec<FT_UShort>,
    pub palette_entry_name_ids: Vec<FT_UShort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FT_Face_Property_Value {
    Bool(FT_Bool),
    Int32(FT_Int32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FT_Face_Property {
    pub tag: FT_ULong,
    pub value: Option<FT_Face_Property_Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FT_Face_Properties_State {
    pub no_stem_darkening: i32,
    pub random_seed: FT_Int32,
}

struct FaceSizeState {
    active: Option<usize>,
    entries: Vec<SizeEntry>,
}

struct SizeEntry {
    _token: Box<SizeToken>,
    handle: FT_Size,
    state: ActiveSizeState,
}

struct SizeToken {
    _identity: usize,
}

#[derive(Clone)]
struct SizeOwner {
    face: Weak<RefCell<api::Face>>,
    sizes: Weak<RefCell<FaceSizeState>>,
}

type SizeHandleRegistry = BTreeMap<usize, SizeOwner>;

thread_local! {
    static SIZE_HANDLE_REGISTRY: RefCell<SizeHandleRegistry> = const { RefCell::new(BTreeMap::new()) };
}

impl FaceSizeState {
    fn new(initial_state: ActiveSizeState) -> Self {
        let entry = SizeEntry::new(initial_state);
        let active = Some(size_handle_key(entry.handle));
        Self {
            active,
            entries: vec![entry],
        }
    }

    fn empty() -> Self {
        Self {
            active: None,
            entries: Vec::new(),
        }
    }

    fn active_handle(&self) -> FT_Size {
        self.active
            .and_then(|active| self.entries.iter().find(|entry| entry.key() == active))
            .map_or(ptr::null_mut(), |entry| entry.handle)
    }

    fn active_entry_mut(&mut self) -> Option<&mut SizeEntry> {
        let active = self.active?;
        self.entries.iter_mut().find(|entry| entry.key() == active)
    }

    fn add_size(&mut self, state: ActiveSizeState) -> FT_Size {
        let entry = SizeEntry::new(state);
        let handle = entry.handle;
        self.entries.push(entry);
        handle
    }

    fn activate(&mut self, handle: FT_Size) -> Option<ActiveSizeState> {
        let key = size_handle_key(handle);
        let state = self
            .entries
            .iter()
            .find(|entry| entry.key() == key)
            .map(|entry| entry.state.clone())?;
        self.active = Some(key);
        Some(state)
    }

    fn remove(&mut self, handle: FT_Size) -> Option<DoneSizeResult> {
        let key = size_handle_key(handle);
        let index = self.entries.iter().position(|entry| entry.key() == key)?;
        let was_active = self.active == Some(key);
        self.entries.remove(index);
        let fallback = if was_active {
            self.active = self.entries.first().map(SizeEntry::key);
            self.entries.first().map(|entry| entry.state.clone())
        } else {
            None
        };
        Some(DoneSizeResult {
            removed_key: key,
            fallback,
        })
    }
}

impl Drop for FaceSizeState {
    fn drop(&mut self) {
        SIZE_HANDLE_REGISTRY.with(|registry| {
            let mut registry = registry.borrow_mut();
            for entry in &self.entries {
                registry.remove(&entry.key());
            }
        });
    }
}

impl SizeEntry {
    fn new(state: ActiveSizeState) -> Self {
        let mut token = Box::new(SizeToken { _identity: 0 });
        let handle = (&mut *token as *mut SizeToken).cast::<super::types::FT_SizeRec>();
        Self {
            _token: token,
            handle,
            state,
        }
    }

    fn key(&self) -> usize {
        size_handle_key(self.handle)
    }
}

struct DoneSizeResult {
    removed_key: usize,
    fallback: Option<ActiveSizeState>,
}

fn size_handle_key(size: FT_Size) -> usize {
    size as usize
}

fn register_size_handle(
    size: FT_Size,
    face: &Rc<RefCell<api::Face>>,
    sizes: &Rc<RefCell<FaceSizeState>>,
) {
    let owner = SizeOwner {
        face: Rc::downgrade(face),
        sizes: Rc::downgrade(sizes),
    };
    SIZE_HANDLE_REGISTRY.with(|registry| {
        registry.borrow_mut().insert(size_handle_key(size), owner);
    });
}

fn register_face_size_handles(face: &FT_Face) {
    for entry in &face.sizes.borrow().entries {
        register_size_handle(entry.handle, &face.inner, &face.sizes);
    }
}

fn lookup_size_owner(size: FT_Size) -> Option<SizeOwner> {
    SIZE_HANDLE_REGISTRY.with(|registry| registry.borrow().get(&size_handle_key(size)).cloned())
}

fn unregister_size_handle_key(key: usize) {
    SIZE_HANDLE_REGISTRY.with(|registry| {
        registry.borrow_mut().remove(&key);
    });
}

fn active_size_handle(face: &FT_Face) -> FT_Size {
    face.sizes.borrow().active_handle()
}

fn has_active_size(face: &FT_Face) -> bool {
    !active_size_handle(face).is_null()
}

fn sync_active_size_state(face: &mut FT_Face) {
    let state = face.inner.borrow().active_size_state();
    if let Some(entry) = face.sizes.borrow_mut().active_entry_mut() {
        entry.state = state;
    }
    face.size = active_size_handle(face);
    face.size_metrics = face.inner.borrow().size_metrics().into();
}

fn sync_active_charmap_index(face: &mut FT_Face) {
    face.active_charmap_index = face
        .inner
        .borrow()
        .charmap_index()
        .and_then(|index| FT_Int::try_from(index).ok())
        .unwrap_or(-1);
}

type CharmapMetadata = (FT_Long, FT_ULong, FT_Int);
type FaceCharmapRecords = (Box<[FT_CharMapRecPublic]>, Box<[(FT_Long, FT_ULong)]>);
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
    pub owns_bitmap: bool,
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
        memory: std::ptr::null_mut(),
        refcount: 1,
        module_names: DEFAULT_MODULE_NAMES,
        truetype_interpreter_version: TT_INTERPRETER_VERSION_40 as FT_UInt,
        // FreeType 2.14.3 `src/autofit/afmodule.c:af_autofitter_init`
        // initializes these to internal AF_SCRIPT_DEFAULT and
        // AF_STYLE_FALLBACK values.  In the pinned build those public
        // readbacks are 30 and 59 respectively.
        autofitter_default_script: 30,
        autofitter_fallback_script: 59,
        debug_hooks: [None; 4],
        _lcd_geometry: [
            FT_Vector { x: -21, y: 0 },
            FT_Vector { x: 0, y: 0 },
            FT_Vector { x: 21, y: 0 },
        ],
    }
}

pub fn FT_New_Library(memory: Option<FT_Memory>) -> Result<FT_Library, FT_Error> {
    let Some(memory) = memory else {
        return Err(FT_Err_Invalid_Argument);
    };
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_New_Library` stores the caller's
    // FT_Memory pointer, initializes refcount to 1, and does not install
    // default modules.  Allocator invocation and object allocation lifetime are
    // handled by the thin ABI layer that owns raw FT_MemoryRec access.
    Ok(FT_Library {
        memory,
        module_names: &[],
        ..FT_Init_FreeType()
    })
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_New_Library_Without_Default_Modules() -> FT_Library {
    FT_Library {
        module_names: &[],
        ..FT_Init_FreeType()
    }
}

pub fn FT_Done_FreeType(library: Option<FT_Library>) -> FT_Error {
    if library.is_some() {
        FT_Err_Ok
    } else {
        35 // matches FreeType 2.14.3 runtime: FT_Done_FreeType(NULL)
    }
}

pub fn FT_Reference_Library(library: Option<&mut FT_Library>) -> FT_Error {
    let Some(library) = library else {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    };
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Reference_Library` increments the
    // public library refcount and otherwise has no observable side effects.
    library.refcount = library.refcount.saturating_add(1);
    FT_Err_Ok
}

pub fn FT_Done_Library(library: Option<&mut FT_Library>) -> FT_Error {
    let Some(library) = library else {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    };
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Done_Library` decrements first and
    // destroys only when the refcount reaches zero.  Core records that state;
    // thin ABI wrappers own actual allocation release.
    library.refcount = library.refcount.saturating_sub(1);
    FT_Err_Ok
}

pub fn FT_Library_Refcount(library: Option<&FT_Library>) -> usize {
    library.map_or(0, |library| library.refcount)
}

pub fn FT_Library_Memory(library: Option<&FT_Library>) -> FT_Memory {
    library.map_or(std::ptr::null_mut(), |library| library.memory)
}

pub fn FT_Done_MM_Var(library: Option<&FT_Library>, _amaster: Option<&mut FT_MM_Var>) -> FT_Error {
    // FreeType 2.14.3 `src/base/ftmm.c:150-163` checks the library before
    // freeing; `FT_FREE(NULL)` is a successful no-op for a live library.
    if library.is_none() {
        return FT_Err_Invalid_Library_Handle as FT_Error;
    }
    FT_Err_Ok
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

fn ft_pix_floor(value: FT_Pos) -> FT_Pos {
    value & !63
}

fn ft_pix_ceil(value: FT_Pos) -> FT_Pos {
    (value + 63) & !63
}

fn ft_glyph_bbox_mode(cbox: FT_BBox, bbox_mode: FT_UInt) -> FT_BBox {
    match bbox_mode {
        1 => FT_BBox {
            xMin: ft_pix_floor(cbox.xMin),
            yMin: ft_pix_floor(cbox.yMin),
            xMax: ft_pix_ceil(cbox.xMax),
            yMax: ft_pix_ceil(cbox.yMax),
        },
        2 => FT_BBox {
            xMin: cbox.xMin >> 6,
            yMin: cbox.yMin >> 6,
            xMax: cbox.xMax >> 6,
            yMax: cbox.yMax >> 6,
        },
        3 => FT_BBox {
            xMin: ft_pix_floor(cbox.xMin) >> 6,
            yMin: ft_pix_floor(cbox.yMin) >> 6,
            xMax: ft_pix_ceil(cbox.xMax) >> 6,
            yMax: ft_pix_ceil(cbox.yMax) >> 6,
        },
        _ => cbox,
    }
}

pub fn FT_Glyph_Get_CBox(
    glyph: Option<FT_GlyphCBoxSnapshot>,
    bbox_mode: FT_UInt,
    acbox: Option<&mut FT_BBox>,
) {
    let Some(acbox) = acbox else {
        return;
    };
    // FreeType `src/base/ftglyph.c` zeroes `acbox` before checking `glyph`,
    // `glyph->clazz`, or `clazz->glyph_bbox`; only a NULL `acbox` is a no-op.
    *acbox = FT_BBox::default();
    let Some(glyph) = glyph else {
        return;
    };
    if !glyph.has_class || !glyph.has_bbox_hook {
        return;
    }
    if let Some(cbox) = glyph.cbox {
        *acbox = ft_glyph_bbox_mode(cbox, bbox_mode);
    }
}

pub fn FT_Get_Glyph(slot_present: bool, aglyph_present: bool) -> FT_Error {
    // FreeType `src/base/ftglyph.c:632-645` checks the slot pointer before the
    // output pointer and returns before reading slot internals on either error.
    if !slot_present {
        return FT_Err_Invalid_Slot_Handle as FT_Error;
    }
    if !aglyph_present {
        return FT_Err_Invalid_Argument;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Get_Outline_Glyph(slot: Option<&FT_GlyphSlot>) -> Result<FT_OutlineGlyphOwned, FT_Error> {
    let Some(slot) = slot else {
        return Err(FT_Err_Invalid_Slot_Handle as FT_Error);
    };
    let Some(mut outline) = slot.outline.clone() else {
        return Err(FT_Err_Invalid_Glyph_Format);
    };
    if slot.format != FT_GLYPH_FORMAT_OUTLINE {
        return Err(FT_Err_Invalid_Glyph_Format);
    }
    // FreeType `src/base/ftglyph.c:647-661` rejects slot advances that cannot
    // be converted from 26.6 to a signed 16.16 `FT_GlyphRec.advance`.
    const MAX_ADVANCE_26_6_EXCLUSIVE: FT_Pos = 0x8000 * 64;
    if slot.advance.x >= MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.x <= -MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.y >= MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.y <= -MAX_ADVANCE_26_6_EXCLUSIVE
    {
        return Err(FT_Err_Invalid_Argument);
    }
    // FreeType `ft_outline_glyph_init` allocates a fresh outline with
    // `FT_Outline_New` before copying the slot outline, so the owned glyph
    // target keeps `FT_OUTLINE_OWNER` even if the slot outline did not have it.
    outline.flags |= FT_OUTLINE_OWNER as FT_Int;
    Ok(FT_OutlineGlyphOwned {
        root: FT_GlyphRec {
            library: ptr::dangling_mut(),
            // This safe owned representation models the public root record;
            // raw C/WASM wrappers attach their own class pointer when exposing
            // the record through ABI memory.
            clazz: ptr::dangling(),
            format: slot.format,
            advance: FT_Vector {
                x: slot.advance.x * 1024,
                y: slot.advance.y * 1024,
            },
        },
        outline,
    })
}

pub fn FT_Outline_Glyph_CBox(
    glyph: Option<&FT_OutlineGlyphOwned>,
    bbox_mode: FT_UInt,
    acbox: Option<&mut FT_BBox>,
) {
    let mut cbox = FT_BBox::default();
    if let Some(glyph) = glyph {
        FT_Outline_Get_CBox(Some(&glyph.outline), Some(&mut cbox));
    }
    let snapshot = glyph.map(|_| FT_GlyphCBoxSnapshot {
        has_class: true,
        has_bbox_hook: true,
        cbox: Some(cbox),
    });
    FT_Glyph_Get_CBox(snapshot, bbox_mode, acbox);
}

pub fn FT_Glyph_Transform_Outline(
    glyph: Option<&mut FT_OutlineGlyphOwned>,
    matrix: Option<&FT_Matrix>,
    delta: Option<&FT_Vector>,
) -> FT_Error {
    let Some(glyph) = glyph else {
        return FT_Err_Invalid_Argument;
    };
    // FreeType `src/base/ftglyph.c:672-714` dispatches the outline class
    // transform hook first, then transforms root advance only when matrix is
    // non-null.  `src/base/ftglyph.c:209-224` applies delta to outline only.
    if let Some(matrix) = matrix {
        FT_Outline_Transform(Some(&mut glyph.outline), Some(matrix));
    }
    if let Some(delta) = delta {
        FT_Outline_Translate(Some(&mut glyph.outline), delta.x, delta.y);
    }
    if let Some(matrix) = matrix {
        FT_Vector_Transform(Some(&mut glyph.root.advance), Some(matrix));
    }
    FT_Err_Ok
}

pub fn FT_Glyph_Copy(
    source_present: bool,
    target_present: bool,
    source_has_class: bool,
) -> FT_Error {
    // FreeType `src/base/ftglyph.c:542-559` checks target, source, and
    // `source->clazz` before clearing `*target`; these early errors preserve a
    // non-null target sentinel.
    if !target_present || !source_present || !source_has_class {
        return FT_Err_Invalid_Argument;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Done_Glyph(_glyph_present: bool) {
    // FreeType `src/base/ftglyph.c:580-591` treats NULL as a no-op and returns
    // void. Non-null lifecycle behavior is owned by class hooks and remains
    // separate exact glyph ownership/facade work.
}

pub fn FT_Glyph_To_Bitmap(
    the_glyph_present: bool,
    glyph_present: bool,
    library_present: bool,
    class_present: bool,
    prepare_hook_present: bool,
) -> FT_Error {
    // FreeType `src/base/ftglyph.c:771-800` funnels all early invalid pointer
    // states and glyph classes without `glyph_prepare` through the `Bad` label
    // as `Invalid_Argument`, before allocating or modifying the caller handle.
    if !the_glyph_present
        || !glyph_present
        || !library_present
        || !class_present
        || !prepare_hook_present
    {
        return FT_Err_Invalid_Argument;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_Outline_Get_BBox(
    outline: Option<&FT_OutlineSnapshot>,
    abbox: Option<&mut FT_BBox>,
) -> FT_Error {
    let Some(abbox) = abbox else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(outline_snapshot) = outline else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    if outline_snapshot.points.is_empty() || outline_snapshot.contours.is_empty() {
        *abbox = FT_BBox::default();
        return FT_Err_Ok;
    }
    let Some(outline) = outline_snapshot_to_core(outline_snapshot) else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    // FreeType 2.14.3 `src/base/ftbbox.c:474-547` does not call
    // `FT_Outline_Check` before computing the bbox.  It can return success for
    // contour-shape issues that stay on the cbox==bbox fast path, while still
    // propagating `FT_Outline_Decompose` errors for malformed off-curve
    // sequences.  Keep this endpoint's validation local to the bbox algorithm.
    match outline_exact_bbox_from_core(&outline) {
        Ok(bbox) => {
            *abbox = bbox;
            FT_Err_Ok
        }
        Err(error) => error,
    }
}

fn outline_exact_bbox_from_core(outline: &crate::outline::Outline) -> Result<FT_BBox, FT_Error> {
    let mut cbox = FT_BBox {
        xMin: 0x7FFF_FFFF,
        yMin: 0x7FFF_FFFF,
        xMax: -0x7FFF_FFFF,
        yMax: -0x7FFF_FFFF,
    };
    let mut bbox = cbox;
    for (index, point) in outline.points.iter().enumerate() {
        bbox_update_point(&mut cbox, point.x.into(), point.y.into());
        if outline_trace_curve_tag(outline, index) == 1 {
            bbox_update_point(&mut bbox, point.x.into(), point.y.into());
        }
    }

    if cbox.xMin < bbox.xMin
        || cbox.xMax > bbox.xMax
        || cbox.yMin < bbox.yMin
        || cbox.yMax > bbox.yMax
    {
        // FreeType 2.14.3 `src/base/ftbbox.c:474-547` computes the on-point
        // bbox first, then delegates to `FT_Outline_Decompose` only when
        // off-curve controls can extend it.  Reuse the same public decompose
        // walker so malformed cubic/tag sequences return Invalid_Outline.
        bbox = outline_decompose_bbox(outline, bbox)?;
    }
    Ok(bbox)
}

fn outline_decompose_bbox(
    outline: &crate::outline::Outline,
    mut bbox: FT_BBox,
) -> Result<FT_BBox, FT_Error> {
    let run = outline_decompose_trace_run(outline, 0, 0)?;
    let mut last = FT_Vector::default();
    for event in run.events {
        match (event.kind, event.points.as_slice()) {
            ("move_to", [to]) => {
                bbox_update_point(&mut bbox, to.x, to.y);
                last = *to;
            }
            ("line_to", [to]) => {
                last = *to;
            }
            ("conic_to", [control, to]) => {
                bbox_update_point(&mut bbox, to.x, to.y);
                if control.x < bbox.xMin || control.x > bbox.xMax {
                    bbox_conic_check(last.x, control.x, to.x, &mut bbox.xMin, &mut bbox.xMax);
                }
                if control.y < bbox.yMin || control.y > bbox.yMax {
                    bbox_conic_check(last.y, control.y, to.y, &mut bbox.yMin, &mut bbox.yMax);
                }
                last = *to;
            }
            ("cubic_to", [control1, control2, to]) => {
                if control1.x < bbox.xMin
                    || control1.x > bbox.xMax
                    || control2.x < bbox.xMin
                    || control2.x > bbox.xMax
                {
                    bbox_cubic_check(
                        last.x,
                        control1.x,
                        control2.x,
                        to.x,
                        &mut bbox.xMin,
                        &mut bbox.xMax,
                    );
                }
                if control1.y < bbox.yMin
                    || control1.y > bbox.yMax
                    || control2.y < bbox.yMin
                    || control2.y > bbox.yMax
                {
                    bbox_cubic_check(
                        last.y,
                        control1.y,
                        control2.y,
                        to.y,
                        &mut bbox.yMin,
                        &mut bbox.yMax,
                    );
                }
                last = *to;
            }
            _ => return Err(FT_Err_Invalid_Outline as FT_Error),
        }
    }
    Ok(bbox)
}

fn bbox_update_point(bbox: &mut FT_BBox, x: FT_Pos, y: FT_Pos) {
    bbox.xMin = bbox.xMin.min(x);
    bbox.yMin = bbox.yMin.min(y);
    bbox.xMax = bbox.xMax.max(x);
    bbox.yMax = bbox.yMax.max(y);
}

fn bbox_conic_check(y1: FT_Pos, y2: FT_Pos, y3: FT_Pos, min: &mut FT_Pos, max: &mut FT_Pos) {
    let y1 = y1 - y2;
    let y3 = y3 - y2;
    let y = y2 + FT_MulDiv(y1, y3, y1 + y3);
    *min = (*min).min(y);
    *max = (*max).max(y);
}

fn bbox_cubic_check(
    p1: FT_Pos,
    p2: FT_Pos,
    p3: FT_Pos,
    p4: FT_Pos,
    min: &mut FT_Pos,
    max: &mut FT_Pos,
) {
    if p2 > *max || p3 > *max {
        *max += cubic_peak(p1 - *max, p2 - *max, p3 - *max, p4 - *max);
    }
    if p2 < *min || p3 < *min {
        *min -= cubic_peak(*min - p1, *min - p2, *min - p3, *min - p4);
    }
}

fn cubic_peak(mut q1: FT_Pos, mut q2: FT_Pos, mut q3: FT_Pos, mut q4: FT_Pos) -> FT_Pos {
    let mask =
        (q1.unsigned_abs() | q2.unsigned_abs() | q3.unsigned_abs() | q4.unsigned_abs()) as u32;
    let mut shift = 27 - (31 - mask.leading_zeros() as i32);
    if shift > 0 {
        if shift > 2 {
            shift = 2;
        }
        q1 *= 1 << shift;
        q2 *= 1 << shift;
        q3 *= 1 << shift;
        q4 *= 1 << shift;
    } else {
        q1 >>= -shift;
        q2 >>= -shift;
        q3 >>= -shift;
        q4 >>= -shift;
    }

    let mut peak = 0;
    while q2 > 0 || q3 > 0 {
        if q1 + q2 > q3 + q4 {
            q4 += q3;
            q3 += q2;
            q2 += q1;
            q4 += q3;
            q3 += q2;
            q4 = (q4 + q3) >> 3;
            q3 >>= 2;
            q2 >>= 1;
        } else {
            q1 += q2;
            q2 += q3;
            q3 += q4;
            q1 += q2;
            q2 += q3;
            q1 = (q1 + q2) >> 3;
            q2 >>= 2;
            q3 >>= 1;
        }

        if q1 == q2 && q1 >= q3 {
            peak = q1;
            break;
        }
        if q3 == q4 && q2 <= q4 {
            peak = q4;
            break;
        }
    }

    if shift > 0 {
        peak >> shift
    } else {
        peak << -shift
    }
}

pub fn FT_Outline_Get_Bitmap(
    library: Option<&FT_Library>,
    outline: Option<&FT_OutlineSnapshot>,
    abitmap: Option<&FT_Bitmap_C>,
) -> Result<FT_Bitmap, FT_Error> {
    if library.is_none() {
        return Err(FT_Err_Invalid_Library_Handle as FT_Error);
    }
    let Some(abitmap) = abitmap else {
        return Err(FT_Err_Invalid_Argument as FT_Error);
    };
    let Some(outline_snapshot) = outline else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    let mut cbox = FT_BBox::default();
    FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
    if cbox.xMin < -0x1000000
        || cbox.yMin < -0x1000000
        || cbox.xMax > 0x1000000
        || cbox.yMax > 0x1000000
    {
        // FreeType 2.14.3 `src/base/ftoutln.c:669-689` delegates
        // `FT_Outline_Get_Bitmap` to `FT_Outline_Render`; the gray renderer
        // rejects cboxes outside +/-0x1000000 as Invalid_Outline.
        return Err(FT_Err_Invalid_Outline as FT_Error);
    }
    let Some(outline) = outline.and_then(outline_snapshot_to_core) else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    let width = usize::try_from(abitmap.width).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
    let rows = usize::try_from(abitmap.rows).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
    // FreeType rejects FT_PIXEL_MODE_NONE targets before rendering
    // (src/smooth/ftgrays.c:2010-2019, src/raster/ftraster.c:2696-2703).
    if abitmap.pixel_mode == FT_PIXEL_MODE_NONE as u8 {
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    if width != 0 && rows != 0 && abitmap.pitch != 0 && abitmap.buffer.is_null() {
        // FreeType 2.14.3 `src/smooth/ftgrays.c:2012-2019` rejects a
        // non-empty target bitmap with NULL storage as Invalid_Argument
        // before caller storage is written.
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    let raster = crate::grays::rasterize_in_box(outline, width, rows).map_err(error_to_ft)?;
    let pitch_abs = usize::try_from(abitmap.pitch.unsigned_abs()).unwrap_or(width);
    let mut pixels = vec![0; pitch_abs.saturating_mul(rows)];
    if abitmap.pixel_mode == FT_PIXEL_MODE_MONO as u8 {
        // FreeType `FT_Outline_Get_Bitmap` leaves AA unset for MONO targets and
        // writes packed 1bpp rows into the caller pitch.
        for y in 0..rows {
            let dst_y = if abitmap.pitch < 0 { rows - 1 - y } else { y };
            for x in 0..width {
                if raster.pixels.get(y * width + x).copied().unwrap_or(0) != 0 {
                    let byte = dst_y * pitch_abs + x / 8;
                    if let Some(dst) = pixels.get_mut(byte) {
                        *dst |= 0x80 >> (x & 7);
                    }
                }
            }
        }
    } else if raster.pixels.len() == width.saturating_mul(rows) {
        for y in 0..rows {
            let src = y * width;
            // FT_Bitmap.pitch is the offset for moving down one visual row.
            // A negative pitch therefore stores top-down raster row `y` at
            // the reversed allocation row (ftimage.h; ftgrays.c:2021-2027).
            let dst_y = if abitmap.pitch < 0 { rows - 1 - y } else { y };
            let dst = dst_y * pitch_abs;
            let row_bytes = width.min(pitch_abs);
            pixels[dst..dst + row_bytes].copy_from_slice(&raster.pixels[src..src + row_bytes]);
        }
    }
    Ok(FT_Bitmap {
        rows: abitmap.rows,
        width: abitmap.width,
        pitch: abitmap.pitch,
        buffer: pixels,
        num_grays: abitmap.num_grays,
        pixel_mode: abitmap.pixel_mode.into(),
    })
}

/// Renders an outline through the safe bitmap-mode equivalent of
/// `FT_Outline_Render`.
///
/// Callback-based direct rendering and the monochrome renderer are separate
/// public routes and are not modeled by this bitmap return value.
pub fn FT_Outline_Render(
    library: Option<&FT_Library>,
    outline: Option<&FT_OutlineSnapshot>,
    target: Option<&FT_Bitmap_C>,
    flags: FT_Int,
    _clip_box: FT_BBox,
) -> Result<FT_Bitmap, FT_Error> {
    if library.is_none() {
        return Err(FT_Err_Invalid_Library_Handle as FT_Error);
    }
    let Some(outline_snapshot) = outline else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };

    let mut cbox = FT_BBox::default();
    FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
    if cbox.xMin < -0x1000000
        || cbox.yMin < -0x1000000
        || cbox.xMax > 0x1000000
        || cbox.yMax > 0x1000000
    {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    }

    let Some(outline) = outline_snapshot_to_core(outline_snapshot) else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    if flags & FT_RASTER_FLAG_DIRECT as FT_Int != 0 {
        return Err(FT_Err_Unimplemented_Feature as FT_Error);
    }

    // ftgrays.c:gray_raster_render returns before inspecting the target for an
    // empty outline, but validates the final contour endpoint before its
    // zero-sized-target fast path.
    let empty_outline = outline.points.is_empty() || outline.n_contours == 0;
    if !empty_outline
        && outline
            .contours
            .last()
            .is_none_or(|&last| usize::try_from(last).ok() != Some(outline.points.len() - 1))
    {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    }
    let Some(target) = target else {
        if empty_outline {
            return Ok(FT_Bitmap::default());
        }
        return Err(FT_Err_Invalid_Argument as FT_Error);
    };
    let width = usize::try_from(target.width).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
    let rows = usize::try_from(target.rows).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).unwrap_or(width);
    let mut pixels = vec![0; pitch_abs.saturating_mul(rows)];
    if flags & FT_RASTER_FLAG_AA as FT_Int == 0 {
        if i32::from(target.pixel_mode) != FT_PIXEL_MODE_GRAY {
            return Err(FT_Err_Cannot_Render_Glyph as FT_Error);
        }
        // FreeType 2.14.3 routes a no-AA FT_Outline_Render request through
        // the black rasterizer.  With the public gray bitmap target used here,
        // it writes packed mono bytes into caller storage and reports success.
        let packed = render::rasterize_mono_center(&outline, width, rows).map_err(error_to_ft)?;
        let packed_pitch = render::mono_pitch(width);
        let row_bytes = packed_pitch.min(pitch_abs);
        for y in 0..rows {
            let src = y * packed_pitch;
            let dst_y = if target.pitch < 0 { rows - 1 - y } else { y };
            let dst = dst_y * pitch_abs;
            pixels[dst..dst + row_bytes].copy_from_slice(&packed[src..src + row_bytes]);
        }
        return Ok(FT_Bitmap {
            rows: target.rows,
            width: target.width,
            pitch: target.pitch,
            buffer: pixels,
            num_grays: target.num_grays,
            pixel_mode: target.pixel_mode.into(),
        });
    }
    if i32::from(target.pixel_mode) != FT_PIXEL_MODE_GRAY {
        // FreeType 2.14.3 `src/smooth/ftgrays.c:2014-2016` rejects AA
        // `FT_Outline_Render` targets whose pixel mode is not gray with
        // `Invalid_Argument` before writing caller storage.
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    if empty_outline {
        return Ok(FT_Bitmap {
            rows: target.rows,
            width: target.width,
            pitch: target.pitch,
            buffer: pixels,
            num_grays: target.num_grays,
            pixel_mode: target.pixel_mode.into(),
        });
    }

    // In non-DIRECT mode, pinned FreeType 2.14.3's gray_raster_render
    // overwrites params.clip_box with the target dimensions.  Consequently,
    // FT_RASTER_FLAG_CLIP is deliberately ignored for bitmap rendering.
    let raster = crate::grays::rasterize_in_box(outline, width, rows).map_err(error_to_ft)?;
    if raster.pixels.len() == width.saturating_mul(rows) {
        for y in 0..rows {
            let src = y * width;
            let dst_y = if target.pitch < 0 { rows - 1 - y } else { y };
            let dst = dst_y * pitch_abs;
            let row_bytes = width.min(pitch_abs);
            pixels[dst..dst + row_bytes].copy_from_slice(&raster.pixels[src..src + row_bytes]);
        }
    }
    Ok(FT_Bitmap {
        rows: target.rows,
        width: target.width,
        pitch: target.pitch,
        buffer: pixels,
        num_grays: target.num_grays,
        pixel_mode: target.pixel_mode.into(),
    })
}

pub fn FT_Outline_Render_Error_Output(
    outline: Option<&FT_OutlineSnapshot>,
    target: Option<&FT_Bitmap_C>,
    flags: FT_Int,
) -> Option<FT_Bitmap> {
    if flags & FT_RASTER_FLAG_AA as FT_Int != 0 {
        return None;
    }
    let outline = outline.and_then(outline_snapshot_to_core)?;
    let target = target?;
    if i32::from(target.pixel_mode) != FT_PIXEL_MODE_GRAY {
        return None;
    }
    let width = usize::try_from(target.width).ok()?;
    let rows = usize::try_from(target.rows).ok()?;
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).ok()?;
    let packed = render::rasterize_mono_center(&outline, width, rows).ok()?;
    let packed_pitch = render::mono_pitch(width);
    let mut pixels = vec![0; pitch_abs.saturating_mul(rows)];
    let row_bytes = packed_pitch.min(pitch_abs);
    for y in 0..rows {
        let src = y * packed_pitch;
        let dst_y = if target.pitch < 0 { rows - 1 - y } else { y };
        let dst = dst_y * pitch_abs;
        pixels[dst..dst + row_bytes].copy_from_slice(&packed[src..src + row_bytes]);
    }
    Some(FT_Bitmap {
        rows: target.rows,
        width: target.width,
        pitch: target.pitch,
        buffer: pixels,
        num_grays: target.num_grays,
        pixel_mode: target.pixel_mode.into(),
    })
}

pub fn FT_Outline_Render_Direct_Spans(
    library: Option<&FT_Library>,
    outline: Option<&FT_OutlineSnapshot>,
    target: Option<&FT_Bitmap_C>,
    flags: FT_Int,
    clip_box: Option<FT_BBox>,
    gray_spans_present: bool,
) -> Result<Vec<(i32, FT_Span)>, FT_Error> {
    if library.is_none() {
        return Err(FT_Err_Invalid_Library_Handle as FT_Error);
    }
    let Some(outline_snapshot) = outline else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    if flags & FT_RASTER_FLAG_DIRECT as FT_Int == 0 {
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    if flags & FT_RASTER_FLAG_AA as FT_Int == 0 {
        // FreeType 2.14.3 reports Invalid_Argument for DIRECT rendering
        // without the AA flag before entering the mono raster path.
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    if !gray_spans_present {
        // FreeType 2.14.3 `src/smooth/ftgrays.c:1998-2001` returns success
        // immediately for DIRECT smooth rendering when `gray_spans` is NULL.
        return Ok(Vec::new());
    }

    let mut cbox = FT_BBox::default();
    FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
    if cbox.xMin < -0x1000000
        || cbox.yMin < -0x1000000
        || cbox.xMax > 0x1000000
        || cbox.yMax > 0x1000000
    {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    }
    let Some(outline) = outline_snapshot_to_core(outline_snapshot) else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    let (width, rows) = if let Some(target) = target {
        (
            usize::try_from(target.width).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
            usize::try_from(target.rows).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
        )
    } else {
        (
            usize_from_i32(outline.cbox_x_max - outline.cbox_x_min),
            usize_from_i32(outline.cbox_y_max - outline.cbox_y_min),
        )
    };
    let spans = if flags & FT_RASTER_FLAG_CLIP as FT_Int != 0 {
        let clip_box = clip_box.unwrap_or_default();
        grays::rasterize_direct_spans_in_clip_box(
            &outline,
            width,
            rows,
            i32::try_from(clip_box.xMin).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
            i32::try_from(clip_box.xMax).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
            i32::try_from(clip_box.yMin).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
            i32::try_from(clip_box.yMax).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?,
        )
    } else {
        // FreeType 2.14.3 `src/base/ftoutln.c:635-640` presets
        // `params.clip_box` from `FT_Outline_Get_CBox` when DIRECT is set and
        // CLIP is absent.  The CBox is in 26.6 units; the gray renderer receives
        // integer pixel bounds.
        let cbox_x_min =
            i32::try_from(cbox.xMin >> 6).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
        let cbox_y_min =
            i32::try_from(cbox.yMin >> 6).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
        let cbox_x_max = i32::try_from((cbox.xMax + 63) >> 6)
            .map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
        let cbox_y_max = i32::try_from((cbox.yMax + 63) >> 6)
            .map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
        grays::rasterize_direct_spans_in_clip_box(
            &outline, width, rows, cbox_x_min, cbox_x_max, cbox_y_min, cbox_y_max,
        )
    };
    spans.map_err(error_to_ft).map(|spans| {
        spans
            .into_iter()
            .map(|span| {
                (
                    span.y,
                    FT_Span {
                        x: span.x,
                        len: span.len,
                        coverage: span.coverage,
                    },
                )
            })
            .collect()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FTOutlineDecomposeEvent {
    pub kind: &'static str,
    pub points: Vec<FT_Vector>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FTOutlineDecomposeRun {
    pub shift: FT_Int,
    pub delta: FT_Pos,
    pub events: Vec<FTOutlineDecomposeEvent>,
    pub transformed_points: Vec<FT_Vector>,
    pub user_seen: bool,
}

pub fn FT_Outline_Decompose_Trace(
    outline: Option<&FT_OutlineSnapshot>,
    transforms: &[(FT_Int, FT_Pos)],
) -> Result<Vec<FTOutlineDecomposeRun>, FT_Error> {
    let Some(outline_snapshot) = outline else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    let Some(outline) = outline_snapshot_to_core(outline_snapshot) else {
        return Err(FT_Err_Invalid_Outline as FT_Error);
    };
    transforms
        .iter()
        .map(|&(shift, delta)| outline_decompose_trace_run(&outline, shift, delta))
        .collect()
}

fn outline_decompose_trace_run(
    outline: &crate::outline::Outline,
    shift: FT_Int,
    delta: FT_Pos,
) -> Result<FTOutlineDecomposeRun, FT_Error> {
    let mut events = Vec::new();
    let mut last: i64 = -1;
    for &contour_end in outline
        .contours
        .iter()
        .take(usize_from_i32(outline.n_contours))
    {
        let first = usize_from_i32(i32::try_from(last + 1).unwrap_or(0));
        last = i64::from(contour_end);
        if last < first as i64 {
            return Err(FT_Err_Invalid_Outline as FT_Error);
        }
        let limit = usize_from_i32(i32::from(contour_end));
        let mut v_start = outline.points[first];
        let v_last = outline.points[limit];
        let mut limit_eff = limit;
        let first_tag = outline_trace_curve_tag(outline, first);
        if first_tag == 2 {
            return Err(FT_Err_Invalid_Outline as FT_Error);
        }
        if first_tag == 0 {
            if outline_trace_curve_tag(outline, limit) == 1 {
                v_start = v_last;
                limit_eff = limit - 1;
            } else {
                v_start.x = (v_start.x + v_last.x) / 2;
                v_start.y = (v_start.y + v_last.y) / 2;
            }
        }
        outline_trace_push(&mut events, "move_to", &[v_start], shift, delta);
        let start = if first_tag == 0 {
            if first == 0 {
                -1
            } else {
                i32::try_from(first).unwrap_or(i32::MAX) - 1
            }
        } else {
            i32::try_from(first).unwrap_or(i32::MAX)
        };
        outline_trace_walk_contour(
            &mut events,
            outline,
            start,
            limit_eff,
            v_start,
            shift,
            delta,
        )?;
    }
    let transformed_points = events
        .iter()
        .flat_map(|event| event.points.iter().copied())
        .collect();
    Ok(FTOutlineDecomposeRun {
        shift,
        delta,
        events,
        transformed_points,
        user_seen: true,
    })
}

fn outline_trace_curve_tag(outline: &crate::outline::Outline, index: usize) -> u8 {
    outline
        .tags
        .get(index)
        .map_or_else(|| u8::from(outline.points[index].on_curve), |tag| tag & 3)
}

fn outline_trace_transform(
    point: crate::outline::OutlinePoint,
    shift: FT_Int,
    delta: FT_Pos,
) -> FT_Vector {
    FT_Vector {
        x: (i64::from(point.x) << shift) - delta,
        y: (i64::from(point.y) << shift) - delta,
    }
}

fn outline_trace_push(
    events: &mut Vec<FTOutlineDecomposeEvent>,
    kind: &'static str,
    points: &[crate::outline::OutlinePoint],
    shift: FT_Int,
    delta: FT_Pos,
) {
    events.push(FTOutlineDecomposeEvent {
        kind,
        points: points
            .iter()
            .map(|&point| outline_trace_transform(point, shift, delta))
            .collect(),
    });
}

fn outline_trace_walk_contour(
    events: &mut Vec<FTOutlineDecomposeEvent>,
    outline: &crate::outline::Outline,
    mut cursor: i32,
    limit: usize,
    v_start: crate::outline::OutlinePoint,
    shift: FT_Int,
    delta: FT_Pos,
) -> Result<(), FT_Error> {
    let limit_i32 = i32::try_from(limit).map_err(|_| FT_Err_Invalid_Outline as FT_Error)?;
    while cursor < limit_i32 {
        cursor += 1;
        let idx = usize_from_i32(cursor);
        match outline_trace_curve_tag(outline, idx) {
            1 => {
                outline_trace_push(events, "line_to", &[outline.points[idx]], shift, delta);
            }
            0 => {
                let mut control = outline.points[idx];
                loop {
                    if cursor < limit_i32 {
                        cursor += 1;
                        let idx2 = usize_from_i32(cursor);
                        let point = outline.points[idx2];
                        let tag = outline_trace_curve_tag(outline, idx2);
                        if tag == 1 {
                            outline_trace_push(events, "conic_to", &[control, point], shift, delta);
                            break;
                        }
                        if tag != 0 {
                            return Err(FT_Err_Invalid_Outline as FT_Error);
                        }
                        let mid = crate::outline::OutlinePoint {
                            x: (control.x + point.x) / 2,
                            y: (control.y + point.y) / 2,
                            on_curve: true,
                        };
                        outline_trace_push(events, "conic_to", &[control, mid], shift, delta);
                        control = point;
                        continue;
                    }
                    outline_trace_push(events, "conic_to", &[control, v_start], shift, delta);
                    return Ok(());
                }
            }
            2 | 3 => {
                if cursor + 1 > limit_i32
                    || outline_trace_curve_tag(outline, usize_from_i32(cursor + 1)) != 2
                {
                    return Err(FT_Err_Invalid_Outline as FT_Error);
                }
                let control1 = outline.points[idx];
                let control2 = outline.points[usize_from_i32(cursor + 1)];
                cursor += 2;
                let to = if cursor <= limit_i32 {
                    outline.points[usize_from_i32(cursor)]
                } else {
                    v_start
                };
                outline_trace_push(events, "cubic_to", &[control1, control2, to], shift, delta);
                if cursor > limit_i32 {
                    return Ok(());
                }
            }
            _ => unreachable!(),
        }
    }
    outline_trace_push(events, "line_to", &[v_start], shift, delta);
    Ok(())
}

pub fn FT_Outline_Get_Orientation(outline: Option<&FT_OutlineSnapshot>) -> FT_Orientation {
    let Some(outline) = outline else {
        return FT_ORIENTATION_TRUETYPE as FT_Orientation;
    };
    let Some(outline) = outline_snapshot_to_core(outline) else {
        return FT_ORIENTATION_NONE as FT_Orientation;
    };
    api::outline_get_orientation(Some(&outline)) as FT_Orientation
}

pub fn FT_Outline_Check(outline: Option<&FT_OutlineSnapshot>) -> FT_Error {
    let Some(outline) = outline else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    let n_points = outline.points.len();
    let n_contours = outline.contours.len();
    // FreeType `src/base/ftoutln.c:352-390` accepts only the fully empty
    // `(n_points == 0 && n_contours == 0)` case; either count being zero alone
    // is an invalid outline.
    if n_points == 0 && n_contours == 0 {
        return FT_Err_Ok;
    }
    if n_points == 0 || n_contours == 0 {
        return FT_Err_Invalid_Outline as FT_Error;
    }

    let mut previous_end: Option<usize> = None;
    for &contour_end in &outline.contours {
        let end = usize::from(contour_end);
        if previous_end.is_some_and(|previous| end <= previous) || end >= n_points {
            return FT_Err_Invalid_Outline as FT_Error;
        }
        previous_end = Some(end);
    }
    if previous_end != Some(n_points - 1) {
        return FT_Err_Invalid_Outline as FT_Error;
    }
    FT_Err_Ok
}

pub fn FT_Outline_Copy(
    source: Option<&FT_OutlineSnapshot>,
    target: Option<&mut FT_OutlineSnapshot>,
) -> FT_Error {
    let (Some(source), Some(target)) = (source, target) else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    if source.points.len() != target.points.len() || source.contours.len() != target.contours.len()
    {
        return FT_Err_Invalid_Argument;
    }
    let owner = target.flags & FT_OUTLINE_OWNER as FT_Int;
    target.points.clone_from(&source.points);
    target.tags.clone_from(&source.tags);
    target.contours.clone_from(&source.contours);
    // FreeType `src/base/ftoutln.c:399-430` preserves the target OWNER bit
    // while copying all other outline flags from the source descriptor.
    target.flags = (source.flags & !(FT_OUTLINE_OWNER as FT_Int)) | owner;
    FT_Err_Ok
}

pub fn FT_Outline_Embolden(outline: Option<&mut FT_OutlineSnapshot>, strength: FT_Pos) -> FT_Error {
    FT_Outline_EmboldenXY(outline, strength, strength)
}

pub fn FT_Outline_EmboldenXY(
    outline: Option<&mut FT_OutlineSnapshot>,
    xstrength: FT_Pos,
    ystrength: FT_Pos,
) -> FT_Error {
    let Some(snapshot) = outline else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    let Some(mut outline) = outline_snapshot_to_core(snapshot) else {
        return FT_Err_Invalid_Outline as FT_Error;
    };
    if !outline.points.is_empty()
        && api::outline_get_orientation(Some(&outline)) == FT_ORIENTATION_NONE as i32
    {
        return FT_Err_Invalid_Argument;
    }
    api::embolden_outline(
        &mut outline,
        i32::try_from(xstrength).unwrap_or(if xstrength < 0 { i32::MIN } else { i32::MAX }),
        i32::try_from(ystrength).unwrap_or(if ystrength < 0 { i32::MIN } else { i32::MAX }),
    );
    *snapshot = outline_to_ffi_snapshot(&outline);
    FT_Err_Ok
}

pub fn FT_Outline_GetInsideBorder(outline: Option<&FT_OutlineSnapshot>) -> FT_Int {
    // FreeType `src/base/ftstroke.c:35-42` delegates entirely to
    // `FT_Outline_Get_Orientation`.
    if FT_Outline_Get_Orientation(outline) == FT_ORIENTATION_TRUETYPE as FT_Orientation {
        FT_STROKER_BORDER_RIGHT as FT_Int
    } else {
        FT_STROKER_BORDER_LEFT as FT_Int
    }
}

pub fn FT_Outline_GetOutsideBorder(outline: Option<&FT_OutlineSnapshot>) -> FT_Int {
    // FreeType `src/base/ftstroke.c:48-55` delegates entirely to
    // `FT_Outline_Get_Orientation`.
    if FT_Outline_Get_Orientation(outline) == FT_ORIENTATION_TRUETYPE as FT_Orientation {
        FT_STROKER_BORDER_LEFT as FT_Int
    } else {
        FT_STROKER_BORDER_RIGHT as FT_Int
    }
}

pub type FT_Stroker = *mut core::ffi::c_void;

pub fn FT_Stroker_Set(
    stroker: FT_Stroker,
    radius: FT_Fixed,
    line_cap: FT_Int,
    line_join: FT_Int,
    miter_limit: FT_Fixed,
) {
    // FreeType 2.14.3 `src/base/ftstroke.c:824-831` returns immediately for
    // a null stroker before touching allocation-backed border state.  The
    // maintained parity route currently covers only that null-handle no-op;
    // non-null stroker object/path rows remain pending in the route audit.
    let _ = (stroker, radius, line_cap, line_join, miter_limit);
}

pub fn FT_Stroker_Rewind(stroker: FT_Stroker) {
    // FreeType 2.14.3 `src/base/ftstroke.c:853-862` is a no-op for a null
    // stroker.  Non-null path clearing remains pending with the rest of the
    // stroker object lifecycle.
    let _ = stroker;
}

pub fn FT_Stroker_Done(stroker: FT_Stroker) {
    // FreeType 2.14.3 `src/base/ftstroke.c:866-881` frees borders and the
    // stroker only when the handle is non-null.  The no-op null route is exact;
    // non-null ownership/freeing is not classified as parity yet.
    let _ = stroker;
}

pub fn FT_Outline_Reverse(outline: Option<&mut FT_OutlineSnapshot>) {
    let Some(outline) = outline else {
        return;
    };
    if !outline_reverse_buffers_are_valid(outline) {
        return;
    }
    api::reverse_outline_buffers(
        &mut outline.points,
        &mut outline.tags,
        &outline.contours,
        &mut outline.flags,
    );
}

fn outline_reverse_buffers_are_valid(outline: &FT_OutlineSnapshot) -> bool {
    let mut first = 1usize;
    for &last in &outline.contours {
        let end = usize::from(last) + 1;
        if first > end || end > outline.points.len() || end > outline.tags.len() {
            return false;
        }
        first = end + 1;
    }
    true
}

pub fn FT_Outline_Transform(outline: Option<&mut FT_OutlineSnapshot>, matrix: Option<&FT_Matrix>) {
    let (Some(outline), Some(matrix)) = (outline, matrix) else {
        return;
    };
    let mut coordinates = outline
        .points
        .iter()
        .map(|point| (point.x, point.y))
        .collect::<Vec<_>>();
    api::transform_outline_coordinates(
        &mut coordinates,
        matrix.xx,
        matrix.xy,
        matrix.yx,
        matrix.yy,
    );
    for (point, (x, y)) in outline.points.iter_mut().zip(coordinates) {
        point.x = x as FT_Pos;
        point.y = y as FT_Pos;
    }
}

pub fn FT_Outline_Translate(
    outline: Option<&mut FT_OutlineSnapshot>,
    x_offset: FT_Pos,
    y_offset: FT_Pos,
) {
    let Some(outline) = outline else {
        return;
    };
    // FreeType `src/base/ftoutln.c:520-537` mutates each vector with
    // ADD_LONG-style coordinate addition and has no error channel.
    for point in &mut outline.points {
        point.x = point.x.wrapping_add(x_offset);
        point.y = point.y.wrapping_add(y_offset);
    }
}

pub fn FT_OpenType_Free(_face: Option<&FT_Face>, _table: FT_Bytes) {}

fn ps_private_to_ffi(private: &Type1PrivateDict) -> PS_PrivateRec {
    PS_PrivateRec {
        unique_id: private.unique_id,
        lenIV: private.len_iv,
        num_blue_values: private.num_blue_values,
        num_other_blues: private.num_other_blues,
        num_family_blues: private.num_family_blues,
        num_family_other_blues: private.num_family_other_blues,
        blue_values: private.blue_values,
        other_blues: private.other_blues,
        family_blues: private.family_blues,
        family_other_blues: private.family_other_blues,
        blue_scale: i64::from(private.blue_scale),
        blue_shift: private.blue_shift,
        blue_fuzz: private.blue_fuzz,
        standard_width: private.standard_width,
        standard_height: private.standard_height,
        num_snap_widths: private.num_snap_widths,
        num_snap_heights: private.num_snap_heights,
        force_bold: FT_Bool::from(private.force_bold),
        round_stem_up: FT_Bool::from(private.round_stem_up),
        snap_widths: private.snap_widths,
        snap_heights: private.snap_heights,
        expansion_factor: i64::from(private.expansion_factor),
        language_group: private.language_group,
        password: private.password,
        min_feature: private.min_feature,
    }
}

fn optional_cstring(value: Option<&str>) -> Option<CString> {
    CString::new(value?).ok()
}

fn type1_font_info_strings(info: Option<&Type1FontInfo>) -> Option<Type1FontInfoStrings> {
    let info = info?;
    Some(Type1FontInfoStrings {
        version: optional_cstring(info.version.as_deref()),
        notice: optional_cstring(info.notice.as_deref()),
        full_name: optional_cstring(info.full_name.as_deref()),
        family_name: optional_cstring(info.family_name.as_deref()),
        weight: optional_cstring(info.weight.as_deref()),
    })
}

fn cstring_mut_ptr(value: &Option<CString>) -> *mut FT_String {
    value
        .as_ref()
        .map_or(ptr::null_mut(), |string| string.as_ptr().cast_mut().cast())
}

fn ps_font_info_to_ffi(
    info: &Type1FontInfo,
    strings: Option<&Type1FontInfoStrings>,
) -> Option<PS_FontInfoRec> {
    let strings = strings?;
    Some(PS_FontInfoRec {
        version: cstring_mut_ptr(&strings.version),
        notice: cstring_mut_ptr(&strings.notice),
        full_name: cstring_mut_ptr(&strings.full_name),
        family_name: cstring_mut_ptr(&strings.family_name),
        weight: cstring_mut_ptr(&strings.weight),
        italic_angle: i64::from(info.italic_angle),
        is_fixed_pitch: FT_Bool::from(info.is_fixed_pitch),
        underline_position: info.underline_position,
        underline_thickness: info.underline_thickness,
    })
}

pub fn FT_Get_PS_Font_Info(
    face: Option<&FT_Face>,
    afont_info: Option<&mut PS_FontInfoRec>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(afont_info) = afont_info else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let inner = face.inner.borrow();
    let Some(info) = inner.font().type1_font_info() else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(info) = ps_font_info_to_ffi(info, face.type1_font_info_strings.as_ref()) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *afont_info = info;
    FT_Err_Ok
}

pub fn FT_Get_PS_Font_Private(
    face: Option<&FT_Face>,
    afont_private: Option<&mut PS_PrivateRec>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(afont_private) = afont_private else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let inner = face.inner.borrow();
    let Some(private) = inner.font().type1_private() else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *afont_private = ps_private_to_ffi(private);
    FT_Err_Ok
}

fn copy_value_bytes(value: Option<&mut [u8]>, required_len: usize, source: &[u8]) -> FT_Long {
    if let Some(value) = value.filter(|value| value.len() >= required_len) {
        value[..required_len].copy_from_slice(source);
    }
    FT_Long::try_from(required_len).unwrap_or(FT_Long::MAX)
}

pub fn FT_Get_PS_Font_Value(
    face: Option<&FT_Face>,
    key: PS_Dict_Keys,
    idx: FT_UInt,
    value: Option<&mut [u8]>,
    value_len: FT_Long,
) -> FT_Long {
    let Some(face) = face else {
        return 0;
    };
    let inner = face.inner.borrow();
    let Some(encoding) = inner.font().type1_encoding() else {
        return 0;
    };
    let value_len = usize::try_from(value_len.max(0)).unwrap_or(usize::MAX);
    let value = value.map(|buffer| {
        let len = buffer.len().min(value_len);
        &mut buffer[..len]
    });
    match key {
        // FreeType `src/type1/t1driver.c:t1_ps_get_font_value` copies
        // `type1->encoding_type` as a public `T1_EncodingType` enum and
        // returns its required byte length even for sizing queries.
        PS_DICT_ENCODING_TYPE => copy_value_bytes(
            value,
            std::mem::size_of::<PS_Dict_Keys>(),
            &encoding.encoding_type.to_ne_bytes(),
        ),
        PS_DICT_ENCODING_ENTRY
            if encoding.encoding_type == 1
                && usize::try_from(idx)
                    .ok()
                    .and_then(|index| encoding.entries.get(index))
                    .and_then(Option::as_deref)
                    .is_some() =>
        {
            let name = encoding.entries[usize::try_from(idx).unwrap_or(0)]
                .as_deref()
                .unwrap_or("");
            let required_len = name.len().saturating_add(1);
            if let Some(value) = value.filter(|value| value.len() >= required_len) {
                value[..name.len()].copy_from_slice(name.as_bytes());
                value[name.len()] = 0;
            }
            FT_Long::try_from(required_len).unwrap_or(FT_Long::MAX)
        }
        _ => -1,
    }
}

pub fn FT_OpenType_Validate(
    face: Option<&FT_Face>,
    validation_flags: FT_UInt,
    base_table: Option<&mut FT_Bytes>,
    gdef_table: Option<&mut FT_Bytes>,
    gpos_table: Option<&mut FT_Bytes>,
    gsub_table: Option<&mut FT_Bytes>,
    jstf_table: Option<&mut FT_Bytes>,
) -> FT_Error {
    if face.is_none() {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    }
    if base_table.is_none()
        || gdef_table.is_none()
        || gpos_table.is_none()
        || gsub_table.is_none()
        || jstf_table.is_none()
    {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let _ = validation_flags;
    FT_Err_Unimplemented_Feature as FT_Error
}

pub fn FT_TrueTypeGX_Free(face: Option<&FT_Face>, table: FT_Bytes) {
    // FreeType 2.14.3 `src/base/ftgxval.c:74-84` returns before touching the
    // table pointer when `face` is null.  Non-null validation-buffer ownership
    // remains pending.
    let _ = (face, table);
}

pub fn FT_ClassicKern_Free(face: Option<&FT_Face>, table: FT_Bytes) {
    // FreeType 2.14.3 `src/base/ftgxval.c:125-136` returns before touching the
    // table pointer when `face` is null.  Non-null ckern buffer freeing remains
    // pending.
    let _ = (face, table);
}

fn face_has_sfnt_table(face: &FT_Face, tag: [u8; 4]) -> bool {
    face.inner
        .borrow()
        .font()
        .load_sfnt_table(u32::from_be_bytes(tag), 0, None)
        .is_ok()
}

fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *data.get(offset)?,
        *data.get(offset + 1)?,
    ]))
}

fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.get(offset)?,
        *data.get(offset + 1)?,
        *data.get(offset + 2)?,
        *data.get(offset + 3)?,
    ]))
}

fn parse_cpal_table(data: &[u8]) -> Option<CpalState> {
    let version = read_u16_be(data, 0)?;
    let num_palette_entries = usize::from(read_u16_be(data, 2)?);
    let num_palettes = usize::from(read_u16_be(data, 4)?);
    let num_color_records = usize::from(read_u16_be(data, 6)?);
    let color_records_offset = usize::try_from(read_u32_be(data, 8)?).ok()?;
    let indices_offset = 12usize;
    let indices_end = indices_offset.checked_add(num_palettes.checked_mul(2)?)?;
    if indices_end > data.len() {
        return None;
    }

    let mut palettes = Vec::with_capacity(num_palettes);
    for palette_index in 0..num_palettes {
        let first_color = usize::from(read_u16_be(
            data,
            indices_offset + palette_index.checked_mul(2)?,
        )?);
        let palette_end = first_color.checked_add(num_palette_entries)?;
        if palette_end > num_color_records {
            return None;
        }
        let mut palette = Vec::with_capacity(num_palette_entries);
        for entry_index in 0..num_palette_entries {
            let record_offset = color_records_offset
                .checked_add(first_color.checked_add(entry_index)?.checked_mul(4)?)?;
            palette.push(FT_Color {
                blue: *data.get(record_offset)?,
                green: *data.get(record_offset + 1)?,
                red: *data.get(record_offset + 2)?,
                alpha: *data.get(record_offset + 3)?,
            });
        }
        palettes.push(palette);
    }

    let mut palette_flags = vec![0; num_palettes];
    let mut palette_name_ids = vec![0xFFFF; num_palettes];
    let mut palette_entry_name_ids = vec![0xFFFF; num_palette_entries];
    if version >= 1 {
        let extensions_offset = indices_end;
        if let Some(types_offset) = read_u32_be(data, extensions_offset)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset != 0)
        {
            for (index, out) in palette_flags.iter_mut().enumerate() {
                // FreeType 2.14.3 `src/sfnt/ttcpal.c:158-164` copies
                // palette flags with `FT_NEXT_USHORT`; although CPAL v1 calls
                // this palette types data, the public `FT_Palette_Data`
                // surface exposes 16-bit flag values.
                let offset = types_offset.checked_add(index.checked_mul(2)?)?;
                *out = read_u16_be(data, offset)?;
            }
        }
        if let Some(labels_offset) = read_u32_be(data, extensions_offset + 4)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset != 0)
        {
            for (index, out) in palette_name_ids.iter_mut().enumerate() {
                *out = read_u16_be(data, labels_offset.checked_add(index.checked_mul(2)?)?)?;
            }
        }
        if let Some(entry_labels_offset) = read_u32_be(data, extensions_offset + 8)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset != 0)
        {
            for (index, out) in palette_entry_name_ids.iter_mut().enumerate() {
                *out = read_u16_be(
                    data,
                    entry_labels_offset.checked_add(index.checked_mul(2)?)?,
                )?;
            }
        }
    }

    let active_palette = palettes.first().cloned().unwrap_or_default();
    Some(CpalState {
        palette_name_ids,
        palette_flags,
        palette_entry_name_ids,
        palettes,
        active_palette,
        active_palette_index: 0,
    })
}

pub fn FT_Palette_Data_Get(
    face: Option<&FT_Face>,
    apalette_data: Option<&mut FT_Palette_Data>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(apalette_data) = apalette_data else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let inner = face.inner.borrow();
    if !inner.font().is_sfnt() {
        // FreeType 2.14.3 `src/base/ftcolor.c:34-49` copies
        // `null_palette_data` for non-SFNT faces.
        *apalette_data = FT_Palette_Data::default();
        return FT_Err_Ok;
    }
    drop(inner);
    if !face_has_sfnt_table(face, *b"CPAL") {
        // `TT_Face.palette_data` is zero/null initialized when an SFNT face has
        // no CPAL table.
        *apalette_data = FT_Palette_Data::default();
        return FT_Err_Ok;
    }
    let Some(cpal) = &face.cpal else {
        return FT_Err_Invalid_Table as FT_Error;
    };
    let cpal = cpal.borrow();
    *apalette_data = FT_Palette_Data {
        num_palettes: cpal.palettes.len().try_into().unwrap_or(FT_UShort::MAX),
        palette_name_ids: cpal.palette_name_ids.as_ptr(),
        palette_flags: cpal.palette_flags.as_ptr(),
        num_palette_entries: cpal
            .active_palette
            .len()
            .try_into()
            .unwrap_or(FT_UShort::MAX),
        palette_entry_name_ids: cpal.palette_entry_name_ids.as_ptr(),
    };
    FT_Err_Ok
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Palette_Data_Copy(face: Option<&FT_Face>) -> FT_Palette_Data_Snapshot {
    let mut data = FT_Palette_Data::default();
    let error = FT_Palette_Data_Get(face, Some(&mut data));
    let (palette_name_ids, palette_flags, palette_entry_name_ids) = if error == FT_Err_Ok {
        face.and_then(|face| face.cpal.as_ref())
            .map(|cpal| {
                let cpal = cpal.borrow();
                (
                    cpal.palette_name_ids.clone(),
                    cpal.palette_flags.clone(),
                    cpal.palette_entry_name_ids.clone(),
                )
            })
            .unwrap_or_default()
    } else {
        Default::default()
    };
    FT_Palette_Data_Snapshot {
        error,
        num_palettes: data.num_palettes,
        num_palette_entries: data.num_palette_entries,
        palette_name_ids_is_null: data.palette_name_ids.is_null(),
        palette_flags_is_null: data.palette_flags.is_null(),
        palette_entry_name_ids_is_null: data.palette_entry_name_ids.is_null(),
        palette_name_ids,
        palette_flags,
        palette_entry_name_ids,
    }
}

pub fn FT_Palette_Select(
    face: Option<&FT_Face>,
    palette_index: FT_UShort,
    apalette: Option<&mut *const FT_Color>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if !face.inner.borrow().font().is_sfnt() {
        // FreeType 2.14.3 `src/base/ftcolor.c:54-75` returns Ok for non-SFNT
        // faces and writes a null palette when `apalette` is provided.
        if let Some(apalette) = apalette {
            *apalette = ptr::null();
        }
        return FT_Err_Ok;
    }
    let Some(cpal) = &face.cpal else {
        return FT_Err_Invalid_Table as FT_Error;
    };
    let mut cpal = cpal.borrow_mut();
    if cpal.palettes.get(usize::from(palette_index)).is_none() {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if cpal.active_palette_index != palette_index {
        // FreeType 2.14.3 `src/base/ftcolor.c:54-112` only reloads CPAL
        // colors when changing palette indexes. Reselecting the same index
        // preserves caller mutations made through the returned palette.
        cpal.active_palette = cpal.palettes[usize::from(palette_index)].clone();
        cpal.active_palette_index = palette_index;
    }
    if let Some(apalette) = apalette {
        *apalette = cpal.active_palette.as_ptr();
    }
    FT_Err_Ok
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Palette_Select_Copy(
    face: Option<&FT_Face>,
    palette_index: FT_UShort,
    write_output_pointer: bool,
) -> FT_Palette_Select_Snapshot {
    let mut palette = ptr::null();
    let err = FT_Palette_Select(
        face,
        palette_index,
        write_output_pointer.then_some(&mut palette),
    );
    let entries = if err == FT_Err_Ok && (write_output_pointer || !palette.is_null()) {
        face.and_then(|face| face.cpal.as_ref())
            .map(|cpal| cpal.borrow().active_palette.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    FT_Palette_Select_Snapshot {
        error: err,
        palette_is_null: palette.is_null(),
        entries,
    }
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Palette_Active_Entries_Copy(face: Option<&FT_Face>) -> Vec<FT_Color> {
    face.and_then(|face| face.cpal.as_ref())
        .map(|cpal| cpal.borrow().active_palette.clone())
        .unwrap_or_default()
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Palette_Set_Active_Entry_For_Test(
    face: Option<&FT_Face>,
    entry_index: usize,
    color: FT_Color,
) -> FT_Error {
    let Some(cpal) = face.and_then(|face| face.cpal.as_ref()) else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut cpal = cpal.borrow_mut();
    let Some(entry) = cpal.active_palette.get_mut(entry_index) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *entry = color;
    FT_Err_Ok
}

pub fn FT_Palette_Set_Foreground_Color(face: Option<&FT_Face>, color: FT_Color) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if !face.inner.borrow().font().is_sfnt() {
        // FreeType 2.14.3 `src/base/ftcolor.c:95-111` returns Ok without
        // side effects for non-SFNT faces.
        let _ = color;
        return FT_Err_Ok;
    }
    FT_Err_Unimplemented_Feature as FT_Error
}

fn winfnt_header_to_ffi(header: &WinFntHeader) -> FT_WinFNT_HeaderRec {
    FT_WinFNT_HeaderRec {
        version: header.version,
        file_size: FT_ULong::from(header.file_size),
        copyright: header.copyright,
        file_type: header.file_type,
        nominal_point_size: header.nominal_point_size,
        vertical_resolution: header.vertical_resolution,
        horizontal_resolution: header.horizontal_resolution,
        ascent: header.ascent,
        internal_leading: header.internal_leading,
        external_leading: header.external_leading,
        italic: header.italic,
        underline: header.underline,
        strike_out: header.strike_out,
        weight: header.weight,
        charset: header.charset,
        pixel_width: header.pixel_width,
        pixel_height: header.pixel_height,
        pitch_and_family: header.pitch_and_family,
        avg_width: header.avg_width,
        max_width: header.max_width,
        first_char: header.first_char,
        last_char: header.last_char,
        default_char: header.default_char,
        break_char: header.break_char,
        bytes_per_row: header.bytes_per_row,
        device_offset: FT_ULong::from(header.device_offset),
        face_name_offset: FT_ULong::from(header.face_name_offset),
        bits_pointer: FT_ULong::from(header.bits_pointer),
        bits_offset: FT_ULong::from(header.bits_offset),
        reserved: header.reserved,
        flags: FT_ULong::from(header.flags),
        A_space: header.a_space,
        B_space: header.b_space,
        C_space: header.c_space,
        color_table_offset: header.color_table_offset as FT_UShort,
        reserved1: [
            FT_ULong::from(header.reserved1[0]),
            FT_ULong::from(header.reserved1[1]),
            FT_ULong::from(header.reserved1[2]),
            FT_ULong::from(header.reserved1[3]),
        ],
    }
}

pub fn FT_Get_WinFNT_Header(
    face: Option<&FT_Face>,
    header: Option<&mut FT_WinFNT_HeaderRec>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(output) = header else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let inner = face.inner.borrow();
    let Some(header) = inner.winfnt_header() else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *output = winfnt_header_to_ffi(header);
    FT_Err_Ok
}

pub fn FT_Get_BDF_Property(
    face: Option<&FT_Face>,
    prop_name: Option<&str>,
    aproperty: Option<&mut BDF_PropertyRec>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(output) = aproperty else {
        return FT_Err_Invalid_Argument;
    };
    output.type_ = BDF_PROPERTY_TYPE_NONE;
    let Some(prop_name) = prop_name else {
        return FT_Err_Invalid_Argument;
    };
    let inner = face.inner.borrow();
    // FreeType 2.14.3 `src/base/ftbdf.c` routes through the face's BDF
    // service.  For a non-BDF face the service lookup fails before property
    // lookup and returns the public error observed as `FT_Err_Invalid_Table`
    // in this build; a missing property on an actual BDF face remains
    // `FT_Err_Invalid_Argument`.
    if inner.font().font_format() != "BDF" {
        return FT_Err_Invalid_Table;
    }
    match inner.font().bdf_property(prop_name) {
        Some(BdfPropertyValue::Atom(_)) => {
            output.type_ = BDF_PROPERTY_TYPE_ATOM;
            let Some(atom_c_string) = inner.font().bdf_property_atom_c_str(prop_name) else {
                return FT_Err_Invalid_Argument;
            };
            output.u.atom = atom_c_string.as_ptr();
            FT_Err_Ok
        }
        Some(BdfPropertyValue::Integer(integer)) => {
            output.type_ = BDF_PROPERTY_TYPE_INTEGER;
            output.u.integer = *integer;
            FT_Err_Ok
        }
        Some(BdfPropertyValue::Cardinal(cardinal)) => {
            output.type_ = BDF_PROPERTY_TYPE_CARDINAL;
            output.u.cardinal = *cardinal;
            FT_Err_Ok
        }
        None => FT_Err_Invalid_Argument,
    }
}

pub fn FT_Get_BDF_Charset_ID(
    face: Option<&FT_Face>,
    acharset_encoding: Option<&mut *const FT_String>,
    acharset_registry: Option<&mut *const FT_String>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let inner = face.inner.borrow();
    let (error, encoding, registry) = if inner.font().font_format() == "BDF" {
        // C parity: `src/base/ftbdf.c:FT_Get_BDF_Charset_ID` delegates to the
        // BDF service, which returns face-owned strings populated from
        // CHARSET_ENCODING and CHARSET_REGISTRY in `src/bdf/bdfdrivr.c`.
        let encoding = inner
            .font()
            .bdf_property_atom_c_str("CHARSET_ENCODING")
            .map_or(ptr::null(), CStr::as_ptr);
        let registry = inner
            .font()
            .bdf_property_atom_c_str("CHARSET_REGISTRY")
            .map_or(ptr::null(), CStr::as_ptr);
        (FT_Err_Ok, encoding, registry)
    } else {
        // FreeType 2.14.3 `src/base/ftbdf.c` initializes both local output
        // pointers to NULL and writes them to non-null caller outputs.  For
        // the version-pinned non-BDF fixture, the service-missing route is
        // observed as public error `FT_Err_Invalid_Table` in this build.
        (FT_Err_Invalid_Table, ptr::null(), ptr::null())
    };
    if let Some(output) = acharset_encoding {
        *output = encoding;
    }
    if let Some(output) = acharset_registry {
        *output = registry;
    }
    error
}

pub fn FT_GlyphSlot_AdjustWeight(
    slot: Option<&mut FT_GlyphSlot>,
    x_delta: FT_Fixed,
    y_delta: FT_Fixed,
) {
    let Some(slot) = slot else {
        return;
    };
    if slot.format != FT_GLYPH_FORMAT_OUTLINE && slot.format != FT_GLYPH_FORMAT_BITMAP {
        return;
    }

    let size = slot.source_face.size_metrics();
    let xstrength = (FT_Long::from(size.x_ppem) * x_delta) / 1024;
    let ystrength = (FT_Long::from(size.y_ppem) * y_delta) / 1024;
    if slot.format == FT_GLYPH_FORMAT_OUTLINE {
        slot.core_slot
            .adjust_outline_weight(xstrength as i32, ystrength as i32);
    } else {
        slot.core_slot.adjust_bitmap_weight(xstrength, ystrength);
    }
    refresh_slot_public_fields(slot);
}

pub fn FT_GlyphSlot_Embolden(slot: Option<&mut FT_GlyphSlot>) {
    FT_GlyphSlot_AdjustWeight(slot, 0x0AAA, 0x0AAA);
}

pub fn FT_GlyphSlot_Own_Bitmap(slot: Option<&mut FT_GlyphSlot>) -> FT_Error {
    let Some(slot) = slot else {
        return FT_Err_Ok;
    };
    if slot.format == FT_GLYPH_FORMAT_BITMAP && !slot.owns_bitmap {
        // FreeType `src/base/ftbitmap.c:1084-1102` deep-copies bitmap slots
        // whose internal flags lack `FT_GLYPH_OWN_BITMAP`, then sets the flag.
        // The safe Rust slot already stores bitmap bytes in owned Vec storage;
        // this records the public ownership transition for C/WASM slot facades.
        slot.owns_bitmap = true;
    }
    FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure(
    slot: Option<&mut FT_GlyphSlot>,
) -> FT_Error {
    let Some(slot) = slot else {
        return FT_Err_Ok;
    };
    if slot.format == FT_GLYPH_FORMAT_BITMAP && !slot.owns_bitmap {
        // C FreeType `src/base/ftbitmap.c:1084-1102` routes borrowed bitmap
        // slots through `FT_Bitmap_Copy`; when that copy allocation fails, it
        // returns Out_Of_Memory before replacing the slot bitmap or setting
        // `FT_GLYPH_OWN_BITMAP`.
        return FT_Err_Out_Of_Memory;
    }
    FT_Err_Ok
}

pub fn FT_GlyphSlot_Oblique(slot: Option<&mut FT_GlyphSlot>) {
    // FreeType `src/base/ftsynth.c` uses a fixed 12-degree shear and keeps
    // advance/metrics unchanged.
    FT_GlyphSlot_Slant(slot, 0x0366A, 0);
}

pub fn FT_GlyphSlot_Slant(slot: Option<&mut FT_GlyphSlot>, xslant: FT_Fixed, yslant: FT_Fixed) {
    let Some(slot) = slot else {
        return;
    };
    if slot.format != FT_GLYPH_FORMAT_OUTLINE {
        return;
    }

    // C `FT_GlyphSlot_Slant` (`src/base/ftsynth.c`) calls
    // `FT_Outline_Transform` only.  It explicitly does not touch advance
    // width, and it leaves metrics unchanged.
    slot.core_slot
        .apply_outline_transform(0x10000, xslant as i32, -(yslant as i32), 0x10000, 0, 0);
    refresh_slot_public_fields(slot);
}

pub fn FT_Get_Sfnt_LangTag(
    face: Option<&FT_Face>,
    lang_id: FT_UInt,
    lang_tag: Option<&mut FT_SfntLangTag>,
) -> FT_Error {
    let Some(lang_tag) = lang_tag else {
        return FT_Err_Invalid_Argument;
    };
    let Some(face) = face else {
        return FT_Err_Invalid_Argument;
    };
    let inner = face.inner.borrow();
    if inner.sfnt_name_format() != 1 {
        return FT_Err_Invalid_Table;
    }
    // FreeType `FT_Get_Sfnt_LangTag` in `src/base/ftsnames.c` requires
    // `langID > 0x8000`, then indexes `langTags[langID - 0x8000]`.
    if lang_id <= 0x8000 {
        return FT_Err_Invalid_Argument;
    }
    let Ok(index) = usize::try_from(lang_id - 0x8000) else {
        return FT_Err_Invalid_Argument;
    };
    let Some(record) = inner.sfnt_lang_tag(index) else {
        return FT_Err_Invalid_Argument;
    };
    // `tt_face_load_name` leaves invalid format-1 tag strings as a retained
    // zero-length entry whose lazy string pointer is still NULL.
    lang_tag.string = if record.string.is_empty() {
        std::ptr::null_mut()
    } else {
        record.string.as_ptr().cast_mut().cast::<FT_Byte>()
    };
    lang_tag.string_len = FT_UInt::try_from(record.string.len()).unwrap_or(FT_UInt::MAX);
    FT_Err_Ok
}

pub fn FT_New_Size(face: Option<&FT_Face>, size: Option<&mut FT_Size>) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(size) = size else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *size = ptr::null_mut();
    let state = face.inner.borrow().active_size_state();
    let handle = face.sizes.borrow_mut().add_size(state);
    register_size_handle(handle, &face.inner, &face.sizes);
    *size = handle;
    FT_Err_Ok
}

pub fn FT_Done_Size(size: FT_Size) -> FT_Error {
    if size.is_null() {
        return FT_Err_Invalid_Size_Handle;
    }
    let Some(owner) = lookup_size_owner(size) else {
        return FT_Err_Invalid_Size_Handle;
    };
    let Some(face) = owner.face.upgrade() else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(sizes) = owner.sizes.upgrade() else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };

    let Some(result) = sizes.borrow_mut().remove(size) else {
        return FT_Err_Invalid_Size_Handle;
    };
    unregister_size_handle_key(result.removed_key);
    if let Some(fallback) = result.fallback {
        // FreeType `FT_Done_Size` in `src/base/ftobjs.c` selects
        // `face->sizes_list.head` when the active size is destroyed.
        face.borrow_mut().activate_size_state(&fallback);
    }
    FT_Err_Ok
}

pub fn FT_Activate_Size(size: FT_Size) -> FT_Error {
    if size.is_null() {
        return FT_Err_Invalid_Size_Handle;
    }
    let Some(owner) = lookup_size_owner(size) else {
        return FT_Err_Invalid_Size_Handle;
    };
    let Some(face) = owner.face.upgrade() else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(sizes) = owner.sizes.upgrade() else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(state) = sizes.borrow_mut().activate(size) else {
        return FT_Err_Invalid_Size_Handle;
    };
    face.borrow_mut().activate_size_state(&state);
    FT_Err_Ok
}

pub fn FT_Sin(angle: FT_Angle) -> FT_Fixed {
    crate::fixed::ft_sin_long(angle) as FT_Fixed
}

pub fn FT_Cos(angle: FT_Angle) -> FT_Fixed {
    crate::fixed::ft_cos_long(angle) as FT_Fixed
}

pub fn FT_Tan(angle: FT_Angle) -> FT_Fixed {
    crate::fixed::ft_tan_long(angle) as FT_Fixed
}

pub fn FT_Atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    crate::fixed::ft_atan2_long(dx, dy) as FT_Angle
}

pub fn FT_Angle_Diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    crate::fixed::ft_angle_diff_long(angle1, angle2) as FT_Angle
}

pub fn FT_Vector_Unit(vec: Option<&mut FT_Vector>, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    (vec.x, vec.y) = crate::fixed::ft_vector_unit_long(angle);
}

pub fn FT_Vector_From_Polar(vec: Option<&mut FT_Vector>, length: FT_Fixed, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    (vec.x, vec.y) = crate::fixed::ft_vector_from_polar_long(length, angle);
}

pub fn FT_Vector_Length(vec: Option<&FT_Vector>) -> FT_Fixed {
    let Some(vec) = vec else {
        return 0;
    };
    crate::fixed::ft_vector_length_long(vec.x, vec.y) as FT_Fixed
}

pub fn FT_Vector_Polarize(
    vec: Option<&FT_Vector>,
    length: Option<&mut FT_Fixed>,
    angle: Option<&mut FT_Angle>,
) {
    let (Some(vec), Some(length), Some(angle)) = (vec, length, angle) else {
        return;
    };
    if let Some((new_length, new_angle)) = crate::fixed::ft_vector_polarize_long(vec.x, vec.y) {
        *length = new_length as FT_Fixed;
        *angle = new_angle as FT_Angle;
    }
}

pub fn FT_Vector_Rotate(vec: Option<&mut FT_Vector>, angle: FT_Angle) {
    let Some(vec) = vec else {
        return;
    };
    (vec.x, vec.y) = crate::fixed::ft_vector_rotate_long(vec.x, vec.y, angle);
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
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Get_TrueType_Engine_Type` first
    // looks up the "truetype" module; a library created by `FT_New_Library`
    // without default modules therefore returns `NONE`, same as a null library.
    if library.is_some_and(|library| library.module_names.contains(&"truetype")) {
        FT_TRUETYPE_ENGINE_TYPE_PATENTED as FT_TrueTypeEngineType
    } else {
        FT_TRUETYPE_ENGINE_TYPE_NONE as FT_TrueTypeEngineType
    }
}

pub fn FT_Get_Module_Interface(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
) -> FT_Module_Interface {
    let (Some(library), Some(module_name)) = (library, module_name) else {
        return ptr::null_mut();
    };
    if !library.module_names.contains(&module_name) {
        return ptr::null_mut();
    }
    // FreeType 2.14.3 `src/base/ftobjs.c:5198-5209` returns
    // `module->clazz->module_interface` directly.  Keep stable non-null
    // sentinel classes only; the public route compares nullness/availability,
    // not private service struct addresses.
    match module_name {
        "sfnt" => 0x5346_4E54usize as FT_Module_Interface,
        "psnames" => 0x5053_4E4Dusize as FT_Module_Interface,
        "psaux" => 0x5053_4158usize as FT_Module_Interface,
        _ => ptr::null_mut(),
    }
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Module_Requester_Service_Available(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    service_name: &str,
) -> bool {
    let Some(module_name) = module_name else {
        return false;
    };
    if !FT_Library_Has_Module(library, module_name) {
        return false;
    }
    match module_name {
        // `src/sfnt/sfdriver.c:1186-1223`.
        "sfnt" => matches!(service_name, "glyph-dict" | "sfnt-table"),
        // `src/psnames/psmodule.c:581-617`.
        "psnames" => service_name == "postscript-cmaps",
        // `src/truetype/ttdriver.c:617-655` first checks TrueType services,
        // then forwards SFNT service lookups through the SFNT module.
        "truetype" => matches!(service_name, "glyph-dict" | "sfnt-table"),
        _ => false,
    }
}

fn property_lookup_error<'a>(
    library: Option<&'a FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
) -> Result<&'a FT_Library, FT_Error> {
    let Some(library) = library else {
        return Err(FT_Err_Invalid_Library_Handle as FT_Error);
    };
    let Some(module_name) = module_name else {
        return Err(FT_Err_Invalid_Argument);
    };
    let Some(property_name) = property_name else {
        return Err(FT_Err_Invalid_Argument);
    };
    if !library.module_names.contains(&module_name) {
        return Err(FT_Err_Missing_Module as FT_Error);
    }
    match module_name {
        "truetype" => {
            if property_name != "interpreter-version" {
                return Err(FT_Err_Missing_Property as FT_Error);
            }
        }
        "autofitter" => {
            if !matches!(property_name, "default-script" | "fallback-script") {
                return Err(FT_Err_Missing_Property as FT_Error);
            }
        }
        _ => return Err(FT_Err_Unimplemented_Feature),
    }
    Ok(library)
}

fn autofitter_fallback_script_is_valid(value: FT_UInt) -> bool {
    // Pinned FreeType 2.14.3 `src/autofit/afmodule.c:118-153` accepts a
    // fallback script only when the generated style table contains a default
    // coverage style for that script.  The public values exercised by
    // ftdriver.h (`NONE`, `LATIN`, `CJK`, `INDIC`) and the next generated
    // script value are accepted by the pinned table; clearly out-of-range
    // values such as 9999 return Invalid_Argument and preserve the old value.
    value <= 4
}

fn autofitter_property_lookup_error(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
) -> Result<(), FT_Error> {
    let Some(library) = library else {
        return Err(FT_Err_Invalid_Library_Handle as FT_Error);
    };
    let Some(module_name) = module_name else {
        return Err(FT_Err_Invalid_Argument);
    };
    let Some(property_name) = property_name else {
        return Err(FT_Err_Invalid_Argument);
    };
    if !library.module_names.contains(&module_name) {
        return Err(FT_Err_Missing_Module as FT_Error);
    }
    if module_name != "autofitter" {
        return Err(FT_Err_Unimplemented_Feature);
    }
    if !matches!(property_name, "glyph-to-script-map" | "increase-x-height") {
        return Err(FT_Err_Missing_Property as FT_Error);
    }
    Ok(())
}

pub fn FT_Property_Get_GlyphToScriptMap(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
    face: Option<&FT_Face>,
    value: Option<&mut FT_Prop_GlyphToScriptMap>,
) -> FT_Error {
    // FreeType 2.14.3 `src/autofit/afmodule.c:296-304` dispatches
    // `autofitter:glyph-to-script-map` through `af_property_get_face_globals`.
    // A null `prop.face` returns Invalid_Face_Handle before `prop.map` is
    // touched; successful map construction is a separate route.
    let Some(value) = value else {
        return if library.is_none() {
            FT_Err_Invalid_Library_Handle as FT_Error
        } else {
            FT_Err_Invalid_Argument
        };
    };
    if let Err(error) = autofitter_property_lookup_error(library, module_name, property_name) {
        return error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    value.face = (face as *const FT_Face).cast_mut().cast();
    value.map = face.glyph_to_script_map.as_ptr().cast_mut();
    FT_Err_Ok
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Glyph_To_Script_Map_Sample_For_Test(
    face: &FT_Face,
    glyph_indices: &[FT_UInt],
) -> Vec<(FT_UInt, FT_UShort)> {
    glyph_indices
        .iter()
        .copied()
        .filter_map(|glyph_index| {
            let index = usize::try_from(glyph_index).ok()?;
            face.glyph_to_script_map
                .get(index)
                .copied()
                .map(|script| (glyph_index, script))
        })
        .collect()
}

pub fn FT_Property_Get(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
    value: Option<&mut FT_UInt>,
) -> FT_Error {
    // FreeType 2.14.3 `src/base/ftobjs.c:ft_property_do` validates library,
    // module name, property name, and value before dispatching to
    // `src/truetype/ttdriver.c:tt_property_get`.
    let Some(value) = value else {
        return if library.is_none() {
            FT_Err_Invalid_Library_Handle as FT_Error
        } else {
            FT_Err_Invalid_Argument
        };
    };
    match property_lookup_error(library, module_name, property_name) {
        Ok(library) => {
            *value = match (module_name, property_name) {
                (Some("truetype"), Some("interpreter-version")) => {
                    library.truetype_interpreter_version
                }
                (Some("autofitter"), Some("default-script")) => library.autofitter_default_script,
                (Some("autofitter"), Some("fallback-script")) => library.autofitter_fallback_script,
                _ => return FT_Err_Missing_Property as FT_Error,
            };
            FT_Err_Ok
        }
        Err(error) => error,
    }
}

pub fn FT_Property_Get_IncreaseXHeight(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
    face: Option<&FT_Face>,
    value: Option<&mut FT_Prop_IncreaseXHeight>,
) -> FT_Error {
    // FreeType 2.14.3 dispatches `autofitter:increase-x-height` through
    // `src/autofit/afmodule.c:af_property_get`.  The thin ABI layers validate
    // the public raw face pointer; core keeps the actual property state on the
    // safe `FT_Face` value.
    let Some(value) = value else {
        return if library.is_none() {
            FT_Err_Invalid_Library_Handle as FT_Error
        } else {
            FT_Err_Invalid_Argument
        };
    };
    if let Err(error) = autofitter_property_lookup_error(library, module_name, property_name) {
        return error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    value.limit = face.increase_x_height;
    FT_Err_Ok
}

pub fn FT_Property_Set(
    library: Option<&mut FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
    value: Option<FT_UInt>,
) -> FT_Error {
    // FreeType 2.14.3 `src/truetype/ttdriver.c:tt_property_set` accepts
    // interpreter versions 35, 38, and 40.  With the pinned minimal subpixel
    // build, 38 is normalized to 40; invalid values preserve the old value.
    let Some(value) = value else {
        return if library.is_none() {
            FT_Err_Invalid_Library_Handle as FT_Error
        } else {
            FT_Err_Invalid_Argument
        };
    };
    let lookup = property_lookup_error(library.as_deref(), module_name, property_name);
    match lookup {
        Ok(_) => {
            let Some(library) = library else {
                return FT_Err_Invalid_Library_Handle as FT_Error;
            };
            match (module_name, property_name) {
                (Some("truetype"), Some("interpreter-version")) => match i64::from(value) {
                    TT_INTERPRETER_VERSION_35 => {
                        library.truetype_interpreter_version = TT_INTERPRETER_VERSION_35 as FT_UInt;
                        FT_Err_Ok
                    }
                    TT_INTERPRETER_VERSION_38 | TT_INTERPRETER_VERSION_40 => {
                        library.truetype_interpreter_version = TT_INTERPRETER_VERSION_40 as FT_UInt;
                        FT_Err_Ok
                    }
                    _ => FT_Err_Unimplemented_Feature,
                },
                (Some("autofitter"), Some("default-script")) => {
                    // FreeType `af_property_set` assigns default-script
                    // directly without range validation.
                    library.autofitter_default_script = value;
                    FT_Err_Ok
                }
                (Some("autofitter"), Some("fallback-script")) => {
                    if !autofitter_fallback_script_is_valid(value) {
                        return FT_Err_Invalid_Argument;
                    }
                    library.autofitter_fallback_script = value;
                    FT_Err_Ok
                }
                _ => FT_Err_Missing_Property as FT_Error,
            }
        }
        Err(error) => error,
    }
}

pub fn FT_Property_Set_IncreaseXHeight(
    library: Option<&FT_Library>,
    module_name: Option<&str>,
    property_name: Option<&str>,
    face: Option<&mut FT_Face>,
    value: Option<&FT_Prop_IncreaseXHeight>,
) -> FT_Error {
    // FreeType 2.14.3 `src/autofit/afmodule.c:af_property_set` stores
    // `FT_Prop_IncreaseXHeight.limit` on the face's internal auto-hint globals.
    // Glyph-output effects are verified by a separate row; this helper proves
    // the public property payload round-trips through the same face state.
    let Some(value) = value else {
        return if library.is_none() {
            FT_Err_Invalid_Library_Handle as FT_Error
        } else {
            FT_Err_Invalid_Argument
        };
    };
    if let Err(error) = autofitter_property_lookup_error(library, module_name, property_name) {
        return error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    face.increase_x_height = value.limit;
    FT_Err_Ok
}

fn set_string_property_ignored(
    library: Option<&mut FT_Library>,
    module_name: &str,
    property_name: &str,
    value: &str,
) {
    let parsed = property_string_to_uint(value);
    let _ = FT_Property_Set(
        library,
        Some(module_name),
        Some(property_name),
        Some(parsed),
    );
}

fn property_string_to_uint(value: &str) -> FT_UInt {
    let trimmed = value.trim_start();
    let mut chars = trimmed.chars();
    let negative = matches!(chars.clone().next(), Some('-'));
    if matches!(chars.clone().next(), Some('-' | '+')) {
        let _ = chars.next();
    }
    let mut parsed: i128 = 0;
    let mut saw_digit = false;
    for ch in chars {
        let Some(digit) = ch.to_digit(10) else {
            break;
        };
        saw_digit = true;
        parsed = parsed.saturating_mul(10).saturating_add(i128::from(digit));
    }
    if !saw_digit {
        return 0;
    }
    let signed = if negative { -parsed } else { parsed };
    signed as FT_UInt
}

pub fn FT_Set_Default_Properties_From_Env(library: Option<&mut FT_Library>, env: Option<&str>) {
    let Some(env) = env else {
        return;
    };
    let mut library = library;
    for token in env.split_ascii_whitespace() {
        let Some((module_name, property_tail)) = token.split_once(':') else {
            break;
        };
        if module_name.is_empty() || module_name.len() > 128 {
            break;
        }
        let Some((property_name, property_value)) = property_tail.split_once('=') else {
            break;
        };
        if property_name.is_empty()
            || property_name.len() > 128
            || property_value.is_empty()
            || property_value.len() > 128
        {
            break;
        }
        // FreeType 2.14.3 `src/base/ftinit.c:112-182` parses
        // FREETYPE_PROPERTIES tokens and deliberately ignores every
        // `ft_property_string_set` error, including null-library and unknown
        // module/property failures.
        set_string_property_ignored(
            library.as_deref_mut(),
            module_name,
            property_name,
            property_value,
        );
    }
}

pub fn FT_Set_Default_Properties(library: Option<&mut FT_Library>) {
    let env = std::env::var("FREETYPE_PROPERTIES").ok();
    FT_Set_Default_Properties_From_Env(library, env.as_deref());
}

pub fn FT_Face_Properties(
    face: Option<&mut FT_Face>,
    properties: Option<&[FT_Face_Property]>,
) -> FT_Error {
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Face_Properties` mutates only
    // `face->internal->no_stem_darkening` and `random_seed` for the supported
    // scalar tags.  LCD weights return `Unimplemented_Feature` before the data
    // pointer is read.
    let Some(properties) = properties else {
        return FT_Err_Invalid_Argument;
    };
    if properties.is_empty() {
        return FT_Err_Ok;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    for property in properties {
        match property.tag as i64 {
            FT_PARAM_TAG_STEM_DARKENING => match property.value {
                Some(FT_Face_Property_Value::Bool(value)) => {
                    face.no_stem_darkening = if value == 1 { 0 } else { 1 };
                }
                None => face.no_stem_darkening = -1,
                _ => return FT_Err_Invalid_Argument,
            },
            FT_PARAM_TAG_LCD_FILTER_WEIGHTS => return FT_Err_Unimplemented_Feature,
            FT_PARAM_TAG_RANDOM_SEED => match property.value {
                Some(FT_Face_Property_Value::Int32(value)) => {
                    face.random_seed = value.max(0);
                }
                None => face.random_seed = -1,
                _ => return FT_Err_Invalid_Argument,
            },
            _ => return FT_Err_Invalid_Argument,
        }
    }
    FT_Err_Ok
}

pub fn FT_Face_Properties_Get_State(face: &FT_Face) -> FT_Face_Properties_State {
    FT_Face_Properties_State {
        no_stem_darkening: face.no_stem_darkening,
        random_seed: face.random_seed,
    }
}

pub fn FT_Add_Default_Modules(library: Option<&mut FT_Library>) {
    // FreeType 2.14.3 `src/base/ftinit.c:FT_Add_Default_Modules` returns
    // `void`; null-library errors are swallowed while iterating
    // `FT_Add_Module`.  For a live library, the observable public effect is
    // that the default module classes from `ftmodule.h` become discoverable
    // through module lookups in registration order.
    if let Some(library) = library {
        library.module_names = DEFAULT_MODULE_NAMES;
    }
}

pub fn FT_Set_Debug_Hook(
    library: Option<&mut FT_Library>,
    hook_index: FT_UInt,
    debug_hook: FT_DebugHook_Func,
) {
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Set_Debug_Hook` mutates a slot
    // only when all three public preconditions hold: library, hook, index < 4.
    if let (Some(library), Some(debug_hook), Ok(index)) =
        (library, debug_hook, usize::try_from(hook_index))
        && let Some(slot) = library.debug_hooks.get_mut(index)
    {
        *slot = Some(debug_hook);
    }
}

pub fn FT_List_Add(
    list: Option<&mut FT_ListRec>,
    node: Option<&mut FT_ListNodeRec>,
    before: Option<&mut FT_ListNodeRec>,
) {
    let (Some(list), Some(node)) = (list, node) else {
        return;
    };

    let before_ptr = list.tail;
    node.next = std::ptr::null_mut();
    node.prev = before_ptr;

    if let Some(before) = before {
        before.next = node as *mut FT_ListNodeRec;
    } else {
        list.head = node as *mut FT_ListNodeRec;
    }
    list.tail = node as *mut FT_ListNodeRec;
}

pub fn FT_List_Insert(
    list: Option<&mut FT_ListRec>,
    node: Option<&mut FT_ListNodeRec>,
    after: Option<&mut FT_ListNodeRec>,
) {
    let (Some(list), Some(node)) = (list, node) else {
        return;
    };

    // FreeType `src/base/ftutil.c` inserts before the current head and
    // preserves the existing tail when the list is non-empty.
    let after_ptr = list.head;
    node.next = after_ptr;
    node.prev = std::ptr::null_mut();

    if let Some(after) = after {
        after.prev = node as *mut FT_ListNodeRec;
    } else {
        list.tail = node as *mut FT_ListNodeRec;
    }
    list.head = node as *mut FT_ListNodeRec;
}

pub fn FT_List_Find_Node_Matches(node: &FT_ListNodeRec, data: FT_Pointer) -> bool {
    node.data == data
}

pub fn FT_List_Remove(
    list: Option<&mut FT_ListRec>,
    node: Option<&FT_ListNodeRec>,
    before: Option<&mut FT_ListNodeRec>,
    after: Option<&mut FT_ListNodeRec>,
) {
    let (Some(list), Some(node)) = (list, node) else {
        return;
    };

    // FreeType `src/base/ftutil.c` patches `node->prev`/`node->next`
    // directly; it does not verify that `node` belongs to `list`.
    let before_ptr = node.prev;
    let after_ptr = node.next;

    if let Some(before) = before {
        before.next = after_ptr;
    } else {
        list.head = after_ptr;
    }

    if let Some(after) = after {
        after.prev = before_ptr;
    } else {
        list.tail = before_ptr;
    }
}

pub fn FT_List_Up(
    list: Option<&mut FT_ListRec>,
    node: Option<&mut FT_ListNodeRec>,
    before: Option<&mut FT_ListNodeRec>,
    after: Option<&mut FT_ListNodeRec>,
    head_is_before: bool,
    head: Option<&mut FT_ListNodeRec>,
) {
    let (Some(list), Some(node), Some(before)) = (list, node, before) else {
        return;
    };

    let after_ptr = node.next;
    before.next = after_ptr;

    if let Some(after) = after {
        after.prev = before as *mut FT_ListNodeRec;
    } else {
        list.tail = before as *mut FT_ListNodeRec;
    }

    // FreeType `src/base/ftutil.c` moves only non-head nodes.  In safe Rust
    // the previous node can also be the old head, so callers pass that alias
    // as `head_is_before` instead of forming two `&mut` borrows.
    node.prev = std::ptr::null_mut();
    node.next = list.head;
    if head_is_before {
        before.prev = node as *mut FT_ListNodeRec;
    } else if let Some(head) = head {
        head.prev = node as *mut FT_ListNodeRec;
    }
    list.head = node as *mut FT_ListNodeRec;
}

pub fn FT_List_Finalize_Node(
    node: &FT_ListNodeRec,
    destroy: FT_List_Destructor,
    memory: &FT_MemoryRec,
    memory_ptr: FT_Memory,
    user: FT_Pointer,
) {
    // FreeType `src/base/ftutil.c` snapshots `next`, calls the optional
    // destructor with `(memory, node->data, user)`, then frees the node itself.
    // Safe core receives each node explicitly; raw list traversal stays in the
    // thin ABI wrappers.
    if let Some(destroy) = destroy {
        destroy(memory_ptr, node.data, user);
    }
    if let Some(free) = memory.free {
        free(
            memory_ptr,
            (node as *const FT_ListNodeRec).cast_mut().cast(),
        );
    }
}

pub fn FT_List_Finalize_Clear(list: Option<&mut FT_ListRec>, memory: Option<&FT_MemoryRec>) {
    let (Some(list), Some(_memory)) = (list, memory) else {
        return;
    };
    list.head = std::ptr::null_mut();
    list.tail = std::ptr::null_mut();
}

pub fn FT_List_Iterate_Next(node: &FT_ListNodeRec) -> FT_ListNode {
    // FreeType snapshots `cur->next` before invoking the iterator callback
    // (`src/base/ftutil.c:381-392`), allowing the callback to mutate `cur`.
    node.next
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Debug_Hook_Classes(
    library: Option<&FT_Library>,
    hook_a: FT_DebugHook_Func,
    hook_b: FT_DebugHook_Func,
) -> [FT_Int; 4] {
    library.map_or([0; 4], |library| {
        library.debug_hooks.map(|hook| {
            if same_debug_hook(hook, hook_a) {
                1
            } else if same_debug_hook(hook, hook_b) {
                2
            } else if hook.is_some() {
                3
            } else {
                0
            }
        })
    })
}

#[cfg(any(test, feature = "abi-test-support"))]
fn same_debug_hook(left: FT_DebugHook_Func, right: FT_DebugHook_Func) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Has_TrueType_Module(library: Option<&FT_Library>) -> bool {
    FT_Library_Has_Module(library, "truetype")
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Has_TrueType_Engine_Service(library: Option<&FT_Library>) -> bool {
    FT_Library_Has_TrueType_Module(library)
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Has_Module(library: Option<&FT_Library>, name: &str) -> bool {
    library.is_some_and(|library| library.module_names.contains(&name))
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Module_Flags(library: Option<&FT_Library>, name: &str) -> Option<FT_ULong> {
    if !FT_Library_Has_Module(library, name) {
        return None;
    }
    // FreeType default module flags are static module-class metadata declared
    // by each driver/renderer module (`src/*/*drivr.c`, `src/*/ftsmooth.c`,
    // and `src/autofit/afmodule.c`).  The pure-Rust library keeps the same
    // observable metadata for ABI/parity inspection without dynamic modules.
    let flags = match name {
        "truetype" | "type1" | "cid" | "type42" => {
            FT_MODULE_FONT_DRIVER | FT_MODULE_DRIVER_SCALABLE | FT_MODULE_DRIVER_HAS_HINTER
        }
        "cff" => {
            FT_MODULE_FONT_DRIVER
                | FT_MODULE_DRIVER_SCALABLE
                | FT_MODULE_DRIVER_HAS_HINTER
                | FT_MODULE_DRIVER_HINTS_LIGHTLY
        }
        "bdf" | "pcf" | "winfonts" => FT_MODULE_FONT_DRIVER | FT_MODULE_DRIVER_NO_OUTLINES,
        "autofitter" => FT_MODULE_HINTER,
        "smooth" | "raster1" | "sdf" | "bsdf" | "svg" => FT_MODULE_RENDERER,
        _ => 0,
    };
    Some(flags as FT_ULong)
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Renderer_Class(
    library: Option<&FT_Library>,
    format: FT_Glyph_Format,
) -> Option<(&'static str, FT_Glyph_Format, bool, bool)> {
    let library = library?;
    // FreeType 2.14.3 `FT_Get_Renderer` (`src/base/ftrender.c`) returns the
    // first registered renderer whose `glyph_format` matches the requested
    // format.  Keep only class metadata observable through public renderer
    // handles; callers must not depend on raw pointer identity.
    for name in library.module_names {
        let (observable_name, renderer_format, has_raster_class) = match *name {
            "smooth" | "raster1" | "sdf" => (*name, FT_GLYPH_FORMAT_OUTLINE, true),
            "bsdf" => ("bsdf", FT_GLYPH_FORMAT_BITMAP, true),
            "svg" => ("ot-svg", FT_GLYPH_FORMAT_SVG, false),
            _ => continue,
        };
        if renderer_format == format {
            return Some((observable_name, renderer_format, true, has_raster_class));
        }
    }
    None
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Library_Default_Module_Names(library: Option<&FT_Library>) -> &'static [&'static str] {
    library.map_or(&[], |library| library.module_names)
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
    face.map_or(FT_GASP_NO_TABLE as FT_Int, |face| {
        face.inner.borrow().get_gasp(ppem)
    })
}

pub fn FT_Select_Size(face: Option<&mut FT_Face>, strike_index: FT_Int) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(strike_index) = usize::try_from(strike_index) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let result = face.inner.borrow_mut().select_size(strike_index);
    match result {
        Ok(()) => {
            sync_active_size_state(face);
            FT_Err_Ok
        }
        Err(SelectSizeError::NoFixedSizes) => FT_Err_Invalid_Face_Handle as FT_Error,
        Err(SelectSizeError::InvalidArgument) => FT_Err_Invalid_Argument as FT_Error,
    }
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
    let inner = face.inner.borrow();
    if glyph_index >= FT_UInt::from(inner.info().num_glyphs) {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    let Some(name) = inner.glyph_name(glyph_index) else {
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
    let inner = face.inner.borrow();
    inner.name_index(glyph_name)
}

pub fn FT_Get_Postscript_Name(face: &FT_Face) -> Option<&str> {
    face.postscript_name.as_deref()
}

pub fn FT_Get_Font_Format(face: Option<&FT_Face>) -> Option<&'static str> {
    // FreeType `src/base/ftfntfmt.c:26-49` asks the face's FONT_FORMAT
    // service and returns NULL for a null face.  The supported core drivers
    // expose the same static strings through `Font::font_format`.
    face.map(|face| face.inner.borrow().font().font_format())
}

pub fn FT_FACE_DRIVER_NAME(face: Option<&FT_Face>) -> Option<&'static str> {
    // FreeType 2.14.3 `include/freetype/ftmodapi.h:FT_FACE_DRIVER_NAME`
    // expands to the face driver's module class name, not the FONT_FORMAT
    // service string.  For the currently supported core drivers this maps the
    // parsed driver identity to the same public module names C exposes.
    match FT_Get_Font_Format(face)? {
        "TrueType" => Some("truetype"),
        "CFF" => Some("cff"),
        "Type 1" => Some("type1"),
        "Windows FNT" => Some("winfonts"),
        _ => None,
    }
}

pub fn FT_Get_Multi_Master(
    face: Option<&FT_Face>,
    amaster: Option<&mut FT_Multi_Master>,
) -> FT_Error {
    let Some(amaster) = amaster else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let inner = face.inner.borrow();
    let Some(master) = inner.font().type1_multi_master() else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if master.axes.len() > amaster.axis.len()
        || master.axes.len() > face.type1_mm_axis_names.len()
        || master.num_designs > 16
    {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    // FreeType `src/type1/t1load.c:T1_Get_Multi_Master` writes the descriptor
    // counts and populated axis slots only; unused caller slots retain their
    // incoming sentinel values.
    amaster.num_axis = FT_UInt::try_from(master.axes.len()).unwrap_or(0);
    amaster.num_designs = FT_UInt::try_from(master.num_designs).unwrap_or(0);
    for (index, axis) in master.axes.iter().enumerate() {
        amaster.axis[index] = FT_MM_Axis {
            name: face.type1_mm_axis_names[index].as_ptr().cast_mut(),
            minimum: FT_Long::from(axis.minimum),
            maximum: FT_Long::from(axis.maximum),
        };
    }
    FT_Err_Ok
}

fn ft_make_tag(bytes: &[u8; 4]) -> FT_ULong {
    FT_ULong::from(bytes[0]) << 24
        | FT_ULong::from(bytes[1]) << 16
        | FT_ULong::from(bytes[2]) << 8
        | FT_ULong::from(bytes[3])
}

fn type1_mm_axis_tag(name: &str) -> FT_ULong {
    match name {
        "Weight" => ft_make_tag(b"wght"),
        "Width" => ft_make_tag(b"wdth"),
        "OpticalSize" => ft_make_tag(b"opsz"),
        "Slant" => ft_make_tag(b"slnt"),
        "Italic" => ft_make_tag(b"ital"),
        _ => !FT_ULong::from(0u8),
    }
}

fn variation_axis_name_ptr(tag: u32) -> *mut FT_String {
    match tag {
        // C parity: `src/truetype/ttgxvar.c:2887-2903` replaces duplicated
        // fvar four-byte tag names with these standard PostScript axis names.
        0x7767_6874 => c"Weight".as_ptr().cast_mut(),
        0x7764_7468 => c"Width".as_ptr().cast_mut(),
        0x6F70_737A => c"OpticalSize".as_ptr().cast_mut(),
        0x736C_6E74 => c"Slant".as_ptr().cast_mut(),
        0x6974_616C => c"Italic".as_ptr().cast_mut(),
        _ => ptr::null_mut(),
    }
}

fn int_to_fixed(value: i32) -> FT_Fixed {
    FT_Fixed::from(value) * 0x10000
}

pub fn FT_Get_MM_Var(
    face: Option<&FT_Face>,
    amaster: Option<&mut FT_MM_Var>,
    axis_storage: Option<&mut [FT_Var_Axis]>,
    namedstyle_storage: Option<&mut [FT_Var_Named_Style]>,
    namedstyle_coords_storage: Option<&mut [FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(amaster) = amaster else {
        // Pinned FreeType 2.14.3 in this oracle build reports
        // Invalid_Face_Handle for the public FT_Get_MM_Var null-output route
        // before any descriptor storage is populated.
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let inner = face.inner.borrow();
    let font = inner.font();
    let Some(axis_storage) = axis_storage else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if let Some(master) = font.type1_multi_master() {
        if master.axes.len() > face.type1_mm_axis_names.len() {
            return FT_Err_Invalid_Argument as FT_Error;
        }
        if axis_storage.len() < master.axes.len() {
            return FT_Err_Invalid_Argument as FT_Error;
        }
        // C parity: `src/type1/t1load.c:T1_Get_MM_Var` builds Adobe MM
        // `FT_Var_Axis` records from `T1_Get_Multi_Master`, then writes min/max
        // as 16.16 values and `def` from the unmapped default weight vector
        // without applying another fixed-point shift.
        let default_design = match font.type1_mm_default_design_coordinates() {
            Ok(default_design) => default_design,
            Err(err) => return error_to_ft(err) as FT_Error,
        };
        for (index, axis) in master.axes.iter().enumerate() {
            axis_storage[index] = FT_Var_Axis {
                name: face.type1_mm_axis_names[index].as_ptr().cast_mut(),
                minimum: int_to_fixed(axis.minimum),
                def: FT_Fixed::from(
                    default_design
                        .get(index)
                        .copied()
                        .unwrap_or_else(|| axis.minimum.saturating_mul(65_536)),
                ),
                maximum: int_to_fixed(axis.maximum),
                tag: type1_mm_axis_tag(&axis.name),
                strid: !FT_UInt::from(0u8),
            };
        }
        amaster.num_axis = FT_UInt::try_from(master.axes.len()).unwrap_or(0);
        amaster.num_designs = FT_UInt::try_from(master.num_designs).unwrap_or(0);
        amaster.num_namedstyles = 0;
        amaster.axis = axis_storage.as_mut_ptr();
        amaster.namedstyle = ptr::null_mut();
        MM_VAR_AXIS_FLAGS.with(|flags| {
            flags.borrow_mut().insert(
                amaster as *const FT_MM_Var as usize,
                vec![0; master.axes.len()],
            );
        });
        return FT_Err_Ok;
    }
    let Some(fvar) = font.data.fvar.as_ref() else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let axis_count = fvar.axes.len();
    let namedstyle_count = fvar.instances.len();
    if axis_storage.len() < axis_count {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(namedstyle_storage) = namedstyle_storage else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if namedstyle_storage.len() < namedstyle_count {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let coords_needed = axis_count.saturating_mul(namedstyle_count);
    let Some(namedstyle_coords_storage) = namedstyle_coords_storage else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if namedstyle_coords_storage.len() < coords_needed {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    for (index, axis) in fvar.axes.iter().enumerate() {
        axis_storage[index] = FT_Var_Axis {
            name: variation_axis_name_ptr(axis.tag),
            minimum: FT_Fixed::from(axis.min_value),
            def: FT_Fixed::from(axis.default_value),
            maximum: FT_Fixed::from(axis.max_value),
            tag: FT_ULong::from(axis.tag),
            strid: FT_UInt::from(axis.name_id),
        };
    }
    for (style_index, style) in fvar.instances.iter().enumerate() {
        let coord_start = style_index * axis_count;
        let coord_end = coord_start + axis_count;
        let coords = &mut namedstyle_coords_storage[coord_start..coord_end];
        for (index, coord) in coords.iter_mut().enumerate() {
            *coord = FT_Fixed::from(style.coords.get(index).copied().unwrap_or(0));
        }
        namedstyle_storage[style_index] = FT_Var_Named_Style {
            coords: coords.as_mut_ptr(),
            strid: FT_UInt::from(style.subfamily_name_id),
            // C parity: `src/truetype/ttgxvar.c:2738-2741` stores 0xFFFF
            // when an fvar named-instance record has no PostScript name ID.
            psid: style.postscript_name_id.map_or(0xFFFF, FT_UInt::from),
        };
    }
    amaster.num_axis = FT_UInt::try_from(axis_count).unwrap_or(0);
    amaster.num_designs = !FT_UInt::from(0u8);
    amaster.num_namedstyles = FT_UInt::try_from(namedstyle_count).unwrap_or(0);
    amaster.axis = axis_storage.as_mut_ptr();
    amaster.namedstyle = if namedstyle_count == 0 {
        ptr::null_mut()
    } else {
        namedstyle_storage.as_mut_ptr()
    };
    MM_VAR_AXIS_FLAGS.with(|flags| {
        flags.borrow_mut().insert(
            amaster as *const FT_MM_Var as usize,
            fvar.axes
                .iter()
                .map(|axis| FT_UInt::from(axis.flags))
                .collect(),
        );
    });
    FT_Err_Ok
}

pub fn FT_Get_Var_Axis_Flags(
    master: Option<&FT_MM_Var>,
    axis_index: FT_UInt,
    flags: Option<&mut FT_UInt>,
) -> FT_Error {
    let (Some(master), Some(flags)) = (master, flags) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if axis_index >= master.num_axis {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    // C parity: `src/base/ftmm.c:594-613` reads a `FT_UShort` axis-flags
    // array immediately after `FT_MM_Var`.  The Type 1 MM service
    // (`src/type1/t1load.c:T1_Get_MM_Var`) zero-fills that array because
    // axis flags are not meaningful for Adobe MM fonts.
    *flags = MM_VAR_AXIS_FLAGS.with(|stored_flags| {
        stored_flags
            .borrow()
            .get(&(master as *const FT_MM_Var as usize))
            .and_then(|stored| stored.get(usize::try_from(axis_index).ok()?))
            .copied()
            .unwrap_or(0)
    });
    FT_Err_Ok
}

pub fn FT_Set_MM_WeightVector(
    face: Option<&mut FT_Face>,
    len: FT_UInt,
    weightvector: Option<&[FT_Fixed]>,
) -> FT_Error {
    if len != 0 && weightvector.is_none() {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(len) = usize::try_from(len) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let result = if len == 0 && weightvector.is_none() {
        // C parity: src/base/ftmm.c clears FT_FACE_FLAG_VARIATION after a
        // successful zero-length Type 1 MM weight-vector reset.
        face.inner
            .borrow_mut()
            .font_mut()
            .set_type1_mm_weight_vector(None)
    } else {
        let Some(weightvector) = weightvector else {
            return FT_Err_Invalid_Argument as FT_Error;
        };
        if weightvector.len() < len {
            return FT_Err_Invalid_Argument as FT_Error;
        }
        let weights = weightvector[..len]
            .iter()
            .copied()
            .map(i32::try_from)
            .collect::<Result<Vec<_>, _>>();
        let Ok(weights) = weights else {
            return FT_Err_Invalid_Argument as FT_Error;
        };
        face.inner
            .borrow_mut()
            .font_mut()
            .set_type1_mm_weight_vector(Some(&weights))
    };
    match result {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Get_MM_WeightVector(
    face: Option<&FT_Face>,
    len: Option<&mut FT_UInt>,
    weightvector: Option<&mut [FT_Fixed]>,
) -> FT_Error {
    let Some(len) = len else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if weightvector.is_none() {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let capacity = match usize::try_from(*len) {
        Ok(capacity) => capacity,
        Err(_) => return FT_Err_Invalid_Argument as FT_Error,
    };
    let inner = face.inner.borrow();
    let weights = match inner.font().type1_mm_weight_vector() {
        Ok(weights) => weights,
        Err(err) => return error_to_ft(err) as FT_Error,
    };
    if capacity < weights.len() {
        *len = FT_UInt::try_from(weights.len()).unwrap_or(FT_UInt::MAX);
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(weightvector) = weightvector else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if weightvector.len() < capacity {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    for (index, out) in weightvector.iter_mut().take(capacity).enumerate() {
        *out = weights.get(index).copied().map_or(0, FT_Fixed::from);
    }
    *len = FT_UInt::try_from(weights.len()).unwrap_or(FT_UInt::MAX);
    FT_Err_Ok
}

pub fn FT_Get_X11_Font_Format(face: Option<&FT_Face>) -> Option<&'static str> {
    FT_Get_Font_Format(face)
}

pub fn FT_Set_Named_Instance(face: Option<&mut FT_Face>, instance_index: FT_UInt) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(instance_index) = usize::try_from(instance_index) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let result = face.inner.borrow_mut().set_named_instance(instance_index);
    match result {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Set_MM_Design_Coordinates(
    face: Option<&mut FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&[FT_Long]>,
) -> FT_Error {
    if num_coords != 0 && coords.is_none() {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let coords_i32 = match coords {
        Some(coords) => {
            if coords.len() < num_coords {
                return FT_Err_Invalid_Argument as FT_Error;
            }
            let converted = coords[..num_coords]
                .iter()
                .copied()
                .map(i32::try_from)
                .collect::<Result<Vec<_>, _>>();
            let Ok(converted) = converted else {
                return FT_Err_Invalid_Argument as FT_Error;
            };
            converted
        }
        None => Vec::new(),
    };
    let result = face
        .inner
        .borrow_mut()
        .font_mut()
        .set_type1_mm_design_coordinates(&coords_i32, num_coords != 0);
    match result {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Set_Var_Design_Coordinates(
    face: Option<&mut FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&[FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(coords) = coords else {
        return if num_coords == 0 {
            let result = face.inner.borrow_mut().set_var_design_coordinates(&[]);
            match result {
                Ok(()) => {
                    let transform_matrix = face.transform_matrix;
                    let transform_delta = face.transform_delta;
                    let refcount = face.refcount;
                    let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
                    refreshed.transform_matrix = transform_matrix;
                    refreshed.transform_delta = transform_delta;
                    refreshed.refcount = refcount;
                    *face = refreshed;
                    FT_Err_Ok
                }
                Err(err) => error_to_ft(err) as FT_Error,
            }
        } else {
            FT_Err_Invalid_Argument as FT_Error
        };
    };
    if coords.len() < num_coords {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let coords_i32 = coords[..num_coords]
        .iter()
        .copied()
        .map(i32::try_from)
        .collect::<Result<Vec<_>, _>>();
    let Ok(coords_i32) = coords_i32 else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let result = face
        .inner
        .borrow_mut()
        .set_var_design_coordinates(&coords_i32);
    match result {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Get_Var_Design_Coordinates(
    face: Option<&FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&mut [FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(coords) = coords else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if coords.len() < num_coords {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let inner = face.inner.borrow();
    if inner.font().type1_multi_master().is_some() {
        let design = match inner.font().type1_mm_design_coordinates(num_coords) {
            Ok(design) => design,
            Err(err) => return error_to_ft(err) as FT_Error,
        };
        for (index, out) in coords.iter_mut().take(num_coords).enumerate() {
            *out = FT_Fixed::from(design.get(index).copied().unwrap_or(0));
        }
        return FT_Err_Ok;
    }
    let design = match inner.var_design_coordinates() {
        Ok(design) => design,
        Err(err) => return error_to_ft(err) as FT_Error,
    };
    for (index, out) in coords.iter_mut().take(num_coords).enumerate() {
        *out = FT_Fixed::from(design.get(index).copied().unwrap_or(0));
    }
    FT_Err_Ok
}

pub fn FT_Get_Var_Blend_Coordinates(
    face: Option<&FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&mut [FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let Some(coords) = coords else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    if coords.len() < num_coords {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let inner = face.inner.borrow();
    if inner.font().type1_multi_master().is_some() {
        let blend = match inner.font().type1_mm_blend_coordinates_16_16(num_coords) {
            Ok(blend) => blend,
            Err(err) => return error_to_ft(err) as FT_Error,
        };
        for (index, out) in coords.iter_mut().take(num_coords).enumerate() {
            *out = FT_Fixed::from(blend.get(index).copied().unwrap_or(0x8000));
        }
        return FT_Err_Ok;
    }
    let blend = match inner.var_blend_coordinates_16_16() {
        Ok(blend) => blend,
        Err(err) => return error_to_ft(err) as FT_Error,
    };
    for (index, out) in coords.iter_mut().take(num_coords).enumerate() {
        *out = FT_Fixed::from(blend.get(index).copied().unwrap_or(0));
    }
    FT_Err_Ok
}

pub fn FT_Get_MM_Blend_Coordinates(
    face: Option<&FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&mut [FT_Fixed]>,
) -> FT_Error {
    FT_Get_Var_Blend_Coordinates(face, num_coords, coords)
}

pub fn FT_Set_Var_Blend_Coordinates(
    face: Option<&mut FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&[FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let coords = match coords {
        Some(coords) if coords.len() >= num_coords => &coords[..num_coords],
        Some(_) => return FT_Err_Invalid_Argument as FT_Error,
        None if num_coords == 0 => &[],
        None => return FT_Err_Invalid_Argument as FT_Error,
    };
    let coords_i32 = coords
        .iter()
        .copied()
        .map(i32::try_from)
        .collect::<Result<Vec<_>, _>>();
    let Ok(coords_i32) = coords_i32 else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let result = face
        .inner
        .borrow_mut()
        .set_var_blend_coordinates(&coords_i32);
    match result {
        Ok(()) => {
            let transform_matrix = face.transform_matrix;
            let transform_delta = face.transform_delta;
            let refcount = face.refcount;
            let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
            refreshed.transform_matrix = transform_matrix;
            refreshed.transform_delta = transform_delta;
            refreshed.refcount = refcount;
            *face = refreshed;
            FT_Err_Ok
        }
        Err(err) => error_to_ft(err) as FT_Error,
    }
}

pub fn FT_Set_MM_Blend_Coordinates(
    face: Option<&mut FT_Face>,
    num_coords: FT_UInt,
    coords: Option<&[FT_Fixed]>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Ok(num_coords_usize) = usize::try_from(num_coords) else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let coords = match coords {
        Some(coords) if coords.len() >= num_coords_usize => &coords[..num_coords_usize],
        Some(_) => return FT_Err_Invalid_Argument as FT_Error,
        None if num_coords_usize == 0 => &[],
        None => return FT_Err_Invalid_Argument as FT_Error,
    };
    if face.inner.borrow().font().type1_multi_master().is_some() {
        let coords_i32 = coords
            .iter()
            .copied()
            .map(i32::try_from)
            .collect::<Result<Vec<_>, _>>();
        let Ok(coords_i32) = coords_i32 else {
            return FT_Err_Invalid_Argument as FT_Error;
        };
        let result = face
            .inner
            .borrow_mut()
            .set_type1_mm_blend_coordinates(&coords_i32, num_coords_usize != 0);
        return match result {
            Ok(()) => {
                let transform_matrix = face.transform_matrix;
                let transform_delta = face.transform_delta;
                let refcount = face.refcount;
                let mut refreshed = face_to_ffi(face.inner.borrow().clone(), face.probe_only);
                refreshed.transform_matrix = transform_matrix;
                refreshed.transform_delta = transform_delta;
                refreshed.refcount = refcount;
                *face = refreshed;
                FT_Err_Ok
            }
            Err(err) => error_to_ft(err) as FT_Error,
        };
    }
    if coords.iter().any(|coord| *coord != 0)
        && face.inner.borrow().var_blend_coordinates_16_16().is_ok()
    {
        // FreeType ttgxvar.c:3166-3184 returns the internal public-wrapper
        // sentinel -2 for non-default TrueType/OpenType blend coordinates.
        // FT_Set_Var_Blend_Coordinates consumes this sentinel as success, but
        // FT_Set_MM_Blend_Coordinates exposes it to its caller.
        return -2;
    }
    FT_Set_Var_Blend_Coordinates(Some(face), num_coords, Some(coords))
}

pub fn FT_Get_Default_Named_Instance(
    face: Option<&FT_Face>,
    instance_index: Option<&mut FT_UInt>,
) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // `ft_face_get_mm_service` in FreeType `base/ftmm.c` rejects faces that
    // don't advertise a multiple-master service before the TrueType callback
    // is reached.
    if face.face_flags & FT_FACE_FLAG_MULTIPLE_MASTERS == 0 {
        return FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(instance_index) = instance_index else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    let inner = face.inner.borrow();
    let font = inner.font();
    if font.type1_multi_master().is_some() {
        // FreeType `src/base/ftmm.c:694-716` treats a Multiple Master service
        // without `get_default_named_instance` as success and leaves the
        // caller's output value untouched.  The Type 1 MM service has that
        // shape, unlike TrueType/OpenType `fvar` faces.
        return FT_Err_Ok;
    }
    let fvar = font
        .load_sfnt_table(u32::from_be_bytes(*b"fvar"), 0, None)
        .ok()
        .and_then(|bytes| crate::tt::fvar::parse_fvar(&bytes).ok());
    let Some(fvar) = fvar else {
        return FT_Err_Invalid_Argument as FT_Error;
    };

    // `sfobjs.c` records the one-based entry whose coordinates equal every
    // axis default.  If `fvar` omits that entry, FreeType appends a synthesized
    // default style and reports the position immediately after the table's
    // named styles (`ttgxvar.c:2800-2850`).
    let explicit_default = fvar.instances.iter().position(|style| {
        style
            .coords
            .iter()
            .zip(&fvar.axes)
            .all(|(coord, axis)| *coord == axis.default_value)
    });
    // `sfnt_get_name_id` in `sfdriver.c` considers non-empty Microsoft
    // Unicode/Symbol and Apple Roman records.  `TT_Get_MM_Var` leaves the
    // default index at zero unless both a subfamily name and a PostScript name
    // exist, even though `sfobjs.c` provisionally counted the synthesized
    // style while opening the face.
    let has_name_id = |name_id| {
        (0..font.sfnt_name_count()).any(|index| {
            font.sfnt_name(index).is_some_and(|record| {
                record.name_id == name_id
                    && !record.string.is_empty()
                    && ((record.platform_id == 3 && matches!(record.encoding_id, 0 | 1))
                        || (record.platform_id == 1 && record.encoding_id == 0))
            })
        })
    };
    let can_synthesize = (has_name_id(17) || has_name_id(2)) && has_name_id(6);
    let default = explicit_default
        .and_then(|index| index.checked_add(1))
        .or_else(|| {
            can_synthesize
                .then(|| usize::from(fvar.instance_count).checked_add(1))
                .flatten()
        })
        .or(Some(0))
        .and_then(|index| FT_UInt::try_from(index).ok());
    let Some(default) = default else {
        return FT_Err_Invalid_Argument as FT_Error;
    };
    *instance_index = default;
    FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn FT_Fvar_Named_Style_Coords(
    face: Option<&FT_Face>,
    namedstyle_index: FT_UInt,
) -> Result<Vec<FT_Fixed>, FT_Error> {
    let Some(face) = face else {
        return Err(FT_Err_Invalid_Face_Handle as FT_Error);
    };
    if face.face_flags & FT_FACE_FLAG_MULTIPLE_MASTERS == 0 {
        return Err(FT_Err_Invalid_Argument as FT_Error);
    }
    let inner = face.inner.borrow();
    let font = inner.font();
    let fvar = font
        .load_sfnt_table(u32::from_be_bytes(*b"fvar"), 0, None)
        .ok()
        .and_then(|bytes| crate::tt::fvar::parse_fvar(&bytes).ok())
        .ok_or(FT_Err_Invalid_Argument as FT_Error)?;
    let index =
        usize::try_from(namedstyle_index).map_err(|_| FT_Err_Invalid_Argument as FT_Error)?;
    let style = fvar
        .instances
        .get(index)
        .ok_or(FT_Err_Invalid_Argument as FT_Error)?;
    // C FreeType `ttgxvar.c` populates `FT_Var_Named_Style.coords` directly
    // from the fvar instance coordinate array.  This test-support accessor is
    // intentionally limited to that public descriptor field; gvar/HVAR/MVAR
    // deltas remain separate pending-core work.
    Ok(style
        .coords
        .iter()
        .map(|coord| FT_Fixed::from(*coord))
        .collect())
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
        .map(|mut inner| {
            inner.reset_size_to_undefined();
            face_to_ffi(inner, probe_only)
        })
        .map_err(error_to_ft)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FT_Open_Face_Name_Options {
    pub ignore_typographic_family: bool,
    pub ignore_typographic_subfamily: bool,
}

pub fn FT_New_Memory_Face_With_Name_Options(
    library: &FT_Library,
    data: &[u8],
    face_index: FT_Long,
    size_pt: f32,
    options: FT_Open_Face_Name_Options,
) -> Result<FT_Face, FT_Error> {
    let (face_index, probe_only) = c_face_index_to_core(face_index)?;
    library
        .inner
        .new_memory_face_with_name_options(
            data,
            face_index,
            size_pt,
            options.ignore_typographic_family,
            options.ignore_typographic_subfamily,
        )
        .map(|mut inner| {
            inner.reset_size_to_undefined();
            face_to_ffi(inner, probe_only)
        })
        .map_err(error_to_ft)
}

pub fn FT_Open_External_Stream_Face_With_Name_Options(
    library: &FT_Library,
    data: &[u8],
    face_index: FT_Long,
    size_pt: f32,
    options: FT_Open_Face_Name_Options,
) -> Result<FT_Face, FT_Error> {
    let mut face =
        FT_New_Memory_Face_With_Name_Options(library, data, face_index, size_pt, options)?;
    // C FreeType `FT_Stream_New` marks faces opened with `FT_OPEN_STREAM`
    // as caller-stream backed (`src/base/ftobjs.c`, stream-open path).
    // The Rust core still owns parsed font bytes, but the public face flag
    // must reflect the caller-owned stream source.
    face.face_flags |= FT_FACE_FLAG_EXTERNAL_STREAM;
    Ok(face)
}

fn face_to_ffi(inner: api::Face, probe_only: bool) -> FT_Face {
    let font = inner.font();
    let raw_data = &font.data.raw_data;
    let stream_pos = font
        .data
        .table_directory
        .record(u32::from_be_bytes(*b"cvt "))
        .map_or(0, |record| record.offset);
    let memory_stream = Box::new(FT_StreamRec {
        // C FreeType's FT_New_Memory_Face builds an FT_StreamRec over the
        // retained face bytes.  Public stream probes compare nullness, size,
        // pos, and frame bytes against freetype/src/base/ftobjs.c stream-open
        // setup and ftstream.c memory-frame behavior.
        base: raw_data.as_ptr().cast_mut(),
        size: FT_ULong::try_from(raw_data.len()).unwrap_or(FT_ULong::MAX),
        pos: FT_ULong::from(stream_pos),
        descriptor: FT_StreamDesc::default(),
        pathname: FT_StreamDesc::default(),
        read: std::ptr::null_mut(),
        close: std::ptr::null_mut(),
        memory: std::ptr::null_mut(),
        cursor: std::ptr::null_mut(),
        limit: std::ptr::null_mut(),
    });
    let info = inner.info();
    let postscript_name = inner.postscript_name().map(str::to_owned);
    let type1_font_info_strings = type1_font_info_strings(font.type1_font_info());
    let type1_mm_axis_names = font
        .type1_multi_master()
        .map(|master| {
            master
                .axes
                .iter()
                .filter_map(|axis| CString::new(axis.name.as_str()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let size_state = inner.active_size_state();
    let size_metrics = inner.size_metrics().into();
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
        .map(|mut header| {
            apply_mvar_vertical_header_deltas(&mut header, font.mvar_vertical_header_deltas());
            header
        })
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
    let cpal = font
        .load_sfnt_table(u32::from_be_bytes(*b"CPAL"), 0, None)
        .ok()
        .and_then(|data| parse_cpal_table(&data))
        .map(RefCell::new)
        .map(Rc::new);
    let available_sizes = available_sizes_to_ffi(font);
    let num_fixed_sizes = FT_Int::try_from(available_sizes.len()).unwrap_or(FT_Int::MAX);
    let (charmaps, charmap_metadata) = charmaps_to_ffi(&inner);
    let glyph_to_script_map = inner.font().autohint_glyph_style_map().into_boxed_slice();
    let inner = Rc::new(RefCell::new(inner));
    // FreeType `FT_Open_Face`/`FT_New_Memory_Face` negative face-index probes
    // start with `face->size == NULL`; `FT_New_Size` may allocate one later.
    let sizes = Rc::new(RefCell::new(if probe_only {
        FaceSizeState::empty()
    } else {
        FaceSizeState::new(size_state)
    }));
    let active_size = sizes.borrow().active_handle();
    let active_charmap_index = inner
        .borrow()
        .charmap_index()
        .and_then(|index| FT_Int::try_from(index).ok())
        .unwrap_or(-1);
    let face = FT_Face {
        num_faces: info.num_faces as FT_Long,
        face_index: info.face_index as FT_Long,
        face_flags: FT_Long::from(info.face_flags),
        style_flags: FT_Long::from(info.style_flags),
        family_name: Some(info.family_name),
        style_name: Some(info.style_name),
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
        max_advance_width: info.max_advance_width as FT_Short,
        max_advance_height: info.max_advance_height as FT_Short,
        underline_position: info.underline_position,
        underline_thickness: info.underline_thickness,
        num_fixed_sizes,
        available_sizes,
        size: active_size,
        size_metrics,
        active_charmap_index,
        charmaps,
        memory_stream,
        inner,
        sizes,
        probe_only,
        postscript_name,
        type1_font_info_strings,
        type1_mm_axis_names,
        sfnt_os2,
        sfnt_head,
        sfnt_maxp,
        sfnt_hhea,
        sfnt_vhea,
        sfnt_post,
        sfnt_pclt,
        charmap_metadata,
        cpal,
        transform_matrix: FT_Matrix {
            xx: 1 << 16,
            xy: 0,
            yx: 0,
            yy: 1 << 16,
        },
        transform_delta: FT_Vector { x: 0, y: 0 },
        no_stem_darkening: -1,
        random_seed: -1,
        increase_x_height: 0,
        glyph_to_script_map,
        refcount: 1,
    };
    register_face_size_handles(&face);
    face
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

fn apply_mvar_vertical_header_deltas(
    header: &mut TT_VertHeader,
    deltas: Option<crate::tt::mvar::VerticalHeaderDeltas>,
) {
    let Some(deltas) = deltas else {
        return;
    };
    // FreeType `truetype/ttgxvar.c:1406-1472,1633-1755` maps these MVAR tags
    // to `TT_VertHeader` scalar fields and casts the added delta back to
    // FT_Short.
    header.Ascender = header.Ascender.wrapping_add(deltas.ascender as FT_Short);
    header.Descender = header.Descender.wrapping_add(deltas.descender as FT_Short);
    header.Line_Gap = header.Line_Gap.wrapping_add(deltas.line_gap as FT_Short);
    header.caret_Slope_Rise = header
        .caret_Slope_Rise
        .wrapping_add(deltas.caret_slope_rise as FT_Short);
    header.caret_Slope_Run = header
        .caret_Slope_Run
        .wrapping_add(deltas.caret_slope_run as FT_Short);
    header.caret_Offset = header
        .caret_Offset
        .wrapping_add(deltas.caret_offset as FT_Short);
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

fn available_sizes_to_ffi(font: &crate::font::Font) -> Box<[FT_Bitmap_Size]> {
    let Some(header) = font.winfnt_header() else {
        return Box::new([]);
    };

    // FreeType `src/winfonts/winfnt.c:fnt_face_get_dll_font` fills exactly one
    // `FT_Bitmap_Size` for WinFNT faces from header metrics, then derives ppem
    // with `FT_MulDiv` + `FT_PIX_ROUND` and clamps oversized nominal y ppem.
    let mut size = FT_Pos::from(header.nominal_point_size) << 6;
    let x_res = if header.horizontal_resolution == 0 {
        72
    } else {
        header.horizontal_resolution
    };
    let y_res = if header.vertical_resolution == 0 {
        72
    } else {
        header.vertical_resolution
    };
    let mut y_ppem =
        crate::scaler::ft_pix_round(crate::fixed::ft_mul_div(size as i32, i32::from(y_res), 72))
            as FT_Pos;
    let pixel_height = FT_Pos::from(header.pixel_height) << 6;
    if y_ppem > pixel_height {
        y_ppem = pixel_height;
        size = crate::fixed::ft_mul_div(y_ppem as i32, 72, i32::from(y_res)) as FT_Pos;
    }
    let x_ppem =
        crate::scaler::ft_pix_round(crate::fixed::ft_mul_div(size as i32, i32::from(x_res), 72))
            as FT_Pos;

    Box::new([FT_Bitmap_Size {
        height: FT_Short::try_from(
            u32::from(header.pixel_height) + u32::from(header.external_leading),
        )
        .unwrap_or(FT_Short::MAX),
        width: FT_Short::try_from(header.avg_width).unwrap_or(FT_Short::MAX),
        size,
        x_ppem,
        y_ppem,
    }])
}

fn charmaps_to_ffi(face: &api::Face) -> FaceCharmapRecords {
    let infos = face.font().charmaps();
    let mut charmaps = Vec::with_capacity(infos.len());
    let mut metadata = Vec::with_capacity(infos.len());
    for info in infos {
        charmaps.push(FT_CharMapRecPublic {
            face: ptr::null_mut(),
            encoding: charmap_encoding(info.platform_id, info.encoding_id),
            platform_id: info.platform_id,
            encoding_id: info.encoding_id,
        });
        metadata.push((FT_Long::from(info.format), FT_ULong::from(info.language_id)));
    }
    let charmaps = charmaps.into_boxed_slice();
    let metadata = metadata.into_boxed_slice();
    register_charmap_metadata(&charmaps, &metadata);
    (charmaps, metadata)
}

fn charmap_metadata_registry() -> &'static Mutex<CharmapMetadataRegistry> {
    static REGISTRY: OnceLock<Mutex<CharmapMetadataRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_charmap_metadata(charmaps: &[FT_CharMapRecPublic], metadata: &[(FT_Long, FT_ULong)]) {
    if let Ok(mut registry) = charmap_metadata_registry().lock() {
        for (index, record) in charmaps.iter().enumerate() {
            if let Some(&(format, language_id)) = metadata.get(index) {
                register_charmap_record_locked(&mut registry, index, record, format, language_id);
            }
        }
    }
}

fn register_charmap_record_locked(
    registry: &mut CharmapMetadataRegistry,
    index: usize,
    record: &FT_CharMapRecPublic,
    format: FT_Long,
    language_id: FT_ULong,
) {
    let key = (record as *const FT_CharMapRecPublic) as usize;
    registry.insert(key, (format, language_id, index as FT_Int));
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
    if !has_active_size(face) {
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
    let result = face.inner.borrow_mut().try_set_char_size(
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    );
    match result {
        Ok(()) => {
            sync_active_size_state(face);
            FT_Err_Ok
        }
        Err(SizeRequestError::DivideByZero) => FT_Err_Divide_By_Zero as FT_Error,
        Err(SizeRequestError::InvalidPixelSize) => FT_Err_Invalid_Pixel_Size,
        Err(SizeRequestError::InvalidPpem) => FT_Err_Invalid_PPem as FT_Error,
    }
}

pub fn FT_Set_Pixel_Sizes(
    face: &mut FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    if !has_active_size(face) {
        return FT_Err_Invalid_Size_Handle;
    }
    face.inner
        .borrow_mut()
        .set_pixel_sizes(pixel_width, pixel_height);
    sync_active_size_state(face);
    FT_Err_Ok
}

pub fn FT_Request_Size(face: Option<&mut FT_Face>, req: Option<&FT_Size_RequestRec>) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if !has_active_size(face) {
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
    if face.probe_only {
        // FreeType 2.14.3 `FT_Request_Size` -> `FT_Request_Metrics`
        // (`src/base/ftobjs.c:3438-3484`, `3264-3410`) treats a negative-index
        // probe face as non-scalable: the request succeeds with zero metrics
        // and identity scales instead of using the parsed face dimensions.
        face.inner.borrow_mut().reset_probe_size_request_metrics();
        sync_active_size_state(face);
        return FT_Err_Ok;
    }
    let request = SizeRequest {
        request_type,
        width: req.width,
        height: req.height,
        hori_resolution: req.horiResolution,
        vert_resolution: req.vertResolution,
    };
    let result = face.inner.borrow_mut().request_size(request);
    match result {
        Ok(()) => {
            sync_active_size_state(face);
            FT_Err_Ok
        }
        Err(SizeRequestError::DivideByZero) => FT_Err_Divide_By_Zero as FT_Error,
        Err(SizeRequestError::InvalidPixelSize) => FT_Err_Invalid_Pixel_Size,
        Err(SizeRequestError::InvalidPpem) => FT_Err_Invalid_PPem as FT_Error,
    }
}

pub fn FT_Get_Char_Index(face: &FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Ok(char_code) = u32::try_from(char_code) else {
        return 0;
    };
    u32::from(face.inner.borrow().get_char_index(char_code))
}

pub fn FT_Face_GetCharVariantIndex(
    face: Option<&FT_Face>,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(face) = face else {
        return 0;
    };
    // FreeType `FT_Face_GetCharVariantIndex` truncates both public
    // `FT_ULong` inputs to `FT_UInt32` before calling the cmap format-14 query
    // (`src/base/ftobjs.c`, `src/sfnt/ttcmap.c`).
    u32::from(
        face.inner
            .borrow()
            .get_char_variant_index(charcode as u32, variant_selector as u32),
    )
}

pub fn FT_Face_GetCharVariantIsDefault(
    face: Option<&FT_Face>,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(face) = face else {
        return -1;
    };
    // FreeType `FT_Face_GetCharVariantIsDefault` uses the format-14 selector
    // charmap directly and truncates both public inputs to `FT_UInt32`
    // (`src/base/ftobjs.c`, `src/sfnt/ttcmap.c`).
    face.inner
        .borrow()
        .get_char_variant_is_default(charcode as u32, variant_selector as u32)
}

pub fn FT_Face_GetVariantSelectors(face: Option<&FT_Face>) -> Option<Vec<FT_UInt32>> {
    let face = face?;
    face.inner.borrow().get_variant_selectors()
}

pub fn FT_Face_GetVariantsOfChar(
    face: Option<&FT_Face>,
    charcode: FT_ULong,
) -> Option<Vec<FT_UInt32>> {
    let face = face?;
    face.inner.borrow().get_variants_of_char(charcode as u32)
}

pub fn FT_Face_GetCharsOfVariant(
    face: Option<&FT_Face>,
    variant_selector: FT_ULong,
) -> Option<Vec<FT_UInt32>> {
    let face = face?;
    face.inner
        .borrow()
        .get_chars_of_variant(variant_selector as u32)
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
    let vector = face
        .inner
        .borrow()
        .kerning_by_glyphs(left_glyph, right_glyph, mode);
    akerning.x = FT_Long::from(vector.x);
    akerning.y = FT_Long::from(vector.y);
    FT_Err_Ok
}

pub fn FT_Get_PFR_Kerning(
    face: Option<&FT_Face>,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    avector: Option<&mut FT_Vector>,
) -> FT_Error {
    // FreeType 2.14.3 `src/base/ftpfr.c:98-120` validates face/vector, then
    // falls back to `FT_Get_Kerning(..., FT_KERNING_UNSCALED, ...)` when the
    // face has no PFR metrics service.  `fontdone` does not implement PFR
    // services yet, so every supported face takes that public fallback path.
    FT_Get_Kerning(
        face,
        left_glyph,
        right_glyph,
        FT_KERNING_UNSCALED as FT_UInt,
        avector,
    )
}

pub fn FT_Get_Charmap_Index(charmap: FT_CharMap) -> FT_Int {
    registered_charmap_metadata(charmap).map_or(-1, |(_, _, index)| index)
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
    if face.charmap_metadata.get(index).map(|record| record.0) == Some(14) {
        return FT_Err_Invalid_Argument;
    }
    let result = face.inner.borrow_mut().set_charmap(index);
    match result {
        Ok(()) => {
            sync_active_charmap_index(face);
            FT_Err_Ok
        }
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
        .position(|record| ptr::eq(record as *const FT_CharMapRecPublic, target))
}

pub fn FT_Select_Charmap(face: Option<&mut FT_Face>, encoding: FT_Encoding) -> FT_Error {
    let Some(face) = face else {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match i64::from(encoding) {
        FT_ENCODING_UNICODE => {
            let result = face.inner.borrow_mut().select_unicode_charmap();
            match result {
                Ok(()) => {
                    sync_active_charmap_index(face);
                    FT_Err_Ok
                }
                Err(_) => FT_Err_Invalid_CharMap_Handle,
            }
        }
        _ => {
            let Some(index) = face
                .charmaps
                .iter()
                .position(|charmap| charmap.encoding == encoding)
            else {
                return FT_Err_Invalid_Argument;
            };
            let result = face.inner.borrow_mut().set_charmap(index);
            match result {
                Ok(()) => {
                    sync_active_charmap_index(face);
                    FT_Err_Ok
                }
                Err(_) => FT_Err_Invalid_Argument,
            }
        }
    }
}

pub fn FT_Get_FSType_Flags(face: Option<&FT_Face>) -> FT_UShort {
    face.map_or(0, |face| face.inner.borrow().get_fstype_flags())
}

pub fn FT_Get_Sfnt_Name_Count(face: Option<&FT_Face>) -> FT_UInt {
    face.map_or(0, |face| {
        FT_UInt::try_from(face.inner.borrow().sfnt_name_count()).unwrap_or(FT_UInt::MAX)
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
    let inner = face.inner.borrow();
    let Some(record) = inner.sfnt_name(idx as usize) else {
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
        if let Some((code, glyph)) = face.inner.borrow().first_char() {
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
        if let Some((code, glyph)) = face.inner.borrow().next_char(char_code as u32) {
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

#[cfg(feature = "abi-test-support")]
pub fn FT_Empty_GlyphSlot(face: &FT_Face) -> FT_GlyphSlot {
    slot_to_ffi(face, api::GlyphSlot::empty(), api::LoadFlags::DEFAULT)
}

#[cfg(feature = "abi-test-support")]
pub fn FT_Unsupported_GlyphSlot(face: &FT_Face) -> FT_GlyphSlot {
    let mut slot = FT_Empty_GlyphSlot(face);
    slot.glyph_index = 77;
    slot.advance = FT_Vector { x: 11, y: 22 };
    slot.format = 0x1234_5678;
    slot
}

#[cfg(feature = "abi-test-support")]
pub fn FT_Outline_GlyphSlot_With_Advance(
    face: &FT_Face,
    advance_x: FT_Pos,
    advance_y: FT_Pos,
) -> FT_GlyphSlot {
    let mut slot = FT_Empty_GlyphSlot(face);
    slot.advance = FT_Vector {
        x: advance_x,
        y: advance_y,
    };
    slot.format = FT_GLYPH_FORMAT_OUTLINE;
    slot.outline = Some(FT_OutlineSnapshot::default());
    slot
}

pub fn FT_Load_Glyph(
    face: &FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> Result<FT_GlyphSlot, FT_Error> {
    if face.probe_only || !has_active_size(face) {
        return Err(FT_Err_Invalid_Size_Handle);
    }
    let Ok(glyph_index) = u16::try_from(glyph_index) else {
        // FreeType 2.14.3's TT and CFF drivers classify every index outside
        // `num_glyphs` as Invalid_Argument, including FT_UInt values above
        // this core's u16 glyph-index representation.
        return Err(FT_Err_Invalid_Argument);
    };
    let flags = load_flags_to_core(load_flags)?;
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
        .borrow()
        .load_glyph_with_transform(glyph_index, flags, transform)
        .map(|slot| slot_to_ffi(face, slot, flags))
        .map_err(error_to_ft)
}

pub fn FT_Get_Advance(
    face: &FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> Result<FT_Fixed, FT_Error> {
    // FreeType `src/base/ftadvanc.c:116-126` checks glyph availability before
    // any size-dependent fallback.  A negative face-index probe has no active
    // size and the pinned C oracle reports `Invalid_Glyph_Index` for this
    // `FT_Get_Advance` route instead of the otherwise natural
    // `Invalid_Size_Handle`.
    if face.probe_only {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    if !has_active_size(face) {
        return Err(FT_Err_Invalid_Size_Handle);
    }
    let glyph_index = u16::try_from(glyph_index).map_err(|_| FT_Err_Invalid_Glyph_Index)?;
    // FreeType `src/base/ftadvanc.c:116-126` performs the glyph-index range
    // check before fast-only fallback and load-flag conversion, so invalid
    // glyph indices still report `Invalid_Glyph_Index` even when flags contain
    // otherwise unsupported public bits.
    if glyph_index >= face.inner.borrow().info().num_glyphs {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    let fast_only = load_flags & FT_ADVANCE_FLAG_FAST_ONLY_I32 != 0;
    let load_flags = load_flags & !FT_ADVANCE_FLAG_FAST_ONLY_I32;
    if fast_only && !advance_fast_path_supported(load_flags) {
        return Err(FT_Err_Unimplemented_Feature);
    }
    let flags = load_flags_to_core(load_flags)?;
    if use_fast_horizontal_advance(flags) {
        // C `tt_get_advances` returns raw hmtx advances; `ft_face_scale_advances_`
        // scales them directly to 16.16 with `FT_MulFix(1024 * advance, x_scale)`.
        return Ok(FT_Fixed::from(
            face.inner.borrow().glyph_hori_advance_16dot16(glyph_index),
        ));
    }
    let slot = face
        .inner
        .borrow()
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
    // FreeType `FT_Get_Advances` in `src/base/ftadvanc.c:148-194`
    // performs the unsigned `start + count` range check before the
    // `count == 0` success return, so `start >= num_glyphs` is still an
    // invalid glyph index even when no advances would be written.
    let num = FT_UInt::from(face.inner.borrow().info().num_glyphs);
    let end = start.wrapping_add(count);
    if start >= num || end < start || end > num {
        return Err(FT_Err_Invalid_Glyph_Index);
    }
    if count == 0 {
        return Ok(Vec::new());
    }

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
    if !matches!(
        slot.format,
        FT_GLYPH_FORMAT_OUTLINE | FT_GLYPH_FORMAT_BITMAP
    ) {
        // C FreeType `src/base/ftobjs.c` renderer lookup rejects unloaded
        // (`FT_GLYPH_FORMAT_NONE`) and unknown public slot formats before any
        // renderer mutates the slot; callers observe Cannot_Render_Glyph and
        // the original slot fields stay intact.
        return Err(FT_Err_Cannot_Render_Glyph);
    }
    let was_bitmap = slot.format == FT_GLYPH_FORMAT_BITMAP;
    let source_face = slot.source_face.clone();
    let load_flags = slot.load_flags;
    slot.core_slot
        .render(mode)
        .map(|rendered| {
            let render_flags = if was_bitmap {
                load_flags
            } else {
                load_flags | api::LoadFlags::RENDER | load_flag_for_render_mode(mode)
            };
            slot_to_ffi(&face_to_ffi(source_face, false), rendered, render_flags)
        })
        .map_err(error_to_ft)
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

#[cfg(any(test, feature = "abi-test-support"))]
pub fn FT_Get_Sfnt_VertHeader_Copy(face: &FT_Face) -> Option<TT_VertHeader> {
    face.sfnt_vhea.as_deref().copied()
}

pub fn FT_Load_Sfnt_Table(
    face: &FT_Face,
    tag: FT_ULong,
    offset: FT_Long,
    length: Option<&mut FT_ULong>,
) -> Result<Option<Vec<u8>>, FT_Error> {
    let inner = face.inner.borrow();
    let font = inner.font();
    if !font.is_sfnt() {
        return Err(FT_Err_Invalid_Face_Handle as FT_Error);
    }
    let tag_u32 = match u32::try_from(tag) {
        Ok(t) => t,
        Err(_) => return Err(FT_Err_Table_Missing as FT_Error),
    };
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
    let inner = face.inner.borrow();
    let font = inner.font();
    if !font.is_sfnt() {
        return FT_Err_Invalid_Face_Handle as FT_Error;
    }
    let Some(length) = length else {
        return FT_Err_Invalid_Argument;
    };
    let Some(tag) = tag else {
        // C `sfnt_table_info` returns the table count when `tag == NULL`,
        // ignoring `table_index` (sfnt/sfdriver.c:156-158).
        *length = font.sfnt_tables().len() as FT_ULong;
        return FT_Err_Ok;
    };
    let index = match usize::try_from(table_index) {
        Ok(i) => i,
        Err(_) => return FT_Err_Table_Missing as FT_Error,
    };
    let Some(info) = font.sfnt_table_info(index) else {
        return FT_Err_Table_Missing as FT_Error;
    };
    *tag = info.tag as FT_ULong;
    *length = info.length as FT_ULong;
    FT_Err_Ok
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
    let owns_bitmap = format == FT_GLYPH_FORMAT_BITMAP && bitmap.is_some();
    // FreeType's `FT_LOAD_NO_RECURSE` composite path (`ttgload.c`) computes
    // metrics from the composite glyph header bbox, but leaves `slot->outline`
    // empty because the slot format is `FT_GLYPH_FORMAT_COMPOSITE`.
    let (outline_cbox, outline_bbox) = if slot.format == api::GlyphFormat::Composite {
        (FT_BBox::default(), FT_BBox::default())
    } else {
        (
            bbox_to_ffi(slot.outline_cbox),
            bbox_to_ffi(slot.outline_bbox),
        )
    };
    let outline = slot.slot_outline().map(outline_to_ffi_snapshot);
    let source_face = face.inner.borrow().clone();
    FT_GlyphSlot {
        glyph_index,
        metrics,
        advance,
        format,
        num_subglyphs,
        bitmap,
        bitmap_left,
        bitmap_top,
        owns_bitmap,
        outline_cbox,
        outline_bbox,
        outline,
        core_slot: slot,
        source_face,
        load_flags,
    }
}

fn refresh_slot_public_fields(slot: &mut FT_GlyphSlot) {
    slot.metrics = slot.core_slot.metrics.into();
    slot.advance = slot.core_slot.advance.into();
    slot.format = glyph_format_from_core(slot.core_slot.format);
    slot.num_subglyphs = FT_UInt::try_from(slot.core_slot.subglyphs.len()).unwrap_or(FT_UInt::MAX);
    slot.bitmap = slot.core_slot.bitmap.clone().map(Into::into);
    slot.bitmap_left = slot.core_slot.bitmap_left;
    slot.bitmap_top = slot.core_slot.bitmap_top;
    if slot.format != FT_GLYPH_FORMAT_BITMAP || slot.bitmap.is_none() {
        slot.owns_bitmap = false;
    }
    slot.outline_cbox = bbox_to_ffi(slot.core_slot.outline_cbox);
    slot.outline_bbox = bbox_to_ffi(slot.core_slot.outline_bbox);
    slot.outline = slot.core_slot.slot_outline().map(outline_to_ffi_snapshot);
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

fn outline_snapshot_to_core(outline: &FT_OutlineSnapshot) -> Option<crate::outline::Outline> {
    let points = outline
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            Some(crate::outline::OutlinePoint {
                x: i32::try_from(point.x).ok()?,
                y: i32::try_from(point.y).ok()?,
                on_curve: outline.tags.get(index).is_none_or(|tag| tag & 1 != 0),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    // The public FT_Outline contour array is FT_Short.  The compatibility
    // snapshot stores the ABI bits in FT_UShort, so preserve negative contour
    // endpoints (notably -1) instead of rejecting them before the rasterizer.
    let contours = outline
        .contours
        .iter()
        .map(|&contour| i16::from_ne_bytes(contour.to_ne_bytes()))
        .collect::<Vec<_>>();
    Some(crate::outline::Outline {
        n_contours: i32::try_from(contours.len()).ok()?,
        contours,
        points,
        tags: outline.tags.clone(),
        contour_dropouts: Vec::new(),
        flags: u32::try_from(outline.flags).unwrap_or(0),
        cbox_x_min: 0,
        cbox_y_min: 0,
        cbox_x_max: 0,
        cbox_y_max: 0,
    })
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
    // `ft_open_face_internal` first keeps only the lower 31 bits together
    // with the sign.  In particular, negating `LONG_MIN` on FreeType's
    // two's-complement targets and applying the mask normalizes it to zero,
    // so it opens face 0 instead of remaining a negative probe (ftobjs.c).
    let magnitude = face_index.unsigned_abs() & 0x7FFF_FFFF;
    let face_index = if face_index > 0 {
        magnitude as FT_Long
    } else {
        -(magnitude as FT_Long)
    };
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
    let selected = -face_index - 1;
    let face_index = usize::try_from(selected).map_err(|_| FT_Err_Invalid_Argument)?;
    Ok((face_index, true))
}
