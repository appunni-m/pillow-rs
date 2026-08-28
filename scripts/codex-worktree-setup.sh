#!/usr/bin/env bash
# Fast, isolated setup for Codex-managed worktrees.
#
# The seed is used only for dependency trees that are safe to clone. Every
# worktree keeps its own .venv, node_modules, target, and generated artifacts.
# uv, npm, Cargo's registry, and sccache are content-addressed user caches.
set -euo pipefail

ROOT_DIR="$(cd "$(git rev-parse --show-toplevel)" && pwd -P)"
cd "$ROOT_DIR"

DRY_RUN="${CODEX_WORKTREE_SETUP_DRY_RUN:-0}"
CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/.cache}"
export UV_CACHE_DIR="${UV_CACHE_DIR:-${CACHE_ROOT}/uv}"
export npm_config_cache="${npm_config_cache:-${NPM_CONFIG_CACHE:-${CACHE_ROOT}/npm}}"
export NPM_CONFIG_CACHE="$npm_config_cache"
export SCCACHE_DIR="${SCCACHE_DIR:-${CACHE_ROOT}/sccache/cache}"

say() {
  printf '[codex-worktree-setup] %s\n' "$*"
}

print_command() {
  printf '[codex-worktree-setup] '
  printf '%q ' "$@"
  printf '\n'
}

run_command() {
  if [ "$DRY_RUN" = "1" ]; then
    print_command "$@"
  else
    "$@"
  fi
}

run_in_dir() {
  local directory="$1"
  shift
  if [ "$DRY_RUN" = "1" ]; then
    printf '[codex-worktree-setup] (cd %q && ' "$directory"
    printf '%q ' "$@"
    printf ')\n'
  else
    (cd "$directory" && "$@")
  fi
}

same_files() {
  local left="$1"
  local right="$2"
  local relative
  shift 2
  for relative in "$@"; do
    [ -f "$left/$relative" ] || return 1
    [ -f "$right/$relative" ] || return 1
    cmp -s "$left/$relative" "$right/$relative" || return 1
  done
}

copy_directory() {
  local source="$1"
  local destination="$2"
  [ -d "$source" ] || return 1
  if [ "$DRY_RUN" = "1" ]; then
    say "would copy-on-write seed $source -> $destination"
    return 0
  fi
  mkdir -p "$(dirname "$destination")"
  case "$(uname -s)" in
    Darwin)
      if ! cp -cR "$source" "$destination" 2>/dev/null; then
        cp -R "$source" "$destination"
      fi
      ;;
    *)
      if cp -a --reflink=auto "$source" "$destination" 2>/dev/null; then
        :
      else
        cp -a "$source" "$destination"
      fi
      ;;
  esac
  say "seeded $destination with an independent copy-on-write clone"
}

copy_file() {
  local source="$1"
  local destination="$2"
  [ -f "$source" ] || return 1
  if [ "$DRY_RUN" = "1" ]; then
    say "would copy-on-write seed $source -> $destination"
    return 0
  fi
  mkdir -p "$(dirname "$destination")"
  case "$(uname -s)" in
    Darwin)
      if ! cp -c "$source" "$destination" 2>/dev/null; then
        cp "$source" "$destination"
      fi
      ;;
    *)
      if cp --reflink=auto "$source" "$destination" 2>/dev/null; then
        :
      else
        cp "$source" "$destination"
      fi
      ;;
  esac
  say "seeded $destination with an independent copy-on-write clone"
}

repair_python_paths() {
  local virtualenv="$1"
  local old_root="$2"
  local new_root="$3"
  if [ "$DRY_RUN" = "1" ]; then
    say "would repair copied Python paths from $old_root to $new_root"
    return 0
  fi
  command -v perl >/dev/null 2>&1 || return 0
  find "$virtualenv/bin" "$virtualenv/lib" -type f -print0 2>/dev/null |
    while IFS= read -r -d '' file; do
      if grep -IqF "$old_root" "$file"; then
        CODEX_SEED_ROOT="$old_root" CODEX_CURRENT_ROOT="$new_root" \
          perl -pi -e 's/\Q$ENV{CODEX_SEED_ROOT}\E/$ENV{CODEX_CURRENT_ROOT}/g' "$file"
      fi
    done
}

