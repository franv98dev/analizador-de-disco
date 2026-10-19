//! Recorrido en paralelo del disco. Solo lectura: nunca abre archivos para escribir.
//!
//! Cada carpeta es una tarea de rayon. Se usa `std::fs::read_dir` directamente porque en
//! Windows trae tamaño y fecha en la misma llamada (`FindNextFile`), sin abrir cada archivo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use crate::categoria::{Categoria, clasificar, es_marcador_proyecto};
use crate::exclusiones::{Exclusiones, es_carpeta_de_usuario, niveles_revisar, puede_ser_proyecto};

#[derive(Debug, Clone)]
pub struct Archivo {
    pub ruta: PathBuf,
    pub tamano: u64,
    pub modificado: Option<SystemTime>,
    pub categoria: Categoria,
    /// Carpeta de AppData (o similar) a la que pertenece: "revisar", no sistema.
    pub revisar: Option<Arc<Path>>,
    /// Raíz del proyecto de código que lo contiene.
    pub proyecto: Option<Arc<Path>>,
}

#[derive(Debug, Clone)]
pub struct Excluido {
    pub ruta: PathBuf,
    pub motivo: &'static str,
}

#[derive(Debug, Clone)]
pub struct ErrorLectura {
    pub ruta: PathBuf,
    pub mensaje: String,
}

#[derive(Debug)]
pub struct Escaneo {
    pub raiz: PathBuf,
    pub archivos: Vec<Archivo>,
    pub excluidos: Vec<Excluido>,
    pub errores: Vec<ErrorLectura>,
    pub carpetas: u64,
    /// Enlaces simbólicos y junctions: no se siguen para no contar dos veces ni entrar en bucles.
    pub enlaces_omitidos: u64,
    pub duracion: Duration,
}

impl Escaneo {
    pub fn archivos_por_segundo(&self) -> f64 {
        let s = self.duracion.as_secs_f64();
        if s > 0.0 {
            self.archivos.len() as f64 / s
        } else {
            0.0
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Opciones {
    /// Hilos de lectura. `None` = uno por núcleo. En un HDD a veces conviene menos.
    pub hilos: Option<usize>,
}

#[derive(Clone, Default)]
struct Contexto {
    proyecto: Option<Arc<Path>>,
    /// Grupo de "revisar" y cuántos niveles más se puede afinar.
    revisar: Option<(Arc<Path>, u8)>,
}

#[derive(Default)]
struct Lote {
    archivos: Vec<Archivo>,
    excluidos: Vec<Excluido>,
    errores: Vec<ErrorLectura>,
    enlaces: u64,
}

struct Comun<'a> {
    exclusiones: Exclusiones,
    tx: mpsc::Sender<Lote>,
    carpetas: AtomicU64,
    progreso: &'a AtomicU64,
}

/// Escanea `raiz`. `progreso` se va incrementando con la cantidad de archivos vistos,
/// para mostrar avance desde otro hilo.
pub fn escanear(raiz: &Path, opciones: &Opciones, progreso: &AtomicU64) -> io::Result<Escaneo> {
    let inicio = Instant::now();
    // std agrega el prefijo de rutas largas por su cuenta cuando hace falta.
    let raiz = sin_prefijo_largo(fs::canonicalize(raiz)?);
    if !raiz.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "la ruta no es una carpeta",
        ));
    }

    let mut pool = rayon::ThreadPoolBuilder::new();
    if let Some(n) = opciones.hilos {
        pool = pool.num_threads(n);
    }
    let pool = pool.build().map_err(io::Error::other)?;

    let (tx, rx) = mpsc::channel();
    let comun = Comun {
        exclusiones: Exclusiones::new(&raiz),
        tx,
        carpetas: AtomicU64::new(0),
        progreso,
    };

    // Si se escanea AppData directamente, todo es "revisar".
    let mut ctx = Contexto::default();
    if let Some(n) = niveles_revisar(&raiz) {
        ctx.revisar = Some((Arc::from(raiz.as_path()), n));
    }

    let raiz_tarea = raiz.clone();
    pool.scope(|s| recorrer(s, raiz_tarea, ctx, &comun));

    let mut escaneo = Escaneo {
        raiz,
        archivos: Vec::new(),
        excluidos: Vec::new(),
        errores: Vec::new(),
        carpetas: comun.carpetas.load(Ordering::Relaxed),
        enlaces_omitidos: 0,
        duracion: Duration::ZERO,
    };
    for lote in rx.try_iter() {
        escaneo.archivos.extend(lote.archivos);
        escaneo.excluidos.extend(lote.excluidos);
        escaneo.errores.extend(lote.errores);
        escaneo.enlaces_omitidos += lote.enlaces;
    }
    escaneo.excluidos.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    escaneo.duracion = inicio.elapsed();
    Ok(escaneo)
}

