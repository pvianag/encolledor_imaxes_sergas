# Distribución

## Estrutura

```
.
├── .github/workflows/
│   ├── ci.yml             # Tests en push/PR
│   └── release.yml        # Compilación multi-OS → assets da Release
├── assets/
├── dist/                  # Saída local (non vai a git)
├── scripts/
├── src/
├── Cargo.toml
├── LICENSE
└── README.md
```

## Que descargan os usuarios

Na [última release](https://github.com/pvianag/encolledor_imaxes_sergas/releases/latest):

| Plataforma | Executábel |
|---|---|
| Linux 64-bit | `sergas-zip-shrinker-linux-x86_64` |
| Windows 64-bit | `sergas-zip-shrinker-windows-x86_64.exe` |
| macOS Intel | `sergas-zip-shrinker-macos-x86_64` |
| macOS Apple Silicon | `sergas-zip-shrinker-macos-aarch64` |

Enlaces directos (sempre a última versión) no [README](README.md#descarga-usuarios).

## Como se publican os binarios (GitHub Actions)

O workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) actívase cando:

1. Empuxas unha etiqueta `v*` (`git tag v1.0.0 && git push origin v1.0.0`), ou
2. Publicas unha **Release** na UI de GitHub, ou
3. Executas manualmente **Actions → Release → Run workflow** (indicando a etiqueta).

En cada caso:

1. Compila en Ubuntu, Windows e macOS (Intel + Apple Silicon).
2. Xera os executábeis e checksums `.sha256`.
3. Crea/actualiza a GitHub Release e **anexa os assets** para descarga.

## Publicar unha release

```bash
git checkout main
git pull
# asegúrate de que Cargo.lock está commitado
git tag v1.0.0
git push origin v1.0.0
```

Despois abre a release en:  
https://github.com/pvianag/encolledor_imaxes_sergas/releases/latest

## Empaquetado local (opcional)

```bash
./scripts/package-all.sh   # Linux + Windows → dist/
```

macOS require runner de Apple (ou SDK); úsase o workflow de GitHub.
