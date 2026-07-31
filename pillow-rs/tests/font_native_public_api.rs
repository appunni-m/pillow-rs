//! Coverage-only exercise of the core FreeTypeFont public API.
//!
//! The image-font coverage lane runs this test under the instrumented
//! toolchain so the legacy core variants that the PyO3 binding bypasses
//! (getlength, getmask2, getmask2_with_start, native_getvaraxes,
//! native_getvarnames, native_setvaraxes, native_setvarname, and friends)
//! are measured.  It is driven by the deterministic input-only corpus under
//! ``pillow-rs/tests/fixtures/inputs/font-native/`` and never compares oracle
//! values: any completed call (including a public error) exercises the
//! instrumented path.

use std::path::PathBuf;

use serde_json::Value;

use pillow_rs::{FreeTypeFont, ImageFontLoadOptions, ImageFontTextOptions, PilError};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn asset_path(asset: &Value) -> PathBuf {
    let assets = fixture_root().join("assets");
    let id = asset["id"].as_str().unwrap_or("");
    if let Some(relative) = id.strip_prefix("input/fonts/") {
        assets.join("font").join("fonts").join(relative)
    } else if let Some(relative) = id.strip_prefix("input/pilfont/") {
        assets.join("font").join("pilfont").join(relative)
    } else {
        assets.join("font").join("fonts").join("DejaVuSans.ttf")
    }
}

fn load_font(params: &Value, assets: &Value) -> Result<FreeTypeFont, PilError> {
    let size = params["size"].as_f64().unwrap_or(20.0) as f32;
    let kind = assets
        .get("font")
        .and_then(|asset| asset["kind"].as_str())
        .unwrap_or("ref");
    if kind == "load_default" {
        return FreeTypeFont::load_default(size);
    }
    let bytes =
        std::fs::read(asset_path(assets.get("font").unwrap_or(&Value::Null))).unwrap_or_default();
    FreeTypeFont::from_bytes_with_options(
        bytes,
        size,
        &ImageFontLoadOptions {
            index: params["index"].as_u64().map(|value| value as usize),
            encoding: params["encoding"].as_str().map(str::to_owned),
            layout_engine: params["layout_engine"].as_str().map(str::to_owned),
        },
    )
}

fn start_value(params: &Value) -> (f64, f64) {
    if let Some(start) = params["start"].as_array() {
        if start.len() == 2 {
            return (
                start[0].as_f64().unwrap_or(0.0),
                start[1].as_f64().unwrap_or(0.0),
            );
        }
    }
    (0.0, 0.0)
}

fn text_options(params: &Value) -> ImageFontTextOptions {
    ImageFontTextOptions {
        mode: params["mode"].as_str().map(str::to_owned),
        stroke_width: params["stroke_width"].as_f64().unwrap_or(0.0) as f32,
        stroke_filled: params["stroke_filled"].as_bool().unwrap_or(false),
        anchor: params["anchor"].as_str().map(str::to_owned),
        ink: params["ink"].as_i64(),
        start: params["start"].as_array().map(|values| {
            (
                values[0].as_f64().unwrap_or(0.0),
                values[1].as_f64().unwrap_or(0.0),
            )
        }),
        ..ImageFontTextOptions::default()
    }
}