fn recorrer<'s>(s: &rayon::Scope<'s>, dir: PathBuf, mut ctx: Contexto, comun: &'s Comun<'s>) {
    comun.carpetas.fetch_add(1, Ordering::Relaxed);
    let mut lote = Lote::default();

    let entradas: Vec<fs::DirEntry> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.map_err(|err| lote.errores.push(error(&dir, err))).ok())
            .collect(),
        Err(err) => {
            lote.errores.push(error(&dir, err));
            let _ = comun.tx.send(lote);
            return;
        }
    };

    if ctx.proyecto.is_none()
        && ctx.revisar.is_none()
        && entradas
            .iter()
            .any(|e| es_marcador_proyecto(&e.file_name().to_string_lossy()))
        && puede_ser_proyecto(&dir)
    {
        ctx.proyecto = Some(Arc::from(dir.as_path()));
    }

    // Los archivos con punto sueltos en la carpeta personal son configuración de programas:
    // "revisar", igual que las carpetas con punto.
    let configuracion: Option<Arc<Path>> =
        (ctx.revisar.is_none() && ctx.proyecto.is_none() && es_carpeta_de_usuario(&dir))
            .then(|| Arc::from(dir.as_path()));

    for entrada in entradas {
        let ruta = entrada.path();
        let tipo = match entrada.file_type() {
            Ok(t) => t,
            Err(err) => {
                lote.errores.push(error(&ruta, err));
                continue;
            }
        };
        // En Windows, las junctions ("Mis documentos", "Application Data") también son symlink.
        if tipo.is_symlink() {
            lote.enlaces += 1;
            continue;
        }
        if let Some(motivo) = comun.exclusiones.motivo(&ruta) {
            lote.excluidos.push(Excluido { ruta, motivo });
            continue;
        }

        if tipo.is_dir() {
            let mut hijo = ctx.clone();
            match &ctx.revisar {
                Some((_, n)) if *n > 0 => hijo.revisar = Some((Arc::from(ruta.as_path()), n - 1)),
                Some(_) => {}
                None if ctx.proyecto.is_none() => {
                    if let Some(n) = niveles_revisar(&ruta) {
                        hijo.revisar = Some((Arc::from(ruta.as_path()), n));
                    }
                }
                None => {}
            }
            s.spawn(move |s| recorrer(s, ruta, hijo, comun));
        } else {
            match entrada.metadata() {
                Ok(meta) => {
                    let categoria = if ctx.proyecto.is_some() {
                        Categoria::Proyectos
                    } else {
                        clasificar(&ruta)
                    };
                    lote.archivos.push(Archivo {
                        ruta,
                        tamano: meta.len(),
                        modificado: meta.modified().ok(),
                        categoria,
                        revisar: ctx
                            .revisar
                            .as_ref()
                            .map(|(g, _)| Arc::clone(g))
                            .or_else(|| {
                                configuracion.clone().filter(|_| {
                                    entrada.file_name().to_string_lossy().starts_with('.')
                                })
                            }),
                        proyecto: ctx.proyecto.clone(),
                    });
                }
                Err(err) => lote.errores.push(error(&ruta, err)),
            }
        }
    }

    comun
        .progreso
        .fetch_add(lote.archivos.len() as u64, Ordering::Relaxed);
    let _ = comun.tx.send(lote);
}

fn error(ruta: &Path, err: io::Error) -> ErrorLectura {
    ErrorLectura {
        ruta: ruta.to_path_buf(),
        mensaje: err.to_string(),
    }
}

/// `canonicalize` en Windows devuelve `\\?\C:\...`; para mostrar se quita el prefijo.
fn sin_prefijo_largo(ruta: PathBuf) -> PathBuf {
    match ruta.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(resto) if !resto.starts_with("UNC") => PathBuf::from(resto),
        _ => ruta,
    }
}
