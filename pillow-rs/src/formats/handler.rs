// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   The FormatHandler trait is the ONE way to add image format support.
//   Before this trait existed, adding a new format required touching 4+
//   separate match statements across image.rs and format.rs. Now it's a
//   single trait implementation + registration.
//
//   New formats should:
//   1. Create a file in pillow-rs/src/formats/ (e.g., formats/avif.rs)
//   2. Implement FormatHandler
//   3. Call register_format() in an init function
//
//   CI enforces: no hard-coded format match in detect_format_from_magic,
//   detect_format_mode, or parse_format_str. (see scripts/check_format_handlers.sh).
// ============================================================================

use std::sync::RwLock;

use pillow_rs_image::DynamicImage;

use crate::error::PilError;

/// Options passed to format encoders.
///
/// Individual formats may ignore options that do not apply to their codec.
#[derive(Debug, Clone, Default)]
pub struct FormatEncodeOptions {
    /// Compression quality in the `0..=100` range, with format-specific meaning.
    pub quality: Option<u8>,
    /// Whether the encoder should prefer smaller output when supported.
    pub optimize: bool,
    /// ICC profile bytes to preserve in formats that support color profiles.
    pub icc_profile: Option<Vec<u8>>,
    /// EXIF metadata bytes to preserve in formats that support EXIF.
    pub exif: Option<Vec<u8>>,
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   FormatHandler trait. Every supported image format implements this.
//   The format registry auto-discovers supported formats via magic bytes
//   and file extensions. No more hard-coded match statements.
// ============================================================================

/// Codec integration point for one image format.
///
/// Each format implementation owns detection, decoding, encoding, and optional
/// mode detection for a single format family. The registry uses this trait to
/// avoid separate hard-coded match tables.
pub trait FormatHandler: Send + Sync {
    /// Returns the canonical format name, for example `"PNG"` or `"JPEG"`.
    fn name(&self) -> &'static str;

    /// Returns recognized lowercase file extensions for this format.
    fn extensions(&self) -> &'static [&'static str];

    /// Returns the canonical MIME type, for example `"image/png"`.
    fn mime_type(&self) -> &'static str;

    /// Returns magic byte signatures used for detection.
    ///
    /// Each signature must match at byte offset zero. Handlers should order
    /// longer or more-specific signatures before shorter signatures.
    fn magic_bytes(&self) -> &'static [&'static [u8]];

    /// Decodes encoded image bytes into a [`DynamicImage`].
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the bytes are malformed or unsupported by the
    /// handler.
    fn decode(&self, data: &[u8]) -> Result<DynamicImage, PilError>;

    /// Encodes a [`DynamicImage`] into this format's byte stream.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the image cannot be represented or the codec
    /// fails.
    fn encode(
        &self,
        img: &DynamicImage,
        options: &FormatEncodeOptions,
    ) -> Result<Vec<u8>, PilError>;

    /// Detects a Pillow mode string without fully decoding when possible.
    ///
    /// Returns `None` when mode detection requires a full decode.
    fn detect_mode(&self, data: &[u8]) -> Option<String>;

    /// Returns whether this format can store palette-indexed images.
    fn supports_palette(&self) -> bool {
        false
    }
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   Format registry. Thread-safe, lazily initialized, auto-discovered.
// ============================================================================

static FORMAT_REGISTRY: std::sync::OnceLock<RwLock<Vec<Box<dyn FormatHandler>>>> =
    std::sync::OnceLock::new();

fn registry() -> &'static RwLock<Vec<Box<dyn FormatHandler>>> {
    FORMAT_REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Registers a format handler.
///
/// # Errors
///
/// Returns an error string when a handler with the same name is already
/// registered or the registry lock is poisoned.
pub fn register_format(handler: Box<dyn FormatHandler>) -> Result<(), String> {
    let name = handler.name();
    match registry().write() {
        Ok(mut reg) => {
            if reg.iter().any(|h| h.name() == name) {
                return Err(format!("FormatHandler: duplicate format name '{}'", name));
            }
            reg.push(handler);
            Ok(())
        }
        Err(_) => {
            // RwLock poisoned (a panic while holding the write lock).
            // This is non-recoverable; clear and re-register.
            Err("FORMAT_REGISTRY poisoned".into())
        }
    }
}

/// AS PER DESIGN: Helper to read the registry, handling poison recovery.
fn read_registry() -> std::sync::RwLockReadGuard<'static, Vec<Box<dyn FormatHandler>>> {
    registry().read().unwrap_or_else(|poisoned| {
        log::warn!("FORMAT_REGISTRY poisoned; recovering");
        poisoned.into_inner()
    })
}

/// Detects an image format from magic bytes.
///
/// Registered handlers are checked in reverse registration order, so newer
/// handlers can override generic signatures.
pub fn detect_format_from_magic(data: &[u8]) -> Option<&'static str> {
    let reg = read_registry();
    // Reverse order: most recently registered = most preferred
    for handler in reg.iter().rev() {
        for magic in handler.magic_bytes() {
            if data.len() >= magic.len() && &data[..magic.len()] == *magic {
                return Some(handler.name());
            }
        }
    }
    None
}

/// Finds a registered format by canonical name or extension.
///
/// Matching is case-insensitive. The return value is the handler's canonical
/// format name.
pub fn find_format_by_name(name_or_ext: &str) -> Option<&'static str> {
    let reg = read_registry();
    let needle = name_or_ext.to_lowercase();
    for handler in reg.iter() {
        if handler.name().to_lowercase() == needle {
            return Some(handler.name());
        }
        if handler
            .extensions()
            .iter()
            .any(|e| e.to_lowercase() == needle)
        {
            return Some(handler.name());
        }
    }
    None
}

/// Returns canonical names for all registered formats.
pub fn registered_formats() -> Vec<&'static str> {
    let reg = read_registry();
    reg.iter().map(|h| h.name()).collect()
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate format registry behavior.
#[cfg(test)]
mod tests {
    use super::*;

    /// Test format handler (not a real image format)
    struct TestFormat;

    impl FormatHandler for TestFormat {
        fn name(&self) -> &'static str {
            "TEST"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &["test", "tst"]
        }
        fn mime_type(&self) -> &'static str {
            "image/x-test"
        }
        fn magic_bytes(&self) -> &'static [&'static [u8]] {
            &[b"TEST"]
        }
        fn decode(&self, _data: &[u8]) -> Result<DynamicImage, PilError> {
            Err(PilError::NotImplementedError(
                "TEST decode not implemented".into(),
            ))
        }
        fn encode(
            &self,
            _img: &DynamicImage,
            _opts: &FormatEncodeOptions,
        ) -> Result<Vec<u8>, PilError> {
            Err(PilError::NotImplementedError(
                "TEST encode not implemented".into(),
            ))
        }
        fn detect_mode(&self, _data: &[u8]) -> Option<String> {
            Some("RGB".into())
        }
    }

    #[test]
    fn detect_format_by_magic() {
        let _ = register_format(Box::new(TestFormat));
        let name = detect_format_from_magic(b"TESTDATA");
        assert_eq!(name, Some("TEST"));
    }

    #[test]
    fn no_match_for_unknown_magic() {
        // AS PER DESIGN: This test doesn't register anything — it only reads.
        // If registry is empty, unknown magic should return None.
        let name = detect_format_from_magic(b"XXXXDATA");
        assert_eq!(name, None);
    }

    #[test]
    fn find_format_by_extension() {
        let _ = register_format(Box::new(TestFormat));
        let name = find_format_by_name("tst");
        assert_eq!(name, Some("TEST"));
    }
}
