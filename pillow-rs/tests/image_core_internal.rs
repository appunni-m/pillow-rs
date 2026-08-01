//! Coverage-only exercise of image-core internal paths that the PIL parity
//! surface cannot reach (backend locking, P/PA pipeline materialization).
//!
//! The image-core coverage lane runs this test under the instrumented
//! toolchain so the compute-lock routing and palette-pipeline materialization
//! branches are measured. It never compares oracle values: any completed call
//! exercises the instrumented path.

use pillow_rs::{Backend, Image, PasteSource};

fn rgb() -> Image {
    Image::new(8, 8, "RGB", (10, 20, 30, 255)).expect("new")
}

fn multi_color_rgb() -> Image {
    let mut data = Vec::with_capacity(8 * 8 * 3);
    for i in 0..64u16 {
        data.extend_from_slice(&[(i * 7) as u8, (i * 13) as u8, (i * 29) as u8]);
    }
    Image::frombytes("RGB", (8, 8), &data).expect("frombytes")
}

#[test]
fn backend_locks_cover_every_op_family() {
    let a = rgb();
    let b = rgb();
    let mut images: Vec<Image> = vec![
        pillow_rs::chops_add(&a, &b, 1.0, 0.0).expect("add"),
        pillow_rs::chops_subtract(&a, &b, 1.0, 0.0).expect("subtract"),
        pillow_rs::chops_multiply(&a, &b).expect("multiply"),
        pillow_rs::chops_screen(&a, &b).expect("screen"),
        pillow_rs::chops_darker(&a, &b).expect("darker"),
        pillow_rs::chops_lighter(&a, &b).expect("lighter"),
        pillow_rs::chops_difference(&a, &b).expect("difference"),
        pillow_rs::chops_overlay(&a, &b).expect("overlay"),
        pillow_rs::chops_hard_light(&a, &b).expect("hard_light"),
        pillow_rs::chops_soft_light(&a, &b).expect("soft_light"),
        pillow_rs::chops_add_modulo(&a, &b).expect("add_modulo"),
        pillow_rs::chops_subtract_modulo(&a, &b).expect("subtract_modulo"),
        pillow_rs::chops_logical_and(&a, &b).expect("logical_and"),
        pillow_rs::chops_logical_or(&a, &b).expect("logical_or"),
        pillow_rs::chops_logical_xor(&a, &b).expect("logical_xor"),
        pillow_rs::image_blend(&a, &b, 0.5).expect("blend"),
        pillow_rs::image_composite(
            &a,
            &b,
            &Image::new(8, 8, "L", (128, 0, 0, 0)).expect("mask"),
        )
        .expect("composite"),
        pillow_rs::image_merge("RGB", &[a.clone(), b.clone(), b.clone()]).expect("merge"),
        rgb().resize((4, 4), None).expect("resize"),
        rgb().crop_box(0, 0, 4, 4).expect("crop"),
    ];
    let mut pasted = rgb();
    pasted
        .paste(
            PasteSource::Image(Image::new(4, 4, "RGB", (1, 2, 3, 255)).expect("source")),
            Some((0, 0, 4, 4)),
            None,
        )
        .expect("paste");
    images.push(pasted);
    let mut composited = Image::new(8, 8, "RGBA", (10, 20, 30, 255)).expect("rgba");
    composited
        .alpha_composite(
            &Image::new(8, 8, "RGBA", (200, 100, 50, 128)).expect("src"),
            (0, 0),
            (0, 0),
        )
        .expect("alpha_composite");
    images.push(composited);
    for image in images {
        let locked = image.use_backend(Backend::Cpu);
        let _ = locked.materialize().expect("materialize");
    }
}

#[test]
fn p_and_pa_pipelines_materialize() {
    let source = multi_color_rgb();
    let quantized = source.quantize(16, 0, None, true, 1).expect("quantize");
    let _ = quantized.materialize().expect("materialize");
    let _ = quantized
        .materialize_indices()
        .expect("materialize_indices");
    let _ = quantized
        .materialize_for_ops()
        .expect("materialize_for_ops");
    let mut pa = quantized;
    pa.putalpha(255).expect("putalpha");
    let _ = pa.materialize().expect("pa materialize");
}

#[test]
fn constructor_and_mode_arms() {
    // P tuple color allocates palette entry zero (Image::new "P" arm).
    let p = Image::new(4, 4, "P", (255, 0, 0, 255)).expect("new p tuple");
    let _ = p.materialize().expect("materialize p");
    // Bilevel and 4-band construction paths.
    let _ = Image::new(4, 4, "1", (1, 0, 0, 0)).expect("new 1");
    let _ = Image::new(4, 4, "LA", (100, 0, 0, 128)).expect("new la");
    let _ = Image::new(4, 4, "PA", (100, 0, 0, 128)).expect("new pa");
    // frombytes per-mode arms, including the packed "1" layout.
    let packed = vec![0b1010_1010u8; 4];
    let _ = Image::frombytes("1", (4, 4), &packed).expect("frombytes 1");
    let _ = Image::frombytes("CMYK", (2, 2), &[0u8; 16]).expect("frombytes cmyk");
    let _ = Image::frombytes("I", (2, 2), &[0u8; 16]).expect("frombytes i");
    let _ = Image::frombytes("F", (2, 2), &[0u8; 16]).expect("frombytes f");
    assert!(Image::frombytes("PA", (2, 2), &[0u8; 8]).is_err());
}
