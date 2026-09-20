//! Coverage driver for font methods that have no Python binding.
//!
//! The maintained Python runner supplies validated arguments and sends text
//! over stdin (including oversized boundary inputs). API errors are observations;
//! malformed driver arguments and process failures are infrastructure failures.

// The example uses the core library; its transitive dependencies are not used directly.
#![allow(unused_crate_dependencies)]

use pillow_rs::{FreeTypeFont, PilError};
use std::io::Read;

fn execute(
    operation: &str,
    font: &FreeTypeFont,
    text: &[u8],
    text_kind: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<String, PilError> {
    // UTF-8 is validated at the driver boundary, before entering an API call.
    let unicode = String::from_utf8_lossy(text);
    match (operation, text_kind) {
        ("text_bbox", "bytes") => font.text_bbox_bytes(text).map(|value| format!("{value:?}")),
        ("text_bbox", _) => font.text_bbox(&unicode).map(|value| format!("{value:?}")),
        ("getbbox_binary", "bytes") => font
            .getbbox_binary_bytes(text)
            .map(|value| format!("{value:?}")),
        ("getbbox_binary", _) => font
            .getbbox_binary(&unicode)
            .map(|value| format!("{value:?}")),
        ("render_text_binary", _) => font
            .render_text_binary(&unicode, fill, spacing)
            .map(|value| format!("{value:?}")),
        _ => unreachable!("operation is validated before loading a font"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err("expected operation, font path, size, text kind, RGBA fill, spacing".into());
    }
    let operation = args[0].as_str();
    if !matches!(
        operation,
        "text_bbox" | "getbbox_binary" | "render_text_binary"
    ) {
        return Err("unknown font operation".into());
    }
    let size = args[2].parse::<f32>()?;
    let text_kind = args[3].as_str();
    if !matches!(text_kind, "utf8" | "bytes")
        || (operation == "render_text_binary" && text_kind != "utf8")
    {
        return Err("invalid text kind for operation".into());
    }
    let channels: Vec<u8> = args[4]
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let [r, g, b, a] = channels.as_slice() else {
        return Err("fill must contain four u8 channels".into());
    };
    let spacing = args[5].parse::<f32>()?;
    let mut text = Vec::new();
    std::io::stdin().read_to_end(&mut text)?;
    if text_kind == "utf8" {
        std::str::from_utf8(&text)?;
    }
    let font = if args[1] == "load_default" {
        FreeTypeFont::load_default(size)
    } else {
        // Missing input files are driver failures, not successful API coverage.
        FreeTypeFont::from_bytes(std::fs::read(&args[1])?, size)
    };
    match font.and_then(|font| {
        execute(
            operation,
            &font,
            &text,
            text_kind,
            (*r, *g, *b, *a),
            spacing,
        )
    }) {
        Ok(value) => println!("returned\t{value}"),
        Err(error) => println!("raised\t{error:?}"),
    }
    Ok(())
}
