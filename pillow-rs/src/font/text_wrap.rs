use super::text_layout::multiline_text_positions;
use super::{
    FreeTypeFont, ImageFontTextInput, ImageFontTextOptions, ImageFontTextScaling,
    ImageFontTextWrapCallbacks, ImageFontTextWrapResult, ImageFontTextWrapWidthCheck,
    pillow_bytes_to_text,
};
use crate::error::PilError;
use crate::raster::Mode;

#[derive(Clone)]
enum WrapUnit {
    Unicode(String),
    Bytes(Vec<u8>),
}

impl WrapUnit {
    fn from_input(input: ImageFontTextInput) -> Result<Self, PilError> {
        match input {
            ImageFontTextInput::Text(text) => Ok(Self::Unicode(text)),
            ImageFontTextInput::Bytes(bytes) => Ok(Self::Bytes(bytes)),
            ImageFontTextInput::InvalidType => {
                Err(PilError::TypeError("expected string or bytes".into()))
            }
        }
    }

    fn empty_like(&self) -> Self {
        match self {
            Self::Unicode(_) => Self::Unicode(String::new()),
            Self::Bytes(_) => Self::Bytes(Vec::new()),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Unicode(text) => text.chars().count(),
            Self::Bytes(bytes) => bytes.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Unicode(text) => text.is_empty(),
            Self::Bytes(bytes) => bytes.is_empty(),
        }
    }

    fn concatenate(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unicode(left), Self::Unicode(right)) => {
                Self::Unicode(format!("{left}{right}"))
            }
            (Self::Bytes(left), Self::Bytes(right)) => {
                let mut joined = Vec::with_capacity(left.len() + right.len());
                joined.extend_from_slice(left);
                joined.extend_from_slice(right);
                Self::Bytes(joined)
            }
            _ => unreachable!("wrap tokens keep the source text representation"),
        }
    }

    fn strip_prefix(&self, prefix: &Self) -> Self {
        match (self, prefix) {
            (Self::Unicode(text), Self::Unicode(prefix)) => Self::Unicode(
                text.get(prefix.len()..)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            (Self::Bytes(bytes), Self::Bytes(prefix)) => {
                Self::Bytes(bytes.get(prefix.len()..).unwrap_or_default().to_vec())
            }
            _ => unreachable!("wrap tokens keep the source text representation"),
        }
    }

    fn trim_start(&self) -> Self {
        match self {
            Self::Unicode(text) => {
                Self::Unicode(text.trim_start_matches(is_python_whitespace).to_owned())
            }
            Self::Bytes(bytes) => {
                let start = bytes
                    .iter()
                    .position(|byte| !is_byte_whitespace(*byte))
                    .unwrap_or(bytes.len());
                Self::Bytes(bytes[start..].to_vec())
            }
        }
    }

    fn leading_newlines(&self) -> Vec<Self> {
        match self {
            Self::Unicode(text) => unicode_leading_newlines(text),
            Self::Bytes(bytes) => byte_leading_newlines(bytes),
        }
    }

    fn suffix_from(&self, position: usize) -> Self {
        match self {
            Self::Unicode(text) => Self::Unicode(text.chars().skip(position).collect()),
            Self::Bytes(bytes) => {
                Self::Bytes(bytes.get(position..).unwrap_or_default().to_vec())
            }
        }
    }

    fn as_input(&self) -> ImageFontTextInput {
        match self {
            Self::Unicode(text) => ImageFontTextInput::Text(text.clone()),
            Self::Bytes(bytes) => ImageFontTextInput::Bytes(bytes.clone()),
        }
    }

    fn tokens(&self) -> Vec<Self> {
        match self {
            Self::Unicode(text) => unicode_word_tokens(text),
            Self::Bytes(bytes) => byte_word_tokens(bytes),
        }
    }
}

struct WrapState<'a, C> {
    callbacks: &'a mut C,
    has_height: bool,
    lines: Vec<WrapUnit>,
    position: usize,
    offset: usize,
}

