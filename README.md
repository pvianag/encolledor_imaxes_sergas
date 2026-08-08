# Sergas ZIP Shrinker

![SERGAS](assets/sergas_logo.png)

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-teal.svg)](#compilar)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![UI](https://img.shields.io/badge/UI-gl%20%7C%20es%20%7C%20en%20%7C%20fr%20%7C%20pt-informational.svg)](#idioma)
[![Deps](https://img.shields.io/badge/runtime-sen%20dependencias-success.svg)](#uso)

Ferramenta de escritorio para **reducir os ZIP** descargados de **e-Saúde (SERGAS)** eliminando o visualizador Alma3D e outros ficheiros innecesarios. 
O resultado é un ZIP máis pequeno con só o contido DICOM, apto para abrilo en visualizadores estándar como **Weasis**.

> PRIVADA: Esta aplicación NON accede a e-saúde nin ao Sergas. Tampouco almacena ningún tipo de información persoal.

> LOCAL: Esta aplicación non fai seguemento de datos nin envía ou captura ningunha clase de información.

> **Non oficial.** Este proxecto **non está afiliado** ao SERGAS nin a e-Saúde. Os nomes e marcas citados pertencen aos seus respectivos titulares.

## Que fai

1. Escolle ou arrastra **un ou varios** ZIP.
2. Mostra tamaño de entrada, saída estimada e aforro.
3. Xera `nome_reduced.zip` no mesmo cartafol.
4. Pregunta se queres borrar o ZIP orixinal.

## Ver os ficheiros DICOM con Weasis

Para abrir os estudos médicos do ZIP reducido, recoméndase [**Weasis**](https://weasis.org/en/getting-started/download-dicom-viewer/), un visualizador DICOM libre e multiplataforma (Windows, macOS e Linux). Podes instalalo desde a [páxina oficial de descarga](https://weasis.org/en/getting-started/download-dicom-viewer/) ou cos xestores de paquetes indicados alí (por exemplo `flatpak`, `snap` ou `winget`).

> **Aviso médico.** Esta ferramenta e o visualizador só serven para **consultar** os teus ficheiros. **Non te autodiagnostiques.** Ante dúbidas sobre a túa saúde, consulta sempre a unha profesional sanitaria cualificada.

> **Privacidade.** Gardar e controlar a propia información de saúde é un **dereito**. Debemos defendelo: non compartas estudos con datos persoais sen necesidade, e evita subir ZIP reais a Internet ou a repositorios públicos.

## Uso

```bash
cargo run --release
# ou
./target/release/sergas-zip-shrinker
```

Podes arrastrar **varios ZIP á vez**. Cada saída gárdase como `nome_reduced.zip` no mesmo cartafol.

Binario autónomo: non precisa instalador nin librerías extra no equipo do usuario (no eixe compilado).

Verificación local dun ZIP (non subir mostras clínicas ao repo):

```bash
cargo run --bin verify_sample -- ./o_teu_ficheiro.zip
```

## Idioma

Galego por defecto. Tamén: español, inglés, francés e portugués (selector na ventá).

## Compilar

Requisitos: [Rust](https://rustup.rs/) estable.

```bash
cargo build --release
```

| Sistema | Binario |
|---------|---------|
| Linux   | `target/release/sergas-zip-shrinker` |
| Windows | `target/release/sergas-zip-shrinker.exe` |
| macOS   | `target/release/sergas-zip-shrinker` |

### Notas multiplataforma

- **Windows 7+ (x64)**, macOS e Linux.
- En Linux poden facer falta paquetes de desenvolvemento X11/GTK para compilar (`libgtk-3-dev`, `libxcb-*-dev`, `libxkbcommon-dev`).
- Configuración local: `sergas_shrink.cfg` no cartafol de configuración do usuario.

## Privacidade no repositorio

Este repositorio **non contén** estudos clínicos nin ZIP reais de pacientes. Non subas mostras con datos persoais a GitHub. A confidencialidade dos datos de saúde é un dereito; protéxeos e non os expós innecesariamente.

## Licenza

[GPL-3.0-or-later](LICENSE)
