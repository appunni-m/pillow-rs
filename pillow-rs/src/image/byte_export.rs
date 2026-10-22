//! Pillow-compatible byte export methods for [`Image`].

use crate::error::PilError;
use crate::format::parse_format_str;

use super::{Image, ImageByteExport, is_l16_mode, l16_uses_big_endian, pack_mode1_rows};

#[cfg(test)]
mod tests;

impl Image {
    /// Resolves Pillow's explicit save format or a host-supplied path
    /// extension before encoding.
    ///
    /// Filesystem access and extension extraction remain in host bindings, but
    /// format validity and Pillow's public error categories are shared here.
    pub fn resolve_save_format(
        explicit: Option<&str>,
        extension: Option<&str>,
    ) -> Result<String, PilError> {
        if let Some(format) = explicit {
            parse_format_str(format)
                .map(|_| format.to_owned())
                .map_err(|_| PilError::KeyError(format.to_owned()))
        } else {
            let extension = extension.ok_or_else(|| {
                // Pillow's Image.save extension lookup reports an empty
                // suffix as a ValueError with this exact public message.
                PilError::ValueError("unknown file extension: ".into())
            })?;
            parse_format_str(extension)
                .map(|_| extension.to_owned())
                .map_err(|_| PilError::ValueError(format!("unknown file extension: .{extension}")))
        }
    }

    /// Returns raw image bytes in the image's current Pillow mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes(&self) -> Result<Vec<u8>, PilError> {
        let mode = self.mode()?;
        self.tobytes_formatted(&mode)
    }

    /// Returns one byte per logical image sample.
    ///
    /// Unlike [`Image::tobytes`], mode `"1"` is expanded to one `0` or `255`
    /// byte per pixel. This matches the sequence representation of Pillow's
    /// internal `ImagingCore`, which bitmap-font masks expose directly.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the image cannot be materialized.
    pub fn tobytes_unpacked(&self) -> Result<Vec<u8>, PilError> {
        if self.mode()? == "1" {
            return Ok(self.materialized_shared()?.to_luma8().into_raw());
        }
        self.tobytes()
    }

    /// Returns raw image bytes using an explicit Pillow mode override.
    ///
    /// This is the binding-facing form of [`Image::tobytes`]. Modes `"F"` and
    /// `"I"` return the internal little-endian 4-byte scalar representation.
    /// Mode `"1"` packs pixels eight per byte, most-significant bit first.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes_formatted(&self, mode: &str) -> Result<Vec<u8>, PilError> {
        // Fast path for Paletted: return raw index bytes
        if let Image::Paletted(data) = self {
            return Ok(data.indices.as_raw().to_vec());
        }
        let img = self.materialized_shared()?;

        // F/I modes: stored as RGBA8 internally (f32/i32 LE bytes packed as RGBA).
        // Read the raw bytes directly — img.as_bytes() already contains the correct
        // f32/i32 LE representation (4 bytes per pixel).
        if matches!(mode, "F" | "I") {
            return Ok(img.as_bytes().to_vec());
        }

        if is_l16_mode(mode) {
            let native_is_big = cfg!(target_endian = "big");
            let raw_is_big = l16_uses_big_endian(mode);
            let bytes = img.as_bytes();
            if native_is_big == raw_is_big {
                return Ok(bytes.to_vec());
            }
            let mut swapped = Vec::with_capacity(bytes.len());
            for sample in bytes.chunks_exact(2) {
                swapped.extend_from_slice(&[sample[1], sample[0]]);
            }
            return Ok(swapped);
        }

        // For mode "1" images, pack 8 pixels per byte (MSB first) matching PIL.
        if mode == "1" && img.color() == crate::raster::ColorType::L8 {
            return Ok(pack_mode1_rows(
                img.as_bytes(),
                img.width() as usize,
                img.height() as usize,
            ));
        }
        Ok(img.as_bytes().to_vec())
    }

    /// Returns public image bytes using Pillow's raw encoder arguments.
    ///
    /// `BGR` and `BGRA` preserve channel width while exchanging red and blue;
    /// raw `RGBA` from an RGB image appends Pillow's opaque filler alpha.
    /// Other encoder names or raw modes retain the ordinary mode byte layout.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific byte packing
    /// fails.
    pub fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: &[String],
    ) -> Result<Vec<u8>, PilError> {
        if encoder_name != "raw" {
            return Err(PilError::IOError(format!(
                "encoder {encoder_name} not available"
            )));
        }
        let raw_mode = args.first().map(String::as_str).unwrap_or(mode);
        let mode_supports_raw = raw_mode == mode
            || (mode == "RGB" && matches!(raw_mode, "BGR" | "RGBA"))
            || (mode == "RGBA" && raw_mode == "BGRA");
        if !mode_supports_raw {
            return Err(PilError::ValueError(format!(
                "No packer found from {mode} to {raw_mode}"
            )));
        }
        // Pillow's default Image.tobytes() path uses the image's current mode
        // without a raw-mode override. Keep that public route on the canonical
        // core entry point; explicit raw modes still use the packer below.
        if args.is_empty() && self.mode()? == mode {
            return self.tobytes();
        }
        let mut data = self.tobytes_formatted(mode)?;
        if mode == "RGB" && raw_mode == "RGBA" {
            let mut expanded = Vec::with_capacity(data.len() / 3 * 4);
            for pixel in data.chunks_exact(3) {
                expanded.extend_from_slice(pixel);
                expanded.push(255);
            }
            data = expanded;
        }
        match args.first().map(String::as_str) {
            Some("BGRA") => {
                for pixel in data.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            Some("BGR") => {
                for pixel in data.chunks_exact_mut(3) {
                    pixel.swap(0, 2);
                }
            }
            _ => {}
        }
        Ok(data)
    }

    /// Returns an owned byte view using Pillow's raw encoder arguments.
    ///
    /// Native default raw layouts share immutable materialized storage, so a
    /// binding can copy directly into its host byte object without an
    /// intermediate buffer. Packed bits, palette indices, endian conversion,
    /// and explicit encoder arguments retain [`Image::tobytes_encoded`]'s
    /// packing behavior. The view remains valid after this image is changed or
    /// dropped; host bindings must still copy it for independent byte objects.
    ///
    /// # Errors
    ///
    /// Returns the same materialization and packing errors as
    /// [`Image::tobytes_encoded`].
    pub fn tobytes_encoded_shared(
        &self,
        mode: &str,
        encoder_name: &str,
        args: &[String],
    ) -> Result<impl AsRef<[u8]> + Send + Sync + 'static + use<>, PilError> {
        if encoder_name != "raw"
            || !args.is_empty()
            || matches!(self, Image::Paletted(_))
            || mode == "1"
            || (is_l16_mode(mode) && cfg!(target_endian = "big") != l16_uses_big_endian(mode))
        {
            return self
                .tobytes_encoded(mode, encoder_name, args)
                .map(ImageByteExport::Packed);
        }

        // The default encoder checks the image mode before materialization,
        // even when a binding supplies a different mode override. For every
        // remaining override, tobytes_formatted exposes the storage unchanged.
        self.mode()?;
        self.materialized_shared().map(ImageByteExport::Shared)
    }
}

