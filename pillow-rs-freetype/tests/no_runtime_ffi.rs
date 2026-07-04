#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]
#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn runtime_crate_contains_no_ffi_or_native_build_hooks() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();

    for path in runtime_files(manifest_dir) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        for forbidden in [
            "extern \"C\"",
            "native_ft",
            "freetype-sys",
            "bindgen",
            "pkg-config",
            "cc::",
            "rustc-link-lib=freetype",
            "rustc-link-lib=static",
        ] {
            if text.contains(forbidden) {
                violations.push(format!("{} contains {forbidden}", path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "runtime FFI boundary violations:\n{}",
        violations.join("\n")
    );
}

fn runtime_files(manifest_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files.push(manifest_dir.join("Cargo.toml"));
    let build_rs = manifest_dir.join("build.rs");
    if build_rs.exists() {
        files.push(build_rs);
    }
    collect_rs_files(&manifest_dir.join("src"), &mut files);
    files
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
