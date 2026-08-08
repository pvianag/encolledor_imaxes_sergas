# Empaquetado e distribución

## Estrutura do repositorio

```
.
├── assets/                 # Logos e recursos embebidos
├── src/                    # Código fonte
├── scripts/                # Empaquetado local
├── dist/                   # Saída local (non vai a git)
├── .github/workflows/      # CI + releases multi-OS
├── Cargo.toml
├── LICENSE
└── README.md
```

## Releases en GitHub (recomendado)

Os executábeis de **Linux, Windows e macOS** xéranse automaticamente:

```bash
git tag v1.0.0
git push origin v1.0.0
```

O workflow `.github/workflows/release.yml` publica na release:

| Ficheiro | Plataforma |
|---|---|
| `sergas-zip-shrinker-linux-x86_64` | Linux 64-bit |
| `sergas-zip-shrinker-windows-x86_64.exe` | Windows 64-bit |
| `sergas-zip-shrinker-macos-x86_64` | macOS Intel |
| `sergas-zip-shrinker-macos-aarch64` | macOS Apple Silicon |

Os usuarios descargan **só o executábel** da súa plataforma (máis o `.sha256` opcional).

Tamén se pode lanzar manualmente desde a pestana **Actions → Release → Run workflow**.

## Empaquetado local (só Linux neste host)

```bash
./scripts/package-linux.sh
```

Saída en `dist/` e `dist/vX.Y.Z/`.
