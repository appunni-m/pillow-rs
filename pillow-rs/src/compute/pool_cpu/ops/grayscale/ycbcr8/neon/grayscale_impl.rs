//! Original NEON structure-load extraction of the Y channel.

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) fn execute(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let pixels = source.len() / 3;
    let vector_pixels = pixels / 16 * 16;
    let input = source;
    let mut output = Vec::<u8>::with_capacity(pixels);
    // SAFETY: The complete-pixel count is bounded by source.len() / 3.
    // Each structure load reads 48 bytes from a complete sixteen-
    // pixel RGB block, and each store initializes sixteen reserved output
    // bytes. The four-block loop advances by 64 pixels and the remainder
    // loop handles each complete sixteen-pixel block once. `set_len` runs
    // after every prefix byte is initialized; complete triples left after
    // the vector prefix are appended below.
    #[allow(unsafe_code)]
    unsafe {
        use core::arch::aarch64 as neon;
    
        macro_rules! extract_y_block {
            ($pixel:expr) => {{
                let rgb = neon::vld3q_u8(input.as_ptr().add($pixel * 3));
                neon::vst1q_u8(output.as_mut_ptr().add($pixel), rgb.0);
            }};
        }
    
        let unrolled_pixels = vector_pixels / 64 * 64;
        for pixel in (0..unrolled_pixels).step_by(64) {
            extract_y_block!(pixel);
            extract_y_block!(pixel + 16);
            extract_y_block!(pixel + 32);
            extract_y_block!(pixel + 48);
        }
        for pixel in (unrolled_pixels..vector_pixels).step_by(16) {
            extract_y_block!(pixel);
        }
        output.set_len(vector_pixels);
    }
    output.extend(
        input[vector_pixels * 3..]
            .chunks_exact(3)
            .map(|pixel| pixel[0]),
    );
    (
        output,
        (vector_pixels / 16) as u64,
        (pixels - vector_pixels) as u64,
    )
}
