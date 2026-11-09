use crate::error::PilError;

/// Host callbacks for Python-owned font metrics used by the Rust bounds loop.
#[doc(hidden)]
pub trait ImageFontTextBoundsCallbacks {
    /// Bounding-value carrier; bindings preserve original Python number objects.
    type Value;
    /// Callback error type; host errors remain intact while core errors convert.
    type Error: From<PilError>;

    /// Advance the host line iterator exactly once.
    fn next_line(&mut self) -> Result<Option<Self::Value>, Self::Error>;

    /// Ask the host font protocol to measure one positioned line.
    fn begin_line_bounds(&mut self, line: &Self::Value) -> Result<(), Self::Error>;
    /// Read one coordinate from the host font's bounding box.
    fn bbox_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Read the x or y offset for one bounding-box coordinate.
    fn line_offset(&mut self, line: &Self::Value, index: usize)
    -> Result<Self::Value, Self::Error>;
    /// Add a measured coordinate and its line position using host arithmetic.
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error>;

    /// Apply the host's min(left, right) comparison and return value.
    fn minimum(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;

    /// Apply the host's max(left, right) comparison and return value.
    fn maximum(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;

    /// Preserve the host failure for a layout that produces no lines.
    fn empty_bounds(&mut self) -> Result<[Self::Value; 4], Self::Error> {
        Err(PilError::AssertionError("text bounds produced no lines".into()).into())
    }
}

/// Arithmetic used by Pillow's host-compatible multiline placement strategy.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFontTextPositionArithmetic {
    /// Evaluate addition.
    Add,
    /// Evaluate subtraction.
    Subtract,
    /// Evaluate multiplication.
    Multiply,
    /// Evaluate division.
    Divide,
    /// Evaluate in-place addition.
    AddAssign,
    /// Evaluate in-place subtraction.
    SubtractAssign,
}

/// Host comparisons needed by the multiline placement strategy.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFontTextPositionComparison {
    /// Compare the candidate width with the current maximum.
    GreaterThan,
    /// Compare a width difference with zero.
    NotEqual,
}

/// Host operations required by the Rust-owned multiline positioning strategy.
///
/// Bindings preserve Python font and numeric protocols here; the core decides
/// line placement, anchor adjustments, alignment, and justification order.
#[doc(hidden)]
pub trait ImageFontTextPositionCallbacks {
    /// Host value carrier for coordinates, font metrics, anchors, and text.
    type Value: Clone;
    /// Host line value returned by the strategy.
    type Line;
    /// Callback error type; core errors convert into the host error.
    type Error: From<PilError>;

    /// Resolve and validate the requested multiline anchor.
    fn anchor(&mut self) -> Result<Self::Value, Self::Error>;
    /// Read the host sequence length after anchor validation.
    fn line_count(&mut self) -> Result<usize, Self::Error>;
    /// Read the first line for the single-line fast path.
    fn single_line(&mut self) -> Result<Self::Value, Self::Error>;
    /// Start one iteration over the host line sequence.
    fn begin_lines(&mut self) -> Result<(), Self::Error>;
    /// Advance the active host line iterator once.
    fn next_line(&mut self) -> Result<Option<Self::Value>, Self::Error>;
    /// Test whether one anchor component is contained in a string.
    fn anchor_component_contains(
        &mut self,
        anchor: &Self::Value,
        index: usize,
        values: &str,
    ) -> Result<bool, Self::Error>;
    /// Test one anchor component against a string.
    fn anchor_component_is(
        &mut self,
        anchor: &Self::Value,
        index: usize,
        value: &str,
    ) -> Result<bool, Self::Error>;
    /// Build the anchor used by justified words.
    fn justified_anchor(&mut self, anchor: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Compare direction with top-to-bottom using equality.
    fn direction_is_top_to_bottom(&mut self) -> Result<bool, Self::Error>;
    /// Compare direction with top-to-bottom using inequality.
    fn direction_is_not_top_to_bottom(&mut self) -> Result<bool, Self::Error>;
    /// Compute the font mode through the host's observable method.
    fn font_mode(&mut self) -> Result<Self::Value, Self::Error>;
    /// Measure line spacing using the host font protocol.
    fn line_spacing(&mut self, font_mode: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Measure one line using the host font protocol.
    fn line_width(
        &mut self,
        line: &Self::Value,
        font_mode: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Split a line using its host string or byte-string protocol.
    fn split_words(&mut self, line: &Self::Value) -> Result<Vec<Self::Value>, Self::Error>;
    /// Return the requested x coordinate, preserving host indexing behavior.
    fn origin_x(&mut self) -> Result<Self::Value, Self::Error>;
    /// Return the requested y coordinate, preserving host indexing behavior.
    fn origin_y(&mut self) -> Result<Self::Value, Self::Error>;
    /// Compare alignment with a string on the right-hand side.
    fn alignment_is(&mut self, value: &str) -> Result<bool, Self::Error>;
    /// Check the left-or-justify membership fallback used by Pillow.
    fn alignment_is_left_or_justify(&mut self) -> Result<bool, Self::Error>;
    /// Apply host arithmetic, including in-place addition and subtraction.
    fn arithmetic(
        &mut self,
        operation: ImageFontTextPositionArithmetic,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Compare arbitrary host numeric values with greater-than or not-equal.
    fn compare_numbers(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
        operation: ImageFontTextPositionComparison,
    ) -> Result<bool, Self::Error>;
    /// Construct one host line from the computed strategy result.
    fn positioned_line(
        &mut self,
        x: &Self::Value,
        y: &Self::Value,
        anchor: &Self::Value,
        text: &Self::Value,
    ) -> Result<Self::Line, Self::Error>;
    /// Construct an integer host value.
    fn integer(&mut self, value: i64) -> Result<Self::Value, Self::Error>;
    /// Construct a floating-point host value.
    fn float(&mut self, value: f64) -> Result<Self::Value, Self::Error>;
}
