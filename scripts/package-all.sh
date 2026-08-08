#!/usr/bin/env bash
# Local packaging helper.
# Release Windows binaries are built on GitHub Actions (windows-latest / MSVC).
# This script builds Linux locally; Windows only if a mingw/zig toolchain is present
# (optional; prefer the CI MSVC artifact for distribution).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)"
OUT="${ROOT}/dist/v${VERSION}"
mkdir -p "${OUT}" "${ROOT}/dist"

stage() {
  local src="$1"
  local dest_name="$2"
  if [[ ! -f "$src" ]]; then
    echo "SKIP missing: $src" >&2
    return 1
  fi
  cp -f "$src" "${OUT}/${dest_name}"
  if [[ "$dest_name" != *.exe ]]; then
    strip "${OUT}/${dest_name}" 2>/dev/null || true
    chmod +x "${OUT}/${dest_name}"
  fi
  cp -f "${OUT}/${dest_name}" "${ROOT}/dist/${dest_name}"
  echo "OK  ${dest_name} ($(du -h "${OUT}/${dest_name}" | cut -f1))"
}

echo "==> Toolchain"
rustc --version
cargo --version

echo "==> Linux x86_64"
cargo build --release --locked --bin sergas-zip-shrinker
stage "target/release/sergas-zip-shrinker" "sergas-zip-shrinker-linux-x86_64"

if command -v cargo-zigbuild >/dev/null && command -v zig >/dev/null; then
  echo "==> Windows x86_64 (optional local zigbuild; CI releases use MSVC)"
  rustup target add x86_64-pc-windows-gnu >/dev/null
  cargo zigbuild --release --locked --bin sergas-zip-shrinker --target x86_64-pc-windows-gnu
  stage "target/x86_64-pc-windows-gnu/release/sergas-zip-shrinker.exe" \
    "sergas-zip-shrinker-windows-x86_64.exe" || true
else
  echo "==> Windows skipped locally (download MSVC build from GitHub Releases)"
fi

echo "==> macOS: produced only by GitHub Actions (native Apple runners)"

cp -f LICENSE "${OUT}/LICENSE"
cp -f LICENSE "${ROOT}/dist/LICENSE"

(
  cd "${OUT}"
  sha256sum sergas-zip-shrinker-linux-x86_64 \
    $(ls sergas-zip-shrinker-windows-x86_64.exe 2>/dev/null || true) > SHA256SUMS
  cp -f SHA256SUMS "${ROOT}/dist/SHA256SUMS"
)

echo
echo "Release folder: ${OUT}"
ls -lh "${OUT}"
echo
echo "For Windows distribution, use the MSVC artifact from GitHub Actions — see DISTRIBUTION.md"
