//! Acciones sobre los archivos: ordenar (mover o copiar a un destino ordenado), mandar a
//! `_para_borrar` y deshacer. Siempre en dos pasos: `planificar_*` es el dry-run (no toca
//! nada) y `ejecutar` corre después de confirmar.
//!
//! Garantías: nunca se pisa un archivo (si el nombre existe se agrega ` (2)`), una copia se
//! escribe a un `.parcial` y se verifica con BLAKE3 antes de darle su nombre, y un original
//! se borra solo después de verificar su copia (al mover entre discos). Dentro del mismo
//! disco, mover es renombrar: instantáneo y sin copiar nada. Todo queda anotado en un
//! registro CSV que permite deshacerlo.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};

use crate::antiguedad;
use crate::borrable::{self, Motivo};
use crate::categoria::Categoria;
use crate::duplicados::Duplicados;
use crate::escaner::Escaneo;
use crate::exclusiones::{self, CARPETA_PARA_BORRAR, CARPETA_REGISTROS, Exclusiones};
use crate::exportar::campo_csv;
use crate::fotos;

const BUFFER: usize = 1024 * 1024;
const ENCABEZADO_REGISTRO: &str = "fecha;accion;origen;destino;bytes;resultado";
/// Nombre del registro dentro de `_para_borrar`.
pub const MANIFIESTO: &str = "manifiesto.csv";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Mover,
    Copiar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Ordenar(Modo),
    ParaBorrar,
    Deshacer,
}

impl Tipo {
    fn accion(self) -> &'static str {
        match self {
            Tipo::Ordenar(Modo::Mover) => "mover",
            Tipo::Ordenar(Modo::Copiar) => "copiar",
            Tipo::ParaBorrar => "para_borrar",
            Tipo::Deshacer => "deshacer",
        }
    }

    fn modo(self) -> Modo {
        match self {
            Tipo::Ordenar(m) => m,
            Tipo::ParaBorrar | Tipo::Deshacer => Modo::Mover,
        }
    }
}

/// Lo que hace falta de cada archivo para planificar (sin depender del escaneo, así el
/// plan se puede armar en otro hilo).
#[derive(Debug, Clone)]
pub struct Elemento {
    pub indice: u32,
    pub ruta: PathBuf,
    pub tamano: u64,
    pub modificado: Option<SystemTime>,
    pub categoria: Categoria,
    pub proyecto: Option<Arc<Path>>,
    /// Si es probablemente borrable, por qué (los temporales no se ordenan).
    pub motivo: Option<Motivo>,
}

