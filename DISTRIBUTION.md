# Distribución e paridade CI ↔ local

## Toolchain compartido

| Compoñente | Versión |
|---|---|
| Rust | **1.97.1** ([`rust-toolchain.toml`](rust-toolchain.toml)) |
| Dependencias | [`Cargo.lock`](Cargo.lock) (`--locked`) |
| Perfil | `release`: LTO + `strip` + `opt-level = "s"` |
| Zig | **0.13.0** (só Windows cross) |
| cargo-zigbuild | **0.23.0** |
| Windows target | `x86_64-pc-windows-gnu` |

O script local [`scripts/package-all.sh`](scripts/package-all.sh) e o workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) usan **os mesmos comandos**:

```bash
# Linux
cargo build --release --locked --bin sergas-zip-shrinker

# Windows (desde Linux)
cargo zigbuild --release --locked --bin sergas-zip-shrinker \
  --target x86_64-pc-windows-gnu
```

macOS compílase só en runners nativos de GitHub (`macos-15-intel` / `macos-15`) co mesmo Rust e `Cargo.lock`.

## Que descargan os usuarios

[Última release](https://github.com/pvianag/encolledor_imaxes_sergas/releases/latest):

| Plataforma | Executábel | Como se constrúe |
|---|---|---|
| Linux x86_64 | `sergas-zip-shrinker-linux-x86_64` | Ubuntu 22.04 + Rust 1.97.1 |
| Windows x86_64 | `sergas-zip-shrinker-windows-x86_64.exe` | Zig 0.13 + windows-gnu |
| macOS Intel | `sergas-zip-shrinker-macos-x86_64` | macos-15-intel |
| macOS Apple Silicon | `sergas-zip-shrinker-macos-aarch64` | macos-15 |

## Publicar

```bash
git tag v1.0.1
git push origin v1.0.1
```

## Nota sobre glibc (Linux)

O CI usa **Ubuntu 22.04** (glibc máis antiga → máis compatible). Unha compilación local en Debian/Ubuntu máis nova pode enlazar unha glibc superior; a funcionalidade é a mesma, pero para distribuír usa os artefactos do CI.
