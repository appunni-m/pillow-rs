"""Typed value-tree encoding shared by project adapters and PostgreSQL."""

from __future__ import annotations

from array import array
from dataclasses import dataclass
import hashlib
import math
import struct
from collections.abc import Mapping, Sequence
import sys
from typing import Any
import zlib

from .model import AssetReference, StepBinding

MAX_VALUE_DEPTH = 128
MAX_VALUE_NODES = 5_000_000
PACKED_ARRAY_THRESHOLD = 256


@dataclass(slots=True)
class FlatValue:
    local_id: int
    path: tuple[str | int, ...]
    kind: str
    text_value: str | None = None
    integer_value: int | None = None
    boolean_value: bool | None = None
    float64_bits: bytes | None = None
    blob_sha256: bytes | None = None
    blob_raw: bytes | None = None
    reference_id: str | None = None
    numeric_array_dtype: str | None = None
    numeric_array_shape: tuple[int, ...] | None = None


@dataclass(frozen=True, slots=True)
class FlatChild:
    parent_local_id: int
    child_ordinal: int
    object_key: str | None
    child_local_id: int


@dataclass(slots=True)
class FlatTree:
    nodes: list[FlatValue]
    children: list[FlatChild]
    path_ids: dict[tuple[str | int, ...], int]
    root_local_id: int


def flatten_value(root: Any) -> FlatTree:
    nodes: list[FlatValue] = []
    children: list[FlatChild] = []
    path_ids: dict[tuple[str | int, ...], int] = {}
    active_containers: set[int] = set()

    def visit(value: Any, path: tuple[str | int, ...], depth: int) -> int:
        if depth > MAX_VALUE_DEPTH:
            raise ValueError(f"typed value exceeds maximum depth {MAX_VALUE_DEPTH}")
        if len(nodes) >= MAX_VALUE_NODES:
            raise ValueError(f"typed value exceeds maximum node count {MAX_VALUE_NODES}")

        local_id = len(nodes)
        node = _flat_scalar(local_id, path, value)
        nodes.append(node)
        path_ids[path] = local_id

        if node.numeric_array_dtype is not None:
            return local_id
        if isinstance(value, Mapping):
            _enter_container(value, active_containers)
            try:
                for ordinal, (key, child) in enumerate(value.items()):
                    if not isinstance(key, str):
                        raise TypeError("object keys must be strings")
                    child_id = visit(child, path + (key,), depth + 1)
                    children.append(FlatChild(local_id, ordinal, key, child_id))
            finally:
                active_containers.remove(id(value))
        elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
            _enter_container(value, active_containers)
            try:
                for ordinal, child in enumerate(value):
                    child_id = visit(child, path + (ordinal,), depth + 1)
                    children.append(FlatChild(local_id, ordinal, None, child_id))
            finally:
                active_containers.remove(id(value))
        return local_id

    root_local_id = visit(root, (), 0)
    return FlatTree(nodes, children, path_ids, root_local_id)


def _flat_scalar(local_id: int, path: tuple[str | int, ...], value: Any) -> FlatValue:
    if value is None:
        return FlatValue(local_id, path, "null")
    if isinstance(value, bool):
        return FlatValue(local_id, path, "boolean", boolean_value=value)
    if isinstance(value, int):
        return FlatValue(local_id, path, "integer", integer_value=value)
    if isinstance(value, float):
        return FlatValue(
            local_id,
            path,
            "float64",
            float64_bits=struct.pack(">d", value),
        )
    if isinstance(value, str):
        return FlatValue(local_id, path, "text", text_value=value)
    if isinstance(value, (bytes, bytearray, memoryview)):
        raw = bytes(value)
        return FlatValue(
            local_id,
            path,
            "bytes",
            blob_sha256=hashlib.sha256(raw).digest(),
            blob_raw=raw,
        )
    packed = _pack_numeric_array(value)
    if packed is not None:
        raw, dtype, shape = packed
        return FlatValue(
            local_id,
            path,
            "array",
            blob_sha256=hashlib.sha256(raw).digest(),
            blob_raw=raw,
            numeric_array_dtype=dtype,
            numeric_array_shape=shape,
        )
    if isinstance(value, AssetReference):
        return FlatValue(local_id, path, "asset-reference", reference_id=value.asset_id)
    if isinstance(value, StepBinding):
        return FlatValue(local_id, path, "step-binding", reference_id=value.step_id)
    if isinstance(value, Mapping):
        return FlatValue(local_id, path, "object")
    if isinstance(value, Sequence):
        return FlatValue(local_id, path, "array")
    raise TypeError(f"unsupported typed value: {type(value).__name__}")


def _enter_container(value: Any, active: set[int]) -> None:
    identity = id(value)
    if identity in active:
        raise ValueError("cyclic values cannot be stored")
    active.add(identity)


