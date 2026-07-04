//! Build script for pillow-rs-freetype.
//!
//! This crate is a **pure-Rust port** of FreeType 2.14.3. The vendored C
//! source in `freetype/` is used only as a read-only algorithmic reference
//! while porting — it is **never compiled or linked**.

const EXPECTED_TAG: &str = "VER-2-14-3";

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let freetype_root = std::path::Path::new(&manifest_dir).join("freetype");

    println!("cargo:rerun-if-changed=build.rs");

    if !freetype_root.join("src").exists() {
        println!("cargo:warning=pillow-rs-freetype: FreeType reference source not found.");
        println!("cargo:warning=  Clone for cross-checking during porting:");
        println!("cargo:warning=    git clone --depth 1 --branch {EXPECTED_TAG} https://github.com/freetype/freetype.git pillow-rs-freetype/freetype/");
        return;
    }

    println!("cargo:rustc-env=FREETYPE_REF_TAG={EXPECTED_TAG}");
    println!(
        "cargo:rustc-env=FREETYPE_REF_PATH={}",
        freetype_root.display()
    );
}
