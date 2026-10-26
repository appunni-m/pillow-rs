use crate::pipeline::ResampleFilter;

/// Evaluate the scalar sine used by Pillow's ARM64 resampler on WASM.
///
/// The native Pillow build uses the Apple ARM64 `libsystem_m` implementation
/// rather than a correctly-rounded libm.  WebAssembly's `f64::sin` follows
/// the target's bundled libm and therefore chooses a different last bit for a
/// small fraction of Lanczos coefficients.  Port the short/medium argument
/// paths of Apple's routine so coefficient generation has the same result
/// without crossing the runtime FFI boundary.  Resize kernels only call this
/// helper for finite arguments in the interval supported by the medium path.
#[inline]
pub(crate) fn pillow_sin_f64(value: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        value.sin()
    }

    #[cfg(target_arch = "wasm32")]
    {
        if value == 0.0 || !value.is_finite() {
            return value.sin();
        }

        // Constants are the exact bit patterns loaded by Apple's ARM64
        // sin() entry point (libsystem_m.dylib, _sin at offset 0x968).
        const PI_OVER_FOUR: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
        const MEDIUM_BOUND: f64 = f64::from_bits(0x4120_0001_3be5_7a40);
        const TWO_OVER_PI: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
        const HALF_PI_HIGH: f64 = f64::from_bits(0x3ff9_21fb_5444_0000);
        const HALF_PI_LOW: f64 = f64::from_bits(0x3d86_8c23_4c4c_0000);
        const HALF_PI_TAIL: f64 = f64::from_bits(0x3b29_8a2e_0370_7345);

        // The coefficients below are loaded in pairs by the native routine.
        // Keeping their exact f64 encodings avoids a decimal-parser change in
        // the final coefficient bit.
        const SIN_C7: f64 = f64::from_bits(0x3de5_d8fd_1fd1_9ccd);
        const SIN_C6: f64 = f64::from_bits(0xbe5a_e5e5_a929_1f5d);
        const SIN_C5: f64 = f64::from_bits(0x3ec7_1de3_567d_48a1);
        const SIN_C4: f64 = f64::from_bits(0xbf2a_01a0_19bf_df03);
        const SIN_C3: f64 = f64::from_bits(0x3f81_1111_1110_f7d0);
        const SIN_C2: f64 = f64::from_bits(0xbfc5_5555_5555_5548);
        const COS_C7: f64 = f64::from_bits(0xbda8_fa49_a086_1a9b);
        const COS_C6: f64 = f64::from_bits(0x3e21_ee9d_7b4e_3f05);
        const COS_C5: f64 = f64::from_bits(0xbe92_7e4f_7eac_4bc6);
        const COS_C4: f64 = f64::from_bits(0x3efa_01a0_19c8_44f5);
        const COS_C3: f64 = f64::from_bits(0xbf56_c16c_16c1_4f91);
        const COS_C2: f64 = f64::from_bits(0x3fa5_5555_5555_554b);
        const COS_C1: f64 = f64::from_bits(0xbfe0_0000_0000_0000);

        #[inline]
        fn sin_polynomial(z: f64) -> f64 {
            let mut polynomial = SIN_C7.mul_add(z, SIN_C6);
            polynomial = polynomial.mul_add(z, SIN_C5);
            polynomial = polynomial.mul_add(z, SIN_C4);
            polynomial = polynomial.mul_add(z, SIN_C3);
            polynomial = polynomial.mul_add(z, SIN_C2);
            z * polynomial
        }

        #[inline]
        fn cos_polynomial(z: f64) -> f64 {
            let mut polynomial = COS_C7.mul_add(z, COS_C6);
            polynomial = polynomial.mul_add(z, COS_C5);
            polynomial = polynomial.mul_add(z, COS_C4);
            polynomial = polynomial.mul_add(z, COS_C3);
            polynomial = polynomial.mul_add(z, COS_C2);
            polynomial = polynomial.mul_add(z, COS_C1);
            z.mul_add(polynomial, 1.0)
        }

        let absolute = value.abs();
        if absolute <= PI_OVER_FOUR {
            let z = value * value;
            return value.mul_add(sin_polynomial(z), value);
        }
        if absolute >= MEDIUM_BOUND {
            return value.sin();
        }

        // ARM64 uses round-to-nearest-even (`frintn`) for the nearest
        // half-pi multiple.  The residual is kept as two f64 words and the
        // split is retained through the final polynomial add.
        let quotient = value * TWO_OVER_PI;
        let nearest = quotient.round_ties_even();
        let quadrant = nearest as i64;
        let high = nearest * HALF_PI_HIGH;
        let low = nearest * HALF_PI_LOW;
        let tail = nearest * HALF_PI_TAIL;
        let first = value - high;
        let residual_high = first - low;
        let compensation = (first - residual_high) - low;
        let residual_low = compensation - tail;

        let result = if quadrant & 1 == 0 {
            let residual = residual_high + residual_low;
            let z = residual * residual;
            let correction = sin_polynomial(z);
            // Keep the native two-word final grouping: fma(residual,
            // correction, residual_low) + residual_high.
            residual.mul_add(correction, residual_low) + residual_high
        } else {
            // Apple's cosine path uses the high/low split to retain the
            // residual square's leading bits before evaluating its polynomial.
            let z = residual_high * residual_low;
            let z = z + z;
            let z = residual_high.mul_add(residual_high, z);
            cos_polynomial(z)
        };
        if (quadrant >> 1) & 1 == 0 {
            result
        } else {
            -result
        }
    }
}

