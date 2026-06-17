fn main() {
    // Only link to libwebp on non-WASM targets
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch != "wasm32" {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-search=native={}/vendor/lib", manifest_dir);
        println!("cargo:rustc-link-lib=dylib=webp");
    }
}
