//! Carpeta de prueba con un poco de todo: lo del usuario, sistema que hay que excluir
//! y AppData para revisar. La usan `analizador demo` y los tests de integración.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Marca que identifica una carpeta creada por la demo, para poder recrearla sin miedo.
pub const MARCA: &str = "LEEME-demo.txt";
const TEXTO_MARCA: &str = "Carpeta creada por `analizador demo`. Se puede borrar.\n";

/// (ruta relativa, tamaño en KB, año de última modificación)
pub const ARCHIVOS: &[(&str, u64, Option<i32>)] = &[
    // Fotos
    ("Imágenes/Vacaciones 2019/IMG_0001.jpg", 300, Some(2019)),
    ("Imágenes/Vacaciones 2019/IMG_0002.JPG", 250, Some(2019)),
    ("Imágenes/escaneo-dni.png", 120, Some(2021)),
    ("DCIM/Camera/20230405_101010.heic", 400, Some(2023)),
    // Videos
    ("Videos/cumple de la abuela.mp4", 2048, Some(2015)),
    ("DCIM/Camera/VID_20230405.mov", 1536, Some(2023)),
    // Música
    ("Música/Soda Stereo/De música ligera.mp3", 500, Some(2012)),
    ("Música/tema.flac", 800, Some(2012)),
    // Grabaciones de voz
    ("Grabaciones/Grabación 001.m4a", 200, Some(2020)),
    (
        "WhatsApp/Media/WhatsApp Voice Notes/PTT-20190101-WA0001.opus",
        30,
        Some(2019),
    ),
    // Documentos (incluye una carpeta llamada "Windows" que NO es del sistema)
    ("Documentos/CV.pdf", 150, Some(2009)),
    ("Documentos/presupuesto.xlsx", 40, Some(2024)),
    ("Documentos/notas.txt", 2, None),
    ("Documentos/Windows/trucos de windows.docx", 25, Some(2010)),
    // Proyectos: todo lo que hay adentro cuenta como proyecto, aunque sea una imagen
    ("Proyectos/mi-web/package.json", 1, None),
    ("Proyectos/mi-web/src/index.js", 4, None),
    ("Proyectos/mi-web/public/logo.png", 20, None),
    ("Proyectos/juego/.git/HEAD", 1, None),
    ("Proyectos/juego/src/main.rs", 8, None),
    // Comprimidos
    ("Descargas/fotos-viejas.zip", 1024, Some(2018)),
    ("Descargas/backup.7z", 512, Some(2018)),
    // Instaladores
    ("Descargas/setup.exe", 3072, Some(2022)),
    ("Descargas/ubuntu.iso", 4096, Some(2024)),
    ("Descargas/driver.msi", 700, Some(2016)),
    // Otros
    ("Varios/archivo.xyz", 3, None),
    ("Varios/SIN_EXTENSION", 1, None),
    // Revisar: AppData no es sistema, puede tener cosas valiosas
    ("AppData/Roaming/WhatsApp/databases/msgstore.db", 600, None),
    (
        "AppData/Local/Google/Chrome/User Data/Default/Bookmarks",
        50,
        None,
    ),
    ("AppData/LocalLow/Juego/partida1.sav", 10, None),
    // Sistema: tiene que quedar excluido
    ("Windows/System32/kernel32.dll", 900, None),
    ("Program Files/Programa/programa.exe", 2000, None),
    ("ProgramData/cache.bin", 100, None),
    ("$Recycle.Bin/S-1-5-21/$R0001.jpg", 300, None),
    ("System Volume Information/tracking.log", 5, None),
    ("pagefile.sys", 64, None),
];

/// Crea la demo en `destino`. Si ya existe y es una demo anterior la recrea;
/// si existe y no es una demo, no toca nada.
pub fn crear(destino: &Path) -> io::Result<()> {
    if destino.exists() {
        let es_demo = fs::read_to_string(destino.join(MARCA)).is_ok_and(|t| t == TEXTO_MARCA);
        if !es_demo {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} ya existe y no es una carpeta de demo; elegí otra",
                    destino.display()
                ),
            ));
        }
        fs::remove_dir_all(destino)?;
    }
    fs::create_dir_all(destino)?;
    fs::write(destino.join(MARCA), TEXTO_MARCA)?;

    for (relativa, kb, anio) in ARCHIVOS {
        let ruta = destino.join(relativa);
        fs::create_dir_all(ruta.parent().expect("las rutas de la demo tienen carpeta"))?;
        let mut f = File::create(&ruta)?;
        f.write_all(&vec![0u8; (*kb * 1024) as usize])?;
        if let Some(anio) = anio {
            f.set_modified(inicio_de(*anio))?;
        }
    }
    Ok(())
}

/// 1 de enero del año a las 12:00 UTC (aproximado, alcanza para la demo).
fn inicio_de(anio: i32) -> SystemTime {
    let dias = (1970..anio)
        .map(|a| if es_bisiesto(a) { 366 } else { 365 })
        .sum::<u64>();
    SystemTime::UNIX_EPOCH + Duration::from_secs(dias * 86_400 + 12 * 3600)
}

fn es_bisiesto(a: i32) -> bool {
    (a % 4 == 0 && a % 100 != 0) || a % 400 == 0
}
