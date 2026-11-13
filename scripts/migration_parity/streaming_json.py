"""Small standard-library streaming reader for large JSON array documents."""

from __future__ import annotations

import json
from typing import Any, Iterator, TextIO

_BUFFER_SIZE = 64 * 1024
_WHITESPACE = " \t\r\n"
_NUMBER_CONTINUATION = "0123456789.eE+-"


class _JsonStream:
    def __init__(self, source: TextIO) -> None:
        self._source = source
        self._decoder = json.JSONDecoder()
        self._buffer = ""
        self._position = 0
        self._eof = False

    def value(self) -> Any:
        self._skip_whitespace()
        while True:
            try:
                value, end = self._decoder.raw_decode(self._buffer, self._position)
            except json.JSONDecodeError:
                if self._eof:
                    raise
                self._read_more()
                continue
            number_continues = (
                isinstance(value, (int, float))
                and end < len(self._buffer)
                and self._buffer[end] in _NUMBER_CONTINUATION
            )
            if not self._eof and (end == len(self._buffer) or number_continues):
                self._read_more()
                continue
            self._position = end
            return value

    def expect(self, character: str) -> None:
        if len(character) != 1:
            raise ValueError("expected token must be one character")
        self._skip_whitespace()
        if self._position >= len(self._buffer) or self._buffer[self._position] != character:
            raise ValueError(f"expected {character!r} in streamed JSON document")
        self._position += 1

    def peek(self) -> str:
        self._skip_whitespace()
        if self._position >= len(self._buffer):
            raise ValueError("unexpected end of streamed JSON document")
        return self._buffer[self._position]

    def require_eof(self) -> None:
        self._skip_whitespace()
        if self._position != len(self._buffer) or not self._eof:
            raise ValueError("trailing content after streamed JSON document")

    def _skip_whitespace(self) -> None:
        while True:
            while self._position < len(self._buffer) and self._buffer[self._position] in _WHITESPACE:
                self._position += 1
            if self._position < len(self._buffer) or self._eof:
                return
            self._compact()
            self._read_more()

    def _compact(self) -> None:
        if self._position:
            self._buffer = self._buffer[self._position :]
            self._position = 0

    def _read_more(self) -> None:
        chunk = self._source.read(_BUFFER_SIZE)
        if chunk:
            self._buffer += chunk
        else:
            self._eof = True


def iter_object_array(source: TextIO, array_key: str) -> Iterator[Any]:
    """Yield one item at a time from a named top-level JSON object array.

    Python's JSON decoder is used for each element, so authored fixtures using
    Python's supported ``NaN`` and ``Infinity`` number spellings keep their
    original floating-point values. The input stream is consumed through EOF
    to validate the trailing document structure.
    """

    stream = _JsonStream(source)
    stream.expect("{")
    keys: set[str] = set()
    array_seen = False
    first_member = True
    while stream.peek() != "}":
        if not first_member:
            stream.expect(",")
        key = stream.value()
        if not isinstance(key, str) or key in keys:
            raise ValueError("streamed JSON document has an invalid or duplicate key")
        keys.add(key)
        stream.expect(":")
        if key != array_key:
            stream.value()
        else:
            if array_seen:
                raise ValueError(f"streamed JSON document repeats {array_key!r}")
            array_seen = True
            stream.expect("[")
            first_item = True
            while stream.peek() != "]":
                if not first_item:
                    stream.expect(",")
                yield stream.value()
                first_item = False
            stream.expect("]")
        first_member = False
    stream.expect("}")
    if not array_seen:
        raise ValueError(f"streamed JSON document has no {array_key!r} array")
    stream.require_eof()
