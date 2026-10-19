//! Clasificación de archivos por tipo.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Categoria {
    Fotos,
    Videos,
    Musica,
    Grabaciones,
    Documentos,
    Proyectos,
    Comprimidos,
    Instaladores,
    Otros,
}

impl Categoria {
    pub const TODAS: [Categoria; 9] = [
        Categoria::Fotos,
        Categoria::Videos,
        Categoria::Musica,
        Categoria::Grabaciones,
        Categoria::Documentos,
        Categoria::Proyectos,
        Categoria::Comprimidos,
        Categoria::Instaladores,
        Categoria::Otros,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Categoria::Fotos => "Fotos",
            Categoria::Videos => "Videos",
            Categoria::Musica => "Música",
            Categoria::Grabaciones => "Grabaciones de voz",
            Categoria::Documentos => "Documentos",
            Categoria::Proyectos => "Proyectos/código",
            Categoria::Comprimidos => "Comprimidos",
            Categoria::Instaladores => "Instaladores",
            Categoria::Otros => "Otros",
        }
    }
}

const FOTOS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "bmp", "tif", "tiff", "webp", "heic", "heif", "avif",
    "raw", "dng", "cr2", "cr3", "nef", "arw", "orf", "rw2", "raf", "psd", "svg", "ico",
];
const VIDEOS: &[&str] = &[
    "mp4", "m4v", "mkv", "avi", "mov", "wmv", "flv", "webm", "mpg", "mpeg", "3gp", "mts", "m2ts",
    "vob", "ts",
];
/// Formatos que casi siempre son notas de voz (WhatsApp, Telegram, celulares viejos).
const SOLO_GRABACION: &[&str] = &["amr", "3ga", "opus", "awb"];
/// Audio que puede ser música o una grabación: decide la ruta.
const AUDIO: &[&str] = &[
    "mp3", "m4a", "wav", "flac", "aac", "ogg", "wma", "aiff", "aif", "alac", "mid", "midi",
];
const DOCUMENTOS: &[&str] = &[
    "pdf", "doc", "docx", "dot", "dotx", "xls", "xlsx", "xlsm", "ppt", "pptx", "pps", "ppsx",
    "odt", "ods", "odp", "rtf", "txt", "md", "csv", "epub", "mobi", "pages", "numbers", "key",
];
const COMPRIMIDOS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab",
];
const INSTALADORES: &[&str] = &[
    "exe", "msi", "msix", "iso", "img", "dmg", "apk", "deb", "rpm", "appimage",
];

/// Pistas en la ruta que indican que un audio es una grabación y no música.
const PISTAS_GRABACION: &[&str] = &[
    "grabaci",
    "recording",
    "voice",
    "nota de voz",
    "notas de voz",
    "ptt-",
    "dictafono",
    "dictáfono",
];

/// Archivos o carpetas cuya presencia convierte a la carpeta en un proyecto de código.
const MARCADORES_PROYECTO: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "package.json",
    "cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "composer.json",
    "gemfile",
    "cmakelists.txt",
    "makefile",
    "projectsettings", // Unity
    "project.godot",
];
const EXTENSIONES_PROYECTO: &[&str] = &["sln", "csproj", "vbproj", "vcxproj", "xcodeproj"];

/// Clasifica un archivo por su extensión (y, para el audio, por su ruta).
pub fn clasificar(ruta: &Path) -> Categoria {
    let Some(ext) = ruta.extension().and_then(|e| e.to_str()) else {
        return Categoria::Otros;
    };
    let ext = ext.to_ascii_lowercase();
    let ext = ext.as_str();
    if FOTOS.contains(&ext) {
        Categoria::Fotos
    } else if VIDEOS.contains(&ext) {
        Categoria::Videos
    } else if SOLO_GRABACION.contains(&ext) {
        Categoria::Grabaciones
    } else if AUDIO.contains(&ext) {
        if parece_grabacion(ruta) {
            Categoria::Grabaciones
        } else {
            Categoria::Musica
        }
    } else if DOCUMENTOS.contains(&ext) {
        Categoria::Documentos
    } else if COMPRIMIDOS.contains(&ext) {
        Categoria::Comprimidos
    } else if INSTALADORES.contains(&ext) {
        Categoria::Instaladores
    } else {
        Categoria::Otros
    }
}

