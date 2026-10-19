//! Fecha en que se sacó una foto, según su EXIF. Las fotos copiadas desde un celular suelen
//! tener como fecha de modificación el día en que se copiaron, no el día en que se sacaron.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Tag, Value};

/// Año de la foto según EXIF (`DateTimeOriginal`, si no `DateTimeDigitized`, si no `DateTime`).
/// `None` si el archivo no tiene EXIF o no se puede leer.
pub fn anio_exif(ruta: &Path) -> Option<i32> {
    let archivo = File::open(ruta).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(archivo))
        .ok()?;
    [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime]
        .into_iter()
        .find_map(|tag| {
            let campo = exif.get_field(tag, In::PRIMARY)?;
            match &campo.value {
                Value::Ascii(v) => v.first().and_then(|t| anio_de_texto(t)),
                _ => None,
            }
        })
}

/// `2019:01:15 10:20:30` → 2019. Las cámaras sin reloj configurado escriben ceros o 1970.
fn anio_de_texto(t: &[u8]) -> Option<i32> {
    let anio: i32 = std::str::from_utf8(t.get(..4)?).ok()?.parse().ok()?;
    (1980..=2100).contains(&anio).then_some(anio)
}

/// Un JPEG mínimo con EXIF `DateTimeOriginal`. Lo usan la demo y los tests.
/// `fecha` con formato EXIF: `"2015:07:04 12:00:00"` (19 caracteres).
pub fn jpeg_con_exif(fecha: &str) -> Vec<u8> {
    assert_eq!(fecha.len(), 19, "formato EXIF: AAAA:MM:DD HH:MM:SS");
    // TIFF little-endian: encabezado, IFD0 con un puntero al IFD de EXIF, y el IFD de EXIF
    // con DateTimeOriginal apuntando al texto.
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II*\0");
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 en el byte 8
    // IFD0: 1 entrada (0x8769 = puntero al IFD de EXIF), tipo LONG.
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x8769u16.to_le_bytes());
    tiff.extend_from_slice(&4u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&26u32.to_le_bytes()); // IFD de EXIF en el byte 26
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no hay más IFD
    // IFD de EXIF: 1 entrada (0x9003 = DateTimeOriginal), tipo ASCII, 20 bytes.
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x9003u16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&20u32.to_le_bytes());
    tiff.extend_from_slice(&44u32.to_le_bytes()); // el texto en el byte 44
    tiff.extend_from_slice(&0u32.to_le_bytes());
    tiff.extend_from_slice(fecha.as_bytes());
    tiff.push(0);

    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(&app1);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);
    jpeg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_el_anio_del_exif() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("foto.jpg");
        std::fs::write(&ruta, jpeg_con_exif("2015:07:04 12:00:00")).unwrap();
        assert_eq!(anio_exif(&ruta), Some(2015));
    }

    #[test]
    fn sin_exif_no_hay_anio() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("foto.jpg");
        std::fs::write(&ruta, [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        assert_eq!(anio_exif(&ruta), None);
        assert_eq!(anio_exif(&dir.path().join("no-existe.jpg")), None);
    }

    #[test]
    fn fechas_sin_configurar_no_cuentan() {
        assert_eq!(anio_de_texto(b"2019:01:15 10:20:30"), Some(2019));
        assert_eq!(anio_de_texto(b"0000:00:00 00:00:00"), None);
        assert_eq!(anio_de_texto(b"1970:01:01 00:00:00"), None);
        assert_eq!(anio_de_texto(b"20"), None);
    }
}