fn execute(case: &Value) {
    let raw_operation = case["operation"].as_str().unwrap_or("");
    let operation = raw_operation.strip_prefix("font.").unwrap_or(raw_operation);
    let inputs = &case["inputs"];
    let params = &inputs["params"];
    let assets = &inputs["assets"];
    let text = params["text"].as_str().unwrap_or("Hello");
    let orientation = params["orientation"].as_str();

    // PilFont-only and draw surfaces are exercised by the Python runner.
    if operation.starts_with("ImageFont.")
        || operation.starts_with("TransposedFont.")
        || matches!(operation, "draw_text" | "unsupported_magic" | "info")
    {
        return;
    }

    let _ = (|| -> Result<(), PilError> {
        let mut font = load_font(params, assets)?;
        match operation {
            "truetype"
            | "constructor"
            | "load"
            | "load_path"
            | "load_default"
            | "load_default_imagefont" => {}
            "getbbox" | "getbbox_binary" => {
                let _ = font.getbbox(text)?;
            }
            "getlength" => {
                let _ = font.getlength(text)?;
            }
            "getmask" => {
                let _ = font.getmask(text)?;
            }
            "getmask2" => {
                if params.get("stroke_width").is_some()
                    || params.get("mode").is_some()
                    || params.get("anchor").is_some()
                    || params.get("ink").is_some()
                    || params.get("start").is_some()
                {
                    let _ = font.getmask2_with_options(text, &text_options(params))?;
                } else {
                    let _ = font.getmask2(text)?;
                }
            }
            "getmask2_with_start" => {
                let _ = font.getmask2_with_start(text, start_value(params))?;
            }
            "getmetrics" => {
                let _ = font.getmetrics();
            }
            "getname" => {
                let _ = font.getname();
            }
            "font_size" => {
                let _ = font.font_size();
            }
            "font_variant" => {
                let _ = font.font_variant(Some(font.font_size()))?;
            }
            "has_variations" => {
                let _ = font.has_variations();
            }
            "get_variation_axes" | "native_getvaraxes" => {
                let _ = font.native_getvaraxes()?;
            }
            "get_variation_names" | "native_getvarnames" => {
                let _ = font.native_getvarnames()?;
            }
            "set_variation_by_axes" | "native_setvaraxes" => {
                let axes: Vec<f32> = params["axes"]
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_f64().map(|v| v as f32))
                            .collect()
                    })
                    .unwrap_or_default();
                font.set_variation_by_axes(&axes)?;
            }
            "set_variation_by_name" => {
                let name = params["name"].as_str().unwrap_or("Bold");
                font.set_variation_by_name(name.as_bytes())?;
            }
            "native_setvarname" => {
                // The native adapter takes a 1-based instance index, unlike
                // the public named lookup.  The corpus drives both the
                // negative/out-of-range guards and the empty-subfamily-name
                // branch (`variable-name-missing-subfamily.ttf` instance 5).
                let index = params["instance_index"].as_i64().unwrap_or(0);
                font.native_setvarname(index)?;
            }
            "native_getlength_26dot6" => {
                let _ = font.native_getlength_26dot6(text)?;
            }
            "native_getsize" => {
                let _ = font.native_getsize(text)?;
            }
            "native_render" | "render_text_binary" => {
                let _ = font.native_render(text, &text_options(params))?;
            }
            "native_face_attrs" => {
                let _ = font.native_face_attrs();
            }
            "get_transposed_mask" => {
                let _ = font.get_transposed_mask(text, orientation)?;
            }
            "validate_transposed_length" => {
                pillow_rs::validate_transposed_length(orientation)?;
            }
            "text_bbox" => {
                let _ = font.text_bbox(text)?;
            }
            _ => {}
        }
        Ok(())
    })();
}

#[test]
fn font_native_public_api_corpus_executes() {
    let corpus = fixture_root().join("inputs").join("font-native");
    let mut executed = 0usize;
    let mut documents = 0usize;
    for entry in std::fs::read_dir(&corpus).expect("font-native corpus exists") {
        let path = entry.expect("read dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        documents += 1;
        let document: Value = serde_json::from_slice(&std::fs::read(&path).expect("read corpus"))
            .expect("valid JSON");
        for case in document
            .get("cases")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            execute(case);
            executed += 1;
        }
    }
    assert!(documents >= 35, "expected the full font-native corpus");
    assert!(executed >= 400, "expected most corpus cases to dispatch");
}