struct NativeWrapCallbacks<'a> {
    base_font: &'a FreeTypeFont,
    variant_font: Option<FreeTypeFont>,
    mode: Mode,
    width: f64,
    height: Option<f64>,
    spacing: f64,
    stroke_width: f32,
    embedded_color: bool,
    scaling: Option<ImageFontTextScaling>,
}

impl<C: ImageFontTextWrapCallbacks> WrapState<'_, C> {
    fn add_line(&mut self, line: &WrapUnit) -> Result<bool, C::Error> {
        if self.has_height {
            let mut candidate = self.lines.iter().map(WrapUnit::as_input).collect::<Vec<_>>();
            candidate.push(line.as_input());
            if !self.callbacks.height_fits(&candidate, &line.as_input())? {
                return Ok(false);
            }
        }

        self.lines.push(line.clone());
        self.position += line.len() + self.offset;
        self.offset = 0;
        Ok(true)
    }
}

impl NativeWrapCallbacks<'_> {
    fn metric_font(&self) -> &FreeTypeFont {
        self.variant_font.as_ref().unwrap_or(self.base_font)
    }

    fn bbox_options(&self) -> ImageFontTextOptions {
        let font_mode = super::text_layout::text_layout_font_mode(self.mode, self.embedded_color);
        ImageFontTextOptions {
            mode: Some(font_mode.to_owned()),
            stroke_width: self.stroke_width,
            ..ImageFontTextOptions::default()
        }
    }

    fn variant(&mut self, size: i64) -> Result<(), PilError> {
        self.variant_font = Some(self.base_font.font_variant(Some(size as f32))?);
        Ok(())
    }
}

impl ImageFontTextWrapCallbacks for NativeWrapCallbacks<'_> {
    type Error = PilError;

    fn width_check(
        &mut self,
        text: &ImageFontTextInput,
        check: ImageFontTextWrapWidthCheck,
    ) -> Result<bool, Self::Error> {
        let bbox = self
            .metric_font()
            .getbbox_input_with_options(text.clone(), &self.bbox_options())?;
        Ok(match check {
            ImageFontTextWrapWidthCheck::Fits => f64::from(bbox.2) <= self.width,
            ImageFontTextWrapWidthCheck::Exceeds => f64::from(bbox.2) > self.width,
        })
    }

    fn height_fits(
        &mut self,
        lines: &[ImageFontTextInput],
        last_line: &ImageFontTextInput,
    ) -> Result<bool, Self::Error> {
        let height = self
            .height
            .ok_or_else(|| PilError::InternalError("missing text wrap height".into()))?;
        let candidate = lines
            .iter()
            .map(text_input_to_unicode)
            .collect::<Vec<_>>();
        let candidate = candidate.iter().map(String::as_str).collect::<Vec<_>>();
        let options = ImageFontTextOptions {
            embedded_color: self.embedded_color,
            stroke_width: self.stroke_width,
            align: Some("left".to_owned()),
            ..ImageFontTextOptions::default()
        };
        let positions = multiline_text_positions(
            0.0,
            0.0,
            &candidate,
            self.base_font,
            self.spacing,
            &options,
            self.mode,
        )?;
        let last_line_y = positions
            .last()
            .map(|position| position.y)
            .ok_or_else(|| PilError::InternalError("text wrap produced no line".into()))?;
        let bbox = self
            .metric_font()
            .getbbox_input_with_options(last_line.clone(), &self.bbox_options())?;
        Ok(last_line_y + f64::from(bbox.3) <= height)
    }

    fn has_remaining_text(
        &mut self,
        remaining: &ImageFontTextInput,
        _consumed: usize,
        _wrap_id: u64,
    ) -> Result<bool, Self::Error> {
        Ok(remaining_text_is_nonempty(remaining))
    }

    fn scaling_is_shrink(&mut self) -> Result<bool, Self::Error> {
        Ok(matches!(self.scaling, Some(ImageFontTextScaling::Shrink { .. })))
    }

    fn initial_size(&mut self, round_up: bool) -> Result<i64, Self::Error> {
        let size = f64::from(self.base_font.font_size());
        let rounded = if round_up { size.ceil() } else { size.floor() };
        i64::try_from(rounded as i128)
            .map_err(|_| PilError::OverflowError("font size is outside the integer range".into()))
    }

    fn shrink_limit_reached(&mut self, size: i64) -> Result<bool, Self::Error> {
        let Some(ImageFontTextScaling::Shrink { limit }) = self.scaling else {
            return Ok(false);
        };
        let minimum = if 1.0 > limit { 1.0 } else { limit };
        Ok(size as f64 == minimum)
    }

    fn grow_limit_reached(&mut self, size: i64) -> Result<bool, Self::Error> {
        let Some(ImageFontTextScaling::Grow { limit }) = self.scaling else {
            return Ok(false);
        };
        Ok(size as f64 == limit)
    }

    fn font_variant(&mut self, size: i64) -> Result<(), Self::Error> {
        self.variant(size)
    }

    fn restore_font(&mut self, size: i64) -> Result<(), Self::Error> {
        if size as f64 != f64::from(self.base_font.font_size()) {
            self.variant(size)?;
        } else {
            self.variant_font = None;
        }
        Ok(())
    }
}

