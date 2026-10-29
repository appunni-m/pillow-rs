#[path = "i32_cross_smooth.rs"]
pub(super) mod i32_cross_smooth;

pub(super) fn matches(kernel: &[f32; 9], offset: f32) -> bool {
    i32_cross_smooth::matches(kernel, offset)
}

pub(super) fn apply(raw: &[u8], output: &mut [u8], width: usize, height: usize) {
    i32_cross_smooth::apply(raw, output, width, height);
}