impl Elemento {
    pub fn de(e: &Escaneo, i: u32) -> Self {
        let a = &e.archivos[i as usize];
        Elemento {
            indice: i,
            ruta: a.ruta.clone(),
            tamano: a.tamano,
            modificado: a.modificado,
            categoria: a.categoria,
            proyecto: a.proyecto.clone(),
            motivo: borrable::motivo(a, &e.raiz).map(|(m, _)| m),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operacion {
    pub origen: PathBuf,
    pub destino: PathBuf,
    pub tamano: u64,
    /// Al deshacer: la fila del registro que se revierte.
    pub fila: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub tipo: Tipo,
    /// Carpeta de destino (ordenar), carpeta `_para_borrar`, o registro que se deshace.
    pub destino: PathBuf,
    pub operaciones: Vec<Operacion>,
    pub bytes: u64,
    /// Bytes que hay que escribir de verdad (lo que se mueve dentro del mismo disco no cuenta).
    pub bytes_a_escribir: u64,
    pub espacio_libre: Option<u64>,
    /// Destinos a los que se les agregó ` (2)` para no pisar nada.
    pub renombrados: usize,
    /// Copias duplicadas que no se mueven: queda solo la que se conserva.
    pub omitidos_duplicados: usize,
    pub omitidos_instaladores: usize,
    /// Temporales (`~$`, `.tmp`, descargas a medias…): no son algo que se conserve.
    pub omitidos_temporales: usize,
    /// Ya estaban dentro del destino.
    pub omitidos_en_destino: usize,
    /// Si hay alguno, no se puede ejecutar.
    pub problemas: Vec<String>,
    /// Advertencias que no impiden ejecutar.
    pub avisos: Vec<String>,
    /// Registro CSV donde queda anotado (para deshacer).
    pub registro: PathBuf,
}

impl Plan {
    fn nuevo(tipo: Tipo, destino: PathBuf, registro: PathBuf) -> Self {
        Plan {
            tipo,
            destino,
            operaciones: Vec::new(),
            bytes: 0,
            bytes_a_escribir: 0,
            espacio_libre: None,
            renombrados: 0,
            omitidos_duplicados: 0,
            omitidos_instaladores: 0,
            omitidos_temporales: 0,
            omitidos_en_destino: 0,
            problemas: Vec::new(),
            avisos: Vec::new(),
            registro,
        }
    }

    pub fn se_puede_ejecutar(&self) -> bool {
        self.problemas.is_empty() && !self.operaciones.is_empty()
    }

    fn agregar(&mut self, origen: PathBuf, destino: PathBuf, tamano: u64, fila: Option<usize>) {
        if self.tipo.modo() == Modo::Copiar || !mismo_volumen(&origen, &destino) {
            self.bytes_a_escribir += tamano;
        }
        self.bytes += tamano;
        self.operaciones.push(Operacion {
            origen,
            destino,
            tamano,
            fila,
        });
    }

    /// Avisa si no entra en el destino.
    fn revisar_espacio(&mut self) {
        self.espacio_libre =
            carpeta_existente(&self.destino).and_then(|d| fs4::available_space(d).ok());
        if let Some(libre) = self.espacio_libre
            && self.bytes_a_escribir > libre
        {
            self.problemas.push(format!(
                "no hay espacio: hacen falta {} y en el destino hay {} libres",
                crate::formato::tamano(self.bytes_a_escribir),
                crate::formato::tamano(libre)
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpcionesOrdenar {
    pub modo: Option<Modo>,
    pub incluir_instaladores: bool,
}

/// Carpeta de cada categoría en el destino.
pub fn carpeta_categoria(c: Categoria) -> &'static str {
    match c {
        Categoria::Fotos => "Fotos",
        Categoria::Videos => "Videos",
        Categoria::Musica => "Música",
        Categoria::Grabaciones => "Grabaciones de voz",
        Categoria::Documentos => "Documentos",
        Categoria::Proyectos => "Proyectos",
        Categoria::Comprimidos => "Comprimidos",
        Categoria::Instaladores => "Instaladores",
        Categoria::Otros => "Otros",
    }
}

/// Ruta (relativa al destino) donde va un archivo:
/// - Fotos, videos y grabaciones: `Categoría/Año/archivo` (el año de las fotos sale del EXIF).
/// - Proyectos: enteros, `Proyectos/<proyecto>/<lo de adentro>`.
/// - El resto: `Categoría/<carpeta original>/archivo`, sin las carpetas personales de arriba
///   (`Users/papá/Documents/Facultad/tp.docx` → `Documentos/Facultad/tp.docx`).
pub fn ruta_ordenada(el: &Elemento, raiz: &Path, anio_exif: Option<i32>) -> PathBuf {
    let mut destino = PathBuf::from(carpeta_categoria(el.categoria));
    let nombre = el.ruta.file_name().unwrap_or(el.ruta.as_os_str());
    match el.categoria {
        Categoria::Fotos | Categoria::Videos | Categoria::Grabaciones => {
            let anio = anio_exif.or_else(|| antiguedad::anio(el.modificado));
            destino.push(antiguedad::nombre(anio));
            destino.push(nombre);
        }
        Categoria::Proyectos => match &el.proyecto {
            Some(p) => {
                destino.push(p.file_name().unwrap_or(p.as_os_str()));
                destino.push(el.ruta.strip_prefix(p).unwrap_or(Path::new(nombre)));
            }
            None => destino.push(nombre),
        },
        _ => {
            let relativa = el.ruta.strip_prefix(raiz).unwrap_or(Path::new(nombre));
            destino.push(sin_carpetas_personales(relativa));
        }
    }
    destino
}

/// Saca de adelante todo hasta la última carpeta personal conocida (Documentos, Escritorio,
/// Descargas…) o la carpeta del usuario (`Users/<usuario>`, `home/<usuario>`).
fn sin_carpetas_personales(relativa: &Path) -> PathBuf {
    let partes: Vec<Component> = relativa.components().collect();
    let carpetas = partes.len().saturating_sub(1);
    let mut desde = 0;
    for (i, parte) in partes.iter().take(carpetas).enumerate() {
        let nombre = parte.as_os_str().to_string_lossy().to_lowercase();
        if exclusiones::CARPETAS_PERSONALES.contains(&nombre.as_str()) {
            desde = i + 1;
        } else if (nombre == "users" || nombre == "home") && i + 1 < carpetas {
            desde = i + 2;
        }
    }
    partes[desde..].iter().collect()
}

/// Plan para ordenar en `destino` los elementos elegidos. Con `dups`, de cada grupo se
/// lleva solo la copia que se conserva.
pub fn planificar_ordenar(
    elementos: &[Elemento],
    raiz: &Path,
    destino: &Path,
    opciones: OpcionesOrdenar,
    dups: Option<&Duplicados>,
    ahora: DateTime<Local>,
) -> Plan {
    let modo = opciones.modo.unwrap_or(Modo::Mover);
    let destino = absoluta(destino);
    let registro = destino
        .join(CARPETA_REGISTROS)
        .join(format!("ordenar-{}.csv", ahora.format("%Y-%m-%d_%H%M%S")));
    let mut plan = Plan::nuevo(Tipo::Ordenar(modo), destino.clone(), registro);

    if let Some(p) = problema_de_destino(&destino) {
        plan.problemas.push(p);
        return plan;
    }
    let copias: HashSet<u32> = dups
        .map(|d| {
            d.grupos
                .iter()
                .flat_map(|g| g.archivos[1..].iter().copied())
                .collect()
        })
        .unwrap_or_default();
    if dups.is_none() {
        plan.avisos
            .push("No se buscaron duplicados: si hay copias, se van a ordenar todas.".to_string());
    }

    let mut usados = HashSet::new();
    for el in elementos {
        if el.ruta.starts_with(&destino) {
            plan.omitidos_en_destino += 1;
            continue;
        }
        if el.categoria == Categoria::Instaladores && !opciones.incluir_instaladores {
            plan.omitidos_instaladores += 1;
            continue;
        }
        // Dentro de un proyecto se lleva todo, para que el proyecto siga andando.
        if el.motivo == Some(Motivo::Temporal) && el.proyecto.is_none() {
            plan.omitidos_temporales += 1;
            continue;
        }
        if copias.contains(&el.indice) {
            plan.omitidos_duplicados += 1;
            continue;
        }
        let anio = (el.categoria == Categoria::Fotos)
            .then(|| fotos::anio_exif(&el.ruta))
            .flatten();
        let (final_, renombrado) = unico(destino.join(ruta_ordenada(el, raiz, anio)), &mut usados);
        plan.renombrados += renombrado as usize;
        plan.agregar(el.ruta.clone(), final_, el.tamano, None);
    }
    plan.revisar_espacio();
    plan
}

/// Plan para mandar los elementos a `<raíz>/_para_borrar/<ruta original>`: en el mismo disco,
/// así es un renombre, y con la ruta original para que se sepa de dónde vino cada cosa.
pub fn planificar_para_borrar(elementos: &[Elemento], raiz: &Path) -> Plan {
    let carpeta = raiz.join(CARPETA_PARA_BORRAR);
    let mut plan = Plan::nuevo(Tipo::ParaBorrar, carpeta.clone(), carpeta.join(MANIFIESTO));
    let mut usados = HashSet::new();
    for el in elementos {
        let Ok(relativa) = el.ruta.strip_prefix(raiz) else {
            plan.avisos.push(format!(
                "{} no está dentro de lo escaneado",
                el.ruta.display()
            ));
            continue;
        };
        if relativa.starts_with(CARPETA_PARA_BORRAR) {
            continue;
        }
        let (final_, renombrado) = unico(carpeta.join(relativa), &mut usados);
        plan.renombrados += renombrado as usize;
        plan.agregar(el.ruta.clone(), final_, el.tamano, None);
    }
    plan.revisar_espacio();
    plan
}

/// Plan para deshacer: lee un registro (el `manifiesto.csv` de `_para_borrar` o un
/// `ordenar-….csv` de `_analizador`) y devuelve cada archivo a su lugar. Acepta el archivo
/// o la carpeta que lo contiene (`_para_borrar` o la raíz donde está).
pub fn planificar_deshacer(ruta: &Path) -> io::Result<Plan> {
    let registro = encontrar_registro(ruta)?;
    let texto = fs::read_to_string(&registro)?;
    let filas = parsear_csv(texto.trim_start_matches('\u{feff}'));
    let mut plan = Plan::nuevo(Tipo::Deshacer, registro.clone(), registro.clone());
    let mut usados = HashSet::new();
    for (i, f) in filas.iter().enumerate().skip(1) {
        let [_, accion, origen, destino, bytes, resultado] = f.as_slice() else {
            continue;
        };
        if resultado != "ok" || !(accion == "mover" || accion == "para_borrar") {
            continue;
        }
        let (actual, original) = (PathBuf::from(destino), PathBuf::from(origen));
        if !actual.exists() {
            plan.avisos.push(format!(
                "{} ya no está (¿lo borraste o lo moviste?)",
                actual.display()
            ));
            continue;
        }
        let (final_, renombrado) = unico(original, &mut usados);
        plan.renombrados += renombrado as usize;
        plan.agregar(actual, final_, bytes.parse().unwrap_or(0), Some(i));
    }
    // Para deshacer el destino es "donde estaban": el espacio se mira ahí.
    if let Some(op) = plan.operaciones.first() {
        plan.destino = op.destino.parent().unwrap_or(&op.destino).to_path_buf();
    }
    plan.revisar_espacio();
    plan.destino = registro;
    Ok(plan)
}

fn encontrar_registro(ruta: &Path) -> io::Result<PathBuf> {
    if ruta.is_file() {
        return Ok(ruta.to_path_buf());
    }
    for candidato in [
        ruta.join(MANIFIESTO),
        ruta.join(CARPETA_PARA_BORRAR).join(MANIFIESTO),
    ] {
        if candidato.is_file() {
            return Ok(candidato);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "no encontré un registro en {}: pasá el {MANIFIESTO} de _para_borrar o un ordenar-….csv de _analizador",
            ruta.display()
        ),
    ))
}

/// Por qué no se puede usar esta carpeta como destino, si hay motivo.
fn problema_de_destino(destino: &Path) -> Option<String> {
    if destino.as_os_str().is_empty() {
        return Some("falta el destino".to_string());
    }
    if destino.is_file() {
        return Some(format!(
            "{} es un archivo, no una carpeta",
            destino.display()
        ));
    }
    // Sobre la ruta normalizada y no con `Path::ancestors`: así `C:\Windows\x` se reconoce
    // igual en Linux (donde `\` no separa carpetas) y los tests andan en los dos.
    let sistema = Exclusiones::new(Path::new(""));
    let norm = exclusiones::normalizar(destino);
    let prefijos = norm
        .match_indices('/')
        .map(|(i, _)| &norm[..i])
        .chain(std::iter::once(norm.as_str()));
    for a in prefijos {
        if let Some(motivo) = sistema.motivo(Path::new(a)) {
            return Some(format!(
                "{} no sirve como destino ({motivo})",
                destino.display()
            ));
        }
    }
    None
}

/// Si el nombre ya existe (en disco o en este mismo plan), agrega ` (2)`, ` (3)`…
fn unico(ruta: PathBuf, usados: &mut HashSet<String>) -> (PathBuf, bool) {
    // Sin distinguir mayúsculas: en Windows `Foto.jpg` y `foto.jpg` son el mismo archivo.
    let clave = |p: &Path| p.to_string_lossy().to_lowercase();
    if !usados.contains(&clave(&ruta)) && !existe(&ruta) {
        usados.insert(clave(&ruta));
        return (ruta, false);
    }
    let padre = ruta.parent().unwrap_or(Path::new("")).to_path_buf();
    let base = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = ruta
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2.. {
        let candidata = padre.join(format!("{base} ({n}){ext}"));
        if !usados.contains(&clave(&candidata)) && !existe(&candidata) {
            usados.insert(clave(&candidata));
            return (candidata, true);
        }
    }
    unreachable!()
}

fn existe(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn absoluta(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// La carpeta existente más cercana (el destino puede no existir todavía).
fn carpeta_existente(p: &Path) -> Option<&Path> {
    p.ancestors().find(|a| a.is_dir())
}

/// ¿Están en el mismo disco? Ahí mover es renombrar, sin copiar nada.
pub fn mismo_volumen(a: &Path, b: &Path) -> bool {
    match (volumen(a), volumen(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(windows)]
fn volumen(p: &Path) -> Option<String> {
    match absoluta(p).components().next()? {
        Component::Prefix(pre) => Some(pre.as_os_str().to_string_lossy().to_lowercase()),
        _ => None,
    }
}

#[cfg(unix)]
fn volumen(p: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    let p = absoluta(p);
    let existente = p.ancestors().find(|a| a.exists())?;
    fs::metadata(existente).ok().map(|m| m.dev())
}

/// Avance de `ejecutar`, para mostrarlo desde otro hilo. También sirve para cancelar.
#[derive(Debug, Default)]
pub struct ProgresoAccion {
    pub hechos: AtomicU64,
    pub bytes: AtomicU64,
    pub cancelar: AtomicBool,
    pub actual: Mutex<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct Resultado {
    pub hechos: usize,
    pub bytes: u64,
    pub fallidos: Vec<(Operacion, String)>,
    pub cancelado: bool,
    pub duracion: Duration,
    /// Destinos renombrados al ejecutar porque apareció un archivo con ese nombre.
    pub renombrados: usize,
}

/// Ejecuta el plan, de a un archivo (en un HDD es lo más rápido). Sigue con los demás si
/// uno falla, y anota cada operación en el registro.
pub fn ejecutar(plan: &Plan, progreso: &ProgresoAccion) -> io::Result<Resultado> {
    if !plan.se_puede_ejecutar() {
        return Err(io::Error::other(
            plan.problemas
                .first()
                .cloned()
                .unwrap_or_else(|| "no hay nada para hacer".to_string()),
        ));
    }
    let inicio = Instant::now();
    let mut resultado = Resultado::default();
    let mut registro = if plan.tipo == Tipo::Deshacer {
        None
    } else {
        Some(abrir_registro(&plan.registro)?)
    };
    let mut restauradas = HashSet::new();
    let ahora = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for op in &plan.operaciones {
        if progreso.cancelar.load(Ordering::Relaxed) {
            resultado.cancelado = true;
            break;
        }
        if let Ok(mut actual) = progreso.actual.lock() {
            actual.clone_from(&op.origen);
        }
        let hecho = ejecutar_una(op, plan.tipo.modo(), progreso);
        let estado = match &hecho {
            Ok(destino) => {
                resultado.hechos += 1;
                resultado.bytes += op.tamano;
                resultado.renombrados += (*destino != op.destino) as usize;
                if let Some(f) = op.fila {
                    restauradas.insert(f);
                }
                "ok".to_string()
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {
                resultado.cancelado = true;
                break;
            }
            Err(err) => {
                resultado.fallidos.push((op.clone(), err.to_string()));
                err.to_string()
            }
        };
        if let Some(r) = registro.as_mut() {
            let destino = hecho.as_ref().unwrap_or(&op.destino);
            let fila = [
                ahora.clone(),
                plan.tipo.accion().to_string(),
                op.origen.display().to_string(),
                destino.display().to_string(),
                op.tamano.to_string(),
                estado,
            ];
            let fila: Vec<String> = fila.iter().map(|c| campo_csv(c)).collect();
            writeln!(r, "{}\r", fila.join(";"))?;
            // Se escribe enseguida: si se corta la luz, el registro sigue sirviendo para deshacer.
            r.flush()?;
        }
        progreso.hechos.fetch_add(1, Ordering::Relaxed);
    }

    if plan.tipo == Tipo::Deshacer && !restauradas.is_empty() {
        quitar_del_registro(&plan.registro, &restauradas)?;
    }
    resultado.duracion = inicio.elapsed();
    Ok(resultado)
}

/// Mueve o copia un archivo. Devuelve dónde quedó (puede llevar ` (2)` si apareció otro
/// con ese nombre después de planificar).
fn ejecutar_una(op: &Operacion, modo: Modo, progreso: &ProgresoAccion) -> io::Result<PathBuf> {
    let meta = fs::metadata(&op.origen)?;
    if meta.len() != op.tamano {
        return Err(io::Error::other("el archivo cambió desde el escaneo"));
    }
    let destino = if existe(&op.destino) {
        unico(op.destino.clone(), &mut HashSet::new()).0
    } else {
        op.destino.clone()
    };
    if let Some(padre) = destino.parent() {
        fs::create_dir_all(padre)?;
    }
    if modo == Modo::Mover && mismo_volumen(&op.origen, &destino) {
        // `rename` pisa en Linux: se vuelve a mirar justo antes.
        if existe(&destino) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "el destino ya existe",
            ));
        }
        fs::rename(&op.origen, &destino)?;
        progreso.bytes.fetch_add(op.tamano, Ordering::Relaxed);
        return Ok(destino);
    }
    copiar_verificado(&op.origen, &destino, meta.modified().ok(), progreso)?;
    if modo == Modo::Mover {
        fs::remove_file(&op.origen)?;
    }
    Ok(destino)
}

/// Copia a `<destino>.parcial`, verifica con BLAKE3 y recién ahí le pone el nombre final.
/// Si algo falla o se cancela, no queda ningún `.parcial`.
fn copiar_verificado(
    origen: &Path,
    destino: &Path,
    modificado: Option<SystemTime>,
    progreso: &ProgresoAccion,
) -> io::Result<()> {
    let nombre = destino.file_name().unwrap_or_default().to_string_lossy();
    let parcial = destino.with_file_name(format!("{nombre}.parcial"));
    let r = (|| {
        let mut entrada = File::open(origen)?;
        let mut salida = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&parcial)?;
        let mut hash = blake3::Hasher::new();
        let mut buffer = vec![0u8; BUFFER];
        loop {
            if progreso.cancelar.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
            }
            let n = entrada.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            salida.write_all(&buffer[..n])?;
            progreso.bytes.fetch_add(n as u64, Ordering::Relaxed);
        }
        if let Some(t) = modificado {
            salida.set_modified(t)?;
        }
        salida.sync_all()?;
        drop(salida);
        if hash_de(&parcial)? != hash.finalize() {
            return Err(io::Error::other("la copia no coincide con el original"));
        }
        if existe(destino) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "el destino ya existe",
            ));
        }
        fs::rename(&parcial, destino)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&parcial);
    }
    r
}

fn hash_de(ruta: &Path) -> io::Result<blake3::Hash> {
    let mut f = File::open(ruta)?;
    let mut h = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER];
    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            return Ok(h.finalize());
        }
        h.update(&buffer[..n]);
    }
}

fn abrir_registro(ruta: &Path) -> io::Result<BufWriter<File>> {
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre)?;
    }
    let nuevo = !ruta.exists();
    let mut f = BufWriter::new(OpenOptions::new().create(true).append(true).open(ruta)?);
    if nuevo {
        write!(f, "\u{feff}{ENCABEZADO_REGISTRO}\r\n")?;
    }
    Ok(f)
}