/// Wrap Unicode or byte ImageText text using Pillow's ImageText rules.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wrap_text(
    font: &FreeTypeFont,
    mode: Mode,
    text: ImageFontTextInput,
    width: f64,
    height: Option<f64>,
    spacing: f64,
    stroke_width: f32,
    embedded_color: bool,
) -> Result<ImageFontTextWrapResult, PilError> {
    let mut callbacks = NativeWrapCallbacks {
        base_font: font,
        variant_font: None,
        mode,
        width,
        height,
        spacing,
        stroke_width,
        embedded_color,
        scaling: None,
    };
    wrap_with_callbacks(&mut callbacks, text, height.is_some(), false)?.ok_or_else(|| {
        PilError::InternalError("unscaled text wrap returned no result".into())
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn wrap_text_scaled(
    font: &FreeTypeFont,
    mode: Mode,
    text: ImageFontTextInput,
    width: f64,
    height: f64,
    spacing: f64,
    stroke_width: f32,
    embedded_color: bool,
    scaling: ImageFontTextScaling,
) -> Result<Option<(ImageFontTextWrapResult, Option<FreeTypeFont>)>, PilError> {
    let mut callbacks = NativeWrapCallbacks {
        base_font: font,
        variant_font: None,
        mode,
        width,
        height: Some(height),
        spacing,
        stroke_width,
        embedded_color,
        scaling: Some(scaling),
    };
    let result = wrap_with_callbacks(&mut callbacks, text, true, true)?;
    Ok(result.map(|result| (result, callbacks.variant_font.take())))
}

pub(crate) fn wrap_with_callbacks<C: ImageFontTextWrapCallbacks>(
    callbacks: &mut C,
    text: ImageFontTextInput,
    has_height: bool,
    has_scaling: bool,
) -> Result<Option<ImageFontTextWrapResult>, C::Error> {
    let text = WrapUnit::from_input(text).map_err(C::Error::from)?;
    let initial = wrap_once(callbacks, &text, has_height, has_scaling, 0)?;
    if !has_scaling {
        return Ok(Some(initial));
    }

    let mut next_wrap_id = 1_u64;
    if callbacks.scaling_is_shrink()? {
        shrink_with_callbacks(callbacks, &text, initial, has_height, &mut next_wrap_id)
    } else {
        grow_with_callbacks(callbacks, &text, initial, has_height, &mut next_wrap_id)
    }
}

fn wrap_once<C: ImageFontTextWrapCallbacks>(
    callbacks: &mut C,
    text: &WrapUnit,
    has_height: bool,
    scaled: bool,
    wrap_id: u64,
) -> Result<ImageFontTextWrapResult, C::Error> {
    let mut state = WrapState {
        callbacks,
        has_height,
        lines: Vec::new(),
        position: 0,
        offset: 0,
    };
    wrap_tokens(&mut state, text, scaled)?;
    let consumed = state.position;
    let remaining = text.suffix_from(consumed).as_input();
    state
        .callbacks
        .capture_remaining_text(&remaining, consumed, wrap_id)?;
    Ok(ImageFontTextWrapResult {
        lines: state.lines.into_iter().map(|line| line.as_input()).collect(),
        remaining,
        consumed,
        wrap_id,
    })
}

fn shrink_with_callbacks<C: ImageFontTextWrapCallbacks>(
    callbacks: &mut C,
    text: &WrapUnit,
    initial: ImageFontTextWrapResult,
    has_height: bool,
    next_wrap_id: &mut u64,
) -> Result<Option<ImageFontTextWrapResult>, C::Error> {
    if !callbacks.has_remaining_text(
        &initial.remaining,
        initial.consumed,
        initial.wrap_id,
    )? {
        callbacks.discard_remaining_text(initial.wrap_id);
        return Ok(None);
    }
    let mut current = initial;
    let mut size = callbacks.initial_size(true)?;
    loop {
        if !callbacks.has_remaining_text(&current.remaining, current.consumed, current.wrap_id)? {
            return Ok(Some(current));
        }
        if callbacks.shrink_limit_reached(size)? {
            return Err(PilError::ValueError("Text could not be scaled".into()).into());
        }
        size = adjusted_size(size, -1).map_err(C::Error::from)?;
        callbacks.font_variant(size)?;
        let wrapped = wrap_once(callbacks, text, has_height, true, *next_wrap_id)?;
        *next_wrap_id += 1;
        callbacks.discard_remaining_text(current.wrap_id);
        current = wrapped;
    }
}

fn grow_with_callbacks<C: ImageFontTextWrapCallbacks>(
    callbacks: &mut C,
    text: &WrapUnit,
    initial: ImageFontTextWrapResult,
    has_height: bool,
    next_wrap_id: &mut u64,
) -> Result<Option<ImageFontTextWrapResult>, C::Error> {
    if callbacks.has_remaining_text(
        &initial.remaining,
        initial.consumed,
        initial.wrap_id,
    )? {
        callbacks.discard_remaining_text(initial.wrap_id);
        return Err(PilError::ValueError("Text could not be scaled".into()).into());
    }
    let mut size = callbacks.initial_size(false)?;
    let mut current = initial;
    let mut last_wrap = None;
    loop {
        if callbacks.has_remaining_text(&current.remaining, current.consumed, current.wrap_id)? {
            size = adjusted_size(size, -1).map_err(C::Error::from)?;
            callbacks.restore_font(size)?;
            let Some(previous) = last_wrap.take() else {
                callbacks.missing_grow_wrap()?;
                return Ok(None);
            };
            callbacks.discard_remaining_text(current.wrap_id);
            return Ok(Some(previous));
        }
        if callbacks.grow_limit_reached(size)? {
            return Err(PilError::ValueError("Text could not be scaled".into()).into());
        }
        size = adjusted_size(size, 1).map_err(C::Error::from)?;
        callbacks.font_variant(size)?;
        if let Some(previous) = last_wrap.take() {
            callbacks.discard_remaining_text(previous.wrap_id);
        }
        last_wrap = Some(current);
        let candidate = wrap_once(callbacks, text, has_height, true, *next_wrap_id)?;
        *next_wrap_id += 1;
        current = candidate;
    }
}

fn adjusted_size<E: From<PilError>>(size: i64, amount: i64) -> Result<i64, E> {
    size.checked_add(amount)
        .ok_or_else(|| PilError::OverflowError("font size is outside the integer range".into()))
        .map_err(E::from)
}

fn remaining_text_is_nonempty(text: &ImageFontTextInput) -> bool {
    match text {
        ImageFontTextInput::Text(text) => !text.is_empty(),
        ImageFontTextInput::Bytes(bytes) => !bytes.is_empty(),
        ImageFontTextInput::InvalidType => true,
    }
}

fn text_input_to_unicode(text: &ImageFontTextInput) -> String {
    match text {
        ImageFontTextInput::Text(text) => text.clone(),
        ImageFontTextInput::Bytes(bytes) => pillow_bytes_to_text(bytes),
        ImageFontTextInput::InvalidType => String::new(),
    }
}

fn wrap_tokens<C: ImageFontTextWrapCallbacks>(
    state: &mut WrapState<'_, C>,
    text: &WrapUnit,
    scaled: bool,
) -> Result<(), C::Error> {
    let mut line = text.empty_like();
    let mut completed = true;
    for mut word in text.tokens() {
        let newlines = word.leading_newlines();
        if !newlines.is_empty() {
            if !state.add_line(&line)? {
                completed = false;
                break;
            }
            for (index, newline) in newlines.iter().enumerate() {
                if index != 0 && !state.add_line(&text.empty_like())? {
                    break;
                }
                state.position += newline.len();
                word = word.strip_prefix(newline);
            }
            line = text.empty_like();
        }

        let new_line = line.concatenate(&word);
        if state
            .callbacks
            .width_check(&new_line.as_input(), ImageFontTextWrapWidthCheck::Fits)?
        {
            line = new_line;
            continue;
        }
        if !line.is_empty() && !state.add_line(&line)? {
            completed = false;
            break;
        }
        let original_length = word.len();
        word = word.trim_start();
        state.offset = original_length - word.len();
        if state
            .callbacks
            .width_check(&word.as_input(), ImageFontTextWrapWidthCheck::Exceeds)?
        {
            if !scaled {
                return Err(PilError::ValueError("Word does not fit within line".into()).into());
            }
            completed = false;
            break;
        }
        line = word;
    }

    if completed && !line.is_empty() {
        let _ = state.add_line(&line)?;
    }
    Ok(())
}

fn unicode_word_tokens(text: &str) -> Vec<WrapUnit> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        while index < chars.len() && is_python_whitespace(chars[index].1) {
            index += 1;
        }
        let word_start = index;
        while index < chars.len() && !is_python_whitespace(chars[index].1) {
            index += 1;
        }
        if word_start == index {
            break;
        }
        let start_byte = chars[start].0;
        let end_byte = chars.get(index).map_or(text.len(), |item| item.0);
        tokens.push(WrapUnit::Unicode(text[start_byte..end_byte].to_owned()));
    }
    tokens
}

fn byte_word_tokens(text: &[u8]) -> Vec<WrapUnit> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let start = index;
        while index < text.len() && is_byte_whitespace(text[index]) {
            index += 1;
        }
        let word_start = index;
        while index < text.len() && !is_byte_whitespace(text[index]) {
            index += 1;
        }
        if word_start == index {
            break;
        }
        tokens.push(WrapUnit::Bytes(text[start..index].to_vec()));
    }
    tokens
}

fn unicode_leading_newlines(text: &str) -> Vec<WrapUnit> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let start = index;
        while index < chars.len()
            && chars[index].1 != '\n'
            && is_python_whitespace(chars[index].1)
        {
            index += 1;
        }
        if index < chars.len() && chars[index].1 == '\n' {
            index += 1;
            let start_byte = chars[start].0;
            let end_byte = chars.get(index).map_or(text.len(), |item| item.0);
            segments.push(WrapUnit::Unicode(text[start_byte..end_byte].to_owned()));
        } else {
            break;
        }
    }
    segments
}

fn byte_leading_newlines(text: &[u8]) -> Vec<WrapUnit> {
    let mut segments = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let start = index;
        while index < text.len() && text[index] != b'\n' && is_byte_whitespace(text[index]) {
            index += 1;
        }
        if index < text.len() && text[index] == b'\n' {
            index += 1;
            segments.push(WrapUnit::Bytes(text[start..index].to_vec()));
        } else {
            break;
        }
    }
    segments
}

fn is_python_whitespace(character: char) -> bool {
    character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}')
}

fn is_byte_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c')
}
