#!/usr/bin/env bash
# Run the Pillow oracle corpus through the public Python package while
# collecting line and branch coverage for the Python wrapper layer.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_data="$coverage_dir/.pillow-python-wrapper.coverage"
coverage_output="$coverage_dir/pillow-python-wrapper.json"
python_bin="${PYTHON:-$repo_dir/.venv/bin/python}"
test_timeout="${TIMEOUT:-30}"
pytest_report="${REPORT:-/tmp/report.json}"

mkdir -p "$coverage_dir"
cd "$repo_dir"

if ! "$python_bin" -m coverage --version >/dev/null 2>&1; then
    printf 'Python coverage.py is required; run make setup first\n' >&2
    exit 2
fi

export COVERAGE_FILE="$coverage_data"
# A previously installed PyO3 extension may itself be LLVM-instrumented. Keep
# any incidental profile beside generated coverage artifacts rather than
# leaking a default_*.profraw file into the repository root.
export LLVM_PROFILE_FILE="$coverage_dir/python-wrapper-%p-%m.profraw"
"$python_bin" -m coverage erase

set +e
make fixtures
fixture_status=$?
if [[ "$fixture_status" -eq 0 ]]; then
    "$python_bin" -m coverage run \
        --branch \
        --source=pillow-rs-py/python/pillow_rs \
        -m pytest tests/test_parity.py -q --tb=short \
        --timeout="$test_timeout" \
        --json-report \
        --json-report-file="$pytest_report" \
        --strict-covers
    test_status=$?
else
    test_status=$fixture_status
fi
set -e

"$python_bin" -m coverage json \
    --show-contexts \
    -o "$coverage_output"

printf 'Python-wrapper coverage: %s\n' "$coverage_output"
exit "$test_status"
