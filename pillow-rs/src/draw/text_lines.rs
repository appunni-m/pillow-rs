use super::Mode;

/// One line object yielded by Pillow's `ImageText._split` callback.
#[doc(hidden)]
pub struct DrawTextLine<Value> {
    /// The host-owned record whose fields are read by the selected text path.
    pub value: Value,
}

/// Host operations used by the Rust-owned ImageDraw line-routing strategy.
#[doc(hidden)]
pub trait DrawTextLinesCallbacks {
    /// Host value carrier used by line records.
    type Value: Clone;
    /// Iterator returned by the host's `_split` method.
    type Lines;
    /// Host error carrier; core errors are translated at the binding boundary.
    type Error;

    /// Run the ordered direct FreeType route checks.
    fn uses_direct_route(&mut self) -> Result<bool, Self::Error>;
    /// Draw multiline text through the direct FreeType strategy.
    fn draw_direct(&mut self) -> Result<(), Self::Error>;
    /// Call `ImageText._split` after the direct route has been rejected.
    fn split_lines(&mut self) -> Result<Self::Lines, Self::Error>;
    /// Fetch and unpack the next four-field line from the host iterator.
    fn next_line(
        &mut self,
        lines: &mut Self::Lines,
    ) -> Result<Option<DrawTextLine<Self::Value>>, Self::Error>;
    /// Convert the current ImageText embedded-color value to bool when a fill
    /// mask is actually requested.
    fn embedded_color_is_truthy(&mut self) -> Result<bool, Self::Error>;
    /// Draw the custom font's stroked mask for one line.
    fn draw_custom_stroke(
        &mut self,
        line: &DrawTextLine<Self::Value>,
        mode: Mode,
    ) -> Result<(), Self::Error>;
    /// Draw the custom font's fill mask for one line.
    fn draw_custom_fill(
        &mut self,
        line: &DrawTextLine<Self::Value>,
        mode: Mode,
        embedded_color: bool,
    ) -> Result<(), Self::Error>;
}

/// Select and execute ImageDraw's direct, split-line, and custom-font paths.
///
/// Ink resolution and palette allocation happen before this strategy is
/// entered. The strategy therefore preserves Pillow's ordering while owning
/// route selection and iteration instead of leaving those decisions in the
/// Python facade.
#[doc(hidden)]
pub fn draw_text_lines_with_callbacks<C: DrawTextLinesCallbacks>(
    mode: Mode,
    stroke_ink: Option<i64>,
    fill_ink: i64,
    callbacks: &mut C,
) -> Result<(), C::Error> {
    if callbacks.uses_direct_route()? {
        return callbacks.draw_direct();
    }

    let mut lines = callbacks.split_lines()?;
    let stroke_mode = crate::imagefont_text_font_mode(mode, false);
    while let Some(line) = callbacks.next_line(&mut lines)? {
        if let Some(stroke_ink) = stroke_ink {
            callbacks.draw_custom_stroke(&line, stroke_mode)?;
            if stroke_ink == fill_ink {
                continue;
            }
        }
        let embedded_color = callbacks.embedded_color_is_truthy()?;
        let fill_mode = crate::imagefont_text_font_mode(mode, embedded_color);
        callbacks.draw_custom_fill(&line, fill_mode, embedded_color)?;
    }
    Ok(())
}
