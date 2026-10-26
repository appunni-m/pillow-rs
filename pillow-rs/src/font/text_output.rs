use super::{
    FontMaskAndOffset, FreeTypeFont, ImageFontTextInput, ImageFontTextOptions, PilError, imagingft,
    pillow_bytes_to_text,
};
impl FreeTypeFont {
    /// Return Pillow's public text bounding box.
    pub fn getbbox(&self, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
        imagingft::getbbox(self, text)
    }

    /// Return Pillow's public text bounding box for a Python `bytes` text argument.
    pub fn getbbox_bytes(&self, text: &[u8]) -> Result<(i32, i32, i32, i32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getbbox(&text)
    }

    /// Return Pillow's public text bounding box using optional layout arguments.
    pub fn getbbox_with_options(
        &self,
        text: &str,
        options: &ImageFontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        if options.uses_default_layout() {
            return self.getbbox(text).map(|(left, top, right, bottom)| {
                (left as f32, top as f32, right as f32, bottom as f32)
            });
        }
        imagingft::getbbox_with_options(self, text, options)
    }

    /// Return Pillow's public text bounding box for byte text using optional layout arguments.
    pub fn getbbox_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        if options.uses_default_layout() {
            return self.getbbox_bytes(text).map(|(left, top, right, bottom)| {
                (left as f32, top as f32, right as f32, bottom as f32)
            });
        }
        let text = pillow_bytes_to_text(text);
        self.getbbox_with_options(&text, options)
    }

    /// Return a text bounding box after applying Pillow's text-input rules.
    pub fn getbbox_input_with_options(
        &self,
        text: ImageFontTextInput,
        options: &ImageFontTextOptions,
    ) -> Result<(f32, f32, f32, f32), PilError> {
        match text {
            ImageFontTextInput::Text(text) => self.getbbox_with_options(&text, options),
            ImageFontTextInput::Bytes(text) => self.getbbox_bytes_with_options(&text, options),
        }
    }

    /// Return text length after applying Pillow's text-input rules.
    pub fn getlength_input_with_options(
        &self,
        text: ImageFontTextInput,
        options: &ImageFontTextOptions,
    ) -> Result<f32, PilError> {
        match text {
            ImageFontTextInput::Text(text) => self.getlength_with_options(&text, options),
            ImageFontTextInput::Bytes(text) => self.getlength_bytes_with_options(&text, options),
        }
    }

    /// Render a mask after applying Pillow's text-input rules.
    pub fn getmask_input_with_options(
        &self,
        text: ImageFontTextInput,
        options: &ImageFontTextOptions,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        match text {
            ImageFontTextInput::Text(text) => self.getmask_with_options(&text, options),
            ImageFontTextInput::Bytes(text) => self.getmask_bytes_with_options(&text, options),
        }
    }

    /// Render a mask with offset after applying Pillow's text-input rules.
    pub fn getmask2_input_with_options(
        &self,
        text: ImageFontTextInput,
        options: &ImageFontTextOptions,
    ) -> Result<FontMaskAndOffset, PilError> {
        match text {
            ImageFontTextInput::Text(text) => self.getmask2_with_options(&text, options),
            ImageFontTextInput::Bytes(text) => self.getmask2_bytes_with_options(&text, options),
        }
    }

    /// Return Pillow's public binary-mode text bounding box.
    #[cfg(feature = "test-api")]
    pub fn getbbox_binary(&self, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
        imagingft::getbbox_binary(self, text)
    }

    /// Return Pillow's public binary-mode text bounding box for byte text.
    #[cfg(feature = "test-api")]
    pub fn getbbox_binary_bytes(&self, text: &[u8]) -> Result<(i32, i32, i32, i32), PilError> {
        let text = pillow_bytes_to_text(text);
        self.getbbox_binary(&text)
    }

    /// Return Pillow's public `getmask2` mask and offset tuple.
    pub fn getmask2(&self, text: &str) -> Result<FontMaskAndOffset, PilError> {
        imagingft::getmask2(self, text)
    }

    /// Return Pillow's public `getmask2` mask and offset tuple for byte text.
    pub fn getmask2_bytes(&self, text: &[u8]) -> Result<FontMaskAndOffset, PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask2(&text)
    }

    /// Return Pillow's public `getmask2` result using optional render arguments.
    pub fn getmask2_with_options(
        &self,
        text: &str,
        options: &ImageFontTextOptions,
    ) -> Result<FontMaskAndOffset, PilError> {
        if let Some(start) = options.start
            && options.uses_default_mask_except_start()
        {
            return self.getmask2_with_start(text, start);
        }
        if options.uses_default_mask() {
            return self.getmask2(text);
        }
        imagingft::getmask2_with_options(self, text, options)
    }

    /// Return Pillow's public `getmask2` result for byte text using optional render arguments.
    pub fn getmask2_bytes_with_options(
        &self,
        text: &[u8],
        options: &ImageFontTextOptions,
    ) -> Result<FontMaskAndOffset, PilError> {
        if let Some(start) = options.start
            && options.uses_default_mask_except_start()
        {
            return self.getmask2_bytes_with_start(text, start);
        }
        if options.uses_default_mask() {
            return self.getmask2_bytes(text);
        }
        let text = pillow_bytes_to_text(text);
        self.getmask2_with_options(&text, options)
    }

    /// `getmask2` variant with Pillow's fractional start parameter.
    pub fn getmask2_with_start(
        &self,
        text: &str,
        start: (f64, f64),
    ) -> Result<FontMaskAndOffset, PilError> {
        imagingft::getmask2_with_start(self, text, start)
    }

    /// `getmask2` byte-text variant with Pillow's fractional start parameter.
    pub fn getmask2_bytes_with_start(
        &self,
        text: &[u8],
        start: (f64, f64),
    ) -> Result<FontMaskAndOffset, PilError> {
        let text = pillow_bytes_to_text(text);
        self.getmask2_with_start(&text, start)
    }

    /// Return a transposed Pillow-compatible grayscale text mask.
    pub fn get_transposed_mask(
        &self,
        text: &str,
        orientation: Option<&str>,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::get_transposed_mask(self, text, orientation)
    }

    /// Return a transposed mask after applying Pillow's text-input rules.
    pub fn get_transposed_mask_input(
        &self,
        text: ImageFontTextInput,
        orientation: Option<&str>,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        let text = text.into_text();
        self.get_transposed_mask(&text, orientation)
    }

    /// Return Pillow-compatible binary-mode RGBA text rendering.
    #[cfg(feature = "test-api")]
    pub fn render_text_binary(
        &self,
        text: &str,
        fill: (u8, u8, u8, u8),
        spacing: f32,
    ) -> Result<(u32, u32, Vec<u8>), PilError> {
        imagingft::render_text_binary(self, text, fill, spacing)
    }
}
