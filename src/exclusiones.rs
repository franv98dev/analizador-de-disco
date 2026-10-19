//! Qué se considera sistema (se excluye), qué hay que "revisar" (AppData y similares)
//! y dónde no tiene sentido buscar proyectos.
//!
//! Las reglas trabajan sobre rutas normalizadas como texto, así se pueden testear las
//! reglas de Windows desde Linux y viceversa.

use std::path::Path;

/// Se excluyen en cualquier nivel: nunca son del usuario.
const SIEMPRE_SISTEMA: &[(&str, &str)] = &[
    ("$recycle.bin", "papelera de Windows"),
    ("system volume information", "datos internos de Windows"),
];

/// Se excluyen solo en la raíz de una unidad (o en la raíz del escaneo, para discos montados).
/// Una carpeta "Windows" dentro de Documentos sigue siendo del usuario.
const RAIZ_WINDOWS: &[(&str, &str)] = &[
    ("windows", "sistema Windows"),
    ("program files", "programas instalados"),
    ("program files (x86)", "programas instalados"),
    ("programdata", "datos de programas"),
    ("pagefile.sys", "memoria virtual de Windows"),
    ("hiberfil.sys", "hibernación de Windows"),
    ("swapfile.sys", "memoria virtual de Windows"),
];

/// Rutas absolutas del sistema Linux.
const LINUX: &[&str] = &["/usr", "/proc", "/sys", "/dev", "/run", "/boot", "/var/lib"];

/// Carpetas personales conocidas: un `package.json` suelto ahí no las vuelve proyecto.
const CARPETAS_PERSONALES: &[&str] = &[
    "documents",
    "documentos",
    "mis documentos",
    "desktop",
    "escritorio",
    "downloads",
    "descargas",
    "pictures",
    "imágenes",
    "imagenes",
    "music",
    "música",
    "musica",
    "videos",
    "vídeos",
    "onedrive",
];

/// Pasa a minúsculas, usa `/` como separador, quita el prefijo `\\?\` y la barra final.
pub fn normalizar(ruta: &Path) -> String {
    let mut s = ruta.to_string_lossy().replace('\\', "/").to_lowercase();
    if let Some(resto) = s.strip_prefix("//?/unc/") {
        s = format!("//{resto}");
    } else if let Some(resto) = s.strip_prefix("//?/") {
        s = resto.to_string();
    }
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    s
}

