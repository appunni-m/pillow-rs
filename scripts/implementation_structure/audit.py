"""Conservative changed-path audit; findings never trigger file mutations."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
STANDARD = Path("docs/IMPLEMENTATION_STRUCTURE.md")
ARCH_SUFFIX = re.compile(r"_(?:avx2|avx512|neon|ssse3|sse4_1|sse4_2)\.rs$")
ROOT_CONFIGURATION = {
    "Cargo.lock",
    "Cargo.toml",
    "CLAUDE.md",
    "CONTRIBUTING.md",
    "Makefile",
    "README.md",
    "documentation.json",
    "mkdocs.yml",
    "pyproject.toml",
    "rust-toolchain.toml",
}
ROOT_DOCUMENTATION = {
    "BENCHMARKS.md",
    "CHANGELOG.md",
    "CODE_OF_CONDUCT.md",
    "LICENSE",
    "RELEASING.md",
    "SECURITY.md",
    "SUPPORT.md",
}
ROOT_MAINTAINED_FILES = ROOT_CONFIGURATION | ROOT_DOCUMENTATION | {".gitignore"}
PACKAGE_ROOT_FILES = {
    "Cargo.toml",
    ".gitignore",
    "LICENSE",
    "Makefile",
    ".npmignore",
    "README.md",
    "index.d.ts",
    "node.js",
    "package-lock.json",
    "package.json",
    "pyproject.toml",
}
MAINTAINED_ROOTS = (
    ".github/workflows/",
    ".claude/",
    "docs/",
    "pillow-rs/",
    "pillow-rs-js/",
    "pillow-rs-parallel/",
    "pillow-rs-py/",
    "scripts/",
    "site-assets/",
    "tools/",
)
DOCUMENTED_MIGRATION_DEBT: dict[str, tuple[str, str]] = {
    "pillow-rs/src/ops/imageops.rs": (
        "pillow-rs/src/ops/imageops/<family>.rs",
        "This public ImageOps module still combines functions from several operation families; keep the facade where needed and split implementations only at clear family boundaries.",
    ),
    "pillow-rs/src/compute/pool_cpu/ops/color.rs": (
        "pillow-rs/src/compute/pool_cpu/ops/<family>.rs",
        "This CPU module combines color conversion, LAB conversion, palette remapping, and band extraction; trace each route before splitting it.",
    ),
    "pillow-rs/src/compute/pool_cpu/ops/effects.rs": (
        "pillow-rs/src/compute/pool_cpu/ops/<family>.rs",
        "This CPU module groups multiple effects families; split only where operation ownership and shared helpers are clear.",
    ),
    "pillow-rs/src/compute/pool_cpu/ops/geometry.rs": (
        "pillow-rs/src/compute/pool_cpu/ops/<family>.rs",
        "This CPU module groups multiple geometry operations; split only where operation ownership and shared helpers are clear.",
    ),
    "pillow-rs/src/compute/pool_cpu/ops/imageops.rs": (
        "pillow-rs/src/compute/pool_cpu/ops/<family>.rs",
        "This CPU module groups ImageOps implementations from multiple operation families; preserve route and mode behavior when splitting it.",
    ),
    "pillow-rs/src/compute/pool_cpu/ops/filter.rs": (
        "pillow-rs/src/compute/pool_cpu/ops/filter.rs plus filter/kernels/<responsibility>.rs",
        "This CPU module still groups several filter implementations; retain genuinely shared helpers and place distinct loops by responsibility.",
    ),
    "pillow-rs/src/compute/pool_simd/ops/adapters.rs": (
        "pillow-rs/src/compute/pool_simd/ops/adapters.rs as dispatch/re-export facade; adapters/<family>.rs as orchestrators",
        "The SIMD adapters entry still contains dispatch and family implementations; keep hot loops specialized while moving one family at a time.",
    ),
}


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=root, check=True, text=True, capture_output=True
    )
    return result.stdout


def changed_paths(root: Path, base: str = "HEAD") -> list[dict[str, str]]:
    """Return added, copied, renamed, and untracked maintained paths."""
    rows: dict[str, str] = {}
    diff = _git(
        root,
        "diff",
        "--name-status",
        "--find-renames",
        "--find-copies",
        "--diff-filter=ACR",
        base,
        "--",
    )
    for line in diff.splitlines():
        fields = line.split("\t")
        status = fields[0]
        if status.startswith("R") and len(fields) == 3:
            rows[fields[2]] = "renamed"
        elif status.startswith("C") and len(fields) == 3:
            rows[fields[2]] = "copied"
        elif status == "A" and len(fields) == 2:
            rows[fields[1]] = "added"

    untracked = _git(root, "ls-files", "--others", "--exclude-standard", "-z")
    for path in untracked.split("\0"):
        if path:
            rows.setdefault(path, "untracked")

    return [
        {"path": path, "change": rows[path]}
        for path in sorted(rows)
        if path in ROOT_MAINTAINED_FILES
        or path.startswith(MAINTAINED_ROOTS)
    ]


def maintained_tree_paths(root: Path) -> list[dict[str, str]]:
    """Return current tracked and non-ignored untracked maintained paths."""
    tracked = _git(root, "ls-files", "-z").split("\0")
    deleted = set(_git(root, "ls-files", "--deleted", "-z").split("\0"))
    untracked = _git(root, "ls-files", "--others", "--exclude-standard", "-z").split("\0")
    rows: dict[str, str] = {}
    for path in tracked:
        if path and path not in deleted:
            rows[path] = "tracked"
    for path in untracked:
        if path:
            rows.setdefault(path, "untracked")
    return [
        {"path": path, "change": rows[path]}
        for path in sorted(rows)
        if path in ROOT_MAINTAINED_FILES
        or path.startswith(MAINTAINED_ROOTS)
    ]


def classify_path(
    path: str, content: str = "", *, exhaustive: bool = False
) -> dict[str, Any]:
    """Classify only clear layout contracts; leave ambiguous roles reviewable."""
    parts = Path(path).as_posix().split("/")

    if path == "pillow-rs/src/compute/pool_gpu/mod.rs":
        return {
            "assessment": "supported_backend_support_role",
            "rule": "gpu_pool_coordinator",
            "reason": "The GPU root composes focused backend modules and owns lazy device initialization and backend registration.",
        }

    if path in DOCUMENTED_MIGRATION_DEBT:
        expected_shape, reason = DOCUMENTED_MIGRATION_DEBT[path]
        return {
            "assessment": "documented_migration_debt" if exhaustive else "review_candidate",
            "rule": "documented_multi_family_aggregator",
            "expected_shape": expected_shape,
            "reason": reason,
        }

    if path.startswith(".github/workflows/") and len(parts) == 3:
        return {
            "assessment": "matches_lifecycle_workflow_shape",
            "rule": "one_workflow_per_lifecycle",
            "reason": "Repository lifecycle workflows are grouped under the workflow owner.",
        }
    if path == ".claude/settings.json" or path.startswith(".claude/agents/"):
        return {
            "assessment": "matches_agent_configuration_shape",
            "rule": "repository_agent_configuration",
            "reason": "Repository-specific agent configuration is grouped under .claude/.",
        }
    if path.startswith(".claude/skills/") and len(parts) >= 4:
        return {
            "assessment": "matches_skill_resource_shape",
            "rule": "skill_owned_resource",
            "reason": "A skill and its supporting references or examples are grouped under the skill owner.",
        }
    if path.startswith("docs/generated/"):
        return {
            "assessment": "matches_generated_document_shape",
            "rule": "generated_document_output",
            "reason": "Generated documentation is grouped separately from maintained guides.",
        }
    if path.startswith("docs/design/") and len(parts) >= 3:
        return {
            "assessment": "matches_document_concern_shape",
            "rule": "design_document_concern",
            "reason": "Design notes and diagrams are grouped by documentation concern.",
        }
    if path.startswith("pillow-rs/tests/") and not path.startswith("pillow-rs/tests/fixtures/"):
        return {
            "assessment": "matches_test_suite_shape",
            "rule": "crate_test_suite",
            "reason": "Crate tests are grouped under the tests owner or an adjacent module test file.",
        }
    if path == "pillow-rs/tests/fixtures/manifest.yaml":
        return {
            "assessment": "supported_root_role",
            "rule": "canonical_fixture_manifest",
            "reason": "The canonical parity/workload manifest owns fixture and operation identity.",
        }
    if path.startswith("pillow-rs/tests/fixtures/inputs/"):
        if len(parts) >= 6:
            return {
                "assessment": "matches_domain_input_shape",
                "rule": "fixture_input_domain",
                "reason": "Maintained input payloads are grouped under an input domain.",
            }
        return {
            "assessment": "review_candidate",
            "rule": "fixture_domain_missing",
            "expected_shape": "pillow-rs/tests/fixtures/inputs/<domain>/<payload>",
            "reason": "The input payload path does not identify its domain.",
        }
    if path.startswith("pillow-rs/tests/fixtures/"):
        if len(parts) >= 5:
            return {
                "assessment": "matches_fixture_domain_shape",
                "rule": "fixture_domain_asset",
                "reason": "Fixture assets are grouped under a named fixture domain.",
            }
        return {
            "assessment": "review_candidate",
            "rule": "fixture_domain_missing",
            "expected_shape": "pillow-rs/tests/fixtures/<domain>/<payload>",
            "reason": "The fixture asset path does not identify its domain.",
        }
    if path.startswith("pillow-rs-py/bench/"):
        return {
            "assessment": "matches_benchmark_concern_shape",
            "rule": "package_benchmark_tool",
            "reason": "Binding-specific benchmark tools are grouped under their package bench concern.",
        }
    if path.startswith("pillow-rs/benches/") and path.endswith(".rs"):
        return {
            "assessment": "matches_benchmark_concern_shape",
            "rule": "package_benchmark_tool",
            "reason": "Rust core benchmark targets are grouped under the owning crate's benches directory.",
        }
    if path.startswith("pillow-rs/examples/") and path.endswith(".rs"):
        return {
            "assessment": "matches_example_entrypoint_shape",
            "rule": "cargo_example_target",
            "reason": "Runnable examples are separate named Cargo targets beneath their owning crate.",
        }
    if path.startswith(("pillow-rs/src/data/", "pillow-rs/src/font/")) and not path.endswith(".rs"):
        return {
            "assessment": "matches_domain_resource_shape",
            "rule": "embedded_domain_resource",
            "reason": "Embedded data and font assets are grouped beneath their owning core domain.",
        }
    if path.startswith("pillow-rs/src/ops/lut_") and path.endswith(".bin"):
        return {
            "assessment": "matches_operation_resource_shape",
            "rule": "embedded_operation_lut",
            "reason": "Embedded lookup tables are named for the operation family and kept beside core operations.",
        }
    if path == "pillow-rs-py/coverage/report.json":
        return {
            "assessment": "documented_migration_debt" if exhaustive else "review_candidate",
            "rule": "checked_in_coverage_output",
            "expected_shape": "ignored build output; never a maintained source path",
            "reason": "A generated coverage report is present in the source tree and should be reviewed as generated output, not source.",
        }
    if path.startswith("site-assets/"):
        return {
            "assessment": "matches_site_asset_shape",
            "rule": "documentation_site_asset",
            "reason": "Documentation assets are grouped under the site asset owner.",
        }
    if len(parts) == 2 and parts[0] in {
        "pillow-rs", "pillow-rs-py", "pillow-rs-js", "pillow-rs-parallel"
    } and parts[1] in PACKAGE_ROOT_FILES:
        return {
            "assessment": "supported_package_root_role",
            "rule": "package_boundary_file",
            "reason": "Package manifests, package-level guides, and lifecycle metadata stay at their owning package root.",
        }
    if path == "pillow-rs-js/public.js":
        return {
            "assessment": "supported_package_root_role",
            "rule": "javascript_public_entrypoint",
            "reason": "The browser package entry point adapts generated core bindings to the public JavaScript API.",
        }
    if path.startswith("pillow-rs-js/host/") and path.endswith(".js"):
        return {
            "assessment": "matches_binding_module_shape",
            "rule": "javascript_host_integration",
            "reason": "JavaScript host I/O and display integrations are grouped under the binding package's host layer.",
        }
    if path == "scripts/test_operation_source_crosswalk.py" and re.search(
        r"(?m)^\s*(?:from|import)\s+source_crosswalk\b", content
    ):
        return {
            "assessment": "review_candidate",
            "rule": "workflow_test_separate_from_tool_package",
            "expected_shape": "scripts/source_crosswalk/test_<contract>.py",
            "reason": "The test imports a multi-file workflow package while living beside the CLI shim; confirm and co-locate it with that workflow's implementation.",
        }
    if path in {
        "scripts/rust_size_baseline.json",
        "scripts/public_api_boundary_baseline.json",
    }:
        return {
            "assessment": "supported_tool_configuration",
            "rule": "quality_tool_ratchet_baseline",
            "reason": "Reviewed quality ratchets are kept beside the checker that enforces them.",
        }
    if path.startswith("scripts/") and len(parts) == 2 and path.endswith((".py", ".sh", ".mjs", ".cjs")):
        return {
            "assessment": "matches_standalone_tool_shape",
            "rule": "standalone_script_entry",
            "reason": "A small standalone command can remain directly under scripts/.",
        }
    if path.startswith("scripts/") and len(parts) >= 3:
        return {
            "assessment": "matches_tool_concern_shape",
            "rule": "tool_concern_package",
            "reason": "A multi-file tool or workflow is grouped beneath its scripts/ concern.",
        }
    if path.startswith("pillow-rs/src/ops/") and path.endswith(".rs"):
        relative = parts[3:]
        if len(relative) == 1 and relative[0] not in {"mod.rs", "module_fns.rs", "utils.rs", "param_filters.rs"}:
            return {
                "assessment": "matches_core_operation_shape",
                "rule": "core_operation_family",
                "reason": "Core image semantics are grouped in a family-named operation module.",
            }
        if len(relative) >= 2:
            return {
                "assessment": "matches_core_family_child_shape",
                "rule": "core_operation_child",
                "reason": "A family-specific child module is nested below its core operation family.",
            }
        if relative[0] in {"mod.rs", "module_fns.rs", "utils.rs", "param_filters.rs"}:
            return {
                "assessment": "supported_shared_operation_role",
                "rule": "shared_operation_support",
                "reason": "This is an explicitly named shared operation registration or support module.",
            }
    if path.startswith("pillow-rs/src/compute/pool_cpu/ops/") and path.endswith(".rs"):
        relative = parts[5:]
        if len(relative) == 1 and relative[0] == "mod.rs":
            return {
                "assessment": "supported_backend_support_role",
                "rule": "cpu_operation_registration",
                "reason": "The CPU operations module declares and registers operation families; it is a shared backend entry point.",
            }
        if len(relative) == 1 and relative[0].endswith("_tests.rs"):
            return {
                "assessment": "review_candidate",
                "rule": "cpu_test_module_at_family_root",
                "expected_shape": "place focused tests beside the owning family or its test owner",
                "reason": "This is a test module directly under the CPU operations root, not an operation family implementation.",
            }
        if len(relative) == 1:
            return {
                "assessment": "matches_cpu_family_shape",
                "rule": "cpu_family_module",
                "reason": "Serial CPU operations are owned by a family module under the CPU execution profile.",
            }
        if len(relative) == 2 and relative[0] == "filter" and not relative[1].endswith("_tests.rs"):
            return {
                "assessment": "documented_migration_debt" if exhaustive else "review_candidate",
                "rule": "cpu_filter_helper_outside_kernel_layer",
                "expected_shape": "pillow-rs/src/compute/pool_cpu/ops/filter/kernels/<kernel>.rs or a documented shared filter helper",
                "reason": "This executable Filter child currently sits beside the kernels directory; call sites must establish whether it is a kernel or a shared family helper.",
            }
        if len(relative) == 2 and relative[1] in {"mod.rs", "ops.rs"}:
            return {
                "assessment": "matches_cpu_family_shape",
                "rule": "cpu_family_orchestrator",
                "reason": "The family module exports a simple mod.rs and keeps support declaration and mode/architecture selection in ops.rs.",
            }
        if len(relative) == 3 and relative[2] == "ops.rs":
            return {
                "assessment": "matches_cpu_family_shape",
                "rule": "cpu_mode_strategy",
                "reason": "A mode-specific CPU implementation is owned by its mode strategy module.",
            }
        if (len(relative) >= 3 and relative[1] == "kernels") or (
            len(relative) >= 4 and relative[2] == "kernels"
        ):
            return {
                "assessment": "matches_cpu_kernel_shape",
                "rule": "cpu_family_kernel",
                "reason": "A CPU kernel is nested under its operation family and kernel responsibility.",
            }
    if path.startswith("pillow-rs/src/compute/pool_simd/ops/") and path.endswith(".rs") and len(parts) == 7:
        if Path(path).name in {"mod.rs", "scalar.rs", "native_channel.rs", "native_filter.rs", "native_merge.rs"}:
            return {
                "assessment": "supported_backend_support_role",
                "rule": "simd_backend_support",
                "reason": "This module is a backend-level dispatcher or shared native support owner.",
            }
    if path.startswith(("pillow-rs-py/src/", "pillow-rs-js/src/")) and path.endswith(".rs"):
        return {
            "assessment": "matches_binding_module_shape",
            "rule": "rust_binding_module",
            "reason": "Native binding modules are grouped under their owning binding crate.",
        }
    if path.startswith("pillow-rs-js/scripts/"):
        return {
            "assessment": "matches_tool_concern_shape",
            "rule": "javascript_package_tooling",
            "reason": "Build, package, and parity scripts are grouped under the JavaScript package's script owner.",
        }
    if path.startswith("pillow-rs-py/python/pillow_rs/") and path.endswith(".py") and len(parts) == 4:
        return {
            "assessment": "matches_binding_module_shape",
            "rule": "python_binding_module",
            "reason": "Host conversion and delegation modules are grouped under their Python package owner.",
        }
    if path.startswith("pillow-rs-py/python/PIL/") and path.endswith(".py") and len(parts) == 4:
        return {
            "assessment": "matches_binding_module_shape",
            "rule": "public_python_namespace_module",
            "reason": "The public compatibility namespace keeps its user-facing modules together.",
        }

    if path.startswith("docs/evidence/"):
        relative = parts[2:]
        if len(relative) >= 3:
            return {
                "assessment": "review_candidate",
                "rule": "generated_evidence_should_not_be_committed",
                "expected_shape": "target/evidence/raw/<family>/<experiment>/<artifact>",
                "reason": "Raw benchmark and parity evidence is workflow or local output; committed results belong in concise, maintained documentation.",
            }
        return {
            "assessment": "review_candidate",
            "rule": "generated_evidence_should_not_be_committed",
            "expected_shape": "target/evidence/raw/<artifact>",
            "reason": "Generated benchmark and parity evidence belongs in ignored target output; retain only reviewed conclusions in tracked documentation.",
        }

    simd_adapter_prefix = "pillow-rs/src/compute/pool_simd/ops/adapters/"
    if path.startswith(simd_adapter_prefix) and path.endswith(".rs"):
        relative = parts[len(simd_adapter_prefix.rstrip("/").split("/")):]
        if len(relative) >= 3 and relative[1] == "kernels":
            return {
                "assessment": "matches_family_kernel_shape",
                "rule": "simd_family_kernel",
                "reason": "The kernel is nested beneath its family adapter.",
            }
        if len(relative) == 1:
            name = relative[0]
            stem = Path(name).stem
            if stem.endswith("_tests"):
                return {
                    "assessment": "review_candidate",
                    "rule": "test_module_near_adapter",
                    "expected_shape": "place focused tests beside the owning family workflow or implementation",
                    "reason": "A test module is directly under the adapter owner; confirm whether it belongs beside a multi-file family workflow.",
                }
            if ARCH_SUFFIX.search(name):
                return {
                    "assessment": "review_candidate",
                    "rule": "architecture_kernel_at_adapter_root",
                    "expected_shape": "pillow-rs/src/compute/pool_simd/ops/adapters/<family>/kernels/<mode>_<architecture>.rs",
                    "reason": "The filename selects an architecture-specific implementation at the adapter root.",
                }
            if stem.startswith("native_") or stem in {
                "box_blur_luma",
                "gaussian_rgba_uniform",
            }:
                return {
                    "assessment": "review_candidate",
                    "rule": "mode_or_kernel_named_adapter_root",
                    "expected_shape": "pillow-rs/src/compute/pool_simd/ops/adapters/<family>/kernels/<mode-or-responsibility>.rs",
                    "reason": "The path appears to name a native/mode kernel rather than the family-level orchestrator.",
                }
            return {
                "assessment": "matches_family_orchestrator_shape",
                "rule": "simd_family_orchestrator",
                "reason": "A family-named adapter module is directly under the adapter owner.",
            }
        if len(relative) == 2 and relative[1] == "kernels.rs":
            return {
                "assessment": "matches_family_kernel_shape",
                "rule": "simd_kernel_dispatch",
                "reason": "The family has a focused kernel module entry beside its architecture-specific kernel directory.",
            }
        if len(relative) >= 2 and relative[1] != "kernels":
            return {
                "assessment": "review_candidate",
                "rule": "simd_family_child_role_unclear",
                "expected_shape": "pillow-rs/src/compute/pool_simd/ops/adapters/<family>/kernels/<mode-or-responsibility>.rs",
                "reason": "A child module is nested under a SIMD family without an explicit kernels/responsibility layer.",
            }

    if path.startswith("pillow-rs/src/compute/pool_gpu/shaders/") and path.endswith(".wgsl"):
        if len(parts) >= 7:
            return {
                "assessment": "matches_family_shader_shape",
                "rule": "gpu_family_shader",
                "reason": "The shader is nested below its operation family.",
            }
        return {
            "assessment": "documented_migration_debt" if exhaustive else "review_candidate",
            "rule": "flat_gpu_shader",
            "expected_shape": "pillow-rs/src/compute/pool_gpu/shaders/<family>/<stage-or-mode>.wgsl",
            "reason": "This shader is at the GPU shader root instead of below its family; migration must preserve every include, registration, and backend receipt.",
        }

    if path.startswith("pillow-rs/src/compute/pool_cpu/ops/filter/") and path.endswith(".rs"):
        if len(parts) == 7 and parts[-1] not in {"mod.rs"} and not parts[-1].endswith("_tests.rs"):
            return {
                "assessment": "review_candidate",
                "rule": "filter_kernel_role_unclear",
                "expected_shape": "pillow-rs/src/compute/pool_cpu/ops/filter/kernels/<kernel>.rs",
                "reason": "An executable filter-family child sits beside the kernels directory; call sites must decide whether it is a kernel or a shared family helper.",
            }

    if path.startswith("pillow-rs/src/") and path.endswith(".rs"):
        return {
            "assessment": "matches_rust_module_tree_shape",
            "rule": "core_rust_module_hierarchy",
            "reason": "Rust source is grouped below its crate owner by the statically declared module hierarchy; operation-specific roles are reported by narrower rules where recognized.",
        }

    if path in ROOT_CONFIGURATION or path in ROOT_DOCUMENTATION or path == ".gitignore":
        return {
            "assessment": "supported_root_role",
            "rule": "repository_root_role",
            "reason": "This file governs repository-wide configuration or is a repository-level guide/license.",
        }
    if path.startswith("docs/") and len(parts) == 2 and path.endswith(".md"):
        return {
            "assessment": "matches_document_topic_shape",
            "rule": "documentation_topic",
            "reason": "A repository-level guide is grouped by its documented topic under docs/.",
        }
    if any(path.startswith(prefix) for prefix in MAINTAINED_ROOTS):
        if exhaustive:
            return {
                "assessment": "unclassified",
                "rule": "layout_rule_unclassified",
                "reason": "This maintained path is inventoried, but no specific path rule currently proves its family and responsibility placement.",
            }
        return {
            "assessment": "owner_path_needs_no_heuristic",
            "rule": "owner_scoped_path",
            "reason": "The owner is recognized; this conservative audit makes no family-level conformance claim for this path.",
        }
    return {
        "assessment": "outside_maintained_scope",
        "rule": "ignored_or_unowned_path",
        "reason": "The path is outside the maintained first-party roots in the structure guide.",
    }


def build_report(
    root: Path = ROOT, base: str = "HEAD", scope: str = "changed"
) -> dict[str, Any]:
    root = root.resolve()
    if scope not in {"changed", "tree"}:
        raise ValueError(f"unsupported implementation-structure scope: {scope}")
    paths = changed_paths(root, base) if scope == "changed" else maintained_tree_paths(root)
    items = [
        {
            **change,
            **classify_path(
                change["path"],
                (root / change["path"]).read_text(encoding="utf-8", errors="replace")
                if change["path"].endswith(".py") and (root / change["path"]).is_file()
                else "",
                exhaustive=scope == "tree",
            ),
            "owner": change["path"].split("/", 1)[0],
            "path_depth": len(Path(change["path"]).parts),
        }
        for change in paths
    ]
    standard_path = root / STANDARD
    standard_digest = (
        hashlib.sha256(standard_path.read_bytes()).hexdigest()
        if standard_path.is_file()
        else None
    )
    return {
        "schema": "pillow-rs/implementation-structure-review@2",
        "source": {
            "base_revision": _git(root, "rev-parse", base).strip(),
            "head_revision": _git(root, "rev-parse", "HEAD").strip(),
            "dirty": bool(_git(root, "status", "--porcelain")),
            "standard_path": STANDARD.as_posix(),
            "standard_sha256": standard_digest,
        },
        "scope": (
            "added, copied, renamed, and untracked maintained paths only"
            if scope == "changed"
            else "all current tracked and non-ignored untracked maintained paths"
        ),
        "summary": {
            "paths": len(items),
            "review_candidates": sum(item["assessment"] == "review_candidate" for item in items),
            "documented_migration_debt": sum(item["assessment"] == "documented_migration_debt" for item in items),
            "unclassified_paths": sum(item["assessment"] == "unclassified" for item in items),
            "matching_paths": sum(item["assessment"].startswith("matches_") for item in items),
            "supported_paths": sum(item["assessment"] == "supported_root_role" for item in items),
            "family_experiment_evidence": sum(item["rule"] == "family_experiment_evidence" for item in items),
            "domain_input_paths": sum(item["rule"] == "fixture_input_domain" for item in items),
        },
        "items": items,
        "automatic_actions": [],
        "limits": [
            "The whole-tree inventory accounts for maintained paths, but unclassified paths still require explicit family/responsibility rules or a documented exception before conformance can be claimed.",
            "Static path shapes do not prove source ownership, module reachability, runtime loading, or backend execution.",
            "Historical paths and real package boundaries may remain with a documented reason; findings never move or delete code.",
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="pillow-rs checkout")
    parser.add_argument("--base", default="HEAD", help="Git revision to compare added/moved paths against")
    parser.add_argument(
        "--scope",
        choices=("changed", "tree"),
        default="changed",
        help="audit changed paths or inventory the entire maintained tree",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("target/implementation-structure-review.json"),
        help="write the generated report here",
    )
    parser.add_argument(
        "--fail-on-unclassified",
        action="store_true",
        help="return failure after writing the report if a maintained path has no layout rule",
    )
    args = parser.parse_args(argv)
    report = build_report(args.root, args.base, args.scope)
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    output = args.output if args.output.is_absolute() else args.root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")
    print(f"wrote {output}")
    print(json.dumps(report["summary"], sort_keys=True))
    if args.fail_on_unclassified and report["summary"]["unclassified_paths"] > 0:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
