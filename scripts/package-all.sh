#!/usr/bin/env bash
# Local packaging — MUST stay in sync with .github/workflows/release.yml
# (Linux host build + Windows via cargo-zigbuild / Zig 0.13).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)"
OUT="${ROOT}/dist/v${VERSION}"
mkdir -p "${OUT}" "${ROOT}/dist"

ZIG_VERSION="${ZIG_VERSION:-0.13.0}"
export PATH="${HOME}/.local/zig-${ZIG_VERSION}:${HOME}/.local/zig-linux-x86_64-${ZIG_VERSION}:${HOME}/.cargo/bin:${PATH:-}"

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

echo "==> Toolchain (expect Rust 1.97.1, Zig ${ZIG_VERSION})"
rustc --version
cargo --version
command -v zig >/dev/null && zig version || echo "zig: not found"

echo "==> Linux x86_64"
cargo build --release --locked --bin sergas-zip-shrinker
stage "target/release/sergas-zip-shrinker" "sergas-zip-shrinker-linux-x86_64"

if command -v cargo-zigbuild >/dev/null && command -v zig >/dev/null; then
  echo "==> Windows x86_64 (cargo zigbuild + x86_64-pc-windows-gnu)"
  rustup target add x86_64-pc-windows-gnu >/dev/null
  cargo zigbuild --release --locked --bin sergas-zip-shrinker --target x86_64-pc-windows-gnu
  stage "target/x86_64-pc-windows-gnu/release/sergas-zip-shrinker.exe" \
    "sergas-zip-shrinker-windows-x86_64.exe"
else
  echo "==> Windows skipped (need zig ${ZIG_VERSION} + cargo-zigbuild)"
  exit 1
fi

echo "==> macOS: produced only by GitHub Actions (native Apple runners)"

cp -f LICENSE "${OUT}/LICENSE"
cp -f LICENSE "${ROOT}/dist/LICENSE"

(
  cd "${OUT}"
  sha256sum sergas-zip-shrinker-linux-x86_64 \
    sergas-zip-shrinker-windows-x86_64.exe > SHA256SUMS
  cp -f SHA256SUMS "${ROOT}/dist/SHA256SUMS"
)

echo
echo "Release folder: ${OUT}"
ls -lh "${OUT}"
echo
echo "CI uses the same Rust/Zig/cargo-zigbuild versions — see DISTRIBUTION.md"
