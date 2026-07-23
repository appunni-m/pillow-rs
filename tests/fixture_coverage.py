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
                if mode and (
                    call_style in CASE_MODE_CALL_STYLES
                    and (
                        call_style in {"file_open", "palette_method"}
                        or case.get("input") is not None
                        or case.get("input2") is not None
                    )
                ):
                    modes.add(mode)
                parameter_mode = case.get("params", {}).get("mode")
                if isinstance(parameter_mode, str):
                    modes.add(parameter_mode)
                oracle_case = oracle_cases.get(case["id"])
                if oracle_case is not None:
                    modes.update(
                        assertion_image_modes(
                            base_dir,
                            oracle_case.get("assert", {}),
                        )
                    )
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
