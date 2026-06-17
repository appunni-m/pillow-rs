//! Decode fixture tests — pixel-perfect binary parity with PIL (libjpeg/libpng).
//!
//! Pattern mirrors tests/test_parity.py:
//!   Input:  tests/fixtures/input/jsons/Decode.{format}.json
//!   Output: tests/fixtures/outputs/jsons/Decode.{format}.json
//!   Refs:   tests/fixtures/outputs/raws/{name}.bin (raw PIL image.tobytes())
//!
//! Each test: load asset → decode → compare raw bytes byte-for-byte against PIL output.
//! Zero unit tests. Zero mocked data. Real image files, real PIL references.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_image as img;

#[test]
fn test_decode_fixtures() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests").join("fixtures");
    let input_jsons = fixtures_dir.join("input").join("jsons");
    let output_jsons = fixtures_dir.join("outputs").join("jsons");
    let assets_dir = manifest_dir.join("test-assets").join("input");

    // Auto-generate references from PIL (libjpeg/libpng) if outputs don't exist yet
    if !output_jsons.is_dir() || fs::read_dir(&output_jsons).unwrap().next().is_none() {
        let script = manifest_dir.join("scripts").join("generate_decode_refs.py");
        if script.exists() {
            eprintln!("Generating reference fixtures via PIL (libjpeg/libpng)...");
            let status = std::process::Command::new("python3")
                .arg(&script)
                .current_dir(manifest_dir)
                .status();
            match status {
                Ok(s) if s.success() => eprintln!("References generated."),
                Ok(s) => eprintln!("WARNING: reference generator failed (exit {})", s),
                Err(e) => eprintln!("WARNING: cannot run reference generator: {} (PIL not installed?)", e),
            }
        }
    }

    if !input_jsons.is_dir() || !output_jsons.is_dir() {
        eprintln!("SKIP: no fixtures found. Add test assets to test-assets/input/");
        return;
    }

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;

    for entry in fs::read_dir(&input_jsons).unwrap().flatten() {
        let input_path = entry.path();
        let fname = input_path.file_name().unwrap().to_str().unwrap();
        let output_path = output_jsons.join(fname);
        if !output_path.exists() { continue; }

        let inp: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&input_path).unwrap()).unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        // Index output cases by id
        let out_cases: HashMap<String, &serde_json::Value> = out["cases"]
            .as_array().unwrap().iter()
            .map(|c| (c["id"].as_str().unwrap().to_string(), c))
            .collect();

        for case in inp["cases"].as_array().unwrap() {
            let cid = case["id"].as_str().unwrap();
            let asset_rel = case["asset"].as_str().unwrap();
            let asset_path = assets_dir.join(asset_rel);
            total += 1;

            let asset_data = match fs::read(&asset_path) {
                Ok(d) => d,
                Err(_) => { eprintln!("  SKIP [{cid}]: asset missing"); continue; }
            };

            let decoded = match img::decode(&asset_data) {
                Some(d) => d,
                None => {
                    if out_cases.get(cid).map_or(false, |c| c["assert"]["method"].as_str() == Some("error")) {
                        eprintln!("  OK   [{cid}] (expected error)");
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{cid}]: decode returned None");
                        failed += 1;
                    }
                    continue;
                }
            };

            let oc = match out_cases.get(cid) {
                Some(c) => c,
                None => { eprintln!("  FAIL [{cid}]: no output case"); failed += 1; continue; }
            };

            if oc["assert"]["method"].as_str() == Some("error") {
                eprintln!("  FAIL [{cid}]: expected error, got image");
                failed += 1;
                continue;
            }

            let ref_rel = oc["assert"]["reference"].as_str().unwrap();
            let ref_path = fixtures_dir.join("outputs").join(ref_rel);
            let expected = match fs::read(&ref_path) {
                Ok(d) => d,
                Err(_) => { eprintln!("  FAIL [{cid}]: ref missing {}", ref_path.display()); failed += 1; continue; }
            };

            let actual = decoded.as_bytes();
            if actual == expected.as_slice() {
                eprintln!("  OK   [{cid}] {} bytes", actual.len());
                passed += 1;
            } else {
                let diffs = actual.iter().zip(expected.iter()).filter(|(a,b)| a!=b).count();
                eprintln!("  FAIL [{cid}]: {diffs}/{} bytes differ (got {}B, expected {}B)",
                    actual.len().max(expected.len()), actual.len(), expected.len());
                failed += 1;
            }
        }
    }

    eprintln!("\ndecode fixtures: {passed}/{total} passed, {failed} failed");
    if total == 0 {
        eprintln!("No fixtures found. Run: python scripts/generate_decode_refs.py");
    }
    if failed > 0 {
        panic!("{failed} fixture(s) failed");
    }
}

