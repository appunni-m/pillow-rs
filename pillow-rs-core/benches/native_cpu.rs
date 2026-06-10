use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pillow_rs_core::ops::chops;
use pillow_rs_core::ops::PasteSource;
use pillow_rs_core::Image;

mod bench_utils;
use bench_utils::{load_ref_1k, load_ref_2k};

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

criterion_main!(priority, filters, channel_ops, misc);
