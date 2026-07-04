#![allow(missing_docs)]

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/native_ft.c");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let obj = out_dir.join("native_ft.o");
    let lib = out_dir.join("libnative_ft.a");

    let cflags = Command::new("pkg-config")
        .args(["--cflags", "freetype2"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_else(|| "-I/usr/include/freetype2".to_string());

    let mut cc = Command::new("cc");
    cc.arg("-c").arg("src/native_ft.c").arg("-o").arg(&obj);
    for flag in cflags.split_whitespace() {
        cc.arg(flag);
    }
    let status = cc.status().expect("compile native_ft.c");
    assert!(status.success(), "failed to compile native_ft.c");

    let status = Command::new("ar")
        .arg("crs")
        .arg(&lib)
        .arg(&obj)
        .status()
        .expect("archive native_ft.o");
    assert!(status.success(), "failed to archive native_ft.o");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=native_ft");
    println!("cargo:rustc-link-lib=freetype");
}