// ── Filter kernels ──

/// Box / Nearest-neighbor kernel.
fn kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
fn kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
}

/// Catmull-Rom (bicubic) kernel.
fn kernel_catrom(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        // Match Pillow's Resample.c Horner evaluation exactly.  Expanding
        // this polynomial with powi changes the last f64 bits for some
        // heterogeneous F samples, which becomes a visible f32 ULP after
        // ImagingResample stores the accumulated value.
        let t = 1.5f64.mul_add(a, -2.5);
        a.mul_add(t * a, 1.0)
    } else if a < 2.0 {
        let t = (a - 5.0).mul_add(a, 8.0);
        a.mul_add(t, -4.0) * -0.5
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
fn kernel_lanczos(x: f64, a: f64) -> f64 {
    if x.abs() >= a {
        return 0.0;
    }
    if x.abs() < 1e-10 {
        return 1.0;
    }
    let pix = std::f64::consts::PI * x;
    let sa = pillow_sin_f64(pix) / pix;
    // Pillow's src/libImaging/Resample.c forms x/a before multiplying by pi.
    // Reassociating this as pi*x/a changes a few wide cancellation rows by
    // one f64 ULP before the final f32 store.
    let scaled_pix = (x / a) * std::f64::consts::PI;
    let s = pillow_sin_f64(scaled_pix) / scaled_pix;
    sa * s
}

/// Hamming kernel.
fn kernel_hamming(x: f64) -> f64 {
    let x = x.abs();
    if x == 0.0 {
        // Resample.c has an exact-zero special case.  Values merely close to
        // zero still run through sin/cos; the float constants then leave a
        // visible residual (0.54f + 0.46f) in cancellation-sensitive F rows.
        return 1.0;
    }
    if x >= 1.0 {
        return 0.0;
    }
    // Pillow's Hamming resampler is a windowed sinc, not only the cosine
    // window. This mirrors the Hamming branch in Pillow's Resample.c and is
    // observable on downsampled impulses.
    let pix = std::f64::consts::PI * x;
    // Resample.c evaluates sin/cos together and contracts the Hamming
    // window's `0.46f * cos + 0.54f` expression.  Preserve both the
    // float-to-double constants and the fused operation; separating the
    // window terms changes cancellation rows by one f32 ULP.
    let (sin, cos) = pix.sin_cos();
    (sin / pix) * cos.mul_add(0.46_f32 as f64, 0.54_f32 as f64)
}

fn kernel_lanczos3(x: f64) -> f64 {
    kernel_lanczos(x, 3.0)
}

/// Choose kernel function and support based on filter type.
/// Returns the kernel and support used by the generic scalar resampler.
/// Backend adapters use this only to build the same scalar coefficient table;
/// pixel accumulation remains in the selected backend.
pub(crate) fn filter_from_resample(filter: ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (kernel_box, 0.5),
        ResampleFilter::Bilinear => (kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (kernel_lanczos3, 3.0),
        ResampleFilter::Box => (kernel_box, 0.5),
        ResampleFilter::Hamming => (kernel_hamming, 1.0),
    }
}

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to complete 16-tap vector blocks
// that round each product before the ordered additions; any tail remains
// scalar FMA. Its vertical path remains scalar FMA.
// Fractional-box F resizes use the same native resampler contract.