/// Reescribe el registro sin las filas ya deshechas (a un temporal y después renombra).
fn quitar_del_registro(registro: &Path, filas: &HashSet<usize>) -> io::Result<()> {
    let texto = fs::read_to_string(registro)?;
    let todas = parsear_csv(texto.trim_start_matches('\u{feff}'));
    let mut salida = String::from("\u{feff}");
    for (i, f) in todas.iter().enumerate() {
        if filas.contains(&i) {
            continue;
        }
        let f: Vec<String> = f.iter().map(|c| campo_csv(c)).collect();
        salida.push_str(&f.join(";"));
        salida.push_str("\r\n");
    }
    let temporal = registro.with_extension("csv.nuevo");
    fs::write(&temporal, salida)?;
    fs::rename(&temporal, registro)
}

/// CSV separado por `;` con comillas (como lo escribe `exportar::campo_csv`). Soporta
/// `;`, comillas y saltos de línea dentro de un campo.
pub fn parsear_csv(texto: &str) -> Vec<Vec<String>> {
    let mut filas = Vec::new();
    let mut fila = Vec::new();
    let mut campo = String::new();
    let mut entre_comillas = false;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, entre_comillas) {
            ('"', true) if chars.peek() == Some(&'"') => {
                campo.push('"');
                chars.next();
            }
            ('"', _) => entre_comillas = !entre_comillas,
            (';', false) => fila.push(std::mem::take(&mut campo)),
            ('\r', false) => {}
            ('\n', false) => {
                fila.push(std::mem::take(&mut campo));
                filas.push(std::mem::take(&mut fila));
            }
            (c, _) => campo.push(c),
        }
    }
    if !campo.is_empty() || !fila.is_empty() {
        fila.push(campo);
        filas.push(fila);
    }
    filas
}