git_common_dir() {
  local repository="$1"
  local common
  common="$(git -C "$repository" rev-parse --git-common-dir 2>/dev/null || true)"
  [ -n "$common" ] || return 1
  case "$common" in
    /*) ;;
    *) common="$repository/$common" ;;
  esac
  (cd "$common" && pwd -P)
}

SEED_ROOT=""
find_seed() {
  local candidate="${CODEX_WORKTREE_SEED:-}"
  local default_branch
  local origin_head
  local line
  local listed_root=""

  if [ -z "$candidate" ]; then
    origin_head="$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null || true)"
    default_branch="${origin_head#origin/}"
    [ -n "$default_branch" ] || default_branch="main"
    while IFS= read -r line; do
      case "$line" in
        worktree\ *) listed_root="${line#worktree }" ;;
        "branch refs/heads/$default_branch")
          if [ -n "$listed_root" ] && [ "$listed_root" != "$ROOT_DIR" ]; then
            candidate="$listed_root"
            break
          fi
          ;;
      esac
    done < <(git worktree list --porcelain)
  elif [ "${candidate#/}" = "$candidate" ]; then
    candidate="$ROOT_DIR/$candidate"
  fi

  [ -n "$candidate" ] || return 0
  [ -d "$candidate" ] || return 0
  candidate="$(cd "$candidate" && pwd -P)"
  [ "$candidate" != "$ROOT_DIR" ] || return 0
  [ "$(git_common_dir "$candidate")" = "$(git_common_dir "$ROOT_DIR")" ] || return 0
  SEED_ROOT="$candidate"
  say "using main worktree as dependency seed: $SEED_ROOT"
}

require_sccache() {
  if ! command -v sccache >/dev/null 2>&1; then
    printf 'sccache is required for this repository but was not found on PATH\n' >&2
    exit 1
  fi
  say "Rust compiler cache: $(command -v sccache)"
}

setup_rust() {
  require_sccache
  run_command cargo fetch --locked
}

setup_python() {
  local virtualenv="$ROOT_DIR/.venv"
  local python="$virtualenv/bin/python"

  if [ -x "$python" ]; then
    say "reusing $virtualenv"
    return 0
  fi

  if [ -n "$SEED_ROOT" ] && [ -x "$SEED_ROOT/.venv/bin/python" ] &&
     same_files "$SEED_ROOT" "$ROOT_DIR" "pillow-rs-py/pyproject.toml"; then
    copy_directory "$SEED_ROOT/.venv" "$virtualenv"
    repair_python_paths "$virtualenv" "$SEED_ROOT" "$ROOT_DIR"
    return 0
  fi

  if command -v uv >/dev/null 2>&1; then
    run_command uv venv --python "${CODEX_PYTHON_VERSION:-3.12}" "$virtualenv"
    run_command uv pip install --python "$python" \
      'maturin>=1.0,<2.0' coverage pyyaml 'pillow==12.2.0' numpy
  else
    run_command python3 -m venv "$virtualenv"
    run_command "$python" -m pip install --cache-dir "$CACHE_ROOT/pip" \
      'maturin>=1.0,<2.0' coverage pyyaml 'pillow==12.2.0' numpy
  fi
}

setup_node() {
  local package_root="$ROOT_DIR/pillow-rs-js"
  local modules="$package_root/node_modules"

  if [ -d "$modules" ]; then
    say "reusing $modules"
    return 0
  fi

  if [ -n "$SEED_ROOT" ] && [ -d "$SEED_ROOT/pillow-rs-js/node_modules" ] &&
     same_files "$SEED_ROOT" "$ROOT_DIR" \
       "pillow-rs-js/package.json" "pillow-rs-js/package-lock.json"; then
    copy_directory "$SEED_ROOT/pillow-rs-js/node_modules" "$modules"
    return 0
  fi

  require_command() {
    command -v "$1" >/dev/null 2>&1 || {
      printf '%s is required for this repository but was not found on PATH\n' "$1" >&2
      exit 1
    }
  }
  require_command npm
  run_in_dir "$package_root" env npm_config_cache="$npm_config_cache" \
    npm ci --prefer-offline --no-audit --no-fund
}

seed_built_python_extension() {
  local extension_relative="pillow-rs-py/python/pillow_rs/_core.abi3.so"
  [ -n "$SEED_ROOT" ] || return 0
  [ -f "$SEED_ROOT/$extension_relative" ] || return 0
  [ ! -e "$ROOT_DIR/$extension_relative" ] || return 0
  [ "$(git -C "$SEED_ROOT" rev-parse HEAD)" = "$(git -C "$ROOT_DIR" rev-parse HEAD)" ] || return 0
  git -C "$SEED_ROOT" diff --quiet HEAD -- Cargo.toml Cargo.lock pillow-rs pillow-rs-py || return 0
  git -C "$ROOT_DIR" diff --quiet HEAD -- Cargo.toml Cargo.lock pillow-rs pillow-rs-py || return 0
  same_files "$SEED_ROOT" "$ROOT_DIR" Cargo.toml Cargo.lock \
    pillow-rs/Cargo.toml pillow-rs-py/Cargo.toml || return 0
  copy_file "$SEED_ROOT/$extension_relative" "$ROOT_DIR/$extension_relative"
}

find_seed
setup_rust
setup_python
setup_node
seed_built_python_extension
say "worktree setup complete; build outputs remain local to $ROOT_DIR"