def _pack_numeric_array(value: Any) -> tuple[bytes, str, tuple[int, ...]] | None:
    """Pack large rectangular numeric sequences while preserving their values."""

    info = _numeric_array_info(value, set())
    if info is None:
        return None
    scalar_type, shape, item_count = info
    if item_count < PACKED_ARRAY_THRESHOLD:
        return None

    if scalar_type is float:
        packed = array("d", _iter_numeric_items(value))
        if packed.itemsize != 8:
            return None
        if sys.byteorder != "little":
            packed.byteswap()
        return packed.tobytes(), "f64le", shape

    minimum: int | None = None
    maximum: int | None = None
    for item in _iter_numeric_items(value):
        minimum = item if minimum is None else min(minimum, item)
        maximum = item if maximum is None else max(maximum, item)
    if minimum is None or maximum is None:
        return None
    dtype = _integer_array_dtype(minimum, maximum)
    if dtype is None:
        return None
    typecode, width = _ARRAY_TYPES[dtype]
    packed = array(typecode, _iter_numeric_items(value))
    if packed.itemsize != width:
        return None
    if sys.byteorder != "little":
        packed.byteswap()
    return packed.tobytes(), dtype, shape


def _numeric_array_info(
    value: Any, active: set[int], depth: int = 0
) -> tuple[type, tuple[int, ...], int] | None:
    if depth > MAX_VALUE_DEPTH:
        return None
    if not isinstance(value, (list, tuple)) or not value:
        return None
    identity = id(value)
    if identity in active:
        return None
    active.add(identity)
    try:
        first = value[0]
        if type(first) in {int, float}:
            scalar_type = type(first)
            if any(type(item) is not scalar_type for item in value):
                return None
            return scalar_type, (len(value),), len(value)
        if not isinstance(first, (list, tuple)):
            return None
        first_info = _numeric_array_info(first, active, depth + 1)
        if first_info is None:
            return None
        scalar_type, child_shape, child_count = first_info
        for child in value[1:]:
            child_info = _numeric_array_info(child, active, depth + 1)
            if child_info is None or child_info[:2] != first_info[:2]:
                return None
        return scalar_type, (len(value), *child_shape), len(value) * child_count
    finally:
        active.remove(identity)


def _iter_numeric_items(value: Any):
    if isinstance(value, (list, tuple)):
        for child in value:
            yield from _iter_numeric_items(child)
    else:
        yield value


_ARRAY_TYPES = {
    "u8": ("B", 1),
    "u16": ("H", 2),
    "u32": ("I", 4),
    "u64": ("Q", 8),
    "i8": ("b", 1),
    "i16": ("h", 2),
    "i32": ("i", 4),
    "i64": ("q", 8),
    "f64le": ("d", 8),
}


def _integer_array_dtype(minimum: int, maximum: int) -> str | None:
    if minimum >= 0:
        for bits in (8, 16, 32, 64):
            if maximum < 1 << bits:
                return f"u{bits}"
        return None
    for bits in (8, 16, 32, 64):
        if -(1 << (bits - 1)) <= minimum and maximum < 1 << (bits - 1):
            return f"i{bits}"
    return None


def unpack_numeric_array(raw: bytes, dtype: str, shape: Sequence[int]) -> Any:
    """Rebuild the original nested list from its canonical little-endian blob."""

    try:
        typecode, width = _ARRAY_TYPES[dtype]
    except KeyError as exc:
        raise ValueError(f"unknown packed numeric array type: {dtype!r}") from exc
    if not shape or any(type(length) is not int or length <= 0 for length in shape):
        raise ValueError("packed numeric array shape must contain positive dimensions")
    item_count = math.prod(shape)
    if len(raw) != item_count * width:
        raise ValueError("packed numeric array byte length does not match its shape")
    packed = array(typecode)
    packed.frombytes(raw)
    if packed.itemsize != width:
        raise ValueError("packed numeric array host type width is unsupported")
    if sys.byteorder != "little":
        packed.byteswap()
    flat = packed.tolist()

    def rebuild(offset: int, dimension: int) -> tuple[Any, int]:
        length = shape[dimension]
        if dimension == len(shape) - 1:
            return flat[offset : offset + length], offset + length
        result = []
        for _ in range(length):
            child, offset = rebuild(offset, dimension + 1)
            result.append(child)
        return result, offset

    value, end = rebuild(0, 0)
    if end != item_count:
        raise ValueError("packed numeric array reconstruction did not consume its values")
    return value


def compress_blob(raw: bytes) -> tuple[str, bytes]:
    compressed = zlib.compress(raw, level=6)
    if len(compressed) < len(raw):
        return "zlib", compressed
    return "none", raw


