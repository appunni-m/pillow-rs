#!/usr/bin/env python3
"""Generate a three-way FreeType API/ABI surface audit.

The audit compares:

1. Pinned FreeType C public headers under ``freetype/include/freetype``.
2. Servo ``rust-freetype`` bindgen-style Rust source.
3. This crate's public Rust surface plus ``tests/data/interface_map.json``.

The output is intentionally written to ``target/`` because it is a generated
diagnostic artifact, not a committed oracle fixture.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SERVO_ROOT = ROOT / "target" / "external" / "rust-freetype"
OUTPUT_DIR = ROOT / "target" / "api-abi-audit"


def read_text(path: Path) -> str:
    return path.read_text(errors="ignore") if path.exists() else ""


def strip_c_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//.*", " ", text)


def ensure_servo_root(path: Path) -> Path:
    if (path / "src" / "freetype.rs").exists():
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["git", "clone", "--depth", "1", "https://github.com/servo/rust-freetype", str(path)],
        check=True,
    )
    return path


def parse_c_headers(include_root: Path) -> dict:
    inventory = {
        "functions": {},
        "macros": {},
        "typedefs": {},
        "structs": {},
        "enums": {},
        "fields": {},
    }
    header_root = include_root / "freetype"
    for path in sorted(header_root.rglob("*.h")):
        raw = read_text(path)
        text = strip_c_comments(raw)
        rel = str(path.relative_to(include_root))

        for match in re.finditer(
            r"FT_EXPORT\s*\(([^)]*)\)\s*([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*;",
            text,
            re.S,
        ):
            ret, name, params = match.groups()
            inventory["functions"][name] = {
                "return": normalize_ws(ret),
                "params": normalize_ws(params),
                "file": rel,
            }

        for match in re.finditer(r"^\s*#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\b([^\n]*)", raw, re.M):
            name, value = match.groups()
            if c_public_name(name):
                inventory["macros"][name] = {"value": value.strip(), "file": rel}

        for match in re.finditer(r"typedef\s+([^;{}]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", text):
            definition, name = match.groups()
            if c_public_name(name):
                inventory["typedefs"][name] = {
                    "definition": normalize_ws(definition),
                    "file": rel,
                }

        for kind in ("struct", "enum"):
            pattern = (
                r"typedef\s+"
                + kind
                + r"\s*(?:[A-Za-z_][A-Za-z0-9_]*)?\s*\{(.*?)\}\s*([A-Za-z_][A-Za-z0-9_]*)\s*;"
            )
            for match in re.finditer(pattern, text, re.S):
                body, name = match.groups()
                if not c_public_name(name):
                    continue
                bucket = "structs" if kind == "struct" else "enums"
                inventory[bucket][name] = {"file": rel}
                inventory["fields"][name] = parse_c_fields(kind, body)

    return inventory


def parse_c_fields(kind: str, body: str) -> list[str]:
    if kind == "enum":
        return re.findall(r"\b([A-Z][A-Z0-9_]+)\b\s*(?:=|,|$)", body)
    fields = []
    for part in body.split(";"):
        part = normalize_ws(part)
        if not part or part.startswith("#"):
            continue
        declarators = part.split(",")
        if not declarators:
            continue
        first = declarators[0]
        first_match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^]]+\])?$", first)
        if first_match:
            fields.append(first_match.group(1))
        for declarator in declarators[1:]:
            match = re.search(r"^\s*\*?\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^]]+\])?$", declarator)
            if match:
                fields.append(match.group(1))
    return fields


def parse_servo(path: Path) -> dict:
    text = read_text(path / "src" / "freetype.rs")
    inventory = {"functions": {}, "consts": {}, "types": {}, "structs": {}, "enums": {}, "fields": {}}
    for match in re.finditer(
        r"pub\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*(?:->\s*([^;{]+))?;",
        text,
        re.S,
    ):
        name, params, ret = match.groups()
        inventory["functions"][name] = {"params": normalize_ws(params), "return": normalize_ws(ret or "")}
    for match in re.finditer(r"pub\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", text):
        inventory["consts"][match.group(1)] = {}
    for match in re.finditer(r"pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=", text):
        inventory["types"][match.group(1)] = {}
    for kind, bucket in (("struct", "structs"), ("enum", "enums")):
        for match in re.finditer(r"pub\s+" + kind + r"\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{(.*?)\n\}", text, re.S):
            name, body = match.groups()
            inventory[bucket][name] = {}
            if kind == "struct":
                fields = re.findall(r"pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", body)
            else:
                fields = re.findall(r"\b(FT_[A-Z0-9_]+)\b\s*(?:=|,)", body)
            inventory["fields"][name] = fields
    return inventory


def parse_fontdone(src_root: Path) -> dict:
    inventory = {"functions": {}, "consts": {}, "structs": {}, "enums": {}, "fields": {}, "modules": {}}
    for path in sorted(src_root.rglob("*.rs")):
        text = read_text(path)
        rel = str(path.relative_to(src_root))
        for match in re.finditer(r"pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*)", text):
            inventory["modules"][match.group(1)] = {"file": rel}
        for match in re.finditer(
            r"pub(?:\([^)]*\))?\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*(?:->\s*([^\n{]+))?",
            text,
            re.S,
        ):
            name, params, ret = match.groups()
            inventory["functions"][name] = {
                "params": normalize_ws(params),
                "return": normalize_ws(ret or ""),
                "file": rel,
            }
        for match in re.finditer(r"pub\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", text):
            inventory["consts"][match.group(1)] = {"file": rel}
        for kind, bucket in (("struct", "structs"), ("enum", "enums")):
            for match in re.finditer(
                r"pub\s+" + kind + r"\s+([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{]+>)?\s*\{(.*?)\n\}",
                text,
                re.S,
            ):
                name, body = match.groups()
                inventory[bucket][name] = {"file": rel}
                if kind == "struct":
                    fields = re.findall(r"pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", body)
                else:
                    fields = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:,|\(|\{)", body, re.M)
                inventory["fields"][name] = fields
    return inventory


def load_interface_map(path: Path) -> dict:
    if not path.exists():
        return {}
    data = json.loads(path.read_text())
    mapped = {}
    for group in data["paths"]:
        for symbol, meta in group["symbols"].items():
            mapped[symbol] = {
                "path": group["path"],
                "status": meta.get("status", "unknown"),
                "rust": meta.get("rust"),
                "reason": meta.get("reason"),
            }
    return mapped


def c_public_name(name: str) -> bool:
    return name.startswith(("FT_", "TT_", "T1_", "FTC_"))


def normalize_ws(text: str) -> str:
    return " ".join(text.split())


TYPE_MAP = {
    "FT_BBox": "BBox",
    "FT_Bitmap": "RenderedBitmap",
    "FT_CharMapRec": "CharmapInfo",
    "FT_FaceRec": "FaceInfo/Face",
    "FT_GlyphSlotRec": "GlyphSlot",
    "FT_Glyph_Metrics": "GlyphSlotMetrics",
    "FT_Outline": "Outline",
    "FT_Pixel_Mode": "PixelMode",
    "FT_Render_Mode": "RenderMode",
    "FT_Size_Metrics": "SizeMetrics",
    "FT_Vector": "Vector",
}


def servo_has_type(servo: dict, c_type: str) -> bool:
    return (
        c_type in servo["types"]
        or c_type in servo["structs"]
        or f"{c_type}_" in servo["structs"]
        or c_type in servo["enums"]
        or f"{c_type}_" in servo["enums"]
    )


def classify_function(symbol: str, c: dict, servo: dict, interface: dict) -> dict:
    meta = interface.get(symbol, {})
    status = meta.get("status", "unknown")
    rust = meta.get("rust")
    servo_present = symbol in servo["functions"]
    abi = "missing"
    exactness = "unmapped"
    if servo_present:
        abi = "c_ffi_binding"
    if status == "complete":
        exactness = "semantic_mapped"
    elif status == "partial":
        exactness = "partial_semantic"
    elif status == "planned":
        exactness = "not_implemented"
    elif status == "out_of_scope":
        exactness = "intentionally_excluded"
    return {
        "symbol": symbol,
        "c_return": c["functions"][symbol]["return"],
        "c_params": c["functions"][symbol]["params"],
        "c_file": c["functions"][symbol]["file"],
        "servo_present": servo_present,
        "servo_abi": abi,
        "fontdone_status": status,
        "fontdone_mapping": rust or "",
        "exactness": exactness,
    }


def classify_type(name: str, c: dict, servo: dict, fontdone: dict) -> dict:
    mapped = TYPE_MAP.get(name, "")
    fields = c["fields"].get(name, [])
    our_fields = fontdone["fields"].get(mapped, []) if mapped in fontdone["fields"] else []
    field_exact = bool(mapped) and fields == our_fields
    return {
        "type": name,
        "kind": "struct" if name in c["structs"] else "enum" if name in c["enums"] else "typedef",
        "servo_present": servo_has_type(servo, name),
        "fontdone_mapping": mapped,
        "c_field_count": len(fields),
        "fontdone_field_count": len(our_fields),
        "field_order_exact": field_exact,
        "c_fields": fields,
        "fontdone_fields": our_fields,
    }


def classify_constant(name: str, c: dict, servo: dict) -> dict:
    mapped = ""
    if name.startswith("FT_LOAD_"):
        mapped = "LoadFlags subset"
    elif name.startswith("FT_RENDER_MODE_"):
        mapped = "RenderMode"
    elif name.startswith("FT_PIXEL_MODE_"):
        mapped = "PixelMode"
    return {
        "constant": name,
        "c_value": c["macros"][name]["value"],
        "c_file": c["macros"][name]["file"],
        "servo_present": name in servo["consts"] or any(name in fields for fields in servo["fields"].values()),
        "fontdone_mapping": mapped,
    }


def markdown_table(headers: list[str], rows: list[dict], limit: int | None = None) -> str:
    selected = rows if limit is None else rows[:limit]
    lines = ["| " + " | ".join(headers) + " |", "| " + " | ".join(["---"] * len(headers)) + " |"]
    for row in selected:
        values = []
        for header in headers:
            value = str(row.get(header, ""))
            value = value.replace("|", "\\|").replace("\n", " ")
            if len(value) > 180:
                value = value[:177] + "..."
            values.append(value)
        lines.append("| " + " | ".join(values) + " |")
    return "\n".join(lines)


def write_report(data: dict, output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "api_abi_audit.json").write_text(json.dumps(data, indent=2))

    functions = data["functions"]
    types = data["types"]
    constants = data["constants"]
    counts = data["counts"]
    status_counts = {}
    for row in functions:
        status_counts[row["fontdone_status"]] = status_counts.get(row["fontdone_status"], 0) + 1

    critical_types = [
        row
        for row in types
        if row["type"]
        in {
            "FT_FaceRec",
            "FT_Size_Metrics",
            "FT_GlyphSlotRec",
            "FT_Glyph_Metrics",
            "FT_Bitmap",
            "FT_Outline",
            "FT_BBox",
            "FT_Vector",
            "FT_Pixel_Mode",
            "FT_Render_Mode",
        }
    ]

    md = [
        "# FreeType API/ABI Three-Way Audit",
        "",
        "This is a generated diagnostic report. It compares pinned FreeType C headers, Servo `rust-freetype`, and local `fontdone`.",
        "",
        "## Key Point",
        "",
        "Servo `rust-freetype` is a C FFI binding surface, not a complete safe API design. Its C-like names and raw fields are useful as an ABI checklist, but not as proof that the pure Rust API should copy every raw pointer or module-management detail.",
        "",
        "For a future C-library replacement, `fontdone` needs a separate C ABI layer that exports `FT_*` symbols and `repr(C)` records. The current safe Rust API can be semantically compatible without being ABI-compatible.",
        "",
        "## Counts",
        "",
        markdown_table(["metric", "count"], [{"metric": k, "count": v} for k, v in counts.items()]),
        "",
        "## Function Status",
        "",
        markdown_table(["status", "count"], [{"status": k, "count": v} for k, v in sorted(status_counts.items())]),
        "",
        "## Critical Record Exactness",
        "",
        markdown_table(
            [
                "type",
                "servo_present",
                "fontdone_mapping",
                "c_field_count",
                "fontdone_field_count",
                "field_order_exact",
                "c_fields",
                "fontdone_fields",
            ],
            critical_types,
        ),
        "",
        "## Functions",
        "",
        markdown_table(
            [
                "symbol",
                "servo_present",
                "fontdone_status",
                "fontdone_mapping",
                "exactness",
                "c_return",
                "c_params",
                "c_file",
            ],
            functions,
        ),
        "",
        "## Types / Structs / Enums",
        "",
        markdown_table(
            [
                "type",
                "kind",
                "servo_present",
                "fontdone_mapping",
                "c_field_count",
                "fontdone_field_count",
                "field_order_exact",
                "c_fields",
                "fontdone_fields",
            ],
            types,
        ),
        "",
        "## Constants / Macros",
        "",
        markdown_table(["constant", "servo_present", "fontdone_mapping", "c_value", "c_file"], constants),
    ]
    (output_dir / "api_abi_audit.md").write_text("\n".join(md))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--servo-root", type=Path, default=DEFAULT_SERVO_ROOT)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    args = parser.parse_args()

    include_root = ROOT / "freetype" / "include"
    if not (include_root / "freetype" / "freetype.h").exists():
        subprocess.run(["bash", str(ROOT / "scripts" / "fetch_ft.sh")], check=True)

    servo_root = ensure_servo_root(args.servo_root)
    c = parse_c_headers(include_root)
    servo = parse_servo(servo_root)
    fontdone = parse_fontdone(ROOT / "src")
    interface = load_interface_map(ROOT / "tests" / "data" / "interface_map.json")

    functions = [classify_function(symbol, c, servo, interface) for symbol in sorted(c["functions"])]
    types = [
        classify_type(name, c, servo, fontdone)
        for name in sorted(set(c["typedefs"]) | set(c["structs"]) | set(c["enums"]))
    ]
    constants = [classify_constant(name, c, servo) for name in sorted(c["macros"])]

    data = {
        "counts": {
            "c_functions": len(c["functions"]),
            "c_macros": len(c["macros"]),
            "c_typedefs": len(c["typedefs"]),
            "c_structs": len(c["structs"]),
            "c_enums": len(c["enums"]),
            "servo_functions": len(servo["functions"]),
            "servo_consts": len(servo["consts"]),
            "servo_types": len(servo["types"]),
            "servo_structs": len(servo["structs"]),
            "servo_enums": len(servo["enums"]),
            "fontdone_pub_functions": len(fontdone["functions"]),
            "fontdone_pub_consts": len(fontdone["consts"]),
            "fontdone_pub_structs": len(fontdone["structs"]),
            "fontdone_pub_enums": len(fontdone["enums"]),
        },
        "functions": functions,
        "types": types,
        "constants": constants,
    }
    write_report(data, args.output_dir)
    print(args.output_dir / "api_abi_audit.md")
    print(args.output_dir / "api_abi_audit.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
