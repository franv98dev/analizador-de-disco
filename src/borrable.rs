//! "Probablemente borrable": sugerencias con su motivo. Nada se borra desde acá.
//!
//! Las reglas miran la ruta **relativa a la carpeta escaneada**: si se escanea algo que
//! vive dentro de `Temp` (como la demo), no se marca todo como temporal.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::arbol::Suma;
use crate::categoria::Categoria;
use crate::duplicados::Duplicados;
use crate::escaner::{Archivo, Escaneo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Motivo {
    Temporal,
    Cache,
    Instalador,
    Duplicado,
}

impl Motivo {
    pub const TODOS: [Motivo; 4] = [
        Motivo::Temporal,
        Motivo::Cache,
        Motivo::Instalador,
        Motivo::Duplicado,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Motivo::Temporal => "Temporal",
            Motivo::Cache => "Caché",
            Motivo::Instalador => "Instalador",
            Motivo::Duplicado => "Copia",
        }
    }

    pub fn explicacion(self) -> &'static str {
        match self {
            Motivo::Temporal => "archivos temporales o descargas a medias",
            Motivo::Cache => "cachés y dependencias que se regeneran solas",
            Motivo::Instalador => "instaladores: se vuelven a bajar si hacen falta",
            Motivo::Duplicado => "copias exactas de otro archivo que se conserva",
        }
    }
}

const CARPETAS_TEMPORALES: &[&str] = &["temp", "tmp"];
/// Lo que hay acá adentro lo usa Windows para desinstalar o reparar programas: no se sugiere.
const CARPETAS_INTOCABLES: &[&str] = &["package cache"];
/// Dependencias que se regeneran con `npm install`, `pip`, etc., pero solo dentro de un
/// proyecto: el `node_modules` de un programa instalado (o el global de npm) es parte del programa.
const DEPENDENCIAS_DE_PROYECTO: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
];
const CARPETAS_CACHE: &[&str] = &[
    ".cache",
    "cache",
    "caches",
    "code cache",
    "gpucache",
    "shadercache",
    "dawncache",
    "grshadercache",
    "npm-cache",
    "inetcache",
];
const EXTENSIONES_TEMPORALES: &[&str] = &["tmp", "temp", "crdownload", "part", "partial", "dmp"];
const NOMBRES_TEMPORALES: &[&str] = &["thumbs.db", ".ds_store"];
const CARPETAS_DESCARGAS: &[&str] = &["downloads", "descargas"];
/// Un `.exe` o una imagen de disco (`.iso`, `.img`) solo cuentan como instalador si se llaman
/// así o están en Descargas: un programa portable o el `.iso` de un juego para emulador no
/// se sugieren.
const PISTAS_INSTALADOR: &[&str] = &["setup", "install", "instal"];
/// Formatos que casi siempre son para instalar, estén donde estén.
const EXTENSIONES_INSTALADOR: &[&str] = &["msi", "msix", "dmg", "apk", "deb", "rpm"];

/// Motivo de un archivo y, si la regla es por carpeta (temporales, cachés), esa carpeta,
/// para agruparlos.
pub fn motivo(a: &Archivo, raiz: &Path) -> Option<(Motivo, Option<PathBuf>)> {
    let relativa = a.ruta.strip_prefix(raiz).unwrap_or(&a.ruta);
    let componentes: Vec<String> = relativa
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let (nombre, carpetas) = componentes.split_last()?;
    if carpetas
        .iter()
        .any(|c| CARPETAS_INTOCABLES.contains(&c.as_str()))
    {
        return None;
    }

    let carpeta_hasta = |i: usize| -> PathBuf {
        let mut p = raiz.to_path_buf();
        p.extend(relativa.components().take(i + 1));
        p
    };
    if let Some(i) = carpetas
        .iter()
        .position(|c| CARPETAS_TEMPORALES.contains(&c.as_str()))
    {
        return Some((Motivo::Temporal, Some(carpeta_hasta(i))));
    }
    let es_cache = |c: &String| {
        CARPETAS_CACHE.contains(&c.as_str())
            || (a.proyecto.is_some() && DEPENDENCIAS_DE_PROYECTO.contains(&c.as_str()))
    };
    if let Some(i) = carpetas.iter().position(es_cache) {
        return Some((Motivo::Cache, Some(carpeta_hasta(i))));
    }

    let ext = nombre.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if nombre.starts_with("~$")
        || NOMBRES_TEMPORALES.contains(&nombre.as_str())
        || EXTENSIONES_TEMPORALES.contains(&ext)
    {
        return Some((Motivo::Temporal, None));
    }

    if a.categoria == Categoria::Instaladores
        && (EXTENSIONES_INSTALADOR.contains(&ext)
            || PISTAS_INSTALADOR.iter().any(|p| nombre.contains(p))
            || carpetas
                .iter()
                .any(|c| CARPETAS_DESCARGAS.contains(&c.as_str())))
    {
        return Some((Motivo::Instalador, None));
    }
    None
}

