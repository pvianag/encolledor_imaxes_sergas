# Distribución e paridade CI ↔ local

## Toolchain

| Compoñente | Versión / nota |
|---|---|
| Rust | **1.97.1** ([`rust-toolchain.toml`](rust-toolchain.toml)) |
| Dependencias | [`Cargo.lock`](Cargo.lock) (`--locked`) |
| Perfil | `release`: LTO + `strip` + `opt-level = "s"` |
| Linux | Ubuntu 22.04 host build |
| Windows | **MSVC** en `windows-latest` (non mingw/zig para releases) |
| macOS | `macos-15-intel` / `macos-15` |

O workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) publica os executábeis oficiais.

> **Por que MSVC en Windows?** O binario `x86_64-pc-windows-gnu` cruzado con Zig/mingw pode fallar ao arrancar apps GUI (eframe/OpenGL) sen amosar erro. O build nativo MSVC é o que deben descargar os usuarios.

## Que descargan os usuarios

[Última release](https://github.com/pvianag/encolledor_imaxes_sergas/releases/latest):

| Plataforma | Executábel | Como se constrúe |
|---|---|---|
| Linux x86_64 | `sergas-zip-shrinker-linux-x86_64` | Ubuntu 22.04 + Rust 1.97.1 |
| Windows x86_64 | `sergas-zip-shrinker-windows-x86_64.exe` | `windows-latest` + MSVC |
| macOS Intel | `sergas-zip-shrinker-macos-x86_64` | macos-15-intel |
| macOS Apple Silicon | `sergas-zip-shrinker-macos-aarch64` | macos-15 |

## Publicar

```bash
git tag v1.0.1
git push origin v1.0.1
```

## Diagnóstico en Windows

Se a app “non fai nada”, mira:

1. `%TEMP%\sergas-zip-shrinker-crash.log` (creado se hai panic/erro de arranque)
2. Un diálogo de erro (MessageBox) se o fallo chega ao código Rust
3. Historial de Microsoft Defender (pode poñer en corentena o `.exe`)

## Nota sobre glibc (Linux)

O CI usa **Ubuntu 22.04** (glibc máis antiga → máis compatible). Unha compilación local en Debian/Ubuntu máis nova pode enlazar unha glibc superior; para distribuír usa os artefactos do CI.
