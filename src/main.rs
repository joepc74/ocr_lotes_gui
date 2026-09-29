#![windows_subsystem = "windows"]
#[cfg(target_os = "windows")]

use eframe::egui;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::env;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

struct OcrApp {
    logs: Arc<Mutex<Vec<String>>>,
}

impl OcrApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            logs: Arc::new(Mutex::new(vec!["Esperando archivos PDF...".to_string()])),
        }
    }

    fn agregar_log(&self, mensaje: String) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(mensaje);
        }
    }
}

impl eframe::App for OcrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("OCR por Lotes - Avata (Generador de PDF Buscable)");
            });

            ui.add_space(10.0);

            // 1. Zona de Arrastre (Drag and Drop Visual)
            let color_caja = if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                egui::Color32::from_rgb(180, 220, 180) 
            } else {
                egui::Color32::from_rgb(220, 220, 220)
            };

            egui::Frame::canvas(ui.style())
                .fill(color_caja)
                .stroke(egui::Stroke::new(2.0_f32, egui::Color32::GRAY))
                .show(ui, |ui| {
                    let area = egui::vec2(ui.available_width(), ui.available_height() * 0.6);
                    ui.allocate_ui_with_layout(
                        area,
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                        ui.label(
                            egui::RichText::new("Arrastra tus archivos PDF aquí")
                                .size(16.0)
                                .strong()
                                .color(egui::Color32::from_rgb(40, 40, 40)),
                        );
                        },
                    );
                });

            ui.add_space(15.0);
            ui.label(egui::RichText::new("Log de archivos procesados:").strong());

            // 2. Área de Logs con Scrollbar
            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 20.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if let Ok(logs) = self.logs.lock() {
                        for log in logs.iter() {
                            ui.label(log);
                        }
                    }
                });

            // 3. Capturar el evento de soltar archivos (Dropped Files)
            if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
                let archivos = ctx.input(|i| i.raw.dropped_files.clone());

                for archivo in archivos {
                    if let Some(ruta) = archivo.path {
                        if ruta.extension().and_then(|s| s.to_str()) == Some("pdf") {
                            let logs_clonados = Arc::clone(&self.logs);
                            let ctx_clonado = ctx.clone();

                            self.agregar_log(format!("🔄 Iniciando OCR: {:?}", ruta.file_name().unwrap_or_default()));
                            ctx.request_repaint();

                            thread::spawn(move || {
                                generar_pdf_ocr(ruta, logs_clonados, ctx_clonado);
                            });
                        } else {
                            self.agregar_log(format!("❌ Omitido (No es PDF): {:?}", archivo.name));
                        }
                    }
                }
            }
        });
    }
}

