# OCR por Lotes - Avata

Aplicación de escritorio en Rust para procesar archivos PDF mediante OCR y generar una salida PDF buscable. La interfaz permite arrastrar y soltar archivos y muestra el estado del procesamiento.

## Requisitos

- Windows.
- Rust y Cargo para compilar el proyecto.
- Poppler, con `pdftoppm.exe`.
- Tesseract OCR, con el modelo de idioma español `spa.traineddata`.

La aplicación busca los ejecutables externos en rutas relativas al ejecutable de Avata:

```text
<carpeta de la aplicación>/
├── ocr_lotes_gui.exe
├── poppler/
│   └── pdftoppm.exe
└── tesseract/
    └── tesseract.exe
```

Distribuye también los archivos auxiliares que necesiten esas herramientas (por ejemplo, DLL de Poppler/Tesseract y los datos de idioma de Tesseract). Tesseract debe poder encontrar `spa.traineddata` en su carpeta `tessdata`.

## Ejecutar desde el código

Desde la raíz del proyecto:

```powershell
cargo run
```

Para compilar una versión optimizada:

```powershell
cargo build --release
```

El ejecutable se genera en `target/release/ocr_lotes_gui.exe`. Coloca las carpetas `poppler` y `tesseract` junto a ese ejecutable antes de iniciarlo.

## Uso

1. Inicia la aplicación.
2. Arrastra uno o varios archivos con extensión `.pdf` a la ventana.
3. Consulta el log de la interfaz para ver el resultado de cada archivo.

Por cada PDF, la salida se guarda junto al archivo de origen con el sufijo `_ocr`; por ejemplo, `informe.pdf` produce `informe_ocr.pdf`. Las imágenes PNG y el archivo de lista usados durante el procesamiento se eliminan al finalizar correctamente el flujo.

## Dependencias de Rust

La interfaz gráfica utiliza [eframe](https://crates.io/crates/eframe). Las dependencias se descargan automáticamente con Cargo al compilar.