/// Motivo de cada archivo del escaneo (en el mismo orden). Las copias duplicadas cuentan
/// solo si no tenían ya otro motivo; la copia que se conserva nunca se marca.
pub fn por_archivo(
    e: &Escaneo,
    dups: Option<&Duplicados>,
) -> Vec<Option<(Motivo, Option<PathBuf>)>> {
    let mut v: Vec<_> = e.archivos.par_iter().map(|a| motivo(a, &e.raiz)).collect();
    if let Some(d) = dups {
        for g in &d.grupos {
            for &i in &g.archivos[1..] {
                v[i as usize].get_or_insert((Motivo::Duplicado, None));
            }
        }
    }
    v
}

/// Una fila de la lista: un archivo suelto o una carpeta entera (temporales, cachés).
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub motivo: Motivo,
    pub ruta: PathBuf,
    pub suma: Suma,
    /// Si es un archivo suelto, su índice en `Escaneo::archivos`.
    pub archivo: Option<u32>,
}

/// Todas las sugerencias, de mayor a menor.
pub fn analizar(e: &Escaneo, dups: Option<&Duplicados>) -> Vec<Item> {
    let mut carpetas: HashMap<(Motivo, PathBuf), Suma> = HashMap::new();
    let mut items = Vec::new();
    for (i, m) in por_archivo(e, dups).into_iter().enumerate() {
        let Some((motivo, carpeta)) = m else { continue };
        let a = &e.archivos[i];
        let suma = Suma {
            cantidad: 1,
            bytes: a.tamano,
        };
        match carpeta {
            Some(c) => *carpetas.entry((motivo, c)).or_default() += suma,
            None => items.push(Item {
                motivo,
                ruta: a.ruta.clone(),
                suma,
                archivo: Some(i as u32),
            }),
        }
    }
    items.extend(carpetas.into_iter().map(|((motivo, ruta), suma)| Item {
        motivo,
        ruta,
        suma,
        archivo: None,
    }));
    items.sort_by(|a, b| {
        b.suma
            .bytes
            .cmp(&a.suma.bytes)
            .then_with(|| a.ruta.cmp(&b.ruta))
    });
    items
}

