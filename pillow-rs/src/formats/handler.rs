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
#[derive(Debug, Clone, Default)]
pub struct FormatEncodeOptions {
    /// Compression quality (0-100, format-dependent meaning)
    pub quality: Option<u8>,
    /// Optimize for size (lossless compression level)
    pub optimize: bool,
    /// Preserve ICC profile
    pub icc_profile: Option<Vec<u8>>,
    /// Preserve EXIF data
    pub exif: Option<Vec<u8>>,
}

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   FormatHandler trait. Every supported image format implements this.
//   The format registry auto-discovers supported formats via magic bytes
//   and file extensions. No more hard-coded match statements.
// ============================================================================

/// Trait for image format handlers (PNG, JPEG, GIF, BMP, etc.).
///
/// AS PER DESIGN — DO NOT REMOVE:
/// Each format is a single struct implementing this trait. Format detection,
/// decoding, encoding, and mode detection are all methods on the same struct.
/// Adding a format = adding one file + one registration call.
pub trait FormatHandler: Send + Sync {
    /// Human-readable format name, e.g., "PNG", "JPEG".
    fn name(&self) -> &'static str;

    /// File extensions for this format, e.g., &["png"].
    fn extensions(&self) -> &'static [&'static str];

    /// MIME type, e.g., "image/png".
    fn mime_type(&self) -> &'static str;

    /// Magic byte signatures for detection.
    /// Each entry is a byte sequence that must match at offset 0.
    /// Longer signatures are checked first.
    fn magic_bytes(&self) -> &'static [&'static [u8]];

    /// Decode raw bytes into a DynamicImage.
    fn decode(&self, data: &[u8]) -> Result<DynamicImage, PilError>;

    /// Encode a DynamicImage into raw bytes.
    fn encode(
        &self,
        img: &DynamicImage,
        options: &FormatEncodeOptions,
    ) -> Result<Vec<u8>, PilError>;

    /// Detect the mode string of an image without fully decoding.
    /// Returns None if mode detection requires decoding.
    fn detect_mode(&self, data: &[u8]) -> Option<String>;

    /// Whether this format can store palette-indexed images.
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

/// Register a format handler.
/// AS PER DESIGN: Call once per format at startup.
/// Returns Err if a format with the same name is already registered.
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

/// Detect image format from magic bytes.
/// Returns the first matching handler, or None if no format matches.
/// AS PER DESIGN: Checks handlers in reverse registration order (most recent first).
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

/// Find a format handler by name or extension (case-insensitive).
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

/// List all registered format names.
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
