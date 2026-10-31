//! Pillow-compatible scalar rules shared by L-mode Mandelbrot strategies.

#[inline]
pub(crate) fn membership_margin(quality: i32) -> f64 {
    128.0 * f64::EPSILON * (f64::from(quality) + 1.0).powi(2)
}

#[inline]
pub(crate) fn known_interior(cr: f64, ci: f64, margin: f64) -> bool {
    let ci2 = ci * ci;

    let x = cr - 0.25;
    let q = x * x + ci2;
    let cardioid_left = q * (q + x);
    let cardioid_right = 0.25 * ci2;
    if cardioid_left < cardioid_right - margin * (1.0 + cardioid_left.abs() + cardioid_right.abs())
    {
        return true;
    }

    let bulb_x = cr + 1.0;
    let bulb_distance = bulb_x * bulb_x + ci2;
    bulb_distance < 0.0625 - margin * (1.0 + bulb_distance.abs())
}

#[inline(always)]
pub(crate) fn pixel_value(cr: f64, ci: f64, quality: i32) -> u8 {
    let mut zx = 0.0f64;
    let mut zy = 0.0f64;
    let mut zx2 = 0.0f64;
    let mut zy2 = 0.0f64;

    let mut iteration = 1i32;
    loop {
        // Preserve Pillow's operation order: update y, then x from the previous
        // squares, then test escape before testing the requested quality.
        zy = 2.0 * zx * zy + ci;
        zx = zx2 - zy2 + cr;
        zx2 = zx * zx;
        zy2 = zy * zy;

        if zx2 + zy2 > 100.0 {
            return (iteration * 255 / quality) as u8;
        }
        if iteration > quality {
            return 0;
        }
        iteration += 1;
    }
}
