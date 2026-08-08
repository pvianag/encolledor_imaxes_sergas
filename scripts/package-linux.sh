#!/usr/bin/env bash
# Build and stage the Linux x86_64 executable into dist/ for local testing
# or manual upload. Full multi-OS releases are produced by GitHub Actions.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)"
OUT_DIR="${ROOT}/dist/v${VERSION}"
NAME="sergas-zip-shrinker-linux-x86_64"

echo "==> Building release (linux-x86_64)"
cargo build --release --bin sergas-zip-shrinker

mkdir -p "${OUT_DIR}"
cp -f "target/release/sergas-zip-shrinker" "${OUT_DIR}/${NAME}"
strip "${OUT_DIR}/${NAME}" || true
chmod +x "${OUT_DIR}/${NAME}"
cp -f LICENSE "${OUT_DIR}/LICENSE"
(
  cd "${OUT_DIR}"
  sha256sum "${NAME}" > "${NAME}.sha256"
)

# Convenience copy at dist/ root (latest linux build)
cp -f "${OUT_DIR}/${NAME}" "${ROOT}/dist/${NAME}"
cp -f "${OUT_DIR}/${NAME}.sha256" "${ROOT}/dist/${NAME}.sha256"
cp -f LICENSE "${ROOT}/dist/LICENSE"

echo
echo "Packaged:"
ls -lh "${OUT_DIR}"
echo
echo "Also available as:"
ls -lh "${ROOT}/dist/${NAME}"
echo
echo "GitHub multi-OS release: push a tag, e.g."
echo "  git tag v${VERSION} && git push origin v${VERSION}"