/// Cuántos archivos van a cada carpeta de primer nivel del destino (para la vista previa).
pub fn resumen_por_carpeta(plan: &Plan) -> Vec<(String, usize, u64)> {
    let mut m: HashMap<String, (usize, u64)> = HashMap::new();
    for op in &plan.operaciones {
        let rel = op
            .destino
            .strip_prefix(&plan.destino)
            .unwrap_or(&op.destino);
        let primera = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        let e = m.entry(primera).or_default();
        e.0 += 1;
        e.1 += op.tamano;
    }
    let mut v: Vec<(String, usize, u64)> = m.into_iter().map(|(k, (n, b))| (k, n, b)).collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Cómo mostrar cada línea de la vista previa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Titulo,
    Normal,
    Aviso,
    Problema,
}

/// La vista previa de un plan, en líneas, para la consola y para la interfaz.
pub fn describir(plan: &Plan, raiz: &Path, ejemplos: usize) -> Vec<(Nivel, String)> {
    use crate::formato::{cantidad, miles, relativa, tamano};
    let mut v = Vec::new();
    let n = miles(plan.operaciones.len() as u64);
    let peso = tamano(plan.bytes);
    let titulo = match plan.tipo {
        Tipo::Ordenar(Modo::Mover) => {
            format!("Mover {n} archivos ({peso}) a {}", plan.destino.display())
        }
        Tipo::Ordenar(Modo::Copiar) => {
            format!("Copiar {n} archivos ({peso}) a {}", plan.destino.display())
        }
        Tipo::ParaBorrar => format!(
            "Mandar {n} archivos ({peso}) a {} (no se borra nada: lo borrás vos)",
            plan.destino.display()
        ),
        Tipo::Deshacer => format!(
            "Devolver {n} archivos ({peso}) a su lugar, según {}",
            plan.destino.display()
        ),
    };
    v.push((Nivel::Titulo, titulo));
    if plan.bytes_a_escribir == 0 {
        v.push((
            Nivel::Normal,
            "Mismo disco: se renombran, no se copia nada.".to_string(),
        ));
    } else {
        let libre = plan
            .espacio_libre
            .map(|l| format!(" · libres en el destino: {}", tamano(l)))
            .unwrap_or_default();
        v.push((
            Nivel::Normal,
            format!("Hay que escribir {}{libre}", tamano(plan.bytes_a_escribir)),
        ));
    }
    if matches!(plan.tipo, Tipo::Ordenar(_)) {
        let carpetas: Vec<String> = resumen_por_carpeta(plan)
            .into_iter()
            .map(|(c, n, b)| format!("{c} {} ({})", miles(n as u64), tamano(b)))
            .collect();
        if !carpetas.is_empty() {
            v.push((
                Nivel::Normal,
                format!("Por carpeta: {}", carpetas.join(" · ")),
            ));
        }
    }
    for op in plan.operaciones.iter().take(ejemplos) {
        v.push((
            Nivel::Normal,
            format!(
                "  {}  ←  {}",
                relativa(&plan.destino, &op.destino),
                relativa(raiz, &op.origen)
            ),
        ));
    }
    if plan.operaciones.len() > ejemplos {
        v.push((
            Nivel::Normal,
            format!(
                "  … y {} más",
                miles((plan.operaciones.len() - ejemplos) as u64)
            ),
        ));
    }
    let mut no_se_tocan = Vec::new();
    if plan.omitidos_instaladores > 0 {
        no_se_tocan.push(cantidad(
            plan.omitidos_instaladores as u64,
            "instalador",
            "instaladores",
        ));
    }
    if plan.omitidos_duplicados > 0 {
        no_se_tocan.push(format!(
            "{} (queda una de cada grupo)",
            cantidad(
                plan.omitidos_duplicados as u64,
                "copia duplicada",
                "copias duplicadas"
            )
        ));
    }
    if plan.omitidos_temporales > 0 {
        no_se_tocan.push(cantidad(
            plan.omitidos_temporales as u64,
            "temporal",
            "temporales",
        ));
    }
    if plan.omitidos_en_destino > 0 {
        no_se_tocan.push(cantidad(
            plan.omitidos_en_destino as u64,
            "que ya está en el destino",
            "que ya están en el destino",
        ));
    }
    if !no_se_tocan.is_empty() {
        v.push((
            Nivel::Normal,
            format!("No se tocan: {}", no_se_tocan.join(", ")),
        ));
    }
    if plan.renombrados > 0 {
        v.push((
            Nivel::Normal,
            format!(
                "{} con \" (2)\": nunca se pisa nada.",
                cantidad(
                    plan.renombrados as u64,
                    "nombre repetido va",
                    "nombres repetidos van"
                )
            ),
        ));
    }
    for a in &plan.avisos {
        v.push((Nivel::Aviso, format!("Aviso: {a}")));
    }
    for p in &plan.problemas {
        v.push((Nivel::Problema, format!("No se puede: {p}")));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(ruta: &str, categoria: Categoria) -> Elemento {
        Elemento {
            indice: 0,
            ruta: PathBuf::from(ruta),
            tamano: 1,
            modificado: None,
            categoria,
            proyecto: None,
            motivo: None,
        }
    }

    fn texto(p: PathBuf) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn fotos_por_anio_del_exif() {
        let e = el("/raiz/DCIM/IMG_1.jpg", Categoria::Fotos);
        assert_eq!(
            texto(ruta_ordenada(&e, Path::new("/raiz"), Some(2015))),
            "Fotos/2015/IMG_1.jpg"
        );
        assert_eq!(
            texto(ruta_ordenada(&e, Path::new("/raiz"), None)),
            "Fotos/sin fecha/IMG_1.jpg"
        );
    }

    #[test]
    fn documentos_con_su_carpeta_sin_las_carpetas_personales() {
        let raiz = Path::new("/c");
        let r = |ruta: &str| texto(ruta_ordenada(&el(ruta, Categoria::Documentos), raiz, None));
        assert_eq!(
            r("/c/Users/papa/Documents/Facultad/tp.docx"),
            "Documentos/Facultad/tp.docx"
        );
        assert_eq!(
            r("/c/Users/papa/OneDrive/Escritorio/cv.pdf"),
            "Documentos/cv.pdf"
        );
        assert_eq!(r("/c/home/fran/notas/a.txt"), "Documentos/notas/a.txt");
        assert_eq!(
            r("/c/Trabajo/2020/informe.pdf"),
            "Documentos/Trabajo/2020/informe.pdf"
        );
    }

    #[test]
    fn un_archivo_en_la_carpeta_personal_no_pierde_su_nombre() {
        let raiz = Path::new("/c");
        let e = el("/c/Users/papa/lista.txt", Categoria::Documentos);
        assert_eq!(texto(ruta_ordenada(&e, raiz, None)), "Documentos/lista.txt");
    }

    #[test]
    fn proyectos_enteros_con_su_estructura() {
        let mut e = el("/c/code/mi-web/src/index.js", Categoria::Proyectos);
        e.proyecto = Some(Arc::from(Path::new("/c/code/mi-web")));
        assert_eq!(
            texto(ruta_ordenada(&e, Path::new("/c"), None)),
            "Proyectos/mi-web/src/index.js"
        );
    }

    #[test]
    fn nombres_unicos_sin_distinguir_mayusculas() {
        let mut usados = HashSet::new();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("Foto.jpg");
        assert_eq!(unico(a.clone(), &mut usados), (a, false));
        let (b, r) = unico(dir.path().join("foto.JPG"), &mut usados);
        assert!(r);
        assert_eq!(b.file_name().unwrap(), "foto (2).JPG");
        std::fs::write(dir.path().join("x.txt"), "").unwrap();
        let (c, _) = unico(dir.path().join("x.txt"), &mut usados);
        assert_eq!(
            c.file_name().unwrap(),
            "x (2).txt",
            "tampoco pisa lo que ya está en disco"
        );
    }

    #[test]
    fn csv_ida_y_vuelta() {
        let campos = ["a;b", "con \"comillas\"", "dos\nlíneas", "normal"];
        let linea: Vec<String> = campos.iter().map(|c| campo_csv(c)).collect();
        let texto = format!("x;y;z;w\r\n{}\r\n", linea.join(";"));
        let filas = parsear_csv(&texto);
        assert_eq!(filas.len(), 2);
        assert_eq!(filas[1], campos.map(String::from).to_vec());
    }

    #[test]
    fn destinos_de_sistema_no_sirven() {
        assert!(problema_de_destino(Path::new(r"C:\Windows\Ordenado")).is_some());
        assert!(problema_de_destino(Path::new("/usr/local/fotos")).is_some());
        assert!(problema_de_destino(Path::new("")).is_some());
        assert!(problema_de_destino(Path::new(r"D:\Ordenado")).is_none());
    }
}
