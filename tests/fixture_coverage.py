"""Static manifest/fixture coverage validation without importing pillow-rs."""

from __future__ import annotations

import json
from pathlib import Path

import yaml
from PIL import Image as PILImage

from engine import (
    CASE_MODE_CALL_STYLES,
    FONT_METHOD_TARGETS,
    IMAGE_INPUT_CALL_STYLES,
    get_call_style,
)


REPO_ROOT = Path(__file__).parent.parent
FIXTURE_DIRS = {
    "fixtures": REPO_ROOT / "tests" / "fixtures",
    "fixtures_2": REPO_ROOT / "tests" / "fixtures_2",
}
BACKEND_FIXTURE_DIR = (
    REPO_ROOT / "pillow-rs" / "tests" / "fixtures" / "image_backend"
)
BACKEND_PARITY_MANIFEST = BACKEND_FIXTURE_DIR / "backend_parity.json"

# These modes execute the same operation branch and may share one exact oracle
# input. This prevents the inventory gate from rewarding duplicate no-op cases.
MODE_PATH_EQUIVALENCE = {
    "Image.apply_transparency": (
        frozenset({"L", "LA", "RGB", "RGBA"}),
    ),
}


def exact_image_spec(value: object) -> bool:
    """Return whether a backend fixture contains concrete exact image bytes."""
    if not isinstance(value, dict):
        return False
    size = value.get("size")
    pixels = value.get("pixels_hex")
    return (
        isinstance(value.get("mode"), str)
        and isinstance(size, list)
        and len(size) == 2
        and all(isinstance(dimension, int) and dimension > 0 for dimension in size)
        and isinstance(pixels, str)
        and len(pixels) % 2 == 0
    )


def coverage_expected_image(expected: dict) -> dict:
    """Select the exact image fields duplicated into one coverage row."""
    return {
        key: expected.get(key)
        for key in ("mode", "size", "pixels_hex", "palette_hex")
    }


def expected_backend_coverage_rows(
    fixture: dict,
    fixture_dir: Path = BACKEND_FIXTURE_DIR,
) -> tuple[list[dict], list[str]]:
    """Derive trusted coverage rows from concrete generated oracle cases."""
    rows = []
    errors = []

    def add_case(operation: str, case: object, source_key: str | None) -> None:
        if not isinstance(case, dict):
            errors.append(f"invalid concrete backend case for {operation}")
            return
        case_id = case.get("id")
        expected = case.get("expected")
        source = case.get(source_key) if source_key else None
        if (
            not isinstance(case_id, str)
            or not exact_image_spec(expected)
            or (
                source_key is not None
                and not exact_image_spec(source)
            )
        ):
            errors.append(f"invalid concrete backend case: {operation}/{case_id}")
            return
        if source_key is not None and source["mode"] != expected["mode"]:
            errors.append(f"backend case changes mode: {operation}/{case_id}")
            return
        if operation != "ImageDraw.bitmap" and operation != "ImageDraw.text":
            backends = case.get("backends")
            if not isinstance(backends, list) or not backends:
                errors.append(f"backend case has no execution lane: {operation}/{case_id}")
                return
        input_path = case.get("input")
        if input_path is not None and (
            not isinstance(input_path, str)
            or not (fixture_dir / input_path).is_file()
        ):
            errors.append(f"backend case input is missing: {operation}/{case_id}")
            return
        rows.append(
            {
                "operation": operation,
                "mode": expected["mode"],
                "case_id": case_id,
                "expected": coverage_expected_image(expected),
            }
        )

    for case in fixture.get("paste_cases", []):
        if (
            not isinstance(case, dict)
            or not isinstance(case.get("source"), dict)
        ):
            errors.append(f"invalid concrete backend paste case: {case!r}")
            continue
        add_case("Image.paste", case, "destination")
    for case in fixture.get("draw_cases", []):
        operation = case.get("operation") if isinstance(case, dict) else None
        if not isinstance(operation, str):
            errors.append(f"invalid concrete backend draw case: {case!r}")
            continue
        add_case(f"ImageDraw.{operation}", case, "source")
    for case in fixture.get("apply_transparency_cases", []):
        add_case("Image.apply_transparency", case, None)
    for case in fixture.get("indexed_immediate_draw_cases", []):
        operation = case.get("operation") if isinstance(case, dict) else None
        if operation not in {"bitmap", "text"}:
            errors.append(f"invalid indexed draw case: {case!r}")
            continue
        add_case(f"ImageDraw.{operation}", case, None)
    return rows, errors