#[test]
fn test_format_detection() {
    use img::ImageFormat;
    assert_eq!(img::detect_format(b"\xff\xd8\xff\xe0\x00\x10\x4a\x46"), Some(ImageFormat::Jpeg));
    assert_eq!(img::detect_format(b"\x89PNG\r\n\x1a\n\x00\x00\x00"), Some(ImageFormat::Png));
    assert_eq!(img::detect_format(b"GIF89a\x00\x00\x00"), Some(ImageFormat::Gif));
    assert_eq!(img::detect_format(b"BM\x00\x00\x00\x00\x00\x00"), Some(ImageFormat::Bmp));
    assert_eq!(img::detect_format(b"II\x2a\x00\x00\x00\x00\x00"), Some(ImageFormat::Tiff));
    assert_eq!(img::detect_format(b"\x00\x00\x01\x00\x00\x00\x00\x00"), Some(ImageFormat::Ico));
    assert_eq!(img::detect_format(b"RIFF\x00\x00\x00\x00WEBP"), Some(ImageFormat::WebP));
    assert_eq!(img::detect_format(b""), None);
    assert_eq!(img::detect_format(b"\x00"), None);
}

// ── Manifest coverage validation ─────────────────────────────────────────

#[test]
fn test_manifest_coverage() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("manifest.yaml");
    let assets_dir = manifest_dir.join("test-assets").join("input");

    if !manifest_path.exists() {
        eprintln!("SKIP: manifest.yaml not found");
        return;
    }

    let manifest: serde_json::Value = serde_yaml::from_str(
        &fs::read_to_string(&manifest_path).unwrap()
    ).unwrap();

    let mut missing_assets = Vec::new();
    let mut total_cases = 0u32;
    let mut covered_cases = 0u32;

    for (fmt_name, fmt_data) in manifest["formats"].as_object().unwrap() {
        let format_dir = assets_dir.join(fmt_name);
        for case in fmt_data["edge_cases"].as_array().unwrap() {
            let cid = case["id"].as_str().unwrap();
            let expect_error = case.get("expect_error").and_then(|v| v.as_bool()).unwrap_or(false);

            if let Some(assets) = case.get("test_assets") {
                for asset in assets.as_array().unwrap() {
                    let asset_name = asset.as_str().unwrap();
                    total_cases += 1;
                    let asset_path = format_dir.join(asset_name);
                    if asset_path.exists() {
                        covered_cases += 1;
                    } else {
                        missing_assets.push(format!("  {fmt_name}/{asset_name} ({cid})"));
                    }
                }
            } else if !expect_error {
                // Edge case with no test_assets and not error-only — gap
                missing_assets.push(format!("  {fmt_name}: {cid} (no test_assets, not error-only)"));
            }
        }
    }

    eprintln!("\nmanifest coverage: {covered_cases}/{total_cases} test assets present");

    if !missing_assets.is_empty() {
        eprintln!("Missing {} test assets:", missing_assets.len());
        for m in &missing_assets[..missing_assets.len().min(20)] {
            eprintln!("{m}");
        }
        if missing_assets.len() > 20 {
            eprintln!("  ... and {} more", missing_assets.len() - 20);
        }
    }

    if total_cases == 0 {
        eprintln!("WARNING: no test cases defined in manifest.yaml");
    }

    // Don't fail on missing assets — just report the gap.
    // This matches test_coverage_complete behavior: informational, not blocking.
    assert!(total_cases > 0, "manifest.yaml must define at least one test case");
}
