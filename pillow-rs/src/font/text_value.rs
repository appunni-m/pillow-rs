use super::ImageFontTextInput;
use crate::error::PilError;
use std::iter::Copied;
use std::slice::Iter;

/// Borrowed view of text values accepted by Pillow, including surrogate codepoints.
#[derive(Clone, Copy)]
pub(crate) enum FontTextRef<'a> {
    String(&'a str),
    Codepoints(&'a [u32]),
}

/// Owned text value used while a host input is routed through font operations.
pub(crate) enum FontText {
    String(String),
    Codepoints(Vec<u32>),
}

impl FontText {
    pub(crate) fn as_ref(&self) -> FontTextRef<'_> {
        match self {
            Self::String(text) => FontTextRef::String(text),
            Self::Codepoints(text) => FontTextRef::Codepoints(text),
        }
    }
}

impl<'a> FontTextRef<'a> {
    pub(crate) fn to_input(self) -> ImageFontTextInput {
        match self {
            Self::String(text) => ImageFontTextInput::Text(text.to_owned()),
            Self::Codepoints(text) => ImageFontTextInput::TextCodepoints(text.to_vec()),
        }
    }

    pub(crate) fn len(self) -> usize {
        match self {
            Self::String(text) => text.chars().count(),
            Self::Codepoints(text) => text.len(),
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        match self {
            Self::String(text) => text.is_empty(),
            Self::Codepoints(text) => text.is_empty(),
        }
    }

    pub(crate) fn contains(self, value: char) -> bool {
        self.codepoints()
            .any(|codepoint| codepoint == u32::from(value))
    }

    pub(crate) fn split(self, separator: char) -> Vec<Self> {
        match self {
            Self::String(text) => text.split(separator).map(Self::String).collect(),
            Self::Codepoints(text) => text
                .split(|codepoint| *codepoint == u32::from(separator))
                .map(Self::Codepoints)
                .collect(),
        }
    }

    pub(crate) fn codepoints(self) -> FontTextCodepoints<'a> {
        match self {
            Self::String(text) => FontTextCodepoints::String(text.chars()),
            Self::Codepoints(text) => FontTextCodepoints::Codepoints(text.iter().copied()),
        }
    }
}

#[derive(Clone)]
pub(crate) enum FontTextCodepoints<'a> {
    String(std::str::Chars<'a>),
    Codepoints(Copied<Iter<'a, u32>>),
}

impl Iterator for FontTextCodepoints<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::String(text) => text.next().map(u32::from),
            Self::Codepoints(text) => text.next(),
        }
    }
}

impl ImageFontTextInput {
    pub(crate) fn into_font_text(self) -> Result<FontText, PilError> {
        match self {
            Self::Text(text) => Ok(FontText::String(text)),
            Self::TextCodepoints(text) => Ok(FontText::Codepoints(text)),
            Self::Bytes(bytes) => Ok(FontText::String(super::pillow_bytes_to_text(&bytes))),
            Self::InvalidType => Err(PilError::TypeError("expected string or bytes".into())),
        }
    }
}