def backend_parity_coverage_errors(
    fixture: dict,
    fixture_dir: Path = BACKEND_FIXTURE_DIR,
) -> list[str]:
    """Reject missing, orphaned, duplicate, or mismatched coverage rows."""
    expected_rows, errors = expected_backend_coverage_rows(fixture, fixture_dir)
    actual_rows = fixture.get("coverage")
    if not isinstance(actual_rows, list):
        return [*errors, "missing backend coverage metadata"]

    def keyed(rows: list[dict], kind: str) -> tuple[dict[tuple[str, str], dict], list[str]]:
        keyed_rows = {}
        row_errors = []
        for row in rows:
            if not isinstance(row, dict):
                row_errors.append(f"invalid {kind} backend coverage row: {row!r}")
                continue
            key = (row.get("operation"), row.get("case_id"))
            if not all(isinstance(value, str) for value in key):
                row_errors.append(f"invalid {kind} backend coverage key: {row!r}")
            elif key in keyed_rows:
                row_errors.append(
                    f"duplicate {kind} backend coverage row: {key[0]}/{key[1]}"
                )
            else:
                keyed_rows[key] = row
        return keyed_rows, row_errors

    expected, expected_errors = keyed(expected_rows, "expected")
    actual, actual_errors = keyed(actual_rows, "generated")
    errors.extend(expected_errors)
    errors.extend(actual_errors)
    errors.extend(
        f"missing backend coverage row: {operation}/{case_id}"
        for operation, case_id in sorted(expected.keys() - actual.keys())
    )
    errors.extend(
        f"orphan backend coverage row: {operation}/{case_id}"
        for operation, case_id in sorted(actual.keys() - expected.keys())
    )
    errors.extend(
        f"mismatched backend coverage row: {operation}/{case_id}"
        for (operation, case_id) in sorted(expected.keys() & actual.keys())
        if expected[(operation, case_id)] != actual[(operation, case_id)]
    )
    return errors


def backend_parity_operation_modes() -> dict[str, set[str]]:
    """Collect modes only from structurally validated generated coverage."""
    if not BACKEND_PARITY_MANIFEST.is_file():
        return {}
    fixture = json.loads(BACKEND_PARITY_MANIFEST.read_text())
    if backend_parity_coverage_errors(fixture):
        return {}
    covered: dict[str, set[str]] = {}
    for row in fixture["coverage"]:
        covered.setdefault(row["operation"], set()).add(row["mode"])
    return covered


def operation_name(operation: dict) -> str:
    """Return the manifest key covered by one fixture operation."""
    module = operation["module"]
    target = operation["target"]
    owner = operation.get("class")
    if owner:
        return f"{module}.{owner}.{target}"
    if module == "ImageFont" and target in FONT_METHOD_TARGETS:
        return f"{module}.FreeTypeFont.{target}"
    return f"{module}.{target}"


def assertion_image_modes(base_dir: Path, assertion: dict):
    """Yield concrete image modes recorded by an exact oracle assertion."""
    method = assertion.get("method")
    if method == "image":
        reference = assertion.get("reference", "")
        if assertion.get("raw_kind") == "image":
            mode = assertion.get("mode")
            if mode:
                yield mode
        elif reference.endswith(".png"):
            with PILImage.open(base_dir / "outputs" / reference) as image:
                yield image.mode
    elif method in {"image_list", "tuple"}:
        for item in assertion.get("items", []):
            yield from assertion_image_modes(base_dir, item)


def manifest_operation_modes(manifest_path: Path) -> dict[str, set[str]]:
    """Build the implemented manifest operation-to-mode map."""
    manifest = yaml.safe_load(manifest_path.read_text())
    op_modes: dict[str, set[str]] = {}
    for module_name, module_data in manifest.get("modules", {}).items():
        for section in ("class_methods", "methods", "functions"):
            for entry in module_data.get(section, []):
                if entry.get("status") == "implemented":
                    op_modes[f"{module_name}.{entry['name']}"] = set(
                        entry.get("supported_modes", [])
                    )
        for entry in module_data.get("properties", []):
            if isinstance(entry, dict):
                op_modes[f"{module_name}.{entry['name']}"] = set(
                    entry.get("modes", [])
                )
        for class_entry in module_data.get("classes", []):
            if class_entry.get("status") != "implemented":
                continue
            class_name = class_entry["name"]
            class_modes = set(class_entry.get("supported_modes", []))
            op_modes[f"{module_name}.{class_name}"] = class_modes
            for method in class_entry.get("methods", []):
                if method.get("status", "") == "ignored":
                    continue
                op_modes[f"{module_name}.{class_name}.{method['name']}"] = set(
                    method.get("supported_modes", class_modes)
                )
    return op_modes


