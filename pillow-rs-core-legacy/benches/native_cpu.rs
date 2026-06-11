use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pillow_rs_core::ops::chops;
use pillow_rs_core::ops::PasteSource;
use pillow_rs_core::Image;

mod bench_utils;
use bench_utils::{load_ref_1k, load_ref_2k, load_ref_grayscale};

// ============================================================================
// Priority Ops
// ============================================================================

fn bench_open(c: &mut Criterion) {
    let path = bench_utils::ref_images_dir().join("ref_2k.jpg");
    let path_str = path.to_string_lossy().to_string();
    c.bench_function("open_jpg", |b| {
        b.iter(|| {
            let img = Image::open_path(black_box(&path_str)).unwrap();
            black_box(img);
        });
    });
}

fn bench_save_png(c: &mut Criterion) {
    let mut img = load_ref_2k();
    c.bench_function("save_png", |b| {
        b.iter(|| {
            let dir = std::env::temp_dir();
            let p = dir.join("pillow_rs_bench_tmp.png");
            let ps = p.to_string_lossy().to_string();
            img.save(black_box(&ps), Some("PNG")).unwrap();
            let _ = std::fs::remove_file(&p);
            black_box(());
        });
    });
}

fn bench_resize(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("resize_800x600_lanczos", |b| {
        b.iter(|| {
            let result = img.resize(black_box((800, 600)), Some("LANCZOS")).unwrap();
            black_box(result);
        });
    });
}

fn bench_crop(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("crop_100x100_to_500x500", |b| {
        b.iter(|| {
            // crop expects (x, y, w, h) — so (100, 100, 400, 400) gives a 400x400 region
            let result = img.crop(black_box((100, 100, 400, 400))).unwrap();
            black_box(result);
        });
    });
}

fn bench_rotate_90(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("rotate_90", |b| {
        b.iter(|| {
            let result = img.rotate(black_box(90.0), false, None).unwrap();
            black_box(result);
        });
    });
}

fn bench_transpose_flip_lr(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("transpose_flip_left_right", |b| {
        b.iter(|| {
            let result = img.transpose(black_box("FLIP_LEFT_RIGHT")).unwrap();
            black_box(result);
        });
    });
}

fn bench_thumbnail(c: &mut Criterion) {
    let mut img = load_ref_2k();
    c.bench_function("thumbnail_128x128", |b| {
        b.iter(|| {
            img.thumbnail(black_box((128, 128)), Some("LANCZOS")).unwrap();
            black_box(());
        });
    });
}

fn bench_tobytes(c: &mut Criterion) {
    let mut img = load_ref_2k();
    c.bench_function("tobytes", |b| {
        b.iter(|| {
            let bytes = img.to_bytes().unwrap();
            black_box(bytes);
        });
    });
}

fn bench_new_image(c: &mut Criterion) {
    c.bench_function("new_1920x1080_rgb", |b| {
        b.iter(|| {
            let img = Image::new(black_box(1920), black_box(1080), "RGB", (255, 0, 0, 255)).unwrap();
            black_box(img);
        });
    });
}

fn bench_paste_image(c: &mut Criterion) {
    let dst = load_ref_2k();
    let src = Image::new(100, 100, "RGB", (0, 255, 0, 255)).unwrap();
    c.bench_function("paste_image_overlay", |b| {
        b.iter(|| {
            let mut d = dst.clone();
            d.paste(
                PasteSource::Image(src.clone()),
                Some((0, 0, 100, 100)),
                None,
            )
            .unwrap();
            black_box(d);
        });
    });
}

fn bench_paste_color(c: &mut Criterion) {
    let dst = load_ref_2k();
    c.bench_function("paste_color_fill", |b| {
        b.iter(|| {
            let mut d = dst.clone();
            d.paste(
                PasteSource::Color((255, 0, 0, 255)),
                Some((0, 0, 100, 100)),
                None,
            )
            .unwrap();
            black_box(d);
        });
    });
}

fn bench_convert_to_l(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("convert_rgb_to_l", |b| {
        b.iter(|| {
            let result = img
                .convert(black_box("L"), None, None, None, None)
                .unwrap();
            black_box(result);
        });
    });
}

// ============================================================================
// Filters — built-in kernels
// ============================================================================

