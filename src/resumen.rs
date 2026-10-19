//! Totales por categoría, por carpeta de "revisar" y por proyecto.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::categoria::Categoria;
use crate::escaner::{Archivo, Escaneo};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Totales {
    pub cantidad: u64,
    pub bytes: u64,
    pub mas_viejo: Option<SystemTime>,
    pub mas_nuevo: Option<SystemTime>,
}

impl Totales {
    pub fn sumar(&mut self, a: &Archivo) {
        self.cantidad += 1;
        self.bytes += a.tamano;
        if let Some(t) = a.modificado {
            self.mas_viejo = Some(self.mas_viejo.map_or(t, |v| v.min(t)));
            self.mas_nuevo = Some(self.mas_nuevo.map_or(t, |v| v.max(t)));
        }
    }
}

#[derive(Debug, Clone)]
pub struct Grupo {
    pub ruta: Arc<Path>,
    pub totales: Totales,
}

#[derive(Debug, Default)]
pub struct Resumen {
    /// Solo lo del usuario; lo de "revisar" va aparte.
    pub por_categoria: BTreeMap<Categoria, Totales>,
    pub usuario: Totales,
    pub revisar: Vec<Grupo>,
    pub revisar_total: Totales,
    pub proyectos: Vec<Grupo>,
}

impl Resumen {
    pub fn de(escaneo: &Escaneo) -> Self {
        let mut r = Resumen::default();
        let mut revisar: HashMap<Arc<Path>, Totales> = HashMap::new();
        let mut proyectos: HashMap<Arc<Path>, Totales> = HashMap::new();

        for a in &escaneo.archivos {
            if let Some(g) = &a.revisar {
                revisar.entry(Arc::clone(g)).or_default().sumar(a);
                r.revisar_total.sumar(a);
                continue;
            }
            r.por_categoria.entry(a.categoria).or_default().sumar(a);
            r.usuario.sumar(a);
            if let Some(p) = &a.proyecto {
                proyectos.entry(Arc::clone(p)).or_default().sumar(a);
            }
        }

        r.revisar = ordenar(revisar);
        r.proyectos = ordenar(proyectos);
        r
    }

    pub fn categoria(&self, c: Categoria) -> Totales {
        self.por_categoria.get(&c).cloned().unwrap_or_default()
    }
}

/// De mayor a menor tamaño.
fn ordenar(grupos: HashMap<Arc<Path>, Totales>) -> Vec<Grupo> {
    let mut v: Vec<Grupo> = grupos
        .into_iter()
        .map(|(ruta, totales)| Grupo { ruta, totales })
        .collect();
    v.sort_by(|a, b| {
        b.totales
            .bytes
            .cmp(&a.totales.bytes)
            .then_with(|| a.ruta.cmp(&b.ruta))
    });
    v
}

/// Los `n` archivos más pesados de una categoría (sin contar lo de "revisar").
pub fn mas_pesados(escaneo: &Escaneo, c: Categoria, n: usize) -> Vec<&Archivo> {
    let mut v: Vec<&Archivo> = escaneo
        .archivos
        .iter()
        .filter(|a| a.categoria == c && a.revisar.is_none())
        .collect();
    v.sort_by(|a, b| b.tamano.cmp(&a.tamano).then_with(|| a.ruta.cmp(&b.ruta)));
    v.truncate(n);
    v
}