def fixture_operation_modes(
    fixture_dirs: dict[str, Path] = FIXTURE_DIRS,
) -> dict[str, set[str]]:
    """Build the operation-to-genuinely-exercised-mode map."""
    fixture_modes: dict[str, set[str]] = {}
    for base_dir in fixture_dirs.values():
        input_dir = base_dir / "input" / "jsons"
        output_dir = base_dir / "outputs" / "jsons"
        for input_path in sorted(input_dir.glob("*.json")):
            output_path = output_dir / input_path.name
            if not output_path.is_file():
                continue
            fixture = json.loads(input_path.read_text())
            oracle = json.loads(output_path.read_text())
            operation = fixture["operation"]
            operation_key = operation_name(operation)
            call_style = get_call_style(
                operation["module"],
                operation["target"],
                operation.get("class"),
            )
            modes = fixture_modes.setdefault(operation_key, set())
            oracle_cases = {
                case["id"]: case
                for case in oracle.get("cases", [])
            }
            for case in fixture.get("cases", []):
                mode = case.get("mode", "")
                oracle_case = oracle_cases.get(case["id"])
                is_expected_error = (
                    oracle_case is not None
                    and oracle_case.get("assert", {}).get("method") == "error"
                )
                if mode and (
                    not is_expected_error and call_style in CASE_MODE_CALL_STYLES
                    and (
                        call_style in {"file_open", "palette_method"}
                        or case.get("input") is not None
                        or case.get("input2") is not None
                    )
                ):
                    modes.add(mode)
                parameter_mode = case.get("params", {}).get("mode")
                if isinstance(parameter_mode, str) and not is_expected_error:
                    modes.add(parameter_mode)
                if oracle_case is not None:
                    modes.update(
                        assertion_image_modes(
                            base_dir,
                            oracle_case.get("assert", {}),
                        )
                    )
    for operation, modes in backend_parity_operation_modes().items():
        fixture_modes.setdefault(operation, set()).update(modes)
    for operation, groups in MODE_PATH_EQUIVALENCE.items():
        modes = fixture_modes.get(operation, set())
        for group in groups:
            if modes & group:
                modes.update(group)
    return fixture_modes


def coverage_gaps(
    manifest_path: Path = REPO_ROOT / "manifest.yaml",
    fixture_dirs: dict[str, Path] = FIXTURE_DIRS,
) -> tuple[list[str], list[str]]:
    """Return missing operations and exact ``operation: mode`` gaps."""
    declared = manifest_operation_modes(manifest_path)
    covered = fixture_operation_modes(fixture_dirs)
    missing_operations = sorted(set(declared) - set(covered))
    missing_modes = sorted(
        f"{operation}: {mode}"
        for operation, declared_modes in declared.items()
        for mode in declared_modes - covered.get(operation, set())
    )
    return missing_operations, missing_modes


def unknown_fixture_operations(
    manifest_path: Path = REPO_ROOT / "manifest.yaml",
    fixture_dirs: dict[str, Path] = FIXTURE_DIRS,
) -> list[str]:
    """Return fixture operations that are not implemented manifest entries."""
    declared = manifest_operation_modes(manifest_path)
    covered = fixture_operation_modes(fixture_dirs)
    return sorted(set(covered) - set(declared))


def fixture_usage_errors(
    fixture_dirs: dict[str, Path] = FIXTURE_DIRS,
) -> list[str]:
    """Reject input images and top-level modes ignored by a call style."""
    errors = []
    for base_name, base_dir in fixture_dirs.items():
        for path in sorted((base_dir / "input" / "jsons").glob("*.json")):
            fixture = json.loads(path.read_text())
            operation = fixture["operation"]
            call_style = get_call_style(
                operation["module"],
                operation["target"],
                operation.get("class"),
            )
            for case in fixture.get("cases", []):
                prefix = f"{base_name}/{path.name}/{case.get('id')}"
                if call_style not in IMAGE_INPUT_CALL_STYLES and (
                    case.get("input") is not None
                    or case.get("input2") is not None
                ):
                    errors.append(
                        f"{prefix}: {call_style} ignores fixture input images"
                    )
                if (
                    case.get("mode")
                    and call_style not in CASE_MODE_CALL_STYLES
                ):
                    errors.append(
                        f"{prefix}: {call_style} ignores the top-level fixture mode"
                    )
    return errors


def fixture_pair_errors(
    fixture_dirs: dict[str, Path] = FIXTURE_DIRS,
) -> list[str]:
    """Return missing or orphaned input/oracle JSON pairs."""
    errors = []
    for base_name, base_dir in fixture_dirs.items():
        input_names = {
            path.name
            for path in (base_dir / "input" / "jsons").glob("*.json")
        }
        output_names = {
            path.name
            for path in (base_dir / "outputs" / "jsons").glob("*.json")
        }
        errors.extend(
            f"{base_name}: missing oracle output {name}"
            for name in sorted(input_names - output_names)
        )
        errors.extend(
            f"{base_name}: orphan oracle output {name}"
            for name in sorted(output_names - input_names)
        )
    return errors


def main() -> None:
    """Run the dependency-light fixture coverage gate."""
    errors = fixture_pair_errors()
    errors.extend(fixture_usage_errors())
    if BACKEND_PARITY_MANIFEST.is_file():
        errors.extend(
            backend_parity_coverage_errors(
                json.loads(BACKEND_PARITY_MANIFEST.read_text())
            )
        )
    else:
        errors.append("missing backend parity manifest")
    errors.extend(
        f"unknown fixture operation: {operation}"
        for operation in unknown_fixture_operations()
    )
    missing_operations, missing_modes = coverage_gaps()
    errors.extend(
        f"missing fixture operation: {operation}"
        for operation in missing_operations
    )
    errors.extend(f"missing fixture mode: {gap}" for gap in missing_modes)
    if errors:
        raise SystemExit(
            f"fixture coverage failed with {len(errors)} issue(s):\n"
            + "\n".join(f"  {error}" for error in errors)
        )
    print("fixture coverage is complete and uses only semantic inputs/modes")


if __name__ == "__main__":
    main()