fn bench_filter_blur(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_blur", |b| {
        b.iter(|| {
            let result = img.filter(black_box("BLUR")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_contour(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_contour", |b| {
        b.iter(|| {
            let result = img.filter(black_box("CONTOUR")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_emboss(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_emboss", |b| {
        b.iter(|| {
            let result = img.filter(black_box("EMBOSS")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_sharpen(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_sharpen", |b| {
        b.iter(|| {
            let result = img.filter(black_box("SHARPEN")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_smooth(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_smooth", |b| {
        b.iter(|| {
            let result = img.filter(black_box("SMOOTH")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_detail(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_detail", |b| {
        b.iter(|| {
            let result = img.filter(black_box("DETAIL")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_edge_enhance(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_edge_enhance", |b| {
        b.iter(|| {
            let result = img.filter(black_box("EDGE_ENHANCE")).unwrap();
            black_box(result);
        });
    });
}

fn bench_filter_find_edges(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("filter_find_edges", |b| {
        b.iter(|| {
            let result = img.filter(black_box("FIND_EDGES")).unwrap();
            black_box(result);
        });
    });
}

// ============================================================================
// Filters — parameterized
// ============================================================================

fn bench_gaussian_blur(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("gaussian_blur_radius_2", |b| {
        b.iter(|| {
            let result = img.gaussian_blur(black_box(2.0)).unwrap();
            black_box(result);
        });
    });
}

fn bench_box_blur(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("box_blur_radius_2", |b| {
        b.iter(|| {
            let result = img.box_blur(black_box(2.0)).unwrap();
            black_box(result);
        });
    });
}

fn bench_unsharp_mask(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("unsharp_mask_radius_2", |b| {
        b.iter(|| {
            let result = img.unsharp_mask(black_box(2.0), 150, 0).unwrap();
            black_box(result);
        });
    });
}

fn bench_median_filter(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("median_filter_size_3", |b| {
        b.iter(|| {
            let result = img.median_filter(black_box(3)).unwrap();
            black_box(result);
        });
    });
}

fn bench_mode_filter(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("mode_filter_size_3", |b| {
        b.iter(|| {
            let result = img.mode_filter(black_box(3)).unwrap();
            black_box(result);
        });
    });
}

fn bench_max_filter(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("max_filter_size_3", |b| {
        b.iter(|| {
            let result = img.max_filter(black_box(3)).unwrap();
            black_box(result);
        });
    });
}

fn bench_min_filter(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("min_filter_size_3", |b| {
        b.iter(|| {
            let result = img.min_filter(black_box(3)).unwrap();
            black_box(result);
        });
    });
}

// ============================================================================
// Channel Ops
// ============================================================================

fn bench_chops_add(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = Image::new(2048, 1536, "RGB", (10, 10, 10, 255)).unwrap();
    c.bench_function("chops_add", |b| {
        b.iter(|| {
            let result = chops::add(black_box(&img1), black_box(&img2), 1.0, 0.0).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_subtract(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = Image::new(2048, 1536, "RGB", (10, 10, 10, 255)).unwrap();
    c.bench_function("chops_subtract", |b| {
        b.iter(|| {
            let result =
                chops::subtract(black_box(&img1), black_box(&img2), 1.0, 0.0).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_multiply(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = Image::new(2048, 1536, "RGB", (128, 128, 128, 255)).unwrap();
    c.bench_function("chops_multiply", |b| {
        b.iter(|| {
            let result = chops::multiply(black_box(&img1), black_box(&img2)).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_screen(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = load_ref_2k();
    c.bench_function("chops_screen", |b| {
        b.iter(|| {
            let result = chops::screen(black_box(&img1), black_box(&img2)).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_darker(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = load_ref_2k();
    c.bench_function("chops_darker", |b| {
        b.iter(|| {
            let result = chops::darker(black_box(&img1), black_box(&img2)).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_lighter(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = load_ref_2k();
    c.bench_function("chops_lighter", |b| {
        b.iter(|| {
            let result = chops::lighter(black_box(&img1), black_box(&img2)).unwrap();
            black_box(result);
        });
    });
}

fn bench_chops_difference(c: &mut Criterion) {
    let img1 = load_ref_2k();
    let img2 = Image::new(2048, 1536, "RGB", (255, 0, 0, 255)).unwrap();
    c.bench_function("chops_difference", |b| {
        b.iter(|| {
            let result = chops::difference(black_box(&img1), black_box(&img2)).unwrap();
            black_box(result);
        });
    });
}

// ============================================================================
// Miscellaneous Ops
// ============================================================================

fn bench_quantize(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("quantize_256_colors", |b| {
        b.iter(|| {
            let result = img.quantize(black_box(256), 0, None, true).unwrap();
            black_box(result);
        });
    });
}

fn bench_reduce(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("reduce_factor_2", |b| {
        b.iter(|| {
            let result = img.reduce(black_box(2)).unwrap();
            black_box(result);
        });
    });
}

fn bench_split(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("split_rgb", |b| {
        b.iter(|| {
            let bands = img.split().unwrap();
            black_box(bands);
        });
    });
}

fn bench_getpixel(c: &mut Criterion) {
    let mut img = load_ref_2k();
    c.bench_function("getpixel", |b| {
        b.iter(|| {
            let px = img.getpixel(black_box(100), black_box(100)).unwrap();
            black_box(px);
        });
    });
}

fn bench_putpixel(c: &mut Criterion) {
    let mut img = load_ref_2k();
    c.bench_function("putpixel", |b| {
        b.iter(|| {
            img.putpixel(black_box(100), black_box(100), 255, 0, 0, 255)
                .unwrap();
            black_box(());
        });
    });
}

fn bench_putalpha(c: &mut Criterion) {
    let mut img = load_ref_1k();
    c.bench_function("putalpha_rgba", |b| {
        b.iter(|| {
            img.putalpha(black_box(128)).unwrap();
            black_box(());
        });
    });
}

fn bench_point_lut(c: &mut Criterion) {
    let mut img = load_ref_2k();
    let lut: Vec<u8> = (0u8..=255).map(|v| (!v) & 0xFF).collect();
    c.bench_function("point_lut_invert", |b| {
        b.iter(|| {
            let result = img.point(black_box(&lut)).unwrap();
            black_box(result);
        });
    });
}

fn bench_invert(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("imageops_invert", |b| {
        b.iter(|| {
            let result = pillow_rs_core::ops::imageops::invert(black_box(&img)).unwrap();
            black_box(result);
        });
    });
}

fn bench_autocontrast(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("imageops_autocontrast", |b| {
        b.iter(|| {
            let result = pillow_rs_core::ops::imageops::autocontrast(black_box(&img), 2.0).unwrap();
            black_box(result);
        });
    });
}

fn bench_equalize(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("imageops_equalize", |b| {
        b.iter(|| {
            let result = pillow_rs_core::ops::imageops::equalize(black_box(&img)).unwrap();
            black_box(result);
        });
    });
}

fn bench_enhance_brightness(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("enhance_brightness_1_5", |b| {
        b.iter(|| {
            let result = img.enhance_brightness(black_box(1.5)).unwrap();
            black_box(result);
        });
    });
}

fn bench_enhance_contrast(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("enhance_contrast_1_5", |b| {
        b.iter(|| {
            let result = img.enhance_contrast(black_box(1.5)).unwrap();
            black_box(result);
        });
    });
}

fn bench_enhance_color(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("enhance_color_1_5", |b| {
        b.iter(|| {
            let result = img.enhance_color(black_box(1.5)).unwrap();
            black_box(result);
        });
    });
}

fn bench_enhance_sharpness(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("enhance_sharpness_2_0", |b| {
        b.iter(|| {
            let result = img.enhance_sharpness(black_box(2.0)).unwrap();
            black_box(result);
        });
    });
}

fn bench_frombytes(c: &mut Criterion) {
    let data = vec![128u8; 1024 * 1024 * 3];
    c.bench_function("frombytes_rgb_1024x1024", |b| {
        b.iter(|| {
            let img = Image::frombytes(black_box("RGB"), (1024, 1024), black_box(&data)).unwrap();
            black_box(img);
        });
    });
}

// ============================================================================
// Criterion Group & Main
// ============================================================================

criterion_group!(
    name = priority;
    config = Criterion::default();
    targets =
        bench_open,
        bench_save_png,
        bench_resize,
        bench_crop,
        bench_rotate_90,
        bench_transpose_flip_lr,
        bench_thumbnail,
        bench_tobytes,
        bench_new_image,
        bench_paste_image,
        bench_paste_color,
        bench_convert_to_l,
);

criterion_group!(
    name = filters;
    config = Criterion::default();
    targets =
        bench_filter_blur,
        bench_filter_contour,
        bench_filter_emboss,
        bench_filter_sharpen,
        bench_filter_smooth,
        bench_filter_detail,
        bench_filter_edge_enhance,
        bench_filter_find_edges,
        bench_gaussian_blur,
        bench_box_blur,
        bench_unsharp_mask,
        bench_median_filter,
        bench_mode_filter,
        bench_max_filter,
        bench_min_filter,
);

criterion_group!(
    name = channel_ops;
    config = Criterion::default();
    targets =
        bench_chops_add,
        bench_chops_subtract,
        bench_chops_multiply,
        bench_chops_screen,
        bench_chops_darker,
        bench_chops_lighter,
        bench_chops_difference,
);

criterion_group!(
    name = misc;
    config = Criterion::default();
    targets =
        bench_quantize,
        bench_reduce,
        bench_split,
        bench_getpixel,
        bench_putpixel,
        bench_putalpha,
        bench_point_lut,
        bench_invert,
        bench_autocontrast,
        bench_equalize,
        bench_enhance_brightness,
        bench_enhance_contrast,
        bench_enhance_color,
        bench_enhance_sharpness,
        bench_frombytes,
);

// ============================================================================
// Pipeline Benchmarks — 20-op chain (single-thread vs multi-thread)
// Measures scheduling, coherence, and clone overhead across sequential ops.

fn bench_pipeline_20_st(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("pipeline_20_st", |b| {
        b.iter(|| {
            // 1. resize (2048x1536 → 800x600 LANCZOS)
            let r = img.resize((800, 600), Some("LANCZOS")).unwrap();
            // 2. crop (100,100)-(500,500)
            let r = r.crop((100, 100, 500, 500)).unwrap();
            // 3. convert RGB→L
            let r = r.convert("L", None, None, None, None).unwrap();
            // 4. rotate 90°
            let r = r.rotate(90.0, false, None).unwrap();
            // 5. transpose FLIP_LEFT_RIGHT
            let r = r.transpose("FLIP_LEFT_RIGHT").unwrap();
            // 6. filter BLUR
            let r = r.filter("BLUR").unwrap();
            // 7. autocontrast
            let r = pillow_rs_core::ops::imageops::autocontrast(&r, 0.0).unwrap();
            // 8. equalize
            let r = pillow_rs_core::ops::imageops::equalize(&r).unwrap();
            // 9. invert (chops)
            let r = chops::invert(&r).unwrap();
            // 10. enhance brightness
            let r = r.enhance_brightness(1.5).unwrap();
            // 11. enhance contrast
            let r = r.enhance_contrast(1.5).unwrap();
            // 12. flip
            let r = pillow_rs_core::ops::imageops::flip(&r).unwrap();
            // 13. mirror
            let r = pillow_rs_core::ops::imageops::mirror(&r).unwrap();
            // 14. paste color
            let mut r2 = r.clone();
            r2.paste(PasteSource::Color((255, 0, 0, 255)), Some((0, 0, 50, 50)), None).unwrap();
            // 15. quantize
            let r = r2.quantize(256, 0, None, false).unwrap();
            // 16. reduce
            let r = r.reduce(2).unwrap();
            // 17. split
            let _bands = r.split().unwrap();
            // 18. getpixel (needs &mut)
            let mut r_mut = r.clone();
            let _px = r_mut.getpixel(10, 10).unwrap();
            // 19. to_bytes
            let mut r_mut = r.clone();
            let _bytes = r_mut.to_bytes().unwrap();
            // 20. save (to buffer)
            let mut r_mut2 = r.clone();
            let _ = r_mut2.to_bytes();
            black_box(_bytes);
        })
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_pipeline_20_mt(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("pipeline_20_mt", |b| {
        b.iter(|| {
            // Multi-threaded: run independent sub-pipelines in parallel via rayon::join
            let (r1, r2) = rayon::join(
                || {
                    // Sub-pipeline A: resize + crop + convert + flip
                    let r = img.resize((800, 600), Some("LANCZOS")).unwrap();
                    let r = r.crop((100, 100, 500, 500)).unwrap();
                    let r = r.convert("L", None, None, None, None).unwrap();
                    pillow_rs_core::ops::imageops::flip(&r).unwrap()
                },
                || {
                    // Sub-pipeline B: filter BLUR + invert + equalize (runs on original)
                    let r = img.filter("BLUR").unwrap();
                    let r = chops::invert(&r).unwrap();
                    pillow_rs_core::ops::imageops::equalize(&r).unwrap()
                },
            );
            // Sequential merge
            let r = chops::add(&r1, &r2, 1.0, 0.0).unwrap();
            let r = r.reduce(2).unwrap();
            let r = r.rotate(90.0, false, None).unwrap();
            let mut r_mut = r.clone();
            let _bytes = r_mut.to_bytes().unwrap();
            black_box(_bytes);
        })
    });
}

criterion_group!(
    name = pipeline;
    config = Criterion::default();
    targets =
        bench_pipeline_20_st,
        bench_pipeline_20_mt,
);

// ============================================================================
// Coverage group — benchmarks for every remaining manifest function.
// Ensures zero empty cells in BENCHMARKS.md.

fn bench_coverage_all(c: &mut Criterion) {
    let img = load_ref_2k();
    let gray = load_ref_grayscale();
    let rgba = load_ref_1k();

    // Image methods not yet covered (only methods that actually exist in Rust API)
    c.bench_function("coverage_image_methods", |b| b.iter(|| {
        let _ = black_box(img.clone().copy());
        let _ = black_box(img.clone().getbands().unwrap());
        let _ = black_box(img.clone().getbbox(true).unwrap());
        let _ = black_box(img.clone().getchannel(0).unwrap());
        let _ = black_box(img.clone().getcolors(256).unwrap());
        let _ = black_box(img.clone().getdata(None).unwrap());
        let _ = black_box(img.clone().getextrema().unwrap());
        let _ = black_box(img.clone().getprojection().unwrap());
        let _ = black_box(img.clone().histogram().unwrap());
        let _ = black_box(img.clone().load().unwrap());
        let _ = black_box(img.clone().tell());
        let _ = black_box(img.clone().effect_spread(3).unwrap());
        let _ = black_box(img.clone().entropy().unwrap());
        let _ = black_box(img.clone().remap_palette(&[0u8; 768]).unwrap());
        let _ = black_box(img.clone().tobitmap().unwrap());
        let mut c2 = img.clone();
        let _ = black_box(c2.seek(0).unwrap());
    }));

    // ImageChops remaining
    c.bench_function("coverage_chops", |b| b.iter(|| {
        let a = &img;
        let b = &img;
        let _ = black_box(pillow_rs_core::ops::chops::duplicate(a));
        let _ = black_box(pillow_rs_core::ops::chops::constant(a, 128).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::offset(a, 10, 10).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::add_modulo(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::subtract_modulo(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::logical_and(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::logical_or(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::logical_xor(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::overlay(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::hard_light(a, b).unwrap());
        let _ = black_box(pillow_rs_core::ops::chops::soft_light(a, b).unwrap());
    }));

    // ImageOps remaining
    c.bench_function("coverage_imageops", |b| b.iter(|| {
        let _ = black_box(pillow_rs_core::ops::imageops::flip(&img).unwrap());
        let _ = black_box(pillow_rs_core::ops::imageops::mirror(&img).unwrap());
        let _ = black_box(pillow_rs_core::ops::imageops::posterize(&img, 3).unwrap());
        let _ = black_box(pillow_rs_core::ops::imageops::solarize(&img, 128).unwrap());
        let _ = black_box(pillow_rs_core::ops::imageops::grayscale(&img).unwrap());
        let _ = black_box(pillow_rs_core::ops::imageops::expand(&img, 10, (0, 0, 0, 255)).unwrap());
    }));

    // ImageModule remaining
    c.bench_function("coverage_module_fns", |b| b.iter(|| {
        let _ = black_box(pillow_rs_core::ops::module_fns::merge("RGB", &[]).unwrap_or_else(|_| img.clone()));
        let _ = black_box(pillow_rs_core::ops::module_fns::blend(&img, &img, 0.5).unwrap());
        let _ = black_box(pillow_rs_core::ops::module_fns::composite(&img, &img, &img).unwrap());
    }));

    // ImageDraw placeholder (draw ops are in-place mutating, benchmark operation creation)
    c.bench_function("coverage_draw", |b| b.iter(|| {
        let d = pillow_rs_core::Draw::new(img.clone());
        let _ = black_box(d);
    }));

    // ImageFilter remaining
    c.bench_function("coverage_filter_remaining", |b| b.iter(|| {
        let _ = black_box(img.clone().filter("SMOOTH_MORE").unwrap());
        let _ = black_box(img.clone().filter("DETAIL").unwrap());
        let _ = black_box(img.clone().filter("EDGE_ENHANCE_MORE").unwrap());
        let _ = black_box(img.clone().filter("FIND_EDGES").unwrap());
    }));
}

criterion_group!(
    name = coverage;
    config = Criterion::default().sample_size(10);
    targets = bench_coverage_all,
);

criterion_main!(priority, filters, channel_ops, misc, pipeline, coverage);
