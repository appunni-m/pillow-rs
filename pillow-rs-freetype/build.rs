//! Build script for pillow-rs-freetype.
//!
//! This crate is a **pure-Rust port** of FreeType 2.14.1. The vendored C
//! source in `freetype/` (gitignored, cloned from
//! <https://github.com/freetype/freetype> at tag `VER-2-14-1>) is used only as
//! a read-only algorithmic reference while porting — it is **never compiled
//! or linked**.
//!
//! This script merely verifies the reference is present so porting work can
//! cross-check against it, and emits the current reference version.

const EXPECTED_TAG: &str = "VER-2-14-1";

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let freetype_root = std::path::Path::new(&manifest_dir).join("freetype");

    println!("cargo:rerun-if-changed=build.rs");

    if !freetype_root.join("src").exists() {
        println!(
            "cargo:warning=pillow-rs-freetype: FreeType reference source not found at '{}'.",
            freetype_root.display()
        );
        println!("cargo:warning=  (Optional) clone it for cross-checking during the port:");
        println!(
            "cargo:warning=    git clone --depth 1 --branch {EXPECTED_TAG} https://github.com/freetype/freetype.git pillow-rs-freetype/freetype/"
        );
        println!("cargo:warning=  The Rust port compiles and runs without it; this is reference-only.");
        return;
    }

    // Record the reference version the port targets.
    println!("cargo:rustc-env=FREETYPE_REF_TAG={EXPECTED_TAG}");
    println!("cargo:rustc-env=FREETYPE_REF_PATH={}", freetype_root.display());
}
