use pillow_rs_core::Image;
use std::path::PathBuf;

/// Get path to reference images directory
pub fn ref_images_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../scripts/bench_reference_images");
    p
}

/// Load ref_2k.jpg as an Image (2048x1536 RGB)
pub fn load_ref_2k() -> Image {
    let path = ref_images_dir().join("ref_2k.jpg");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_2k.jpg")
}

/// Load ref_1k.png as an Image (1024x1024 RGBA)
pub fn load_ref_1k() -> Image {
    let path = ref_images_dir().join("ref_1k.png");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_1k.png")
}

/// Load ref_grayscale.png as an Image (1024x1024 L)
#[allow(dead_code)]
pub fn load_ref_grayscale() -> Image {
    let path = ref_images_dir().join("ref_grayscale.png");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_grayscale.png")
}