pub fn totales(items: &[Item]) -> BTreeMap<Motivo, Suma> {
    let mut t = BTreeMap::new();
    for i in items {
        *t.entry(i.motivo).or_default() += i.suma;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(ruta: &str, categoria: Categoria) -> Option<Motivo> {
        m_en_proyecto(ruta, categoria, None)
    }

    fn m_en_proyecto(ruta: &str, categoria: Categoria, proyecto: Option<&str>) -> Option<Motivo> {
        let a = Archivo {
            ruta: Path::new("/raiz").join(ruta),
            tamano: 1,
            modificado: None,
            categoria,
            revisar: None,
            proyecto: proyecto.map(|p| std::sync::Arc::from(Path::new("/raiz").join(p).as_path())),
        };
        motivo(&a, Path::new("/raiz")).map(|(m, _)| m)
    }

    #[test]
    fn temporales_por_extension_nombre_y_carpeta() {
        assert_eq!(
            m("Descargas/peli.mp4.crdownload", Categoria::Otros),
            Some(Motivo::Temporal)
        );
        assert_eq!(
            m("Documentos/~$informe.docx", Categoria::Documentos),
            Some(Motivo::Temporal)
        );
        assert_eq!(
            m("Fotos/Thumbs.db", Categoria::Otros),
            Some(Motivo::Temporal)
        );
        assert_eq!(m("x/viejo.TMP", Categoria::Otros), Some(Motivo::Temporal));
        assert_eq!(
            m("AppData/Local/Temp/algo.log", Categoria::Otros),
            Some(Motivo::Temporal)
        );
    }

    #[test]
    fn caches_por_carpeta() {
        assert_eq!(
            m_en_proyecto(
                "code/web/node_modules/react/index.js",
                Categoria::Proyectos,
                Some("code/web")
            ),
            Some(Motivo::Cache)
        );
        assert_eq!(
            m_en_proyecto(
                "code/py/__pycache__/a.pyc",
                Categoria::Proyectos,
                Some("code/py")
            ),
            Some(Motivo::Cache)
        );
        assert_eq!(
            m(
                "AppData/Local/Google/Chrome/User Data/Default/Cache/f_0001",
                Categoria::Otros
            ),
            Some(Motivo::Cache)
        );
    }

    #[test]
    fn el_node_modules_de_un_programa_instalado_no_es_cache() {
        // El global de npm y el de una app Electron son parte del programa: borrarlos lo rompe.
        assert_eq!(
            m(
                "AppData/Roaming/npm/node_modules/typescript/lib/tsc.js",
                Categoria::Otros
            ),
            None
        );
        assert_eq!(
            m(
                "AppData/Local/Programs/Editor/resources/app/node_modules/x/index.js",
                Categoria::Otros
            ),
            None
        );
    }

    #[test]
    fn la_carpeta_de_cache_es_la_que_se_agrupa() {
        let a = Archivo {
            ruta: Path::new("/raiz/web/node_modules/react/index.js").to_path_buf(),
            tamano: 1,
            modificado: None,
            categoria: Categoria::Proyectos,
            revisar: None,
            proyecto: Some(std::sync::Arc::from(Path::new("/raiz/web"))),
        };
        let (_, carpeta) = motivo(&a, Path::new("/raiz")).unwrap();
        assert_eq!(carpeta.unwrap(), Path::new("/raiz/web/node_modules"));
    }

    #[test]
    fn package_cache_de_windows_no_se_toca() {
        assert_eq!(
            m(
                "AppData/Local/Package Cache/{ABC}v1/Programa-Setup.msi",
                Categoria::Instaladores
            ),
            None
        );
    }

    #[test]
    fn instaladores_si_parecen_instaladores() {
        assert_eq!(
            m("Descargas/programa.exe", Categoria::Instaladores),
            Some(Motivo::Instalador)
        );
        assert_eq!(
            m("Escritorio/Setup_Juego.exe", Categoria::Instaladores),
            Some(Motivo::Instalador)
        );
        assert_eq!(
            m("Descargas/ubuntu.iso", Categoria::Instaladores),
            Some(Motivo::Instalador)
        );
        assert_eq!(
            m("Escritorio/driver.msi", Categoria::Instaladores),
            Some(Motivo::Instalador)
        );
    }

    #[test]
    fn un_programa_portable_no_es_borrable() {
        assert_eq!(
            m("Escritorio/pcsx2/pcsx2.exe", Categoria::Instaladores),
            None
        );
        // El .iso de un juego para emulador tampoco.
        assert_eq!(
            m(
                "Escritorio/pcsx2/games/SLUS_219.78.iso",
                Categoria::Instaladores
            ),
            None
        );
    }

    #[test]
    fn lo_comun_no_es_borrable() {
        assert_eq!(m("Fotos/IMG_0001.jpg", Categoria::Fotos), None);
        assert_eq!(
            m("Documentos/temperatura.xlsx", Categoria::Documentos),
            None
        );
        assert_eq!(m("Documentos/cacheros.docx", Categoria::Documentos), None);
    }

    #[test]
    fn las_carpetas_por_encima_de_la_raiz_no_cuentan() {
        let a = Archivo {
            ruta: Path::new("/tmp/demo/Fotos/a.jpg").to_path_buf(),
            tamano: 1,
            modificado: None,
            categoria: Categoria::Fotos,
            revisar: None,
            proyecto: None,
        };
        assert_eq!(motivo(&a, Path::new("/tmp/demo")), None);
    }
}
