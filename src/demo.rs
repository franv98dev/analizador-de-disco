//! Carpeta de prueba con un poco de todo: lo del usuario, sistema que hay que excluir,
//! AppData para revisar, duplicados y cosas probablemente borrables.
//! La usan `analizador demo` y los tests de integración.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Marca que identifica una carpeta creada por la demo, para poder recrearla sin miedo.
pub const MARCA: &str = "LEEME-demo.txt";
const TEXTO_MARCA: &str = "Carpeta creada por `analizador demo`. Se puede borrar.\n";

/// (ruta relativa, tamaño en KB, año de última modificación). Cada archivo tiene contenido
/// propio (derivado de su ruta), así dos archivos del mismo tamaño no son duplicados.
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
    // Instaladores (el portable no está en Descargas ni se llama "setup": no es borrable)
    ("Descargas/setup.exe", 3072, Some(2022)),
    ("Descargas/ubuntu.iso", 4096, Some(2024)),
    ("Descargas/driver.msi", 700, Some(2016)),
    ("Varios/juego-portable/juego.exe", 300, Some(2017)),
    // Otros
    ("Varios/archivo.xyz", 3, None),
    ("Varios/SIN_EXTENSION", 1, None),
    // Probablemente borrable: temporales y cachés
    ("Documentos/~$esupuesto.xlsx", 1, None),
    ("Descargas/pelicula.mp4.crdownload", 500, None),
    ("Varios/viejo.tmp", 10, None),
    ("Imágenes/Thumbs.db", 20, None),
    ("Proyectos/mi-web/node_modules/react/index.js", 50, None),
    // Revisar: AppData no es sistema, puede tener cosas valiosas
    ("AppData/Roaming/WhatsApp/databases/msgstore.db", 600, None),
    (
        "AppData/Local/Google/Chrome/User Data/Default/Bookmarks",
        50,
        None,
    ),
    ("AppData/LocalLow/Juego/partida1.sav", 10, None),
    ("AppData/Local/Temp/instalador-a-medias.tmp", 200, None),
    // Sistema: tiene que quedar excluido
    ("Windows/System32/kernel32.dll", 900, None),
    ("Program Files/Programa/programa.exe", 2000, None),
    ("ProgramData/cache.bin", 100, None),
    ("$Recycle.Bin/S-1-5-21/$R0001.jpg", 300, None),
    ("System Volume Information/tracking.log", 5, None),
    ("pagefile.sys", 64, None),
];

/// Copias exactas: (ruta de la copia, ruta del original, año de la copia).
pub const COPIAS: &[(&str, &str, Option<i32>)] = &[
    (
        "Descargas/IMG_0001 (1).jpg",
        "Imágenes/Vacaciones 2019/IMG_0001.jpg",
        Some(2020),
    ),
    ("Documentos/CV copia.pdf", "Documentos/CV.pdf", Some(2015)),
    ("Descargas/CV.pdf", "Documentos/CV.pdf", Some(2021)),
    (
        "Escritorio/cumple.mp4",
        "Videos/cumple de la abuela.mp4",
        Some(2016),
    ),
];

/// Falsos duplicados: mismo tamaño, mismo principio y mismo final que el original,
/// pero un byte distinto en el medio. Solo el hash completo los distingue.
pub const FALSOS_DUPLICADOS: &[(&str, &str, Option<i32>)] = &[(
    "Imágenes/IMG_0001-editada.jpg",
    "Imágenes/Vacaciones 2019/IMG_0001.jpg",
    Some(2019),
)];

/// Fotos con fecha EXIF: (ruta, fecha de la cámara, año de modificación). La modificación
/// es la del día en que se copió del celular; al ordenar tiene que ganar la del EXIF.
pub const FOTOS_CON_EXIF: &[(&str, &str, Option<i32>)] = &[(
    "DCIM/Camera/IMG_20150704_120000.jpg",
    "2015:07:04 12:00:00",
    Some(2023),
)];

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
        escribir(destino, relativa, &contenido(relativa, *kb), *anio)?;
    }
    for (relativa, original, anio) in COPIAS {
        escribir(destino, relativa, &contenido_de(original), *anio)?;
    }
    for (relativa, fecha, anio) in FOTOS_CON_EXIF {
        escribir(
            destino,
            relativa,
            &crate::fotos::jpeg_con_exif(fecha),
            *anio,
        )?;
    }
    for (relativa, original, anio) in FALSOS_DUPLICADOS {
        let mut datos = contenido_de(original);
        let medio = datos.len() / 2;
        datos[medio] ^= 0xFF;
        escribir(destino, relativa, &datos, *anio)?;
    }
    Ok(())
}

fn escribir(destino: &Path, relativa: &str, datos: &[u8], anio: Option<i32>) -> io::Result<()> {
    let ruta = destino.join(relativa);
    fs::create_dir_all(ruta.parent().expect("las rutas de la demo tienen carpeta"))?;
    let mut f = File::create(&ruta)?;
    f.write_all(datos)?;
    if let Some(anio) = anio {
        f.set_modified(inicio_de(anio))?;
    }
    Ok(())
}

fn contenido_de(original: &str) -> Vec<u8> {
    let (_, kb, _) = ARCHIVOS
        .iter()
        .find(|(r, _, _)| *r == original)
        .expect("el original de una copia tiene que estar en ARCHIVOS");
    contenido(original, *kb)
}

/// Bytes pseudoaleatorios a partir de la ruta (xorshift): siempre los mismos para la misma ruta.
fn contenido(semilla: &str, kb: u64) -> Vec<u8> {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for b in semilla.bytes() {
        x = (x ^ b as u64).wrapping_mul(0x100_0000_01B3);
    }
    let mut datos = Vec::with_capacity((kb * 1024) as usize);
    while datos.len() < (kb * 1024) as usize {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        datos.extend_from_slice(&x.to_le_bytes());
    }
    datos.truncate((kb * 1024) as usize);
    datos
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_contenido_depende_de_la_ruta_y_es_estable() {
        assert_eq!(contenido("a.jpg", 2), contenido("a.jpg", 2));
        assert_ne!(contenido("a.jpg", 2), contenido("b.jpg", 2));
        assert_eq!(contenido("a.jpg", 3).len(), 3 * 1024);
    }
}
