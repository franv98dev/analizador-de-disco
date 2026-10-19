//! Árbol de carpetas con tamaños acumulados, armado a partir del escaneo (sin volver al disco).
//!
//! Los nodos viven en un `Vec` y se referencian por índice. Además se guarda el orden en
//! profundidad de los archivos: los de cualquier subárbol quedan contiguos, así listar
//! "todo lo que hay debajo de esta carpeta" es recorrer un rango, sin buscar.

use std::collections::{HashMap, HashSet};
use std::ops::{AddAssign, Range};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::categoria::Categoria;
use crate::escaner::{Archivo, Escaneo};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Suma {
    pub cantidad: u64,
    pub bytes: u64,
}

impl AddAssign for Suma {
    fn add_assign(&mut self, otra: Suma) {
        self.cantidad += otra.cantidad;
        self.bytes += otra.bytes;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Carpeta,
    /// Raíz de un proyecto de código.
    Proyecto,
    /// Carpeta o archivo de sistema: no se recorrió.
    Excluido(&'static str),
}

#[derive(Debug, Clone)]
pub struct Nodo {
    pub nombre: String,
    pub padre: Option<usize>,
    /// Subcarpetas, de mayor a menor.
    pub hijos: Vec<usize>,
    /// Archivos directos (índices en `Escaneo::archivos`), de mayor a menor.
    pub archivos: Vec<u32>,
    pub tipo: Tipo,
    /// Lo del usuario en todo el subárbol, por categoría (índice = posición en `Categoria::TODAS`).
    pub usuario: [Suma; 9],
    /// Lo de "revisar" (AppData y similares) en todo el subárbol.
    pub revisar: Suma,
    rango: Range<usize>,
}

impl Nodo {
    fn nuevo(nombre: String, padre: Option<usize>, tipo: Tipo) -> Self {
        Nodo {
            nombre,
            padre,
            hijos: Vec::new(),
            archivos: Vec::new(),
            tipo,
            usuario: [Suma::default(); 9],
            revisar: Suma::default(),
            rango: 0..0,
        }
    }

    pub fn categoria(&self, c: Categoria) -> Suma {
        self.usuario[c as usize]
    }

    pub fn usuario_total(&self) -> Suma {
        let mut s = Suma::default();
        for x in self.usuario {
            s += x;
        }
        s
    }

    /// Usuario + revisar.
    pub fn total(&self) -> Suma {
        let mut s = self.usuario_total();
        s += self.revisar;
        s
    }

    /// Solo tiene datos de programas ("revisar"), nada del usuario.
    pub fn es_solo_revisar(&self) -> bool {
        self.revisar.cantidad > 0 && self.usuario_total().cantidad == 0
    }
}

#[derive(Debug)]
pub struct Arbol {
    pub raiz: PathBuf,
    pub nodos: Vec<Nodo>,
    orden: Vec<u32>,
    /// Carpeta (nodo) de cada archivo, en el orden de `Escaneo::archivos`.
    carpeta_de: Vec<u32>,
}

impl Arbol {
    pub const RAIZ: usize = 0;

    pub fn de(e: &Escaneo) -> Self {
        let mut a = Arbol {
            raiz: e.raiz.clone(),
            nodos: vec![Nodo::nuevo(
                e.raiz.display().to_string(),
                None,
                Tipo::Carpeta,
            )],
            orden: Vec::with_capacity(e.archivos.len()),
            carpeta_de: Vec::with_capacity(e.archivos.len()),
        };
        let mut indice: HashMap<PathBuf, usize> = HashMap::new();
        indice.insert(e.raiz.clone(), Self::RAIZ);

        // Los archivos de una misma carpeta suelen venir seguidos: se recuerda la última.
        let mut ultima: Option<(&Path, usize)> = None;
        for (i, archivo) in e.archivos.iter().enumerate() {
            let dir = archivo.ruta.parent().unwrap_or(&e.raiz);
            let id = match ultima {
                Some((d, id)) if d == dir => id,
                _ => a.carpeta(dir, &mut indice),
            };
            ultima = Some((dir, id));
            a.nodos[id].archivos.push(i as u32);
            a.carpeta_de.push(id as u32);
            sumar(&mut a.nodos[id], archivo);
        }

        for x in &e.excluidos {
            let padre = a.carpeta(x.ruta.parent().unwrap_or(&e.raiz), &mut indice);
            let nombre = nombre_de(&x.ruta);
            let id = a.nodos.len();
            a.nodos
                .push(Nodo::nuevo(nombre, Some(padre), Tipo::Excluido(x.motivo)));
            a.nodos[padre].hijos.push(id);
        }

        let proyectos: HashSet<&Arc<Path>> = e
            .archivos
            .iter()
            .filter_map(|x| x.proyecto.as_ref())
            .collect();
        for p in proyectos {
            if let Some(&id) = indice.get(p.as_ref()) {
                a.nodos[id].tipo = Tipo::Proyecto;
            }
        }

        // Un padre siempre se crea antes que sus hijos, así que recorriendo al revés
        // cada nodo ya tiene su subárbol sumado cuando se lo pasa al padre.
        for id in (1..a.nodos.len()).rev() {
            let (usuario, revisar) = (a.nodos[id].usuario, a.nodos[id].revisar);
            if let Some(p) = a.nodos[id].padre {
                for (dst, src) in a.nodos[p].usuario.iter_mut().zip(usuario) {
                    *dst += src;
                }
                a.nodos[p].revisar += revisar;
            }
        }

        let bytes: Vec<u64> = a.nodos.iter().map(|n| n.total().bytes).collect();
        for id in 0..a.nodos.len() {
            // Se sacan los hijos del nodo para poder leer los nombres de los demás mientras se ordena.
            let mut hijos = std::mem::take(&mut a.nodos[id].hijos);
            hijos.sort_by(|x, y| {
                bytes[*y]
                    .cmp(&bytes[*x])
                    .then_with(|| a.nodos[*x].nombre.cmp(&a.nodos[*y].nombre))
            });
            let n = &mut a.nodos[id];
            n.hijos = hijos;
            n.archivos.sort_by(|x, y| {
                let (ax, ay) = (&e.archivos[*x as usize], &e.archivos[*y as usize]);
                ay.tamano
                    .cmp(&ax.tamano)
                    .then_with(|| ax.ruta.cmp(&ay.ruta))
            });
        }

        a.ordenar_en_profundidad(Self::RAIZ);
        a
    }

    /// Id de la carpeta `dir`, creándola (y a sus padres) si hace falta.
    fn carpeta(&mut self, dir: &Path, indice: &mut HashMap<PathBuf, usize>) -> usize {
        if let Some(&id) = indice.get(dir) {
            return id;
        }
        let padre = match dir.parent() {
            Some(p) if p.starts_with(&self.raiz) => self.carpeta(p, indice),
            _ => Self::RAIZ,
        };
        let id = self.nodos.len();
        self.nodos
            .push(Nodo::nuevo(nombre_de(dir), Some(padre), Tipo::Carpeta));
        self.nodos[padre].hijos.push(id);
        indice.insert(dir.to_path_buf(), id);
        id
    }

    fn ordenar_en_profundidad(&mut self, id: usize) {
        let inicio = self.orden.len();
        self.orden.extend_from_slice(&self.nodos[id].archivos);
        for i in 0..self.nodos[id].hijos.len() {
            let hijo = self.nodos[id].hijos[i];
            self.ordenar_en_profundidad(hijo);
        }
        self.nodos[id].rango = inicio..self.orden.len();
    }

    pub fn nodo(&self, id: usize) -> &Nodo {
        &self.nodos[id]
    }

    /// Índices (en `Escaneo::archivos`) de todos los archivos debajo de `id`.
    pub fn archivos_bajo(&self, id: usize) -> &[u32] {
        &self.orden[self.nodos[id].rango.clone()]
    }

    /// Carpeta en la que está un archivo (índice en `Escaneo::archivos`).
    pub fn carpeta_de(&self, archivo: u32) -> usize {
        self.carpeta_de[archivo as usize] as usize
    }

    /// El nodo y todos sus ancestros hasta la raíz.
    pub fn ancestros(&self, id: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(id), |&n| self.nodos[n].padre)
    }

    /// Ruta completa del nodo.
    pub fn ruta(&self, id: usize) -> PathBuf {
        let mut nombres = Vec::new();
        let mut actual = id;
        while let Some(p) = self.nodos[actual].padre {
            nombres.push(self.nodos[actual].nombre.as_str());
            actual = p;
        }
        let mut ruta = self.raiz.clone();
        for n in nombres.into_iter().rev() {
            ruta.push(n);
        }
        ruta
    }

    /// Busca la carpeta (o el excluido) con esa ruta.
    pub fn buscar(&self, ruta: &Path) -> Option<usize> {
        let relativa = ruta.strip_prefix(&self.raiz).ok()?;
        let mut id = Self::RAIZ;
        for componente in relativa.components() {
            let nombre = componente.as_os_str().to_string_lossy();
            id = *self.nodos[id]
                .hijos
                .iter()
                .find(|h| self.nodos[**h].nombre == nombre)?;
        }
        Some(id)
    }
}

fn sumar(n: &mut Nodo, a: &Archivo) {
    let s = Suma {
        cantidad: 1,
        bytes: a.tamano,
    };
    if a.revisar.is_some() {
        n.revisar += s;
    } else {
        n.usuario[a.categoria as usize] += s;
    }
}

fn nombre_de(ruta: &Path) -> String {
    ruta.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ruta.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suma_acumula_cantidad_y_bytes() {
        let mut s = Suma {
            cantidad: 1,
            bytes: 10,
        };
        s += Suma {
            cantidad: 2,
            bytes: 5,
        };
        assert_eq!(
            s,
            Suma {
                cantidad: 3,
                bytes: 15
            }
        );
    }
}
