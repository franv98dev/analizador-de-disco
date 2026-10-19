//! Duplicados en tres pasadas, de la más barata a la más cara:
//! 1. por tamaño exacto (no lee nada: viene del escaneo);
//! 2. por un hash del principio y el final de cada archivo (lee hasta 128 KB por archivo);
//! 3. por el hash completo (BLAKE3), solo para los que siguen empatados.
//!
//! Es lo único del programa que lee el contenido de los archivos, y solo los abre para leer.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use rayon::prelude::*;

use crate::escaner::{ErrorLectura, Escaneo};

/// Los más chicos no valen la pena: liberan poco y generan muchísimos grupos.
pub const TAMANO_MINIMO: u64 = 1024;
/// Cuánto se lee del principio y del final en la segunda pasada.
const BLOQUE: u64 = 64 * 1024;
const BUFFER: usize = 1024 * 1024;

type Hash = [u8; 32];

#[derive(Debug, Clone)]
pub struct Candidato {
    /// Índice en `Escaneo::archivos`.
    pub indice: u32,
    pub ruta: PathBuf,
    pub tamano: u64,
    pub modificado: Option<SystemTime>,
}

/// Avance, para mostrarlo desde otro hilo. También sirve para cancelar.
#[derive(Debug, Default)]
pub struct Progreso {
    /// 0 = no empezó, 1 = principio y final, 2 = completo.
    pub etapa: AtomicU8,
    pub bytes_hechos: AtomicU64,
    pub bytes_total: AtomicU64,
    pub cancelar: AtomicBool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrupoDuplicado {
    pub tamano: u64,
    /// Índices en `Escaneo::archivos`. El primero es el que se sugiere conservar:
    /// el más viejo y, si empatan, el de ruta más corta.
    pub archivos: Vec<u32>,
}

impl GrupoDuplicado {
    /// Lo que se libera borrando todas las copias menos una.
    pub fn liberable(&self) -> u64 {
        self.tamano * (self.archivos.len() as u64 - 1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Duplicados {
    /// De mayor a menor espacio liberable.
    pub grupos: Vec<GrupoDuplicado>,
    /// Archivos que compartían tamaño con otro y hubo que mirar.
    pub candidatos: usize,
    pub bytes_leidos: u64,
    pub duracion: Duration,
    pub errores: Vec<ErrorLectura>,
    pub cancelado: bool,
}

impl Duplicados {
    pub fn liberable(&self) -> u64 {
        self.grupos.iter().map(GrupoDuplicado::liberable).sum()
    }

    /// Cantidad de copias sobrantes (sin contar la que se conserva).
    pub fn copias(&self) -> usize {
        self.grupos.iter().map(|g| g.archivos.len() - 1).sum()
    }

    pub fn mb_por_segundo(&self) -> f64 {
        let s = self.duracion.as_secs_f64();
        if s > 0.0 {
            self.bytes_leidos as f64 / 1_048_576.0 / s
        } else {
            0.0
        }
    }

    /// Para cada archivo duplicado: (número de grupo, es el que se conserva).
    pub fn por_archivo(&self) -> HashMap<u32, (usize, bool)> {
        let mut m = HashMap::new();
        for (g, grupo) in self.grupos.iter().enumerate() {
            for (i, a) in grupo.archivos.iter().enumerate() {
                m.insert(*a, (g, i == 0));
            }
        }
        m
    }
}

/// Archivos del usuario que comparten tamaño con al menos otro. Quedan afuera los de
/// "revisar" (cachés de programas) y los de proyectos (`node_modules` está lleno de
/// archivos iguales), que solo harían ruido.
pub fn candidatos(e: &Escaneo) -> Vec<Candidato> {
    let mut por_tamano: HashMap<u64, Vec<u32>> = HashMap::new();
    for (i, a) in e.archivos.iter().enumerate() {
        if a.tamano >= TAMANO_MINIMO && a.revisar.is_none() && a.proyecto.is_none() {
            por_tamano.entry(a.tamano).or_default().push(i as u32);
        }
    }
    let mut v: Vec<Candidato> = por_tamano
        .into_values()
        .filter(|g| g.len() > 1)
        .flatten()
        .map(|i| {
            let a = &e.archivos[i as usize];
            Candidato {
                indice: i,
                ruta: a.ruta.clone(),
                tamano: a.tamano,
                modificado: a.modificado,
            }
        })
        .collect();
    v.sort_by_key(|c| c.indice);
    v
}

/// Total que se leería en el peor caso (para avisar antes de empezar).
pub fn bytes_candidatos(candidatos: &[Candidato]) -> u64 {
    candidatos.iter().map(|c| c.tamano).sum()
}

pub fn buscar(
    candidatos: Vec<Candidato>,
    progreso: &Progreso,
    hilos: Option<usize>,
) -> io::Result<Duplicados> {
    let mut pool = rayon::ThreadPoolBuilder::new();
    if let Some(n) = hilos {
        pool = pool.num_threads(n);
    }
    let pool = pool.build().map_err(io::Error::other)?;
    Ok(pool.install(|| buscar_en_pool(candidatos, progreso)))
}

fn buscar_en_pool(candidatos: Vec<Candidato>, progreso: &Progreso) -> Duplicados {
    let inicio = Instant::now();
    let mut resultado = Duplicados {
        candidatos: candidatos.len(),
        ..Default::default()
    };

    // Pasada 2: principio y final.
    let total: u64 = candidatos.iter().map(|c| bytes_parciales(c.tamano)).sum();
    empezar_etapa(progreso, 1, total);
    let parciales: Vec<io::Result<Hash>> = candidatos
        .par_iter()
        .map(|c| hash_parcial(c, progreso))
        .collect();
    let empatados = agrupar(&candidatos, parciales, &mut resultado.errores);
    resultado.bytes_leidos += total;

    // Los que entran enteros en el hash parcial ya están confirmados.
    let (confirmados, a_completar): (Vec<Vec<usize>>, Vec<Vec<usize>>) = empatados
        .into_iter()
        .partition(|g| candidatos[g[0]].tamano <= 2 * BLOQUE);

    // Pasada 3: el archivo completo.
    let restantes: Vec<usize> = a_completar.into_iter().flatten().collect();
    let total: u64 = restantes.iter().map(|&i| candidatos[i].tamano).sum();
    empezar_etapa(progreso, 2, total);
    let completos: Vec<io::Result<Hash>> = restantes
        .par_iter()
        .map(|&i| hash_completo(&candidatos[i], progreso))
        .collect();
    let sub: Vec<Candidato> = restantes.iter().map(|&i| candidatos[i].clone()).collect();
    let mut errores = Vec::new();
    let completos = agrupar(&sub, completos, &mut errores)
        .into_iter()
        .map(|g| g.into_iter().map(|j| restantes[j]).collect::<Vec<usize>>());
    resultado.errores.extend(errores);
    resultado.bytes_leidos += total;

    if progreso.cancelar.load(Ordering::Relaxed) {
        resultado.cancelado = true;
        resultado.duracion = inicio.elapsed();
        return resultado;
    }

    for g in confirmados.into_iter().chain(completos) {
        let mut miembros: Vec<&Candidato> = g.iter().map(|&i| &candidatos[i]).collect();
        // Se sugiere conservar el más viejo; si no hay fecha va al final.
        miembros.sort_by(|a, b| {
            let fecha = |c: &Candidato| c.modificado.unwrap_or(SystemTime::now());
            fecha(a)
                .cmp(&fecha(b))
                .then_with(|| a.ruta.as_os_str().len().cmp(&b.ruta.as_os_str().len()))
                .then_with(|| a.ruta.cmp(&b.ruta))
        });
        resultado.grupos.push(GrupoDuplicado {
            tamano: miembros[0].tamano,
            archivos: miembros.iter().map(|c| c.indice).collect(),
        });
    }
    resultado.grupos.sort_by(|a, b| {
        b.liberable()
            .cmp(&a.liberable())
            .then_with(|| a.archivos.cmp(&b.archivos))
    });
    resultado.duracion = inicio.elapsed();
    resultado
}

fn empezar_etapa(p: &Progreso, etapa: u8, total: u64) {
    p.bytes_hechos.store(0, Ordering::Relaxed);
    p.bytes_total.store(total, Ordering::Relaxed);
    p.etapa.store(etapa, Ordering::Relaxed);
}

/// Agrupa por (tamaño, hash) y devuelve los grupos de 2 o más. Los errores se anotan aparte.
fn agrupar(
    candidatos: &[Candidato],
    hashes: Vec<io::Result<Hash>>,
    errores: &mut Vec<ErrorLectura>,
) -> Vec<Vec<usize>> {
    let mut grupos: HashMap<(u64, Hash), Vec<usize>> = HashMap::new();
    for (i, h) in hashes.into_iter().enumerate() {
        match h {
            Ok(h) => grupos.entry((candidatos[i].tamano, h)).or_default().push(i),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => errores.push(ErrorLectura {
                ruta: candidatos[i].ruta.clone(),
                mensaje: err.to_string(),
            }),
        }
    }
    let mut v: Vec<Vec<usize>> = grupos.into_values().filter(|g| g.len() > 1).collect();
    v.sort();
    v
}

fn bytes_parciales(tamano: u64) -> u64 {
    tamano.min(2 * BLOQUE)
}

fn cancelado(p: &Progreso) -> io::Result<()> {
    if p.cancelar.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"))
    } else {
        Ok(())
    }
}

fn hash_parcial(c: &Candidato, p: &Progreso) -> io::Result<Hash> {
    cancelado(p)?;
    let mut f = File::open(&c.ruta)?;
    let mut h = blake3::Hasher::new();
    if c.tamano <= 2 * BLOQUE {
        let mut datos = vec![0u8; c.tamano as usize];
        f.read_exact(&mut datos).map_err(cambio)?;
        h.update(&datos);
    } else {
        let mut datos = vec![0u8; BLOQUE as usize];
        f.read_exact(&mut datos).map_err(cambio)?;
        h.update(&datos);
        f.seek(SeekFrom::Start(c.tamano - BLOQUE))?;
        f.read_exact(&mut datos).map_err(cambio)?;
        h.update(&datos);
    }
    p.bytes_hechos
        .fetch_add(bytes_parciales(c.tamano), Ordering::Relaxed);
    Ok(*h.finalize().as_bytes())
}

fn hash_completo(c: &Candidato, p: &Progreso) -> io::Result<Hash> {
    let mut f = File::open(&c.ruta)?;
    let mut h = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER];
    let mut leidos = 0u64;
    loop {
        cancelado(p)?;
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        h.update(&buffer[..n]);
        leidos += n as u64;
        p.bytes_hechos.fetch_add(n as u64, Ordering::Relaxed);
    }
    if leidos != c.tamano {
        return Err(cambio(io::Error::from(io::ErrorKind::UnexpectedEof)));
    }
    Ok(*h.finalize().as_bytes())
}

/// Si el archivo cambió de tamaño entre el escaneo y ahora, mejor no compararlo.
fn cambio(err: io::Error) -> io::Error {
    if err.kind() == io::ErrorKind::UnexpectedEof {
        io::Error::other("el archivo cambió desde el escaneo")
    } else {
        err
    }
}