fn generar_pdf_ocr(ruta_pdf: PathBuf, logs: Arc<Mutex<Vec<String>>>, ctx: egui::Context) {
    let ctx_clonado = ctx.clone();
    let agregar_log = |msg: String| {
        if let Ok(mut l) = logs.lock() {
            l.push(msg);
        }
        ctx_clonado.request_repaint();
    };

    let nombre_base = ruta_pdf.file_stem().unwrap_or_default().to_string_lossy();
    let directorio = ruta_pdf.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Prefijo para las imágenes temporales
    let prefijo_imagenes = directorio.join(format!("{}_tmp_page", nombre_base));

    let mut ruta_poppler = env::current_exe().expect("No se pudo obtener la ruta del ejecutable");
    ruta_poppler.pop();
    ruta_poppler.push("poppler");
    ruta_poppler.push("pdftoppm.exe");

    // 1. Extraer las páginas del PDF a imágenes PNG
    let output_pdf = Command::new(&ruta_poppler)
        .args(&[
            "-png",
            "-r", "150", // Resolución de 150 DPI para equilibrar velocidad y calidad
            ruta_pdf.to_str().unwrap(), 
            prefijo_imagenes.to_str().unwrap()
        ])
        .creation_flags(0x08000000) // Evitar ventana de consola en Windows
        .output();

    if let Err(e) = output_pdf {
        agregar_log(format!("❌ Error al ejecutar pdftoppm (Verifica Poppler): {}", e));
        return;
    }

    // 2. Reunir la lista de imágenes generadas y guardarlas en un archivo de texto temporal
    let mut lista_imagenes = String::new();
    let mut lista_rutas_borrar = Vec::new();
    let mut pagina = 1;

    loop {
        let ruta_imagen = directorio.join(format!("{}_tmp_page-{}.png", nombre_base, pagina));
        if !ruta_imagen.exists() {
            break;
        }
        // Tesseract necesita las rutas absolutas o relativas separadas por saltos de línea
        lista_imagenes.push_str(&format!("{}\n", ruta_imagen.to_str().unwrap()));
        lista_rutas_borrar.push(ruta_imagen);
        pagina += 1;
    }

    if lista_imagenes.is_empty() {
        agregar_log(format!("❌ No se pudieron generar imágenes para {:?}", ruta_pdf.file_name().unwrap_or_default()));
        return;
    }

    // Guardar el archivo de texto que leerá Tesseract
    let ruta_lista_txt = directorio.join(format!("{}_img_list.txt", nombre_base));
    if let Ok(mut archivo_lista) = File::create(&ruta_lista_txt) {
        let _ = archivo_lista.write_all(lista_imagenes.as_bytes());
    } else {
        agregar_log("❌ Error al crear la lista temporal de imágenes".to_string());
        return;
    }

    // 3. Ejecutar Tesseract pasándole la lista de texto y pidiendo salida en PDF
    let ruta_salida_final = directorio.join(format!("{}_ocr", nombre_base)); // Tesseract le añade automáticamente .pdf

    let mut ruta_tesseract = env::current_exe().expect("No se pudo obtener la ruta del ejecutable");
    ruta_tesseract.pop();
    ruta_tesseract.push("tesseract");
    ruta_tesseract.push("tesseract.exe");

    // Comando: tesseract lista.txt ruta_salida -l spa pdf
    let ocr_output = Command::new(&ruta_tesseract)
        .args(&[
            ruta_lista_txt.to_str().unwrap(),
            ruta_salida_final.to_str().unwrap(),
            "-l", "spa",
            "pdf" // <--- Aquí le indicamos que genere el formato PDF buscable
        ])
        .creation_flags(0x08000000) // Evitar ventana de consola en Windows
        .output();

    match ocr_output {
        Ok(output) if output.status.success() => {
            agregar_log(format!("✅ Completado: guardado como '{}_ocr.pdf'", nombre_base));
        }
        Ok(output) => {
            let error = String::from_utf8_lossy(&output.stderr);
            agregar_log(format!("❌ Error en Tesseract: {}", error));
        }
        Err(e) => {
            agregar_log(format!("❌ No se pudo ejecutar Tesseract: {}", e));
        }
    }

    // 4. Limpieza: Borrar todas las imágenes y la lista de texto temporal
    let _ = std::fs::remove_file(ruta_lista_txt);
    for ruta_img in lista_rutas_borrar {
        let _ = std::fs::remove_file(ruta_img);
    }

    // 5. Renombrado del archivo original a _original.pdf y del generado al nombre original
    let ruta_original_renombrada = directorio.join(format!("{}_original.pdf", nombre_base));
    let ruta_ocr_final = directorio.join(format!("{}_ocr.pdf", nombre_base));

    if ruta_pdf.exists() {
        let _ = std::fs::rename(&ruta_pdf, &ruta_original_renombrada);
    }
    if ruta_ocr_final.exists() {
        let _ = std::fs::rename(&ruta_ocr_final, &ruta_pdf);
    }
}

fn crear_icono() -> egui::IconData {
    const TAMANO: usize = 64;
    let mut rgba = vec![0; TAMANO * TAMANO * 4];

    for y in 0..TAMANO {
        for x in 0..TAMANO {
            let indice = (y * TAMANO + x) * 4;
            let dx = x as i32 - 42;
            let dy = y as i32 - 40;
            let distancia_lupa = dx * dx + dy * dy;

            let color = if (36..=64).contains(&distancia_lupa) {
                [244, 184, 72, 255]
            } else if x >= 47 && y >= 47 && (x as i32 - y as i32).abs() <= 3 {
                [244, 184, 72, 255]
            } else if (12..=41).contains(&x) && (8..=49).contains(&y) {
                if [21, 27, 33].iter().any(|linea| y == *linea) && (18..=35).contains(&x) {
                    [74, 112, 109, 255]
                } else {
                    [250, 250, 244, 255]
                }
            } else {
                [35, 115, 105, 255]
            };

            rgba[indice..indice + 4].copy_from_slice(&color);
        }
    }

    egui::IconData {
        rgba,
        width: TAMANO as u32,
        height: TAMANO as u32,
    }
}

fn main() -> eframe::Result<()> {
    let opciones_ventana = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_icon(Arc::new(crear_icono()))
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "OCR por Lotes - Avata",
        opciones_ventana,
        Box::new(|cc| Ok(Box::new(OcrApp::new(cc)))),
    )
}