/// `""`, `/` o una unidad tipo `c:`.
fn es_raiz(norm: &str) -> bool {
    let b = norm.as_bytes();
    norm.is_empty() || norm == "/" || (b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

fn padre_y_nombre(norm: &str) -> (&str, &str) {
    norm.rsplit_once('/').unwrap_or(("", norm))
}

pub struct Exclusiones {
    raiz: String,
}

impl Exclusiones {
    /// `raiz` es la carpeta que se está escaneando.
    pub fn new(raiz: &Path) -> Self {
        Exclusiones {
            raiz: normalizar(raiz),
        }
    }

    /// Si la ruta es del sistema, devuelve el motivo.
    pub fn motivo(&self, ruta: &Path) -> Option<&'static str> {
        let n = normalizar(ruta);
        let (padre, nombre) = padre_y_nombre(&n);

        if let Some((_, m)) = SIEMPRE_SISTEMA.iter().find(|(x, _)| *x == nombre) {
            return Some(m);
        }
        if (es_raiz(padre) || padre == self.raiz)
            && let Some((_, m)) = RAIZ_WINDOWS.iter().find(|(x, _)| *x == nombre)
        {
            return Some(m);
        }
        if n.starts_with('/') {
            let es_linux = LINUX.iter().any(|p| {
                n.strip_prefix(p)
                    .is_some_and(|r| r.is_empty() || r.starts_with('/'))
            });
            if es_linux {
                return Some("sistema Linux");
            }
        }
        None
    }
}

/// Carpetas de datos de programas que pueden tener cosas valiosas (partidas guardadas,
/// perfiles de navegador, WhatsApp de escritorio). Devuelve cuántos niveles más abajo
/// agrupar: `AppData/Roaming/WhatsApp` → 2, `.config/google-chrome` → 1, `.vscode` → 0.
///
/// `AppData` cuenta en cualquier lado; las carpetas con punto (`.config`, `.vscode`,
/// `.cargo`…) solo directamente en la carpeta personal, para no agarrar el `.git` de un repo.
pub fn niveles_revisar(dir: &Path) -> Option<u8> {
    let n = normalizar(dir);
    let (padre, nombre) = padre_y_nombre(&n);
    if nombre == "appdata" {
        return Some(2);
    }
    if !nombre.starts_with('.') || !es_carpeta_personal(padre) {
        return None;
    }
    match nombre {
        ".local" | ".var" => Some(2),
        ".config" => Some(1),
        _ => Some(0),
    }
}

/// `c:/users/fran` o `/home/fran` (ya normalizada).
fn es_carpeta_personal(norm: &str) -> bool {
    let nombre_padre = padre_y_nombre(padre_y_nombre(norm).0).1;
    nombre_padre == "users" || nombre_padre == "home"
}

/// ¿Puede esta carpeta ser la raíz de un proyecto? No lo son la raíz de un disco,
/// la carpeta personal del usuario ni las carpetas personales conocidas, aunque
/// tengan un `package.json` o un `.git` (pasa: repos de dotfiles, `npm install` por error).
pub fn puede_ser_proyecto(dir: &Path) -> bool {
    let n = normalizar(dir);
    if es_raiz(&n) {
        return false;
    }
    if es_carpeta_personal(&n) {
        return false;
    }
    !CARPETAS_PERSONALES.contains(&padre_y_nombre(&n).1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motivo(raiz: &str, ruta: &str) -> Option<&'static str> {
        Exclusiones::new(Path::new(raiz)).motivo(Path::new(ruta))
    }

    #[test]
    fn excluye_carpetas_de_sistema_en_la_raiz_de_windows() {
        for ruta in [
            r"C:\Windows",
            r"C:\Program Files",
            r"C:\Program Files (x86)",
            r"C:\ProgramData",
            r"C:\pagefile.sys",
            r"C:\hiberfil.sys",
        ] {
            assert!(motivo(r"C:\", ruta).is_some(), "debería excluir {ruta}");
        }
    }

    #[test]
    fn no_distingue_mayusculas_ni_prefijo_largo() {
        assert!(motivo(r"C:\", r"c:\WINDOWS").is_some());
        assert!(motivo(r"\\?\C:\", r"\\?\C:\Program Files").is_some());
    }

    #[test]
    fn una_carpeta_windows_dentro_de_documentos_es_del_usuario() {
        assert_eq!(motivo(r"C:\", r"C:\Users\fran\Documents\Windows"), None);
        assert_eq!(motivo(r"C:\", r"C:\Users\fran\pagefile.sys"), None);
    }

    #[test]
    fn excluye_sistema_en_la_raiz_del_escaneo_para_discos_montados() {
        assert!(motivo("/mnt/disco-viejo", "/mnt/disco-viejo/Windows").is_some());
        assert!(motivo("/mnt/disco-viejo", "/mnt/disco-viejo/Program Files").is_some());
        assert_eq!(
            motivo("/mnt/disco-viejo", "/mnt/disco-viejo/fotos/Windows"),
            None
        );
    }

    #[test]
    fn papelera_y_system_volume_information_en_cualquier_nivel() {
        assert!(motivo(r"E:\", r"E:\$Recycle.Bin").is_some());
        assert!(motivo(r"E:\", r"E:\backup\$RECYCLE.BIN").is_some());
        assert!(motivo(r"E:\", r"E:\System Volume Information").is_some());
    }

    #[test]
    fn excluye_carpetas_de_sistema_de_linux() {
        for ruta in [
            "/usr",
            "/usr/share/icons",
            "/proc",
            "/sys",
            "/dev",
            "/run",
            "/boot",
            "/var/lib",
            "/var/lib/docker",
        ] {
            assert!(motivo("/", ruta).is_some(), "debería excluir {ruta}");
        }
    }

    #[test]
    fn no_excluye_rutas_de_linux_que_solo_se_parecen() {
        assert_eq!(motivo("/", "/usrlocal"), None);
        assert_eq!(motivo("/", "/var/log"), None);
        assert_eq!(motivo("/", "/home/fran/dev"), None);
        assert_eq!(motivo("/home/fran", "/home/fran/proc"), None);
    }

    #[test]
    fn appdata_y_equivalentes_de_linux_se_marcan_para_revisar() {
        let r = |s: &str| niveles_revisar(Path::new(s));
        assert_eq!(r(r"C:\Users\fran\AppData"), Some(2));
        assert_eq!(r("/home/fran/.config"), Some(1));
        assert_eq!(r("/home/fran/.local"), Some(2));
        assert_eq!(r(r"C:\Users\fran\Documents"), None);
    }

    #[test]
    fn carpetas_con_punto_de_la_carpeta_personal_son_de_programas() {
        let r = |s: &str| niveles_revisar(Path::new(s));
        assert_eq!(r(r"C:\Users\fran\.vscode"), Some(0));
        assert_eq!(r("/home/fran/.cargo"), Some(0));
        // El .git de un repo no es una carpeta de programa.
        assert_eq!(r(r"C:\Users\fran\code\repo\.git"), None);
    }

    #[test]
    fn appdata_no_es_sistema() {
        assert_eq!(motivo(r"C:\", r"C:\Users\fran\AppData"), None);
    }

    #[test]
    fn donde_no_se_buscan_proyectos() {
        assert!(!puede_ser_proyecto(Path::new(r"C:\")));
        assert!(!puede_ser_proyecto(Path::new("/")));
        assert!(!puede_ser_proyecto(Path::new(r"C:\Users\fran")));
        assert!(!puede_ser_proyecto(Path::new("/home/fran")));
        assert!(!puede_ser_proyecto(Path::new(r"C:\Users\fran\Documents")));
        assert!(puede_ser_proyecto(Path::new(
            r"C:\Users\fran\Documents\mi-web"
        )));
        assert!(puede_ser_proyecto(Path::new("/home/fran/code/juego")));
    }
}