impl Image {
    /// Converts the image to X11 bitmap (`XBM`) source bytes.
    ///
    /// Mode `"1"` treats any non-zero pixel as white. Other modes are rejected,
    /// matching Pillow's `not a bitmap` contract.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    pub fn tobitmap(&self) -> Result<Vec<u8>, PilError> {
        let mode = self.mode()?;
        if mode != "1" {
            return Err(PilError::ValueError("not a bitmap".to_owned()));
        }
        let img = self.materialized_shared()?;
        let gray = img.to_luma8();
        let (w, h) = (gray.width(), gray.height());
        let row_bytes = w.div_ceil(8) as usize;
        let mut bits = vec![0u8; row_bytes * h as usize];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x, y)[0];
                // The mode check above makes this an `Image` mode "1" path:
                // Pillow stores any non-zero source value as a set bit.
                let is_white = v != 0;
                if is_white {
                    // PIL XBM: 1 = white, 0 = black; LSB = leftmost pixel
                    let byte_idx = (x / 8) as usize;
                    let bit_idx = x % 8;
                    bits[(y as usize) * row_bytes + byte_idx] |= 1u8 << bit_idx;
                }
            }
        }
        // PIL tobitmap format: XBM C source, 15 hex values per line
        let mut xbm = String::new();
        xbm.push_str(&format!("#define image_width {}\n", w));
        xbm.push_str(&format!("#define image_height {}\n", h));
        xbm.push_str("static char image_bits[] = {\n");
        let hexes: Vec<String> = bits.iter().map(|b| format!("0x{:02x}", b)).collect();
        let total = hexes.len();
        for (i, chunk) in hexes.chunks(15).enumerate() {
            let start = i * 15;
            let end = (start + chunk.len()).min(total);
            let is_last = end >= total;
            if is_last {
                // Last line: no trailing comma
                xbm.push_str(&chunk.join(","));
            } else {
                // Full line: trailing comma
                xbm.push_str(&chunk.join(","));
                xbm.push(',');
            }
            xbm.push('\n');
        }
        xbm.push_str("};");
        Ok(xbm.into_bytes())
    }
}
