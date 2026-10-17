#![allow(unsafe_op_in_unsafe_fn)]
use core::arch::aarch64::{uint8x16x4_t, vld3q_u8, vst1q_u8, vst1q_u8_x4};

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn current(src: *const u8, dst: *mut u8, pixels: usize) {
    let groups = pixels / 64;
    for group in 0..groups {
        let pixel = group * 64;
        let a = vld3q_u8(src.add(pixel * 3)).1;
        let b = vld3q_u8(src.add((pixel + 16) * 3)).1;
        let c = vld3q_u8(src.add((pixel + 32) * 3)).1;
        let d = vld3q_u8(src.add((pixel + 48) * 3)).1;
        vst1q_u8(dst.add(pixel), a);
        vst1q_u8(dst.add(pixel + 16), b);
        vst1q_u8(dst.add(pixel + 32), c);
        vst1q_u8(dst.add(pixel + 48), d);
    }
}

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn grouped(src: *const u8, dst: *mut u8, pixels: usize) {
    let groups = pixels / 64;
    for group in 0..groups {
        let pixel = group * 64;
        let a = vld3q_u8(src.add(pixel * 3)).1;
        let b = vld3q_u8(src.add((pixel + 16) * 3)).1;
        let c = vld3q_u8(src.add((pixel + 32) * 3)).1;
        let d = vld3q_u8(src.add((pixel + 48) * 3)).1;
        vst1q_u8_x4(dst.add(pixel), uint8x16x4_t(a, b, c, d));
    }
}