fn parece_grabacion(ruta: &Path) -> bool {
    let texto = ruta.to_string_lossy().to_lowercase();
    PISTAS_GRABACION.iter().any(|p| texto.contains(p))
}

/// ¿Este nombre (archivo o carpeta) indica que su carpeta es un proyecto de código?
pub fn es_marcador_proyecto(nombre: &str) -> bool {
    let nombre = nombre.to_ascii_lowercase();
    if MARCADORES_PROYECTO.contains(&nombre.as_str()) {
        return true;
    }
    nombre
        .rsplit_once('.')
        .is_some_and(|(_, ext)| EXTENSIONES_PROYECTO.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(ruta: &str) -> Categoria {
        clasificar(Path::new(ruta))
    }

    #[test]
    fn fotos_videos_y_documentos_por_extension() {
        assert_eq!(c("viaje/IMG_0001.jpg"), Categoria::Fotos);
        assert_eq!(c("DCIM/20230405.heic"), Categoria::Fotos);
        assert_eq!(c("cumple.mp4"), Categoria::Videos);
        assert_eq!(c("CV.pdf"), Categoria::Documentos);
        assert_eq!(c("presupuesto.xlsx"), Categoria::Documentos);
        assert_eq!(c("notas.txt"), Categoria::Documentos);
    }

    #[test]
    fn la_extension_no_distingue_mayusculas() {
        assert_eq!(c("IMG_0002.JPG"), Categoria::Fotos);
        assert_eq!(c("VIDEO.MoV"), Categoria::Videos);
        assert_eq!(c("SETUP.EXE"), Categoria::Instaladores);
    }

    #[test]
    fn comprimidos_e_instaladores() {
        assert_eq!(c("backup.7z"), Categoria::Comprimidos);
        assert_eq!(c("fotos.tar.gz"), Categoria::Comprimidos);
        assert_eq!(c("ubuntu.iso"), Categoria::Instaladores);
        assert_eq!(c("driver.msi"), Categoria::Instaladores);
    }

    #[test]
    fn audio_es_musica_salvo_que_la_ruta_diga_grabacion() {
        assert_eq!(c("Música/Soda Stereo/tema.mp3"), Categoria::Musica);
        assert_eq!(c("Grabaciones/Grabación 001.m4a"), Categoria::Grabaciones);
        assert_eq!(c("Sound Recordings/clase.wav"), Categoria::Grabaciones);
        assert_eq!(
            c("WhatsApp/Media/WhatsApp Voice Notes/x.mp3"),
            Categoria::Grabaciones
        );
    }

    #[test]
    fn notas_de_voz_por_formato() {
        assert_eq!(c("PTT-20190101-WA0001.opus"), Categoria::Grabaciones);
        assert_eq!(c("llamada.amr"), Categoria::Grabaciones);
    }

    #[test]
    fn sin_extension_o_desconocida_es_otros() {
        assert_eq!(c("Varios/SIN_EXTENSION"), Categoria::Otros);
        assert_eq!(c("raro.xyz"), Categoria::Otros);
        assert_eq!(c(".bashrc"), Categoria::Otros);
    }

    #[test]
    fn marcadores_de_proyecto() {
        assert!(es_marcador_proyecto(".git"));
        assert!(es_marcador_proyecto("package.json"));
        assert!(es_marcador_proyecto("Cargo.toml"));
        assert!(es_marcador_proyecto("MiApp.sln"));
        assert!(!es_marcador_proyecto("package.json.bak"));
        assert!(!es_marcador_proyecto("foto.jpg"));
    }
}
