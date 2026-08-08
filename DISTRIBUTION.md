# Distribución

## Estrutura

```
.
├── .github/workflows/     # CI e releases multiplataforma
│   ├── ci.yml
│   └── release.yml
├── assets/                # Logos (embebidos no binario)
├── dist/                  # Executábeis listos (local, non commit)
├── scripts/
│   ├── package-all.sh     # Empaqueta Linux (+ Windows con Zig)
│   ├── package-linux.sh
│   └── package-release.md
├── src/
├── Cargo.toml
├── LICENSE
└── README.md
```

## Que descargan os usuarios

Na [páxina de Releases](../../releases) de GitHub, só estes ficheiros:

| Plataforma | Executábel |
|---|---|
| Linux 64-bit | `sergas-zip-shrinker-linux-x86_64` |
| Windows 64-bit | `sergas-zip-shrinker-windows-x86_64.exe` |
| macOS Intel | `sergas-zip-shrinker-macos-x86_64` |
| macOS Apple Silicon | `sergas-zip-shrinker-macos-aarch64` |

Non hai instalador nin dependencias externas. En Linux: `chmod +x` e executar. En macOS pode facer falta permitir a app en *Seguridade e privacidade*.

## Como publicar unha release

```bash
# 1) Empaquetado local opcional (Linux + Windows neste host)
./scripts/package-all.sh

# 2) Commit, push, etiqueta
git add -A
git commit -m "Release v1.0.0"
git tag v1.0.0
git push origin main --tags
```

O workflow `Release` compila en runners nativos (Ubuntu, Windows, macOS Intel, macOS ARM) e anexa os executábeis á release de GitHub.

Tamén: **Actions → Release → Run workflow** (sen etiqueta, só artefacts; coa etiqueta `v*` crea a release).

## Compilación local

| OS | Comando |
|---|---|
| Linux | `cargo build --release --bin sergas-zip-shrinker` |
| Windows (desde Linux) | `cargo zigbuild --release --bin sergas-zip-shrinker --target x86_64-pc-windows-gnu` |
| macOS | Compilar en macOS ou via GitHub Actions (precisa SDK de Apple) |
