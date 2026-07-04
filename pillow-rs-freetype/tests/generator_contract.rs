#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
#![allow(unused_crate_dependencies)]

use std::fs;
use std::path::{Path, PathBuf};

use env_logger as _;
use log as _;
use pillow_rs_freetype as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use thiserror as _;

const GENERATORS: &[&str] = &[
    "scripts/build_ft.sh",
    "scripts/gen_ft_refs.c",
    "scripts/build_ft_fixture.py",
    "scripts/build_native_tt_fixture.py",
    "scripts/build_render_mode_fixture.py",
    "scripts/build_fixtures.py",
    "scripts/extract_blues.py",
    "scripts/generate_globals.py",
    "scripts/generate_script_meta.py",
];

const FIXTURE_FAMILIES: &[&str] = &[
    "native_tt_default",
    "force_autohint",
    "no_hinting",
    "metrics_only",
    "outline_cbox",
    "render_mono",
    "render_lcd",
];

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read_project_file(path: &str) -> String {
    fs::read_to_string(manifest_dir().join(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

#[test]
fn maintained_generators_are_documented() {
    let doc = read_project_file("doc/GENERATOR_SYSTEM.md");
    assert!(
        doc.contains("Fixture generation is part of the `pillow-rs-freetype` harness"),
        "generator system doc must state that generation is part of the harness"
    );
    assert!(
        doc.contains("Ad hoc one-off scripts must not be required"),
        "generator system doc must reject ad hoc fixture generation"
    );

    for generator in GENERATORS {
        let path = manifest_dir().join(generator);
        assert!(path.exists(), "{generator} is missing");
        assert!(
            doc.contains(generator),
            "{generator} is not documented in doc/GENERATOR_SYSTEM.md"
        );
    }

    let references = read_project_file("doc/REFERENCES.md");
    assert!(
        references.contains("doc/GENERATOR_SYSTEM.md"),
        "REFERENCES.md must point fixture updates to the generator system doc"
    );
}

#[test]
fn main_fixture_generator_registers_every_fixture_family() {
    let script = read_project_file("scripts/build_ft_fixture.py");
    let c_oracle = read_project_file("scripts/gen_ft_refs.c");
    let doc = read_project_file("doc/GENERATOR_SYSTEM.md");

    for family in FIXTURE_FAMILIES {
        assert!(
            script.contains(family),
            "scripts/build_ft_fixture.py does not register {family}"
        );
        assert!(
            c_oracle.contains(family),
            "scripts/gen_ft_refs.c does not implement {family}"
        );
        assert!(
            doc.contains(family),
            "doc/GENERATOR_SYSTEM.md does not document {family}"
        );
    }
}

#[test]
fn committed_generator_tree_has_no_python_bytecode() {
    let scripts_dir = manifest_dir().join("scripts");
    let mut offenders = Vec::new();
    collect_python_bytecode(&scripts_dir, &mut offenders);
    assert!(
        offenders.is_empty(),
        "generated Python bytecode does not belong in the maintained generator tree: {offenders:?}"
    );
}

fn collect_python_bytecode(dir: &Path, offenders: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_python_bytecode(&path, offenders);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "pyc") {
            offenders.push(path);
        }
    }
}