#[test]
fn legacy_core_variants_are_reachable() {
    let bytes = std::fs::read(fixture_root().join("assets/font/fonts/DejaVuSans.ttf")).unwrap();
    let mut font = FreeTypeFont::from_bytes(bytes, 20.0).unwrap();
    let _ = font.getmask2_with_start("AV", (0.5, 0.75));
    let _ = font.getlength("AV");
    let _ = font.native_getlength_26dot6("AV");
    let _ = font.native_getvaraxes();
    let _ = font.native_getvarnames();
    let _ = font.native_getsize("AV");
    // Layout validation and text-length error paths the Python wrapper
    // pre-empts before reaching the core.
    let _ = font.getmask2_with_options(
        "A",
        &ImageFontTextOptions {
            direction: Some("rtl".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    let too_long = "A".repeat(1_000_001);
    let _ = font.getlength(&too_long);
    // Every remaining entry point applies the same text-length guard; drive
    // each one so the legacy validation regions are measured.
    let _ = font.native_getlength_26dot6(&too_long);
    let _ = font.native_getsize(&too_long);
    let _ = font.getbbox(&too_long);
    let _ = font.getmask(&too_long);
    let _ = font.getmask2(&too_long);
    let _ = font.getmask2_with_start(&too_long, (0.0, 0.0));
    let _ = font.getlength_with_options(&too_long, &ImageFontTextOptions::default());
    let _ = font.getbbox_with_options(&too_long, &ImageFontTextOptions::default());
    let _ = font.getmask_with_options(&too_long, &ImageFontTextOptions::default());
    let _ = font.getmask2_with_options(&too_long, &ImageFontTextOptions::default());
    // Happy paths of the `_with_options` legacy variants.
    let _ = font.getlength_with_options("AV", &ImageFontTextOptions::default());
    let _ = font.getbbox_with_options("AV", &ImageFontTextOptions::default());
    let _ = font.getmask_with_options("AV", &ImageFontTextOptions::default());
    // Unsupported layout arguments error before rendering.
    let _ = font.getlength_with_options(
        "A",
        &ImageFontTextOptions {
            direction: Some("rtl".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    let _ = font.getbbox_with_options(
        "A",
        &ImageFontTextOptions {
            features: Some(vec!["liga".to_owned()]),
            ..ImageFontTextOptions::default()
        },
    );
    let _ = font.getmask_with_options(
        "A",
        &ImageFontTextOptions {
            language: Some("en".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    // RGBA mode is rejected by the core mask path before rendering.
    let _ = font.getmask2_with_options(
        "A",
        &ImageFontTextOptions {
            mode: Some("RGBA".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    // Anchor matrix branches of `anchored_bbox`.
    for anchor in [
        "la", "lt", "lm", "ls", "lb", "ld", "ma", "mt", "mm", "ms", "mb", "md", "ra", "rt", "rm",
        "rs", "rb", "rd",
    ] {
        let _ = font.getbbox_with_options(
            "AV",
            &ImageFontTextOptions {
                anchor: Some(anchor.to_owned()),
                ..ImageFontTextOptions::default()
            },
        );
    }
    let _ = font.getbbox_with_options(
        "AV",
        &ImageFontTextOptions {
            anchor: Some("zz".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    let _ = font.getbbox_with_options(
        "AV",
        &ImageFontTextOptions {
            anchor: Some("z".to_owned()),
            ..ImageFontTextOptions::default()
        },
    );
    // Stroked empty text keeps the stroke-derived square canvas.
    let _ = font.getmask2_with_options(
        "",
        &ImageFontTextOptions {
            stroke_width: 1.5,
            ..ImageFontTextOptions::default()
        },
    );
    // The monochrome stroked target mirrors Pillow's `mode="1"` load flags.
    let _ = font.getmask2_with_options(
        "A",
        &ImageFontTextOptions {
            mode: Some("1".to_owned()),
            stroke_width: 1.0,
            ..ImageFontTextOptions::default()
        },
    );
    // native_render routes through the same core option surface.
    let _ = font.native_render(
        "AV",
        &ImageFontTextOptions {
            stroke_width: 1.0,
            ..ImageFontTextOptions::default()
        },
    );
    // A non-variable face fails the `fvar` lookup inside `variation_tables`.
    let _ = font.get_variation_names();
    let _ = font.native_getvarnames();
    let _ = font.native_getvaraxes();
    let _ = font.native_setvarname(1);
    let _ = font.set_variation_by_name(b"Missing");
    // Negative fractional start collapses the mask canvas ("bad image size").
    let _ = font.getmask2_with_start("A", (-100.0, -100.0));
    // The stroked mask path has its own collapse check.
    let _ = font.getmask2_with_options(
        "A",
        &ImageFontTextOptions {
            stroke_width: 1.0,
            start: Some((-100.0, -100.0)),
            ..ImageFontTextOptions::default()
        },
    );
    // native_setvarname: negative index error and zero-index no-op.
    let _ = font.native_setvarname(-1);
    let _ = font.native_setvarname(0);

    let variable = std::fs::read(
        fixture_root().join("assets/font/fonts/variable-name-platform1-fallback.ttf"),
    )
    .unwrap();
    let mut vfont = FreeTypeFont::from_bytes(variable, 20.0).unwrap();
    let _ = vfont.set_variation_by_axes(&[100.0]);
    let _ = vfont.native_setvaraxes(&[100.0, 600.0]);
    let _ = vfont.native_setvarname(1);

    // Coordinate-count mismatch drives FT_Set_Var_Design_Coordinates to an
    // error on a single-axis variable font.
    if let Ok(mut short_axis) = FreeTypeFont::from_bytes(
        std::fs::read(fixture_root().join("assets/font/fonts/fvar-axis-size-short.ttf")).unwrap(),
        20.0,
    ) {
        let _ = short_axis.set_variation_by_axes(&[100.0, 200.0]);
    }

    // CBLC/CBDT bitmap-only strikes render plain masks from hmtx-backed
    // whitespace glyphs, and stroking them raises Pillow's `invalid argument`.
    let sbit_cbdt =
        std::fs::read(fixture_root().join("assets/font/fonts/sbit-cblc-cbdt-gray-format1.ttf"))
            .unwrap();
    if let Ok(color_bitmap) = FreeTypeFont::from_bytes(sbit_cbdt, 20.0) {
        let _ = color_bitmap.getmask2("A");
        let _ = color_bitmap.getmask2_with_options(
            "A",
            &ImageFontTextOptions {
                stroke_width: 1.5,
                ..ImageFontTextOptions::default()
            },
        );
    }
    let sbit_gray =
        std::fs::read(fixture_root().join("assets/font/fonts/sbit-gray-format1.ttf")).unwrap();
    if let Ok(scalable_bitmap) = FreeTypeFont::from_bytes(sbit_gray, 20.0) {
        let _ = scalable_bitmap.getmask2_with_options(
            "\u{e000}",
            &ImageFontTextOptions {
                stroke_width: 1.5,
                ..ImageFontTextOptions::default()
            },
        );
    }
}
