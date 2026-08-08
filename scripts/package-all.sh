#!/usr/bin/env bash
# Stage distributable executables into dist/ for GitHub Releases.
# Builds native Linux always; Windows via cargo-zigbuild when available;
# macOS is produced by GitHub Actions (cross-compile needs Apple SDK).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)"
OUT="${ROOT}/dist/v${VERSION}"
mkdir -p "${OUT}" "${ROOT}/dist"

export PATH="${HOME}/.local/zig-0.13.0:${HOME}/.cargo/bin:${PATH:-}"

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

echo "==> Linux x86_64"
cargo build --release --bin sergas-zip-shrinker
stage "target/release/sergas-zip-shrinker" "sergas-zip-shrinker-linux-x86_64"

if command -v cargo-zigbuild >/dev/null && command -v zig >/dev/null; then
  echo "==> Windows x86_64 (zigbuild)"
  rustup target add x86_64-pc-windows-gnu >/dev/null
  cargo zigbuild --release --bin sergas-zip-shrinker --target x86_64-pc-windows-gnu
  stage "target/x86_64-pc-windows-gnu/release/sergas-zip-shrinker.exe" \
    "sergas-zip-shrinker-windows-x86_64.exe" || true
else
  echo "==> Windows skipped (install zig + cargo-zigbuild for local cross-build)"
fi

echo "==> macOS: use GitHub Actions release workflow (Apple SDK required)"

cp -f LICENSE "${OUT}/LICENSE"
cp -f LICENSE "${ROOT}/dist/LICENSE"

(
  cd "${OUT}"
  sha256sum sergas-zip-shrinker-* > SHA256SUMS 2>/dev/null || \
    shasum -a 256 sergas-zip-shrinker-* > SHA256SUMS
  cp -f SHA256SUMS "${ROOT}/dist/SHA256SUMS"
)

echo
echo "Release folder: ${OUT}"
ls -lh "${OUT}"
echo
cat <<EOF
Publish on GitHub:
  1. Commit & push
  2. git tag v${VERSION} && git push origin v${VERSION}
  3. Actions builds Linux + Windows + macOS (Intel & Apple Silicon)
     and attaches plain executables to the Release.

Users download only:
  sergas-zip-shrinker-linux-x86_64
  sergas-zip-shrinker-windows-x86_64.exe
  sergas-zip-shrinker-macos-x86_64
  sergas-zip-shrinker-macos-aarch64
EOF
