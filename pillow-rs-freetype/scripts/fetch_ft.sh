#!/usr/bin/env bash
# Fetch the pinned FreeType C oracle source used only for fixture generation.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="2.14.3"
ARCHIVE="freetype-${VERSION}.tar.xz"
URL="https://download.savannah.gnu.org/releases/freetype/${ARCHIVE}"
SHA256="36bc4f1cc413335368ee656c42afca65c5a3987e8768cc28cf11ba775e785a5f"
CACHE_DIR="${ROOT}/target/oracle-cache"
ARCHIVE_PATH="${CACHE_DIR}/${ARCHIVE}"
SRC_DIR="${ROOT}/freetype"

if [ -f "${SRC_DIR}/include/freetype/freetype.h" ]; then
  echo "FreeType ${VERSION} oracle source already exists at ${SRC_DIR}"
  exit 0
fi

mkdir -p "${CACHE_DIR}"
if [ ! -f "${ARCHIVE_PATH}" ]; then
  curl -L --fail --show-error "${URL}" -o "${ARCHIVE_PATH}"
fi

actual="$(sha256sum "${ARCHIVE_PATH}" | awk '{print $1}')"
if [ "${actual}" != "${SHA256}" ]; then
  echo "checksum mismatch for ${ARCHIVE}" >&2
  echo "expected ${SHA256}" >&2
  echo "actual   ${actual}" >&2
  exit 1
fi

tmp_dir="$(mktemp -d "${CACHE_DIR}/extract.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT
tar -xJf "${ARCHIVE_PATH}" -C "${tmp_dir}"
rm -rf "${SRC_DIR}"
mv "${tmp_dir}/freetype-${VERSION}" "${SRC_DIR}"
echo "Fetched FreeType ${VERSION} oracle source to ${SRC_DIR}"