def expand_blob(compression: str, payload: bytes, expected_length: int, digest: bytes) -> bytes:
    if compression == "none":
        value = payload
    elif compression == "zlib":
        value = zlib.decompress(payload)
    else:
        raise ValueError(f"unknown parity blob compression: {compression}")
    if len(value) != expected_length or hashlib.sha256(value).digest() != digest:
        raise ValueError("parity blob failed length or SHA-256 verification")
    return value


def canonical_value_digest(value: Any) -> bytes:
    """Hash a typed value tree without JSON or map-order dependence."""

    digest = hashlib.sha256()

    def feed(raw: bytes) -> None:
        digest.update(len(raw).to_bytes(8, "big"))
        digest.update(raw)

    def visit(item: Any, depth: int) -> None:
        if depth > MAX_VALUE_DEPTH:
            raise ValueError(f"typed value exceeds maximum depth {MAX_VALUE_DEPTH}")
        if item is None:
            digest.update(b"n")
        elif isinstance(item, bool):
            digest.update(b"b1" if item else b"b0")
        elif isinstance(item, int):
            digest.update(b"i")
            feed(str(item).encode("ascii"))
        elif isinstance(item, float):
            digest.update(b"f")
            digest.update(struct.pack(">d", item))
        elif isinstance(item, str):
            digest.update(b"s")
            feed(item.encode("utf-8"))
        elif isinstance(item, (bytes, bytearray, memoryview)):
            raw = bytes(item)
            digest.update(b"x")
            digest.update(len(raw).to_bytes(8, "big"))
            digest.update(hashlib.sha256(raw).digest())
        elif isinstance(item, AssetReference):
            digest.update(b"r")
            feed(item.asset_id.encode("utf-8"))
        elif isinstance(item, StepBinding):
            digest.update(b"q")
            feed(item.step_id.encode("utf-8"))
        elif isinstance(item, Mapping):
            digest.update(b"o")
            keys = sorted(item)
            digest.update(len(keys).to_bytes(8, "big"))
            for key in keys:
                if not isinstance(key, str):
                    raise TypeError("object keys must be strings")
                feed(key.encode("utf-8"))
                visit(item[key], depth + 1)
        elif isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
            digest.update(b"a")
            digest.update(len(item).to_bytes(8, "big"))
            for child in item:
                visit(child, depth + 1)
        else:
            raise TypeError(f"unsupported typed value: {type(item).__name__}")

    visit(value, 0)
    return digest.digest()


def case_set_digest(cases: Sequence[tuple[str, bytes]]) -> bytes:
    """Hash an ordered-by-ID case selection and each immutable case digest."""

    ordered = sorted(cases, key=lambda item: item[0])
    if len({case_id for case_id, _digest in ordered}) != len(ordered):
        raise ValueError("case selection contains duplicate IDs")
    digest = hashlib.sha256(b"parity-case-set-v1\0")
    for case_id, case_digest in ordered:
        if len(case_digest) != 32:
            raise ValueError(f"case {case_id!r} does not have a SHA-256 digest")
        encoded_id = case_id.encode("utf-8")
        digest.update(len(encoded_id).to_bytes(8, "big"))
        digest.update(encoded_id)
        digest.update(case_digest)
    return digest.digest()


def rebuild_value_tree(
    root_id: int,
    rows: Sequence[Mapping[str, Any]],
    children: Mapping[int, Sequence[tuple[int, str | None, int]]],
    blobs: Mapping[bytes, bytes],
) -> Any:
    by_id = {int(row["value_id"]): row for row in rows}
    cache: dict[int, Any] = {}

    def build(value_id: int) -> Any:
        if value_id in cache:
            return cache[value_id]
        row = by_id[value_id]
        kind = row["value_kind"]
        if kind == "numeric-array":
            digest = bytes(row["blob_sha256"])
            value = unpack_numeric_array(
                blobs[digest], row["numeric_array_dtype"], row["numeric_array_shape"]
            )
        elif kind == "null":
            value: Any = None
        elif kind == "boolean":
            value = bool(row["boolean_value"])
        elif kind == "integer":
            value = int(row["integer_value"])
        elif kind == "float64":
            value = struct.unpack(">d", bytes(row["float64_bits"]))[0]
        elif kind == "text":
            value = row["text_value"]
        elif kind == "bytes":
            digest = bytes(row["blob_sha256"])
            value = blobs[digest]
        elif kind == "asset-reference":
            value = AssetReference(row["reference_id"])
        elif kind == "step-binding":
            value = StepBinding(row["reference_id"])
        elif kind == "array":
            ordered = sorted(children.get(value_id, ()))
            value = [build(child_id) for _ordinal, _key, child_id in ordered]
        elif kind == "object":
            ordered = sorted(children.get(value_id, ()))
            value = {key: build(child_id) for _ordinal, key, child_id in ordered}
        else:
            raise ValueError(f"unknown parity value kind: {kind}")
        cache[value_id] = value
        return value

    return build(root_id)
