//! Shared Pillow radial-distance sample formula for native mode strategies.

#[inline]
pub(crate) fn sample(x: usize, y: usize) -> u8 {
    let dx = x as f64 - 128.0;
    let dy = y as f64 - 128.0;
    let distance = ((dx * dx + dy * dy) * 2.0).sqrt() as i32;
    if distance >= 255 { 255 } else { distance as u8 }
}
